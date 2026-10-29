use std::error::Error as _;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, FromRequestParts};
use axum::http::{Method, Request, StatusCode, header};
use axum::middleware::from_fn_with_state;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use http_body_util::BodyExt;
use livr::Validator;
use livr_axum::{
    Livr, LivrForm, LivrQuery, LivrRejection, LivrValidated, validate_json, validate_json_with,
};
use serde_json::{Value, json};
use tower::ServiceExt;

livr::schema! {
    #[derive(Debug)]
    pub struct SignUp {
        name: ["required", {"max_length": 50}],
        email: ["required", "email", "to_lc"],
        age: ["required", "positive_integer", {"min_number": 18}],
        address: {"nested_object": {
            zip: ["required", {"like": "^\\d{5}$"}],
        }},
    }
}

livr::schema! {
    #[derive(Debug)]
    pub struct Search {
        q: ["required", "trim"],
        page: ["positive_integer", {"default": 1}],
        tags: {"list_of": "string"},
    }
}

livr::schema! {
    #[derive(Debug)]
    pub struct Password {
        password: ["required", {"min_length": 8}],
        password2: ["required", {"equal_to_field": "password"}],
    }
}

// The error format of an API, for the middleware
struct ApiError(LivrRejection);

impl From<LivrRejection> for ApiError {
    fn from(rejection: LivrRejection) -> Self {
        Self(rejection)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = json!({"success": false, "code": self.0.code()});
        (StatusCode::OK, axum::Json(body)).into_response()
    }
}

fn validator() -> Validator {
    Validator::new(&json!({
        "title": ["required", "trim"],
        "count": "integer",
    }))
    .unwrap()
}

fn app() -> Router {
    Router::new()
        .route(
            "/users",
            post(|Livr(user): Livr<SignUp>| async move {
                axum::Json(json!({"name": user.name, "email": user.email, "age": user.age}))
            }),
        )
        .route(
            "/optional-users",
            post(|user: Option<Livr<SignUp>>| async move {
                axum::Json(json!(user.map(|Livr(user)| user.name)))
            }),
        )
        .route(
            "/search",
            get(|LivrQuery(search): LivrQuery<Search>| async move {
                axum::Json(json!({"q": search.q, "page": search.page, "tags": search.tags}))
            }),
        )
        .route(
            "/optional-search",
            get(|search: Option<LivrQuery<Search>>| async move {
                axum::Json(json!(search.map(|LivrQuery(search)| search.q)))
            }),
        )
        .route("/forms", get(form_handler).post(form_handler))
        .route(
            "/passwords",
            post(|LivrForm(form): LivrForm<Password>| async move { form.password }),
        )
        .route(
            "/items",
            post(
                |LivrValidated(data): LivrValidated, again: LivrValidated, body: String| async move {
                    assert_eq!(data, again.0);
                    axum::Json(json!({"data": data, "raw": body}))
                },
            )
            .route_layer(from_fn_with_state(validator(), validate_json)),
        )
        .route(
            "/api-items",
            post(|LivrValidated(data): LivrValidated| async move { axum::Json(data) })
                .route_layer(from_fn_with_state(
                    validator(),
                    validate_json_with::<ApiError>,
                )),
        )
        .route(
            "/unvalidated",
            post(|LivrValidated(data): LivrValidated| async move { axum::Json(data) }),
        )
        .route("/schemas/sign-up", get(livr_axum::schema::<SignUp>))
        .layer(DefaultBodyLimit::max(1024))
}

async fn form_handler(LivrForm(search): LivrForm<Search>) -> axum::Json<Value> {
    axum::Json(json!({"q": search.q, "page": search.page, "tags": search.tags}))
}

async fn send(request: Request<Body>) -> (StatusCode, Value) {
    let (status, body) = send_raw(request).await;
    (status, serde_json::from_slice(&body).unwrap())
}

async fn send_raw(request: Request<Body>) -> (StatusCode, Bytes) {
    let response = app().oneshot(request).await.unwrap();
    let status = response.status();
    (
        status,
        response.into_body().collect().await.unwrap().to_bytes(),
    )
}

fn request(uri: &str, content_type: &str, body: impl Into<Body>) -> Request<Body> {
    Request::post(uri)
        .header(header::CONTENT_TYPE, content_type)
        .body(body.into())
        .unwrap()
}

fn json_request(uri: &str, body: &str) -> Request<Body> {
    request(uri, "application/json", body.to_owned())
}

fn get_request(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

// A body without a length, in chunks, as a client streams it
fn streamed(chunks: Vec<Result<&'static str, std::io::Error>>) -> Body {
    Body::from_stream(futures_util::stream::iter(chunks))
}

fn unsupported() -> (StatusCode, Value) {
    (
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        json!({"code": "UNSUPPORTED_MEDIA_TYPE"}),
    )
}

fn too_large() -> (StatusCode, Value) {
    (
        StatusCode::PAYLOAD_TOO_LARGE,
        json!({"code": "BODY_TOO_LARGE"}),
    )
}

const VALID_USER: &str = r#"{"name": "Jane", "email": "JANE@EXAMPLE.COM", "age": "42"}"#;

#[tokio::test]
async fn validates_a_json_body() {
    let (status, body) = send(json_request(
        "/users",
        r#"{"name": "Jane", "email": "JANE@EXAMPLE.COM", "age": "42", "extra": 1}"#,
    ))
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({"name": "Jane", "email": "jane@example.com", "age": 42})
    );
}

#[tokio::test]
async fn reports_all_field_errors_type_errors_too() {
    let (status, body) = send(json_request(
        "/users",
        r#"{"email": "wrong", "age": "abc", "address": {"zip": "1"}}"#,
    ))
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body,
        json!({"code": "VALIDATION_ERROR", "fields": {
            "name": "REQUIRED",
            "email": "WRONG_EMAIL",
            "age": "NOT_POSITIVE_INTEGER",
            "address": {"zip": "WRONG_FORMAT"},
        }}),
    );
}

#[tokio::test]
async fn sorts_the_fields_of_the_errors() {
    let (_, body) = send_raw(json_request("/users", r#"{"name": "", "email": "x"}"#)).await;
    assert_eq!(
        body,
        r#"{"code":"VALIDATION_ERROR","fields":{"age":"REQUIRED","email":"WRONG_EMAIL","name":"REQUIRED"}}"#,
    );
}

#[tokio::test]
async fn rejects_bad_requests() {
    let (status, body) = send(json_request("/users", "[1, 2]")).await;
    assert_eq!(
        (status, body),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({"code": "FORMAT_ERROR"})
        )
    );

    let (status, body) = send(json_request("/users", "{\"name\": ")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "INVALID_JSON");
    assert!(body["message"].is_string());

    assert_eq!(
        send(request("/users", "text/plain", "{}")).await,
        unsupported()
    );

    let large = format!(r#"{{"name": "{}"}}"#, "x".repeat(2000));
    assert_eq!(send(json_request("/users", &large)).await, too_large());
}

#[tokio::test]
async fn checks_the_content_type_of_json() {
    for content_type in [
        "application/json; charset=utf-8",
        "Application/JSON",
        "application/merge-patch+json",
    ] {
        let (status, _) = send(request("/users", content_type, VALID_USER)).await;
        assert_eq!(status, StatusCode::OK, "{content_type}");
    }
    for content_type in ["text/json", "application/x-www-form-urlencoded", ""] {
        assert_eq!(
            send(request("/users", content_type, VALID_USER)).await,
            unsupported(),
            "{content_type}"
        );
    }
    let without_content_type = Request::post("/users")
        .body(Body::from(VALID_USER))
        .unwrap();
    assert_eq!(send(without_content_type).await, unsupported());
}

#[tokio::test]
async fn rejects_empty_and_deep_json() {
    let (status, body) = send(json_request("/users", "")).await;
    assert_eq!(
        (status, &body["code"]),
        (StatusCode::BAD_REQUEST, &json!("INVALID_JSON"))
    );

    // serde_json stops at 128 levels: an error, not a stack overflow
    let deep = "[".repeat(1000);
    let (status, body) = send(json_request("/users", &deep)).await;
    assert_eq!(
        (status, &body["code"]),
        (StatusCode::BAD_REQUEST, &json!("INVALID_JSON"))
    );
    let deep = r#"{"a":"#.repeat(200);
    let (status, body) = send(json_request("/users", &deep)).await;
    assert_eq!(
        (status, &body["code"]),
        (StatusCode::BAD_REQUEST, &json!("INVALID_JSON"))
    );
}

#[tokio::test]
async fn checks_the_limit_of_streamed_bodies() {
    let chunks = || {
        vec![
            Ok(r#"{"name": ""#),
            Ok(&*"x".repeat(2000).leak()),
            Ok(r#""}"#),
        ]
    };
    let json = |uri| request(uri, "application/json", streamed(chunks()));
    assert_eq!(send(json("/users")).await, too_large());
    assert_eq!(send(json("/items")).await, too_large());
    let form = request(
        "/forms",
        "application/x-www-form-urlencoded",
        streamed(vec![Ok("q="), Ok(&*"x".repeat(2000).leak())]),
    );
    assert_eq!(send(form).await, too_large());

    // Under the limit, chunks are joined
    let (status, _) = send(request(
        "/users",
        "application/json",
        streamed(vec![Ok(&VALID_USER[..10]), Ok(&VALID_USER[10..])]),
    ))
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn hides_the_errors_of_the_body() {
    let failing = || {
        streamed(vec![
            Ok("{"),
            Err(std::io::Error::other("connection reset at 10.0.0.1")),
        ])
    };
    for uri in ["/users", "/items"] {
        let (status, body) = send_raw(request(uri, "application/json", failing())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(body, r#"{"code":"INVALID_BODY"}"#, "{uri}");
    }
    let form = request("/forms", "application/x-www-form-urlencoded", failing());
    assert_eq!(
        send(form).await,
        (StatusCode::BAD_REQUEST, json!({"code": "INVALID_BODY"}))
    );
}

#[tokio::test]
async fn gives_none_without_a_content_type() {
    let without_content_type = Request::post("/optional-users")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        send(without_content_type).await,
        (StatusCode::OK, json!(null))
    );
    assert_eq!(
        send(json_request("/optional-users", VALID_USER)).await,
        (StatusCode::OK, json!("Jane"))
    );
    assert_eq!(
        send(request("/optional-users", "text/plain", VALID_USER)).await,
        unsupported()
    );
    let (status, body) = send(json_request("/optional-users", "{}")).await;
    assert_eq!(
        (status, &body["code"]),
        (StatusCode::UNPROCESSABLE_ENTITY, &json!("VALIDATION_ERROR"))
    );
}

#[tokio::test]
async fn converts_query_strings() {
    assert_eq!(
        send(get_request("/search?q=+rust+&tags=a&tags=b")).await,
        (
            StatusCode::OK,
            json!({"q": "rust", "page": 1, "tags": ["a", "b"]})
        ),
    );
    assert_eq!(
        send(get_request("/search?q=x&page=2&tags[]=a")).await,
        (StatusCode::OK, json!({"q": "x", "page": 2, "tags": ["a"]})),
    );
    assert_eq!(
        send(get_request("/search?page=zero")).await,
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({"code": "VALIDATION_ERROR", "fields": {"q": "REQUIRED", "page": "NOT_POSITIVE_INTEGER"}}),
        ),
    );
}

#[tokio::test]
async fn makes_a_list_of_a_single_value() {
    assert_eq!(
        send(get_request("/search?q=x&tags=a")).await,
        (StatusCode::OK, json!({"q": "x", "page": 1, "tags": ["a"]})),
    );
    assert_eq!(
        send(get_request("/search?q=x&tags%5B%5D=a")).await,
        (StatusCode::OK, json!({"q": "x", "page": 1, "tags": ["a"]})),
    );
    // An empty input is no value
    assert_eq!(
        send(get_request("/search?q=x&tags=")).await,
        (StatusCode::OK, json!({"q": "x", "page": 1, "tags": null})),
    );
    // A group of checkboxes with one box checked
    let form = request("/forms", "application/x-www-form-urlencoded", "q=x&tags=a");
    assert_eq!(
        send(form).await,
        (StatusCode::OK, json!({"q": "x", "page": 1, "tags": ["a"]}))
    );
}

#[tokio::test]
async fn decodes_query_strings() {
    assert_eq!(
        send(get_request("/search?q=%FF&a[b]=1&address%5Bzip%5D=1")).await,
        (
            StatusCode::OK,
            json!({"q": "\u{FFFD}", "page": 1, "tags": null})
        ),
    );
}

#[tokio::test]
async fn gives_none_without_a_query_string() {
    assert_eq!(
        send(get_request("/optional-search")).await,
        (StatusCode::OK, json!(null))
    );
    assert_eq!(
        send(get_request("/optional-search?q=rust")).await,
        (StatusCode::OK, json!("rust"))
    );
    let (status, body) = send(get_request("/optional-search?")).await;
    assert_eq!(
        (status, body),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({"code": "VALIDATION_ERROR", "fields": {"q": "REQUIRED"}})
        )
    );
}

#[tokio::test]
async fn validates_forms() {
    let form = request(
        "/forms",
        "application/x-www-form-urlencoded",
        "q=forms&page=3",
    );
    assert_eq!(
        send(form).await,
        (
            StatusCode::OK,
            json!({"q": "forms", "page": 3, "tags": null})
        )
    );

    let request_get = Request::builder()
        .method(Method::GET)
        .uri("/forms?q=query")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        send(request_get).await,
        (
            StatusCode::OK,
            json!({"q": "query", "page": 1, "tags": null})
        )
    );

    assert_eq!(
        send(request("/forms", "application/json", "{}")).await,
        unsupported()
    );
    assert_eq!(
        send(request(
            "/forms",
            "multipart/form-data; boundary=x",
            "--x\r\nContent-Disposition: form-data; name=\"q\"\r\n\r\nx\r\n--x--\r\n",
        ))
        .await,
        unsupported()
    );
    let without_content_type = Request::post("/forms").body(Body::from("q=x")).unwrap();
    assert_eq!(send(without_content_type).await, unsupported());
}

#[tokio::test]
async fn reads_the_query_string_of_head_requests() {
    let head = |uri| Request::head(uri).body(Body::empty()).unwrap();
    let (status, body) = send_raw(head("/forms?q=x")).await;
    assert_eq!((status, body.len()), (StatusCode::OK, 0));
    let (status, _) = send_raw(head("/forms")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn drops_keys_that_are_not_fields() {
    let keys: String = (0..100).map(|i| format!("k{i}=&")).collect();
    let form = request(
        "/forms",
        "application/x-www-form-urlencoded",
        format!("{keys}q=x"),
    );
    assert_eq!(
        send(form).await,
        (StatusCode::OK, json!({"q": "x", "page": 1, "tags": null}))
    );
}

#[tokio::test]
async fn compares_fields_of_forms() {
    let form =
        |body: &'static str| request("/passwords", "application/x-www-form-urlencoded", body);
    let (status, _) = send_raw(form("password=12345678&password2=12345678")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        send(form("password=12345678&password2=87654321")).await,
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({"code": "VALIDATION_ERROR", "fields": {"password2": "FIELDS_NOT_EQUAL"}})
        )
    );
}

#[tokio::test]
async fn validates_with_a_validator_known_at_run_time() {
    let (status, body) = send(json_request(
        "/items",
        r#"{"title": " Book ", "count": "2", "x": 1}"#,
    ))
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({"data": {"title": "Book", "count": 2}, "raw": r#"{"title": " Book ", "count": "2", "x": 1}"#}),
    );

    assert_eq!(
        send(json_request("/items", r#"{"count": 1.5}"#)).await,
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({"code": "VALIDATION_ERROR", "fields": {"title": "REQUIRED", "count": "NOT_INTEGER"}}),
        ),
    );

    let large = format!(r#"{{"title": "{}"}}"#, "x".repeat(2000));
    assert_eq!(send(json_request("/items", &large)).await, too_large());

    assert_eq!(
        send(request("/items", "text/plain", "{}")).await,
        unsupported()
    );
    let (status, body) = send(json_request("/items", "{\"title\": ")).await;
    assert_eq!(
        (status, &body["code"]),
        (StatusCode::BAD_REQUEST, &json!("INVALID_JSON"))
    );

    assert_eq!(
        send(json_request("/unvalidated", "{}")).await,
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"code": "INTERNAL_ERROR"})
        ),
    );
}

#[tokio::test]
async fn formats_the_errors_of_the_middleware() {
    assert_eq!(
        send(json_request("/api-items", r#"{"count": 1.5}"#)).await,
        (
            StatusCode::OK,
            json!({"success": false, "code": "VALIDATION_ERROR"})
        ),
    );
    assert_eq!(
        send(request("/api-items", "text/plain", "{}")).await,
        (
            StatusCode::OK,
            json!({"success": false, "code": "UNSUPPORTED_MEDIA_TYPE"})
        ),
    );
    assert_eq!(
        send(json_request("/api-items", r#"{"title": "Book"}"#)).await,
        (StatusCode::OK, json!({"title": "Book"})),
    );
}

#[tokio::test]
async fn serves_schemas() {
    let (status, body) = send(get_request("/schemas/sign-up")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        serde_json::from_str::<Value>(<SignUp as livr::Schema>::SCHEMA).unwrap()
    );
    assert_eq!(body["email"], json!(["required", "email", "to_lc"]));
}

#[tokio::test]
async fn extracts_validated_data_more_than_once() {
    let (mut parts, ()) = Request::new(()).into_parts();
    parts.extensions.insert(LivrValidated(json!({"a": 1})));
    for _ in 0..2 {
        let LivrValidated(data) = LivrValidated::from_request_parts(&mut parts, &())
            .await
            .unwrap();
        assert_eq!(data, json!({"a": 1}));
    }
}

#[test]
fn wraps_values_like_axum() {
    let mut query = LivrQuery::from(vec![1]);
    query.push(2);
    assert_eq!(*query, [1, 2]);
    let form: LivrForm<_> = "x".into();
    assert_eq!(form.len(), 1);
    let mut livr = Livr::from(5);
    *livr += 1;
    assert_eq!(livr, Livr(6));
    let mut validated = LivrValidated::from(json!({"a": 1}));
    validated["a"] = json!(2);
    assert_eq!(validated.get("a"), Some(&json!(2)));
}

#[test]
fn describes_rejections() {
    let invalid = LivrRejection::from(SignUp::validate(&json!({})).unwrap_err());
    assert_eq!(invalid.to_string(), "invalid data");
    assert_eq!(
        invalid.source().unwrap().to_string(),
        r#"{"age":"REQUIRED","email":"REQUIRED","name":"REQUIRED"}"#
    );
    assert_eq!(invalid.code(), "VALIDATION_ERROR");
    assert!(invalid.errors().is_some());
    let errors = invalid.into_errors().unwrap();
    assert_eq!(
        errors.to_string(),
        r#"{"age":"REQUIRED","email":"REQUIRED","name":"REQUIRED"}"#
    );

    let format = LivrRejection::from(SignUp::validate(&json!([])).unwrap_err());
    assert_eq!(format.code(), "FORMAT_ERROR");
    assert_eq!(format.body(), json!({"code": "FORMAT_ERROR"}));

    let json_error = serde_json::from_str::<Value>("{").unwrap_err();
    let message = json_error.to_string();
    let invalid_json = LivrRejection::InvalidJson(json_error);
    assert_eq!(invalid_json.to_string(), "invalid JSON");
    assert_eq!(invalid_json.source().unwrap().to_string(), message);
    assert_eq!(
        invalid_json.body(),
        json!({"code": "INVALID_JSON", "message": message})
    );
    assert!(invalid_json.into_errors().is_none());

    for (rejection, status, message) in [
        (
            LivrRejection::UnsupportedMediaType,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported media type",
        ),
        (
            LivrRejection::BodyTooLarge,
            StatusCode::PAYLOAD_TOO_LARGE,
            "body too large",
        ),
        (
            LivrRejection::InvalidBody,
            StatusCode::BAD_REQUEST,
            "body could not be read",
        ),
        (
            LivrRejection::MissingMiddleware,
            StatusCode::INTERNAL_SERVER_ERROR,
            "livr_axum::LivrValidated needs the livr_axum::validate_json middleware on the route",
        ),
    ] {
        assert_eq!(rejection.status(), status);
        assert_eq!(rejection.to_string(), message);
        assert!(rejection.source().is_none());
        assert_eq!(rejection.body(), json!({"code": rejection.code()}));
    }
}
