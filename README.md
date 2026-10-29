# livr-axum

[![CI](https://github.com/koorchik/rust-livr-axum/actions/workflows/test.yml/badge.svg)](https://github.com/koorchik/rust-livr-axum/actions/workflows/test.yml)
[![crates.io](https://img.shields.io/crates/v/livr-axum.svg)](https://crates.io/crates/livr-axum)
[![docs.rs](https://img.shields.io/docsrs/livr-axum)](https://docs.rs/livr-axum)
[![LIVR 2.0](https://img.shields.io/badge/LIVR-2.0-blue)](https://livr-spec.org)
[![MSRV 1.88](https://img.shields.io/badge/MSRV-1.88-orange)](https://blog.rust-lang.org/2025/06/26/Rust-1.88.0/)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](https://github.com/koorchik/rust-livr-axum/blob/master/LICENSE)

**Validation errors your frontend can show: every field, at once, as JSON.** [axum](https://github.com/tokio-rs/axum) extractors that validate with [livr](https://github.com/koorchik/rust-livr), an implementation of the [LIVR 2.0 specification](https://livr-spec.org) (Language Independent Validation Rules) that passes its full test suite. The same rules and error codes work in the LIVR implementations of other languages, the JavaScript of your frontend too.

```rust
use axum::{Router, routing::post};
use livr_axum::Livr;

livr::schema! {
    pub struct SignUp {
        name: ["trim", "required", {"max_length": 50}],
        email: ["required", "email", "to_lc"],
        age: ["required", "positive_integer", {"min_number": 18}],
    }
}

async fn sign_up(Livr(user): Livr<SignUp>) -> String {
    format!("Welcome, {} ({})", user.name, user.age) // user.age: i64
}

let app: Router = Router::new().route("/users", post(sign_up));
```

```http
POST /users
{"name": "", "email": "wrong", "age": "abc"}

HTTP/1.1 422 Unprocessable Entity
{"code": "VALIDATION_ERROR", "fields": {"age": "NOT_POSITIVE_INTEGER", "email": "WRONG_EMAIL", "name": "REQUIRED"}}
```

## Why livr-axum

- **All errors in one pass.** The body is validated as JSON before it becomes a Rust type, so a wrong type (`"age": "abc"`) is one more field error, not a failure that hides the others.
- **JSON errors with codes**, shaped like the data: ready to show next to the fields.
- **Forms and query strings just work:** `?page=2` is the number `2`, `?tags=a` is a list when the field is one.
- **Typed handlers** from `schema!`, and **schemas from a database** through a middleware.
- **Your error format**, for the extractors and the middleware.

## Compared with `axum::Json<T>` and axum-valid

The same request, `{"name": "", "email": "wrong", "age": "abc"}`, then `"age": 12`:

| | **livr-axum** | `axum::Json<T>` | axum-valid 0.25 with validator (`Valid<Json<T>>`) |
|---|---|---|---|
| `"age": "abc"` | 422, every field: `{"code": "VALIDATION_ERROR", "fields": {"age": "NOT_POSITIVE_INTEGER", "email": "WRONG_EMAIL", "name": "REQUIRED"}}` | 422, text, one field: `Failed to deserialize the JSON body into the target type: age: invalid type: string "abc", expected i64 at line 1 column 43` | the same text as `axum::Json<T>`: the body is deserialized before it is validated |
| `"age": 12` | 422, every field: `{..., "age": "TOO_LOW"}` | no check of values | 400, text: `age: Validation error: range [{"min": Number(18), "value": Number(12)}]`, one line per field (JSON with its `into_json` feature) |
| `"age": "42"` | ✅ `42` | ❌ invalid type | ❌ invalid type |
| Query strings and forms, with conversions | ✅ `LivrQuery<T>`, `LivrForm<T>` | `Query<T>`, `Form<T>`, no validation | ✅ |
| Schemas from a database | ✅ | ❌ | ❌ |
| The rules for the frontend | ✅ `livr_axum::schema::<T>` | ❌ | ❌ |

```toml
[dependencies]
axum = "0.8"
livr = "0.1"
livr-axum = "0.1"
```

## A JSON body: `Livr<T>`

```rust
use axum::{Router, routing::{get, post}};
use livr_axum::Livr;

livr::schema! {
    pub struct SignUp {
        name: ["trim", "required", {"max_length": 50}],
        email: ["required", "email", "to_lc"],
        age: ["required", "positive_integer", {"min_number": 18}],
        address: ["required", {"nested_object": {
            city: ["required", "string"],
            zip: ["required", {"like": "^\\d{5}$"}],
        }}],
    }
}

// user.name: String, user.age: i64, user.address: SignUpAddress
async fn sign_up(Livr(user): Livr<SignUp>) -> String {
    format!("{} from {}, {}", user.name, user.address.city, user.age)
}

let app: Router = Router::new()
    .route("/users", post(sign_up))
    // For the frontend: the same rules, as JSON
    .route("/schemas/sign-up", get(livr_axum::schema::<SignUp>));
```

`POST /users` with `{"email": "wrong", "age": "abc", "address": {"zip": "1"}}`:

```http
HTTP/1.1 422 Unprocessable Entity
content-type: application/json

{"code": "VALIDATION_ERROR", "fields": {
  "address": {"city": "REQUIRED", "zip": "WRONG_FORMAT"},
  "age": "NOT_POSITIVE_INTEGER",
  "email": "WRONG_EMAIL",
  "name": "REQUIRED"
}}
```

The fields of the errors are in alphabetical order, as `serde_json` sorts them.

The body must be `application/json` (or `application/*+json`). `Option<Livr<T>>` is `None` when the request has no `Content-Type`, like `Option<axum::Json<T>>`.

## Query strings and forms: `LivrQuery<T>`, `LivrForm<T>`

```rust
use axum::{Router, routing::{get, post}};
use livr_axum::{LivrForm, LivrQuery};

livr::schema! {
    pub struct Search {
        q: ["trim", {"max_length": 100}],
        page: ["positive_integer", {"default": 1}],
        tags: {"list_of": "string"},
    }
}

// GET /users?q=jane&page=2&tags=a: search.page is the number 2, search.tags is ["a"]
async fn search(LivrQuery(search): LivrQuery<Search>) -> String {
    format!("{:?}, page {}, {:?}", search.q, search.page, search.tags)
}

// An application/x-www-form-urlencoded body (the query string for GET)
async fn submit(LivrForm(search): LivrForm<Search>) -> String {
    format!("page {}", search.page)
}

let app: Router = Router::new()
    .route("/users", get(search))
    .route("/forms", post(submit));
```

The values are strings, and the rules convert them.

- A list field (`list_of`, `list_of_objects`, ...) is a list even with one value: `?tags=a` is `["a"]`, as a group of checkboxes with one box checked sends it. An empty value, `?tags=`, is no value.
- Any key given more than once is a list, and so is a key ending with `[]` (`tags[]=a`, also percent-encoded as `tags%5B%5D=a`), even once.
- Keys are not nested: `address[city]=Kyiv` is the key `address[city]`, not a field of `address`, as in `axum::Form`.
- Keys that are not fields of the schema are dropped before validation, so a field that `equal_to_field` or a custom rule reads needs rules of its own in the schema.
- `multipart/form-data` is rejected with 415.

`Option<LivrQuery<T>>` is `None` when the URI has no query string.

## Schemas known at run time: `validate_json` and `LivrValidated`

Build a `Validator` once per schema, at startup, and put the middleware on the route. The handler gets the cleaned data; an invalid request does not reach it.

```rust
use axum::{Router, middleware::from_fn_with_state, routing::post};
use livr::Validator;
use livr_axum::{LivrValidated, validate_json};

async fn create_product(LivrValidated(product): LivrValidated) -> String {
    product.to_string()
}

// A schema from the database
let schema = serde_json::json!({
    "title": ["trim", "required", {"max_length": 200}],
    "price": ["required", "positive_decimal"],
});
let validator = Validator::new(&schema).expect("a valid schema");
let app: Router = Router::new().route(
    "/products",
    post(create_product).route_layer(from_fn_with_state(validator, validate_json)),
);
```

The body stays readable by the handler. Each `LivrValidated` is a copy of the output, like `axum::Extension`, so a middleware and the handler can both extract it.

When the request chooses the schema, validate in the handler and return the errors as a `LivrRejection`:

```rust
use axum::Json;
use livr::Validator;
use livr_axum::LivrRejection;
use serde_json::Value;

fn check(validator: &Validator, data: Value) -> Result<Json<Value>, LivrRejection> {
    Ok(Json(validator.validate_owned(data)?))
}
```

## Errors

A rejected request gets a status and a JSON error object:

| | Status | Body |
|---|---|---|
| Invalid fields | 422 | `{"code": "VALIDATION_ERROR", "fields": {...}}` |
| The data is not an object | 422 | `{"code": "FORMAT_ERROR"}` |
| Malformed JSON, an empty body, or more than 128 levels of nesting | 400 | `{"code": "INVALID_JSON", "message": "..."}` |
| Wrong or missing `Content-Type` | 415 | `{"code": "UNSUPPORTED_MEDIA_TYPE"}` |
| Body over the limit | 413 | `{"code": "BODY_TOO_LARGE"}` |
| Body not read (the connection failed) | 400 | `{"code": "INVALID_BODY"}` |
| `LivrValidated` on a route without `validate_json` | 500 | `{"code": "INTERNAL_ERROR"}` |

`fields` has the LIVR error codes, shaped like the data: nested objects for `nested_object`, lists (with `null` for valid items) for `list_of`.

### Your error format

Convert `LivrRejection` into the error type of your API with its `status`, `code`, `body` and `errors` (or `into_errors`). For the extractors, wrap them with `#[derive(FromRequest)]` (axum's `macros` feature); for the middleware, use `validate_json_with::<ApiError>` instead of `validate_json`:

```rust
use axum::{Json, Router, extract::FromRequest, http::StatusCode, routing::post};
use axum::middleware::from_fn_with_state;
use axum::response::{IntoResponse, Response};
use livr::Validator;
use livr_axum::{LivrRejection, LivrValidated, validate_json_with};

#[derive(FromRequest)]
#[from_request(via(livr_axum::Livr), rejection(ApiError))]
struct Valid<T>(T);

struct ApiError(LivrRejection);

impl From<LivrRejection> for ApiError {
    fn from(rejection: LivrRejection) -> Self {
        Self(rejection)
    }
}

// {"success": false, "error": {...}}, HTTP 200 for invalid data
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self.0 {
            LivrRejection::Invalid(_) => StatusCode::OK,
            _ => self.0.status(),
        };
        (status, Json(serde_json::json!({"success": false, "error": self.0.body()}))).into_response()
    }
}

livr::schema! {
    pub struct CreateUser {
        name: ["required", {"max_length": 50}],
    }
}

async fn create_user(Valid(user): Valid<CreateUser>) -> String {
    user.name
}

async fn create_product(LivrValidated(product): LivrValidated) -> String {
    product.to_string()
}

let validator = Validator::new(&serde_json::json!({"title": "required"})).expect("a valid schema");
let app: Router = Router::new()
    .route("/users", post(create_user))
    .route(
        "/products",
        post(create_product).route_layer(from_fn_with_state(validator, validate_json_with::<ApiError>)),
    );
```

## Limits

The body limit is the one of `axum::extract::DefaultBodyLimit`, 2 MB by default, for streamed bodies too. A parsed JSON body takes several times its size in memory, a body of many small keys about ten times, so set a limit that fits the route: `.layer(DefaultBodyLimit::max(64 * 1024))`. Forms and query strings are cheaper: only the fields of the schema are kept.

## Building your own extractor or framework

The extractors are built from public functions, for code that reads requests its own way, like a framework that merges the query string, the body and the path, and validates later:

| Function | |
|---|---|
| `read_json(Request) -> Result<Value, LivrRejection>` | checks the `Content-Type`, reads the body with the limit, parses it |
| `read_form(Request, Option<&[SchemaField]>) -> Result<Map<String, Value>, LivrRejection>` | the query string for `GET` and `HEAD`, else a form body, as `LivrForm` |
| `read_body(Request) -> Result<Bytes, LivrRejection>` | the body with the limit of `DefaultBodyLimit` |
| `parse_json(&[u8]) -> Result<Value, LivrRejection>` | parses a body |
| `parse_urlencoded(&[u8], Option<&[SchemaField]>) -> Map<String, Value>` | a query string or a form as fields, with lists |
| `is_json_content_type(&HeaderMap) -> bool`, `is_form_content_type(&HeaderMap) -> bool` | the `Content-Type` checks |

`parse_urlencoded` and `read_form` take the fields of a schema (`T::FIELDS`) to make lists of single values and drop the other keys. With `None`, for data whose schema is not known yet, they keep every key and leave single values as they are.

```rust
use axum::extract::{FromRequest, Request};
use livr_axum::{LivrRejection, parse_urlencoded, read_json};
use serde_json::{Map, Value};

/// The query string and the JSON body in one object; the body wins
struct Input(Map<String, Value>);

impl<S: Send + Sync> FromRequest<S> for Input {
    type Rejection = LivrRejection;

    async fn from_request(request: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let query = request.uri().query().unwrap_or_default();
        let mut input = parse_urlencoded(query.as_bytes(), None);
        match read_json(request).await? {
            Value::Object(body) => input.extend(body),
            _ => return Err(LivrRejection::from(livr::Errors::Format)),
        }
        Ok(Self(input))
    }
}
```

## Examples

```sh
cargo run --example basic            # body, query, form, schema route
cargo run --example runtime_schemas  # schemas loaded at startup
cargo run --example custom_errors    # the error format of your API
```

Each example lists `curl` commands to try it. `PORT` sets the port (3000 by default).

## Typed schemas

In `livr::schema!`, put modifiers before `required`: `["trim", "required"]`. `trim` can leave an empty string, so after it a field is optional again (`Option<...>`). The types of the rules are in the [README of livr](https://github.com/koorchik/rust-livr#types-of-the-output).

## Contributing

See [CONTRIBUTING.md](https://github.com/koorchik/rust-livr-axum/blob/master/CONTRIBUTING.md). Security issues: [SECURITY.md](https://github.com/koorchik/rust-livr-axum/blob/master/SECURITY.md).

## License

[MIT](https://github.com/koorchik/rust-livr-axum/blob/master/LICENSE).
