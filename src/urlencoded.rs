use axum::http::{HeaderMap, header};
use livr::SchemaField;
use serde_json::{Map, Value};

/// Converts a query string or a form (`application/x-www-form-urlencoded`)
/// into the fields of an object, as strings, as the extractors do. Keys and
/// values are percent-decoded, `+` is a space, and invalid UTF-8 becomes
/// U+FFFD, as in `axum::Form`.
///
/// - A key given more than once is a list: `tags=a&tags=b` is
///   `{"tags": ["a", "b"]}`.
/// - A key ending with `[]` is a list, even once: `tags[]=a` is
///   `{"tags": ["a"]}`.
/// - Keys are not nested: `a[b]=1` is the key `a[b]`, as in `axum::Form`.
///
/// With the fields of a schema ([`livr::Schema::FIELDS`]):
///
/// - A list field is a list even with one value, as a group of checkboxes
///   with one box checked sends it: `tags=a` is `["a"]`. A single empty
///   value (`tags=`) stays `""`, no value, as for a missing field.
/// - Keys that are not fields are dropped as they are read. The validator
///   would drop them anyway, and a body of distinct keys would otherwise be
///   allocated into a map many times its size. A field that
///   `equal_to_field` or a custom rule reads must then have rules of its
///   own.
///
/// With `None`, for data validated by a schema not known yet, every key is
/// kept and single values stay as they are.
///
/// ```
/// use livr::SchemaField;
/// use livr_axum::parse_urlencoded;
/// use serde_json::json;
///
/// let fields = [SchemaField::new("q", false), SchemaField::new("tags", true)];
/// let data = parse_urlencoded(b"q=rust+axum&tags=web&utm_source=x", Some(&fields));
/// assert_eq!(serde_json::Value::Object(data), json!({"q": "rust axum", "tags": ["web"]}));
///
/// let data = parse_urlencoded(b"q=rust&tags=web&ids[]=1", None);
/// assert_eq!(serde_json::Value::Object(data), json!({"q": "rust", "tags": "web", "ids": ["1"]}));
/// ```
#[must_use]
pub fn parse_urlencoded(input: &[u8], fields: Option<&[SchemaField]>) -> Map<String, Value> {
    let mut data = Map::new();
    for (key, value) in form_urlencoded::parse(input) {
        let (key, has_brackets) = match key.strip_suffix("[]") {
            Some(key) => (key, true),
            None => (&*key, false),
        };
        let is_list_field = match fields {
            Some(fields) => match fields.iter().find(|field| field.name() == key) {
                Some(field) => field.is_list() && !value.is_empty(),
                None => continue,
            },
            None => false,
        };
        let value = Value::String(value.into_owned());
        match data.get_mut(key) {
            Some(Value::Array(list)) => list.push(value),
            Some(first) => *first = Value::Array(vec![std::mem::take(first), value]),
            None if has_brackets || is_list_field => {
                data.insert(key.to_owned(), Value::Array(vec![value]));
            }
            None => {
                data.insert(key.to_owned(), value);
            }
        }
    }
    data
}

// The media type, without parameters (`; charset=utf-8`)
fn media_type(headers: &HeaderMap) -> Option<&str> {
    let content_type = headers.get(header::CONTENT_TYPE)?.to_str().ok()?;
    content_type.split(';').next().map(str::trim)
}

/// Whether the `Content-Type` is `application/json`, or a JSON type like
/// `application/cloudevents+json`, as for `axum::Json`. Parameters
/// (`; charset=utf-8`) and case are ignored.
#[must_use]
pub fn is_json_content_type(headers: &HeaderMap) -> bool {
    let Some((type_, subtype)) =
        media_type(headers).and_then(|media_type| media_type.split_once('/'))
    else {
        return false;
    };
    type_.eq_ignore_ascii_case("application")
        && (subtype.eq_ignore_ascii_case("json")
            || subtype.len() > 5
                && subtype
                    .get(subtype.len() - 5..)
                    .is_some_and(|suffix| suffix.eq_ignore_ascii_case("+json")))
}

/// Whether the `Content-Type` is `application/x-www-form-urlencoded`.
/// Parameters and case are ignored.
#[must_use]
pub fn is_form_content_type(headers: &HeaderMap) -> bool {
    media_type(headers).is_some_and(|media_type| {
        media_type.eq_ignore_ascii_case("application/x-www-form-urlencoded")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SCHEMA: &[SchemaField] = &[
        SchemaField::new("name", false),
        SchemaField::new("age", false),
        SchemaField::new("tag", false),
        SchemaField::new("ids", false),
        SchemaField::new("empty", false),
        SchemaField::new("n", false),
        SchemaField::new("tags", true),
        SchemaField::new("a[b]", false),
    ];

    fn object(input: &[u8], fields: &[SchemaField]) -> Value {
        Value::Object(parse_urlencoded(input, Some(fields)))
    }

    #[test]
    fn keeps_every_key_without_fields() {
        let all = |input: &[u8]| Value::Object(parse_urlencoded(input, None));
        assert_eq!(
            all(b"tags=a&other=1&other=2&ids[]=1&a[b]=2&e="),
            json!({"tags": "a", "other": ["1", "2"], "ids": ["1"], "a[b]": "2", "e": ""}),
        );
        assert_eq!(all(b""), json!({}));
    }

    #[test]
    fn parses_fields_and_lists() {
        assert_eq!(
            object(
                b"name=Jane+Doe&age=42&tag=a&tag=b&ids[]=1&empty=&n=%C3%A9",
                SCHEMA
            ),
            json!({"name": "Jane Doe", "age": "42", "tag": ["a", "b"], "ids": ["1"], "empty": "", "n": "é"}),
        );
        assert_eq!(object(b"", SCHEMA), json!({}));
    }

    #[test]
    fn makes_a_list_of_one_value_for_list_fields() {
        assert_eq!(object(b"tags=a", SCHEMA), json!({"tags": ["a"]}));
        assert_eq!(
            object(b"tags=a&tags=b", SCHEMA),
            json!({"tags": ["a", "b"]})
        );
        assert_eq!(object(b"tags[]=a", SCHEMA), json!({"tags": ["a"]}));
        // An empty input of a form is no value, but an explicit list is one
        assert_eq!(object(b"tags=", SCHEMA), json!({"tags": ""}));
        assert_eq!(object(b"tags[]=", SCHEMA), json!({"tags": [""]}));
        assert_eq!(object(b"tags=&tags=a", SCHEMA), json!({"tags": ["", "a"]}));
    }

    #[test]
    fn drops_keys_that_are_not_fields() {
        assert_eq!(
            object(b"name=x&other=1&other=2&tags2=a&names[]=b", SCHEMA),
            json!({"name": "x"}),
        );
        let many: String = (0..10_000).map(|i| format!("k{i}=&")).collect();
        assert_eq!(object(many.as_bytes(), SCHEMA), json!({}));
    }

    #[test]
    fn decodes_keys_and_values() {
        // `[]` percent-encoded, as browsers send it
        assert_eq!(object(b"tags%5B%5D=a", SCHEMA), json!({"tags": ["a"]}));
        // Invalid UTF-8 becomes U+FFFD, as in axum::Form
        assert_eq!(object(b"name=%FF", SCHEMA), json!({"name": "\u{FFFD}"}));
        // No nesting: `a[b]` is a key of its own, as in axum::Form
        assert_eq!(object(b"a[b]=1", SCHEMA), json!({"a[b]": "1"}));
        assert_eq!(object(b"a%5Bb%5D=1", SCHEMA), json!({"a[b]": "1"}));
    }

    #[test]
    fn checks_media_types() {
        let headers = |value: &'static str| {
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, value.parse().unwrap());
            headers
        };
        assert!(is_json_content_type(&headers("application/json")));
        assert!(is_json_content_type(&headers(
            "Application/JSON; charset=utf-8"
        )));
        assert!(is_json_content_type(&headers(
            "application/cloudevents+json"
        )));
        assert!(is_json_content_type(&headers("APPLICATION/PROBLEM+JSON")));
        assert!(!is_json_content_type(&headers("application/+json")));
        assert!(!is_json_content_type(&headers("application/jsonx")));
        assert!(!is_json_content_type(&headers("application/json+xml")));
        assert!(!is_json_content_type(&headers("text/json")));
        assert!(!is_json_content_type(&headers("application")));
        assert!(!is_json_content_type(&HeaderMap::new()));
        assert!(is_form_content_type(&headers(
            "application/x-www-form-urlencoded"
        )));
        assert!(is_form_content_type(&headers(
            "Application/X-WWW-Form-Urlencoded; charset=utf-8"
        )));
        assert!(!is_form_content_type(&headers("multipart/form-data")));
        assert!(!is_form_content_type(&HeaderMap::new()));
    }
}
