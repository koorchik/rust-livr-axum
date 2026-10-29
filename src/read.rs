use axum::RequestExt;
use axum::body::{Body, Bytes};
use axum::extract::Request;
use axum::http::Method;
use livr::SchemaField;
use serde_json::{Map, Value};

use crate::rejection::LivrRejection;
use crate::urlencoded::{is_form_content_type, is_json_content_type, parse_urlencoded};

/// Reads the body of a request, with the limit of
/// `axum::extract::DefaultBodyLimit` (2 MB by default), as the extractors
/// of the body do.
///
/// # Errors
///
/// [`LivrRejection::BodyTooLarge`] over the limit, also for a body streamed
/// without a `Content-Length`, and [`LivrRejection::InvalidBody`] when the
/// body cannot be read; the message of the error of the body is not kept,
/// as it may describe the server.
pub async fn read_body(request: Request) -> Result<Bytes, LivrRejection> {
    read_limited_body(request.with_limited_body().into_body()).await
}

// A body already limited by `with_limited_body`
pub(crate) async fn read_limited_body(body: Body) -> Result<Bytes, LivrRejection> {
    axum::body::to_bytes(body, usize::MAX)
        .await
        .map_err(|error| {
            if is_length_limit(&error) {
                LivrRejection::BodyTooLarge
            } else {
                LivrRejection::InvalidBody
            }
        })
}

// The limit error is wrapped by the bodies: it is somewhere in the sources
fn is_length_limit(error: &axum::Error) -> bool {
    let mut source: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(error) = source {
        if error.is::<http_body_util::LengthLimitError>() {
            return true;
        }
        source = error.source();
    }
    false
}

/// Reads a JSON body: checks the `Content-Type` with
/// [`is_json_content_type`], reads the body with [`read_body`] and parses
/// it with [`parse_json`]. Any JSON value is returned, not only objects:
/// validation gives `FORMAT_ERROR` for the others.
///
/// # Errors
///
/// [`LivrRejection::UnsupportedMediaType`] for another `Content-Type`, or
/// none; the errors of [`read_body`] and [`parse_json`].
pub async fn read_json(request: Request) -> Result<Value, LivrRejection> {
    if !is_json_content_type(request.headers()) {
        return Err(LivrRejection::UnsupportedMediaType);
    }
    parse_json(&read_body(request).await?)
}

/// Parses a JSON body. The parsed value takes several times the size of
/// the body in memory: the limit of the body bounds it.
///
/// # Errors
///
/// [`LivrRejection::InvalidJson`] for malformed JSON, an empty body, or
/// more than 128 levels of nesting.
pub fn parse_json(body: &[u8]) -> Result<Value, LivrRejection> {
    serde_json::from_slice(body).map_err(LivrRejection::InvalidJson)
}

/// Reads a form, as [`LivrForm`](crate::LivrForm) does: the query string
/// for `GET` and `HEAD`, else an `application/x-www-form-urlencoded` body
/// (with the limit of [`read_body`]). The fields are converted by
/// [`parse_urlencoded`].
///
/// # Errors
///
/// [`LivrRejection::UnsupportedMediaType`] when a body is not a form (a
/// `multipart/form-data` body too), and the errors of [`read_body`].
pub async fn read_form(
    request: Request,
    fields: Option<&[SchemaField]>,
) -> Result<Map<String, Value>, LivrRejection> {
    if request.method() == Method::GET || request.method() == Method::HEAD {
        let query = request.uri().query().unwrap_or_default();
        return Ok(parse_urlencoded(query.as_bytes(), fields));
    }
    if !is_form_content_type(request.headers()) {
        return Err(LivrRejection::UnsupportedMediaType);
    }
    Ok(parse_urlencoded(&read_body(request).await?, fields))
}
