//! The functions for extractors and frameworks of your own.

use axum::Router;
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Request};
use axum::http::{StatusCode, header};
use axum::routing::{get, post};
use http_body_util::BodyExt;
use livr::SchemaField;
use livr_axum::{
    LivrRejection, is_form_content_type, is_json_content_type, parse_json, parse_urlencoded,
    read_body, read_form, read_json,
};
use serde_json::{Map, Value, json};
use tower::ServiceExt;

// Each route answers with what the function read, or its rejection
fn app() -> Router {
    Router::new()
        .route(
            "/json",
            post(|request: Request| async { read_json(request).await.map(axum::Json) }),
        )
        .route("/form", get(read_any_form).post(read_any_form))
        .route(
            "/form-of-fields",
            post(|request: Request| async {
                let fields = [SchemaField::new("q", false), SchemaField::new("tags", true)];
                read_form(request, Some(&fields))
                    .await
                    .map(|data| axum::Json(Value::Object(data)))
            }),
        )
        .route(
            "/body",
            post(|request: Request| async {
                read_body(request).await.map(|body| body.len().to_string())
            }),
        )
        .route("/input/{id}", post(framework_input))
        .layer(DefaultBodyLimit::max(64))
}

async fn read_any_form(request: Request) -> Result<axum::Json<Value>, LivrRejection> {
    let data = read_form(request, None).await?;
    Ok(axum::Json(Value::Object(data)))
}

// What a framework does with its own rules: the query string, then a JSON
// body if there is one, then the path parameters, which come last
async fn framework_input(
    axum::extract::Path(id): axum::extract::Path<String>,
    request: Request,
) -> Result<axum::Json<Value>, (StatusCode, String)> {
    let map = |rejection: LivrRejection| (rejection.status(), rejection.code().to_owned());
    let (parts, body) = request.into_parts();
    let query = parts.uri.query().unwrap_or_default();
    let mut input = parse_urlencoded(query.as_bytes(), None);
    let is_json = is_json_content_type(&parts.headers);
    let body = read_body(Request::from_parts(parts, body))
        .await
        .map_err(map)?;
    if !body.is_empty() {
        if !is_json {
            return Err(map(LivrRejection::UnsupportedMediaType));
        }
        match parse_json(&body).map_err(map)? {
            Value::Object(fields) => input.extend(fields),
            _ => return Err(map(LivrRejection::from(livr::Errors::Format))),
        }
    }
    input.insert("id".to_owned(), Value::String(id));
    Ok(axum::Json(Value::Object(input)))
}

async fn send(request: Request) -> (StatusCode, String) {
    let response = app().oneshot(request).await.unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

fn post_request(uri: &str, content_type: Option<&str>, body: &'static str) -> Request {
    let mut request = Request::post(uri);
    if let Some(content_type) = content_type {
        request = request.header(header::CONTENT_TYPE, content_type);
    }
    request.body(Body::from(body)).unwrap()
}

const JSON: Option<&str> = Some("application/json");
const FORM: Option<&str> = Some("application/x-www-form-urlencoded");

#[tokio::test]
async fn reads_json() {
    assert_eq!(
        send(post_request("/json", JSON, r#"{"a": [1]}"#)).await,
        (StatusCode::OK, r#"{"a":[1]}"#.to_owned())
    );
    // Not only objects: validation rejects the others
    assert_eq!(
        send(post_request("/json", JSON, "[1]")).await,
        (StatusCode::OK, "[1]".to_owned())
    );
    let (status, body) = send(post_request("/json", JSON, "{")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body.starts_with(r#"{"code":"INVALID_JSON","message":"#),
        "{body}"
    );
    assert_eq!(
        send(post_request("/json", None, "{}")).await,
        (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            r#"{"code":"UNSUPPORTED_MEDIA_TYPE"}"#.to_owned()
        )
    );
    let large = Box::leak(format!(r#"{{"a": "{}"}}"#, "x".repeat(64)).into_boxed_str());
    assert_eq!(
        send(post_request("/json", JSON, large)).await,
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            r#"{"code":"BODY_TOO_LARGE"}"#.to_owned()
        )
    );
}

#[tokio::test]
async fn reads_forms() {
    assert_eq!(
        send(post_request("/form", FORM, "q=a+b&tags=x&other=1")).await,
        (
            StatusCode::OK,
            r#"{"other":"1","q":"a b","tags":"x"}"#.to_owned()
        )
    );
    assert_eq!(
        send(post_request(
            "/form-of-fields",
            FORM,
            "q=a+b&tags=x&other=1"
        ))
        .await,
        (StatusCode::OK, r#"{"q":"a b","tags":["x"]}"#.to_owned())
    );
    // GET reads the query string, as LivrForm does
    let request = Request::get("/form?ids[]=1&ids[]=2")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        send(request).await,
        (StatusCode::OK, r#"{"ids":["1","2"]}"#.to_owned())
    );
    assert_eq!(
        send(post_request("/form", JSON, "{}")).await.0,
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
}

#[tokio::test]
async fn reads_bodies_with_the_limit() {
    assert_eq!(
        send(post_request("/body", None, "12345")).await,
        (StatusCode::OK, "5".to_owned())
    );
    let large = Box::leak("x".repeat(65).into_boxed_str());
    assert_eq!(
        send(post_request("/body", None, large)).await.0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
}

#[tokio::test]
async fn builds_the_input_of_a_framework() {
    let request = post_request(
        "/input/7?page=2&id=1",
        JSON,
        r#"{"title": "Book", "id": 3}"#,
    );
    assert_eq!(
        send(request).await,
        (
            StatusCode::OK,
            r#"{"id":"7","page":"2","title":"Book"}"#.to_owned()
        )
    );
    // No body: the query string only, without a Content-Type
    let request = Request::post("/input/7?page=2")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        send(request).await,
        (StatusCode::OK, r#"{"id":"7","page":"2"}"#.to_owned())
    );
    assert_eq!(
        send(post_request("/input/7", FORM, "a=1")).await,
        (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "UNSUPPORTED_MEDIA_TYPE".to_owned()
        )
    );
    assert_eq!(
        send(post_request("/input/7", JSON, "[]")).await,
        (StatusCode::UNPROCESSABLE_ENTITY, "FORMAT_ERROR".to_owned())
    );
}

#[test]
fn parses_without_a_request() {
    assert_eq!(
        parse_urlencoded("a=1&a=2&b[]=3&c=".as_bytes(), None),
        Map::from_iter([
            ("a".to_owned(), json!(["1", "2"])),
            ("b".to_owned(), json!(["3"])),
            ("c".to_owned(), json!("")),
        ])
    );
    assert_eq!(parse_json(br#"{"a": 1}"#).unwrap(), json!({"a": 1}));
    assert_eq!(parse_json(b"").unwrap_err().code(), "INVALID_JSON");

    let headers = |value: &'static str| {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, value.parse().unwrap());
        headers
    };
    assert!(is_json_content_type(&headers("application/vnd.api+json")));
    assert!(!is_json_content_type(&headers("text/plain")));
    assert!(is_form_content_type(&headers(
        "application/x-www-form-urlencoded; charset=UTF-8"
    )));
    assert!(!is_form_content_type(&headers("multipart/form-data")));
}
