# Changelog

## 0.1.0 (unreleased)

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
