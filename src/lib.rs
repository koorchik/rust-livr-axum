//! [axum](https://docs.rs/axum) extractors for [LIVR](https://livr-spec.org)
//! validation, with [livr](https://docs.rs/livr), an implementation of LIVR
//! 2.0.
//!
//! - [`Livr<T>`]: a JSON body validated by a typed schema (`livr::schema!`).
//! - [`LivrQuery<T>`], [`LivrForm<T>`]: a query string, a form. Their values
//!   are strings: the rules convert them (`"25"` to `25`).
//! - [`validate_json`] and [`LivrValidated`]: a JSON body validated by a
//!   schema known at run time, like one loaded from a database;
//!   [`validate_json_with`] for your own error type.
//! - [`schema`]: a handler that serves the schema of a type, for clients that
//!   validate with the same rules.
//! - To build your own extractor or framework, the functions that the
//!   extractors use: [`read_json`], [`read_form`], [`read_body`],
//!   [`parse_json`], [`parse_urlencoded`], [`is_json_content_type`] and
//!   [`is_form_content_type`].
//!
//! Validation never stops at the first error: every field is checked, and
//! the response has the errors of all of them, the type errors too, since
//! the data is validated before it becomes a Rust type.
//!
//! ```
//! use axum::{Router, routing::{get, post}};
//! use livr_axum::Livr;
//!
//! livr::schema! {
//!     pub struct SignUp {
//!         name: ["required", {"max_length": 50}],
//!         email: ["required", "email", "to_lc"],
//!         age: ["required", "positive_integer", {"min_number": 18}],
//!     }
//! }
//!
//! async fn sign_up(Livr(user): Livr<SignUp>) -> String {
//!     format!("{} ({}), {}", user.name, user.email, user.age)
//! }
//!
//! let app: Router = Router::new()
//!     .route("/users", post(sign_up))
//!     .route("/schemas/sign-up", get(livr_axum::schema::<SignUp>));
//! ```
//!
//! An invalid request gets a [`LivrRejection`]: `422` with the LIVR error
//! codes, shaped like the data:
//!
//! ```json
//! {"code": "VALIDATION_ERROR", "fields": {"age": "TOO_LOW", "email": "WRONG_EMAIL"}}
//! ```
#![forbid(unsafe_code)]

mod extract;
mod middleware;
mod read;
mod rejection;
mod urlencoded;

pub use extract::{Livr, LivrForm, LivrQuery, schema};
pub use middleware::{LivrValidated, validate_json, validate_json_with};
pub use read::{parse_json, read_body, read_form, read_json};
pub use rejection::LivrRejection;
pub use urlencoded::{is_form_content_type, is_json_content_type, parse_urlencoded};

// The examples of the README are tested
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
