//! Schemas known at run time, like schemas stored in a database: each one is
//! compiled once into a `Validator`, at startup.
//!
//! - `POST /products`: a route with its own schema, validated by the
//!   `validate_json` middleware.
//! - `POST /forms/{name}`: the schema is chosen by the request, validated in
//!   the handler.
//!
//! ```sh
//! cargo run --example runtime_schemas
//!
//! curl -i localhost:3000/products -H 'content-type: application/json' \
//!   -d '{"title": "  Book ", "price": "12.50", "tags": ["paper"]}'
//! curl -i localhost:3000/forms/feedback -H 'content-type: application/json' \
//!   -d '{"email": "jane@example.com", "message": "Thanks!"}'
//! curl -i localhost:3000/forms/feedback -H 'content-type: application/json' -d '{}'
//! curl -i localhost:3000/forms/unknown -H 'content-type: application/json' -d '{}'
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::middleware::from_fn_with_state;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use livr::Validator;
use livr_axum::{LivrRejection, LivrValidated, validate_json};
use serde_json::Value;

// Rows of a `schemas` table, say
const SCHEMAS: &str = r#"{
    "product": {
        "title": ["required", "trim", {"max_length": 200}],
        "price": ["required", "positive_decimal"],
        "tags": {"list_of": ["string", {"max_length": 20}]}
    },
    "feedback": {
        "email": ["required", "email"],
        "message": ["required", {"length_between": [1, 2000]}]
    }
}"#;

#[derive(Clone)]
struct AppState {
    validators: Arc<HashMap<String, Validator>>,
}

// Compiled once: an invalid schema in the database is an error at startup,
// not at the first request
fn load_validators() -> Result<HashMap<String, Validator>, livr::SchemaError> {
    let schemas: HashMap<String, Value> = serde_json::from_str(SCHEMAS).expect("JSON");
    schemas
        .into_iter()
        .map(|(name, schema)| Ok((name, Validator::new(&schema)?)))
        .collect()
}

// The output has the fields of the schema only, cleaned: `title` trimmed,
// `price` a number
async fn create_product(LivrValidated(product): LivrValidated) -> Json<Value> {
    Json(product)
}

// `axum::Json<Value>` reads the body; the validator of the form checks it
async fn submit_form(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(data): Json<Value>,
) -> Response {
    let Some(validator) = state.validators.get(&name) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match validator.validate_owned(data) {
        Ok(output) => Json(output).into_response(),
        Err(errors) => LivrRejection::from(errors).into_response(),
    }
}

fn app(state: AppState) -> Router {
    let product = state.validators["product"].clone();
    Router::new()
        .route(
            "/products",
            post(create_product).route_layer(from_fn_with_state(product, validate_json)),
        )
        .route("/forms/{name}", post(submit_form))
        .with_state(state)
}

#[tokio::main]
async fn main() {
    let state = AppState {
        validators: Arc::new(load_validators().expect("valid schemas")),
    };
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_owned());
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    println!("listening on http://127.0.0.1:{port}");
    axum::serve(listener, app(state)).await.unwrap();
}
