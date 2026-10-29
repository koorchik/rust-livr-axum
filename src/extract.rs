use axum::extract::{
    FromRequest, FromRequestParts, OptionalFromRequest, OptionalFromRequestParts, Request,
};
use axum::http::request::Parts;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use livr::Schema;
use serde_json::Value;

use crate::read::{read_form, read_json};
use crate::rejection::LivrRejection;
use crate::urlencoded::parse_urlencoded;

// Deref, DerefMut and From for the wrappers, as axum has for its extractors
macro_rules! impl_wrapper {
    ($name:ident) => {
        impl<T> std::ops::Deref for $name<T> {
            type Target = T;

            #[inline]
            fn deref(&self) -> &T {
                &self.0
            }
        }

        impl<T> std::ops::DerefMut for $name<T> {
            #[inline]
            fn deref_mut(&mut self) -> &mut T {
                &mut self.0
            }
        }

        impl<T> From<T> for $name<T> {
            #[inline]
            fn from(inner: T) -> Self {
                Self(inner)
            }
        }
    };
}

/// A JSON body validated by a typed schema: the type of `livr::schema!`.
///
/// The body must be `application/json` (or `application/*+json`). It is
/// parsed, then validated with [`Schema::validate_owned`]: values move to
/// the output, nothing is copied. The body limit is the one of
/// `axum::extract::DefaultBodyLimit` (2 MB by default); the parsed body
/// takes several times its size in memory, so a tighter limit is advisable
/// for routes with small bodies.
///
/// ```
/// # livr::schema! { pub struct SignUp { name: "required" } }
/// use livr_axum::Livr;
///
/// async fn sign_up(Livr(user): Livr<SignUp>) -> String {
///     format!("Hello, {}", user.name)
/// }
/// # let _: axum::Router = axum::Router::new().route("/", axum::routing::post(sign_up));
/// ```
///
/// `Option<Livr<T>>` is `None` when the request has no `Content-Type`
/// header, like `Option<axum::Json<T>>`; another `Content-Type` and invalid
/// data are still rejected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[must_use]
pub struct Livr<T>(pub T);

impl_wrapper!(Livr);

impl<T, S> FromRequest<S> for Livr<T>
where
    T: Schema,
    S: Send + Sync,
{
    type Rejection = LivrRejection;

    async fn from_request(request: Request, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(T::validate_owned(read_json(request).await?)?))
    }
}

impl<T, S> OptionalFromRequest<S> for Livr<T>
where
    T: Schema,
    S: Send + Sync,
{
    type Rejection = LivrRejection;

    async fn from_request(request: Request, state: &S) -> Result<Option<Self>, Self::Rejection> {
        if request.headers().contains_key(header::CONTENT_TYPE) {
            <Self as FromRequest<S>>::from_request(request, state)
                .await
                .map(Some)
        } else {
            Ok(None)
        }
    }
}

/// A query string validated by a typed schema.
///
/// The values are strings, so the rules convert them: `positive_integer`
/// makes `?page=2` the number `2`. A list field (`list_of`) is a list even
/// with one value (`?tags=a`). A key given more than once is a list, and so
/// is a key ending with `[]` (`?tags[]=a`). Keys are not nested: `a[b]` is
/// a key of its own, as for `axum::extract::Query`.
///
/// Keys that are not fields of the schema are dropped before validation,
/// so a field that `equal_to_field` or a custom rule reads needs rules of
/// its own in the schema.
///
/// ```
/// livr::schema! {
///     pub struct Search {
///         q: ["required", {"max_length": 100}],
///         page: {"default": 1},
///     }
/// }
///
/// async fn search(livr_axum::LivrQuery(search): livr_axum::LivrQuery<Search>) -> String {
///     format!("{} (page {})", search.q, search.page)
/// }
/// # let _: axum::Router = axum::Router::new().route("/", axum::routing::get(search));
/// ```
///
/// `Option<LivrQuery<T>>` is `None` when the URI has no query string (no
/// `?`), for a route that shows a page without one and validates it
/// otherwise; an invalid query string is still rejected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[must_use]
pub struct LivrQuery<T>(pub T);

impl_wrapper!(LivrQuery);

impl<T, S> FromRequestParts<S> for LivrQuery<T>
where
    T: Schema,
    S: Send + Sync,
{
    type Rejection = LivrRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let query = parts.uri.query().unwrap_or_default();
        validate_fields(query.as_bytes()).map(Self)
    }
}

impl<T, S> OptionalFromRequestParts<S> for LivrQuery<T>
where
    T: Schema,
    S: Send + Sync,
{
    type Rejection = LivrRejection;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        match parts.uri.query() {
            Some(query) => validate_fields(query.as_bytes()).map(|data| Some(Self(data))),
            None => Ok(None),
        }
    }
}

/// A form validated by a typed schema: an `application/x-www-form-urlencoded`
/// body, or the query string for `GET` and `HEAD`, like `axum::Form`.
///
/// The values are strings, and keys that are not fields are dropped, as for
/// [`LivrQuery`]. `multipart/form-data` is rejected with 415.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[must_use]
pub struct LivrForm<T>(pub T);

impl_wrapper!(LivrForm);

impl<T, S> FromRequest<S> for LivrForm<T>
where
    T: Schema,
    S: Send + Sync,
{
    type Rejection = LivrRejection;

    async fn from_request(request: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let data = read_form(request, Some(T::FIELDS)).await?;
        Ok(Self(T::validate_owned(Value::Object(data))?))
    }
}

fn validate_fields<T: Schema>(input: &[u8]) -> Result<T, LivrRejection> {
    let data = parse_urlencoded(input, Some(T::FIELDS));
    Ok(T::validate_owned(Value::Object(data))?)
}

/// A handler that serves the LIVR schema of a type as JSON, for a frontend
/// that validates with the same rules:
/// `.route("/schemas/sign-up", get(livr_axum::schema::<SignUp>))`.
///
/// Custom rules and aliases are not in the schema: the frontend registers
/// them itself.
pub async fn schema<T: Schema>() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        T::SCHEMA,
    )
        .into_response()
}
