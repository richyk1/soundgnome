use schemars::{gen::SchemaSettings, SchemaGenerator};
use serde_json::{json, Value};

pub fn generate_json_schema<T: schemars::JsonSchema>(_: T) -> Value {
    let settings = SchemaSettings::default().with(|s| {
        s.inline_subschemas = true;
    });
    let mut generator = SchemaGenerator::new(settings);
    let schema = generator.root_schema_for::<T>();

    let mut schema = json!(schema);
    close_objects(&mut schema);
    schema
}

/// Add `"additionalProperties": false` to every object node in a schema.
///
/// Strict structured-output implementations (OpenAI strict mode, and proxies
/// such as LiteLLM fronting GitHub Copilot) reject a schema that omits it,
/// including on objects nested inside array `items`. Providers that do not
/// care ignore the extra key, so this is safe to apply unconditionally.
fn close_objects(node: &mut Value) {
    match node {
        Value::Object(map) => {
            if map.get("type").and_then(Value::as_str) == Some("object")
                && !map.contains_key("additionalProperties")
            {
                map.insert("additionalProperties".to_string(), Value::Bool(false));
            }
            for value in map.values_mut() {
                close_objects(value);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(close_objects),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemars::JsonSchema;

    #[derive(JsonSchema)]
    struct Item {
        #[allow(dead_code)]
        id: String,
    }

    #[derive(JsonSchema)]
    struct Wrapper {
        #[allow(dead_code)]
        items: Vec<Item>,
    }

    #[test]
    fn closes_root_and_nested_objects() {
        let schema = generate_json_schema(Wrapper { items: vec![] });

        assert_eq!(schema["additionalProperties"], Value::Bool(false));
        // The nested case is the one strict providers actually reject.
        assert_eq!(
            schema["properties"]["items"]["items"]["additionalProperties"],
            Value::Bool(false)
        );
    }

    #[test]
    fn leaves_an_explicit_setting_alone() {
        let mut schema = json!({
            "type": "object",
            "additionalProperties": true,
            "properties": { "nested": { "type": "object", "properties": {} } }
        });
        close_objects(&mut schema);

        assert_eq!(schema["additionalProperties"], Value::Bool(true));
        assert_eq!(
            schema["properties"]["nested"]["additionalProperties"],
            Value::Bool(false)
        );
    }
}
