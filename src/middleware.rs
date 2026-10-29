use axum::RequestExt;
use axum::body::Body;
use axum::extract::{FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use livr::Validator;
use serde_json::Value;

use crate::read::{parse_json, read_limited_body};
use crate::rejection::LivrRejection;
use crate::urlencoded::is_json_content_type;

/// A middleware that validates the JSON body with a validator known at run
/// time, like one built from a schema in a database. The handler gets the
/// output with [`LivrValidated`]; an invalid request gets a
/// [`LivrRejection`] and does not reach the handler.
///
/// ```
/// use axum::{Router, middleware::from_fn_with_state, routing::post};
/// use livr::Validator;
/// use livr_axum::{LivrValidated, validate_json};
///
/// async fn create(LivrValidated(data): LivrValidated) -> String {
///     data.to_string()
/// }
///
/// let validator = Validator::new(&serde_json::json!({"name": "required"}))?;
/// let app: Router = Router::new().route(
///     "/items",
///     post(create).route_layer(from_fn_with_state(validator, validate_json)),
/// );
/// # Ok::<(), livr::SchemaError>(())
/// ```
///
/// The body stays readable by the handler, so other extractors of the body
/// still work. Its limit is the one of `axum::extract::DefaultBodyLimit`.
///
/// For the error format of your API, use [`validate_json_with`].
pub async fn validate_json(
    State(validator): State<Validator>,
    request: Request,
    next: Next,
) -> Response {
    validate_json_with::<LivrRejection>(State(validator), request, next).await
}

/// [`validate_json`] with the error type of your API: a rejected request
/// gets `E::from(rejection)` as its response.
///
/// ```
/// use axum::{Json, Router, middleware::from_fn_with_state, routing::post};
/// use axum::response::{IntoResponse, Response};
/// use livr::Validator;
/// use livr_axum::{LivrRejection, LivrValidated, validate_json_with};
///
/// struct ApiError(LivrRejection);
///
/// impl From<LivrRejection> for ApiError {
///     fn from(rejection: LivrRejection) -> Self {
///         Self(rejection)
///     }
/// }
///
/// impl IntoResponse for ApiError {
///     fn into_response(self) -> Response {
///         let body = serde_json::json!({"success": false, "error": self.0.body()});
///         (self.0.status(), Json(body)).into_response()
///     }
/// }
///
/// async fn create(LivrValidated(data): LivrValidated) -> String {
///     data.to_string()
/// }
///
/// let validator = Validator::new(&serde_json::json!({"name": "required"}))?;
/// let app: Router = Router::new().route(
///     "/items",
///     post(create).route_layer(from_fn_with_state(validator, validate_json_with::<ApiError>)),
/// );
/// # Ok::<(), livr::SchemaError>(())
/// ```
pub async fn validate_json_with<E>(
    State(validator): State<Validator>,
    request: Request,
    next: Next,
) -> Response
where
    E: From<LivrRejection> + IntoResponse,
{
    match validate(&validator, request).await {
        Ok(request) => next.run(request).await,
        Err(rejection) => E::from(rejection).into_response(),
    }
}

async fn validate(validator: &Validator, request: Request) -> Result<Request, LivrRejection> {
    if !is_json_content_type(request.headers()) {
        return Err(LivrRejection::UnsupportedMediaType);
    }
    // The limit of DefaultBodyLimit, as for read_body. The bytes go back
    // into the request, for the handler
    let mut request = request.with_limited_body();
    let body = read_limited_body(std::mem::take(request.body_mut())).await?;
    let output = validator.validate_owned(parse_json(&body)?)?;
    request.extensions_mut().insert(LivrValidated(output));
    *request.body_mut() = Body::from(body);
    Ok(request)
}

/// The output of the [`validate_json`] middleware: the data cleaned by the
/// rules, with the fields of the schema only.
///
/// Like `axum::Extension`, each extraction is a copy of the output, so it
/// can be extracted more than once in a request, by another middleware and
/// by the handler.
#[derive(Clone, Debug, Default, PartialEq)]
#[must_use]
pub struct LivrValidated(pub Value);

impl std::ops::Deref for LivrValidated {
    type Target = Value;

    #[inline]
    fn deref(&self) -> &Value {
        &self.0
    }
}

impl std::ops::DerefMut for LivrValidated {
    #[inline]
    fn deref_mut(&mut self) -> &mut Value {
        &mut self.0
    }
}

impl From<Value> for LivrValidated {
    #[inline]
    fn from(value: Value) -> Self {
        Self(value)
    }
}

impl<S: Send + Sync> FromRequestParts<S> for LivrValidated {
    type Rejection = LivrRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Self>()
            .cloned()
            .ok_or(LivrRejection::MissingMiddleware)
    }
}
