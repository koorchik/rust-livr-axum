//! Another format for the errors: the one of the API, here an envelope with
//! `success`, where validation errors are expected outcomes (HTTP 200) and
//! only malformed requests are 4xx.
//!
//! An extractor of your own wraps `Livr` with `#[derive(FromRequest)]`
//! (axum's `macros` feature) and maps `LivrRejection` to your error type.
//!
//! ```sh
//! cargo run --example custom_errors
//!
//! curl -i localhost:3000/users -H 'content-type: application/json' \
//!   -d '{"name": "Jane", "email": "jane@example.com"}'
//! curl -i localhost:3000/users -H 'content-type: application/json' -d '{"email": "x"}'
//! curl -i localhost:3000/users -H 'content-type: application/json' -d '{"name": '
//! ```

use axum::extract::FromRequest;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use livr_axum::LivrRejection;
use serde_json::{Value, json};

livr::schema! {
    #[derive(Debug)]
    pub struct CreateUser {
        name: ["required", {"max_length": 50}],
        email: ["required", "email"],
    }
}

// `Valid<T>` is `livr_axum::Livr<T>` with the errors of the API
#[derive(FromRequest)]
#[from_request(via(livr_axum::Livr), rejection(ApiError))]
struct Valid<T>(T);

struct ApiError(LivrRejection);

impl From<LivrRejection> for ApiError {
    fn from(rejection: LivrRejection) -> Self {
        Self(rejection)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // Invalid data is an expected outcome; a malformed request is not
        let status = match self.0 {
            LivrRejection::Invalid(_) => StatusCode::OK,
            _ => self.0.status(),
        };
        let body = json!({"success": false, "error": self.0.body()});
        (status, Json(body)).into_response()
    }
}

async fn create_user(Valid(user): Valid<CreateUser>) -> Json<Value> {
    Json(json!({"success": true, "result": {"name": user.name, "email": user.email}}))
}

fn app() -> Router {
    Router::new().route("/users", post(create_user))
}

#[tokio::main]
async fn main() {
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_owned());
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    println!("listening on http://127.0.0.1:{port}");
    axum::serve(listener, app()).await.unwrap();
}
