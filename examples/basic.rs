//! Typed schemas: a JSON body, a query string, a form, and the schema served
//! to a frontend.
//!
//! ```sh
//! cargo run --example basic
//!
//! curl -i localhost:3000/users -H 'content-type: application/json' \
//!   -d '{"name": "Jane", "email": "JANE@EXAMPLE.COM", "age": "42", "address": {"city": "Kyiv", "zip": "01001"}}'
//! curl -i localhost:3000/users -H 'content-type: application/json' \
//!   -d '{"email": "wrong", "age": "abc", "address": {"zip": "1"}}'
//! curl -i 'localhost:3000/users?q=jane&page=2&role=admin'
//! curl -i localhost:3000/login -d 'login=jane&password=secret-password'
//! curl -i localhost:3000/schemas/sign-up
//! ```

use axum::routing::{get, post};
use axum::{Json, Router};
use livr_axum::{Livr, LivrForm, LivrQuery};
use serde_json::{Value, json};

livr::schema! {
    #[derive(Debug)]
    pub struct SignUp {
        name: ["trim", "required", {"max_length": 50}],
        email: ["required", "email", "to_lc"],
        age: ["required", "positive_integer", {"min_number": 18}],
        role: {"one_of": ["admin", "user"]},
        address: ["required", {"nested_object": {
            city: ["required", "string"],
            zip: ["required", {"like": "^\\d{5}$"}],
        }}],
    }
}

// Query values are strings: the rules convert them (`page=2` is `2`)
livr::schema! {
    #[derive(Debug)]
    pub struct UserSearch {
        q: ["trim", {"max_length": 100}],
        page: ["positive_integer", {"default": 1}],
        role: {"one_of": ["admin", "user"]},
    }
}

livr::schema! {
    #[derive(Debug)]
    pub struct Login {
        login: ["trim", "required", "string"],
        password: ["required", {"min_length": 8}],
    }
}

// The types follow from the rules: `name` is a `String`, `age` an `i64`,
// `role` an `Option<SignUpRole>`, `address` a `SignUpAddress`. Modifiers
// come before `required`: `trim` can leave an empty string, so after it a
// field is optional again
async fn sign_up(Livr(user): Livr<SignUp>) -> Json<Value> {
    Json(json!({
        "name": user.name,
        "email": user.email,
        "age": user.age,
        "role": user.role.map(|role| role.as_str()),
        "city": user.address.city,
    }))
}

async fn search(LivrQuery(search): LivrQuery<UserSearch>) -> Json<Value> {
    Json(json!({"q": search.q, "page": search.page, "role": search.role}))
}

async fn login(LivrForm(login): LivrForm<Login>) -> String {
    format!("Welcome, {}", login.login)
}

fn app() -> Router {
    Router::new()
        .route("/users", post(sign_up).get(search))
        .route("/login", post(login))
        // The frontend validates with the same rules (js-validator-livr)
        .route("/schemas/sign-up", get(livr_axum::schema::<SignUp>))
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
