# Changelog

## 0.2.0 (2026-11-07)

Validation with livr 0.2, which adds the rules of the LIVR 2.5 draft
(`boolean`, `required_if`, `when`, `values`, `keys`, `uuid`, `iban` and
more, see its
[CHANGELOG](https://github.com/koorchik/rust-livr/blob/master/CHANGELOG.md)).
The extractors and the middleware take them as any other rule.

- Requires livr 0.2: `livr::Schema` of livr 0.1 does not work with this
  version, and livr-axum 0.1 does not work with livr 0.2. Upgrade both.
- No changes to the API of livr-axum.

## 0.1.0 (2026-10-29)

Validation with livr 0.1, a LIVR 2.0 implementation (https://livr-spec.org).

- Extractors for JSON bodies (`Livr<T>`), query strings (`LivrQuery<T>`) and
  forms (`LivrForm<T>`), generic over `livr::Schema`. They implement
  `Deref`, `DerefMut` and `From<T>`, as the extractors of axum.
  `Option<Livr<T>>` is `None` without a `Content-Type`, and
  `Option<LivrQuery<T>>` without a query string.
- Query strings and forms: a list field is a list even with one value
  (`?tags=a` is `["a"]`), and keys that are not fields of the schema are
  dropped as they are read, so a body of many keys is not allocated into a
  map many times its size.
- `validate_json` with `LivrValidated`: a middleware for schemas known at
  run time; `validate_json_with::<E>` gives its errors the format of your
  API. `LivrValidated` can be extracted more than once in a request.
- `schema::<T>`: a route that serves a schema to the frontend.
- `LivrRejection`: JSON error responses, or your own format through
  `status`, `code`, `body`, `errors` and `into_errors`. Its message does not
  repeat its source.
- `read_json`, `read_form`, `read_body`, `parse_json`, `parse_urlencoded`,
  `is_json_content_type` and `is_form_content_type`: the reading of
  requests that the extractors use, for extractors and frameworks of your
  own.
