use axum::body::Body;
use axum::extract::rejection::BytesRejection;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use livr::Errors;
use serde_json::{Value, json};

/// Why a request was rejected. As a response, it is a status and a JSON
/// error object with a `SCREAMING_SNAKE_CASE` code:
///
/// | | Status | Body |
/// |---|---|---|
/// | Invalid fields | 422 | `{"code": "VALIDATION_ERROR", "fields": {"email": "WRONG_EMAIL"}}` |
/// | The data is not an object | 422 | `{"code": "FORMAT_ERROR"}` |
/// | Malformed JSON | 400 | `{"code": "INVALID_JSON", "message": "..."}` |
/// | Not JSON or not a form (`Content-Type`) | 415 | `{"code": "UNSUPPORTED_MEDIA_TYPE"}` |
/// | Body over the limit (`DefaultBodyLimit`) | 413 | `{"code": "BODY_TOO_LARGE"}` |
/// | Body not read | 400 | `{"code": "INVALID_BODY"}` |
/// | [`LivrValidated`](crate::LivrValidated) without [`validate_json`](crate::validate_json) | 500 | `{"code": "INTERNAL_ERROR"}` |
///
/// The keys of `fields` are in alphabetical order, as `serde_json` sorts
/// them.
///
/// For another format, convert it into your own error type with
/// [`status`](Self::status), [`code`](Self::code), [`body`](Self::body) and
/// [`errors`](Self::errors): wrap the extractors with
/// `#[derive(FromRequest)]`, and use
/// [`validate_json_with`](crate::validate_json_with) for the middleware.
/// The examples of the crate have one.
#[derive(Debug)]
#[non_exhaustive]
pub enum LivrRejection {
    /// The data is invalid: the errors of the fields.
    Invalid(Errors),
    /// The body is not valid JSON.
    InvalidJson(serde_json::Error),
    /// The `Content-Type` is not the one of the extractor.
    UnsupportedMediaType,
    /// The body is over the limit of `DefaultBodyLimit`.
    BodyTooLarge,
    /// The body could not be read.
    InvalidBody,
    /// [`LivrValidated`](crate::LivrValidated) is used on a route without
    /// the [`validate_json`](crate::validate_json) middleware.
    MissingMiddleware,
}

impl LivrRejection {
    /// The errors of the fields, when the data is invalid.
    #[must_use]
    pub fn errors(&self) -> Option<&Errors> {
        match self {
            Self::Invalid(errors) => Some(errors),
            _ => None,
        }
    }

    /// The errors of the fields, when the data is invalid, by value: for an
    /// error type of your own that keeps them.
    #[must_use]
    pub fn into_errors(self) -> Option<Errors> {
        match self {
            Self::Invalid(errors) => Some(errors),
            _ => None,
        }
    }

    /// The machine-readable code of the error.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid(Errors::Format) => "FORMAT_ERROR",
            Self::Invalid(_) => "VALIDATION_ERROR",
            Self::InvalidJson(_) => "INVALID_JSON",
            Self::UnsupportedMediaType => "UNSUPPORTED_MEDIA_TYPE",
            Self::BodyTooLarge => "BODY_TOO_LARGE",
            Self::InvalidBody => "INVALID_BODY",
            Self::MissingMiddleware => "INTERNAL_ERROR",
        }
    }

    /// The HTTP status of the response.
    #[must_use]
    pub fn status(&self) -> StatusCode {
        match self {
            Self::Invalid(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::InvalidJson(_) | Self::InvalidBody => StatusCode::BAD_REQUEST,
            Self::UnsupportedMediaType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::BodyTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::MissingMiddleware => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// The JSON error object of the response.
    #[must_use]
    pub fn body(&self) -> Value {
        match self {
            Self::Invalid(Errors::Fields(fields)) => json!({"code": self.code(), "fields": fields}),
            Self::InvalidJson(error) => json!({"code": self.code(), "message": error.to_string()}),
            _ => json!({"code": self.code()}),
        }
    }
}

impl From<Errors> for LivrRejection {
    fn from(errors: Errors) -> Self {
        Self::Invalid(errors)
    }
}

impl From<BytesRejection> for LivrRejection {
    fn from(rejection: BytesRejection) -> Self {
        if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
            Self::BodyTooLarge
        } else {
            Self::InvalidBody
        }
    }
}

impl IntoResponse for LivrRejection {
    fn into_response(self) -> Response {
        json_response(self.status(), &self.body())
    }
}

// The errors of the fields and of the JSON parser are the source, not part
// of the message, so that error reports print each of them once
impl std::fmt::Display for LivrRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(_) => f.write_str("invalid data"),
            Self::InvalidJson(_) => f.write_str("invalid JSON"),
            Self::UnsupportedMediaType => f.write_str("unsupported media type"),
            Self::BodyTooLarge => f.write_str("body too large"),
            Self::InvalidBody => f.write_str("body could not be read"),
            Self::MissingMiddleware => f.write_str(
                "livr_axum::LivrValidated needs the livr_axum::validate_json middleware on the route",
            ),
        }
    }
}

impl std::error::Error for LivrRejection {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Invalid(errors) => Some(errors),
            Self::InvalidJson(error) => Some(error),
            _ => None,
        }
    }
}

// Without the json feature of axum, which pulls serde_path_to_error
pub(crate) fn json_response(status: StatusCode, body: &Value) -> Response {
    let mut response = Response::new(Body::from(body.to_string()));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/json"),
    );
    response
}
