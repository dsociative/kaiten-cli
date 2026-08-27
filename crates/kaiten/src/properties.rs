//! Custom property values (`properties` on create/update) must be a JSON
//! OBJECT keyed as `id_{property_id}`. Both front ends — the CLI's
//! `--properties-json` and the MCP tools — validate through this module so
//! a wrong shape is an error instead of a silent no-op (issue #15).

/// The shape Kaiten expects, used verbatim in error messages.
const EXAMPLE: &str = r#"{"id_612634": [18929916]}"#;

/// Coerce a `properties` value into the JSON object Kaiten expects.
///
/// - an object passes through unchanged;
/// - a string is parsed as JSON and must contain an object (agents routinely
///   stringify untyped fields — accept that instead of dropping the write);
/// - anything else, including a top-level `null` (its API semantics are not
///   documented; per-property `{"id_N": null}` is how a value is cleared), is
///   an error whose message names the expected shape.
///
/// The message has no subject: callers prefix it with theirs
/// (`--properties-json …` / `properties …`).
/// `properties` counts as absent when the key is missing, `null` (a card that
/// never had custom properties) or `{}` (after the last one is cleared, and
/// on tariffs without custom properties at all).
#[allow(
    clippy::ref_option,
    reason = "the signature is dictated by serde's skip_serializing_if"
)]
pub(crate) fn no_properties(v: &Option<serde_json::Value>) -> bool {
    match v {
        None => true,
        Some(v) => v.is_null() || v.as_object().is_some_and(serde_json::Map::is_empty),
    }
}

pub(crate) fn coerce_object(value: serde_json::Value) -> Result<serde_json::Value, String> {
    let value = match value {
        serde_json::Value::String(raw) => serde_json::from_str::<serde_json::Value>(&raw)
            .map_err(|e| {
                format!("must be a JSON object like '{EXAMPLE}' (got a string that is not valid JSON: {e})")
            })?,
        other => other,
    };
    if value.is_object() {
        Ok(value)
    } else {
        Err(format!("must be a JSON object like '{EXAMPLE}'"))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::coerce_object;

    #[test]
    fn object_passes_through_unchanged() {
        let value = json!({ "id_612634": [18_929_916], "id_1": "x" });
        assert_eq!(coerce_object(value.clone()).unwrap(), value);
    }

    #[test]
    fn stringified_object_is_parsed() {
        let value = json!("{\"id_612634\": [18929916]}");
        assert_eq!(
            coerce_object(value).unwrap(),
            json!({ "id_612634": [18_929_916] })
        );
    }

    /// Top-level `null` has no documented meaning on the API (per-property
    /// `{"id_N": null}` clears a value); it must not slip through as a
    /// possible "clear everything" (review of #15).
    #[test]
    fn null_is_rejected_bare_and_stringified() {
        for value in [json!(null), json!("null")] {
            let err = coerce_object(value.clone()).unwrap_err();
            assert!(err.contains("JSON object"), "{value}: {err}");
        }
    }

    #[test]
    fn string_that_is_not_json_is_rejected_with_the_parse_error() {
        let err = coerce_object(json!("abc")).unwrap_err();
        assert!(err.contains("not valid JSON"), "{err}");
    }

    #[test]
    fn stringified_non_object_is_rejected() {
        let err = coerce_object(json!("[1]")).unwrap_err();
        assert!(err.contains("JSON object"), "{err}");
        assert!(err.contains(super::EXAMPLE), "{err}");
    }

    #[test]
    fn non_object_values_are_rejected() {
        for value in [json!(42), json!([]), json!(true), json!([{ "id_1": 2 }])] {
            let err = coerce_object(value.clone()).unwrap_err();
            assert!(err.contains("JSON object"), "{value}: {err}");
        }
    }
}
