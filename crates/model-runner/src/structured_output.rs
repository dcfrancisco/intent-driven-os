//! Backend-neutral JSON output constraints and their supported schema subset.

use serde_json::Value;

const MAX_SCHEMA_DEPTH: usize = 16;
const MAX_SCHEMA_NODES: usize = 128;
const MAX_OBJECT_PROPERTIES: usize = 8;

/// A JSON output constraint that a backend may enforce during decoding.
#[derive(Clone, Debug, PartialEq)]
pub enum StructuredOutput {
    /// Generate one syntactically valid JSON object.
    JsonObject,
    /// Generate JSON conforming to the supported subset of JSON Schema.
    JsonSchema(Value),
}

impl StructuredOutput {
    /// Validate the schema against the portable structured-output profile.
    ///
    /// # Errors
    ///
    /// Returns an actionable error for malformed or unsupported schemas.
    pub fn validate(&self) -> Result<(), String> {
        if let Self::JsonSchema(schema) = self {
            let mut nodes = 0;
            validate_schema_node(schema, "$", 0, &mut nodes)?;
        }
        Ok(())
    }

    /// Validate a completed output without repairing or transforming it.
    ///
    /// # Errors
    ///
    /// Returns an error when the output is invalid JSON or does not satisfy
    /// the requested JSON object or schema constraint.
    pub fn validate_output(&self, output: &str) -> Result<(), String> {
        self.validate()?;
        let value: Value = serde_json::from_str(output)
            .map_err(|error| format!("constrained generation returned invalid JSON: {error}"))?;
        match self {
            Self::JsonObject if !value.is_object() => {
                Err("constrained generation must return a JSON object".to_owned())
            }
            Self::JsonObject => Ok(()),
            Self::JsonSchema(schema) => validate_instance(schema, &value, "$"),
        }
    }
}

#[allow(clippy::too_many_lines)]
fn validate_schema_node(
    schema: &Value,
    path: &str,
    depth: usize,
    nodes: &mut usize,
) -> Result<(), String> {
    *nodes += 1;
    if depth > MAX_SCHEMA_DEPTH {
        return Err(format!("{path}: schema exceeds maximum nesting depth"));
    }
    if *nodes > MAX_SCHEMA_NODES {
        return Err("$: schema exceeds maximum complexity".to_owned());
    }
    let Some(object) = schema.as_object() else {
        return Err(format!("{path}: schema must be an object"));
    };
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "type"
                | "properties"
                | "required"
                | "items"
                | "enum"
                | "additionalProperties"
                | "title"
                | "description"
        ) {
            return Err(format!("{path}: unsupported JSON Schema keyword `{key}`"));
        }
    }
    for key in ["title", "description"] {
        if object.get(key).is_some_and(|value| !value.is_string()) {
            return Err(format!("{path}.{key}: expected a string"));
        }
    }

    let schema_type = object
        .get("type")
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| format!("{path}.type: only a single string type is supported"))
        })
        .transpose()?;
    if let Some(schema_type) = schema_type {
        if !matches!(
            schema_type,
            "object" | "array" | "string" | "integer" | "number" | "boolean" | "null"
        ) {
            return Err(format!("{path}.type: unsupported type `{schema_type}`"));
        }
    }

    if let Some(values) = object.get("enum") {
        let Some(values) = values.as_array().filter(|values| !values.is_empty()) else {
            return Err(format!("{path}.enum: expected a non-empty array"));
        };
        if values
            .iter()
            .any(|value| value.is_array() || value.is_object())
        {
            return Err(format!(
                "{path}.enum: only scalar enum values are supported"
            ));
        }
        if let Some(schema_type) = schema_type {
            if values
                .iter()
                .any(|value| !value_matches_type(value, schema_type))
            {
                return Err(format!(
                    "{path}.enum: a value does not match `{schema_type}`"
                ));
            }
        }
    } else if schema_type.is_none() {
        return Err(format!("{path}: expected `type` or `enum`"));
    }

    match schema_type {
        Some("object") => {
            if object
                .get("additionalProperties")
                .is_some_and(|value| value != &Value::Bool(false))
            {
                return Err(format!(
                    "{path}.additionalProperties: only `false` is supported"
                ));
            }
            let properties = object
                .get("properties")
                .map(|value| {
                    value
                        .as_object()
                        .ok_or_else(|| format!("{path}.properties: expected an object"))
                })
                .transpose()?;
            if properties.is_some_and(|properties| properties.len() > MAX_OBJECT_PROPERTIES) {
                return Err(format!(
                    "{path}.properties: at most {MAX_OBJECT_PROPERTIES} properties are supported"
                ));
            }
            let required = object
                .get("required")
                .map(|value| {
                    value.as_array().ok_or_else(|| {
                        format!("{path}.required: expected an array of property names")
                    })
                })
                .transpose()?;
            if let Some(required) = required {
                let mut seen = std::collections::HashSet::new();
                for (index, value) in required.iter().enumerate() {
                    let Some(name) = value.as_str() else {
                        return Err(format!("{path}.required[{index}]: expected a string"));
                    };
                    if !seen.insert(name) {
                        return Err(format!("{path}.required: duplicate property `{name}`"));
                    }
                    if !properties.is_some_and(|properties| properties.contains_key(name)) {
                        return Err(format!("{path}.required: property `{name}` is not defined"));
                    }
                }
            }
            if let Some(properties) = properties {
                for (name, property_schema) in properties {
                    validate_schema_node(
                        property_schema,
                        &format!("{path}.properties.{name}"),
                        depth + 1,
                        nodes,
                    )?;
                }
            }
            if object.contains_key("items") {
                return Err(format!("{path}.items: only valid for array schemas"));
            }
        }
        Some("array") => {
            if object.contains_key("properties")
                || object.contains_key("required")
                || object.contains_key("additionalProperties")
            {
                return Err(format!(
                    "{path}: object keywords are only valid for object schemas"
                ));
            }
            let items = object
                .get("items")
                .ok_or_else(|| format!("{path}.items: an item schema is required"))?;
            validate_schema_node(items, &format!("{path}.items"), depth + 1, nodes)?;
        }
        Some("string" | "integer" | "number" | "boolean" | "null") => {
            for key in ["properties", "required", "items", "additionalProperties"] {
                if object.contains_key(key) {
                    return Err(format!(
                        "{path}.{key}: not valid for `{}` schemas",
                        schema_type.unwrap_or_default()
                    ));
                }
            }
        }
        None => {
            if object.keys().any(|key| {
                matches!(
                    key.as_str(),
                    "properties" | "required" | "items" | "additionalProperties"
                )
            }) {
                return Err(format!(
                    "{path}: structural keywords require an explicit `type`"
                ));
            }
        }
        Some(_) => unreachable!("schema type was validated above"),
    }
    Ok(())
}

fn value_matches_type(value: &Value, schema_type: &str) -> bool {
    match schema_type {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "integer" => {
            value.as_i64().is_some()
                || value.as_u64().is_some()
                || value.as_f64().is_some_and(|number| number.fract() == 0.0)
        }
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => false,
    }
}

fn validate_instance(schema: &Value, value: &Value, path: &str) -> Result<(), String> {
    let object = schema
        .as_object()
        .ok_or_else(|| format!("{path}: schema must be an object"))?;
    if let Some(values) = object.get("enum").and_then(Value::as_array) {
        if !values.contains(value) {
            return Err(format!(
                "{path}: value is not one of the allowed enum values"
            ));
        }
    }
    let Some(schema_type) = object.get("type").and_then(Value::as_str) else {
        return Ok(());
    };
    if !value_matches_type(value, schema_type) {
        return Err(format!("{path}: expected a `{schema_type}` value"));
    }
    match schema_type {
        "object" => {
            let properties = object.get("properties").and_then(Value::as_object);
            for key in value.as_object().expect("object type was checked").keys() {
                if properties.is_none_or(|properties| !properties.contains_key(key)) {
                    return Err(format!(
                        "{path}.{key}: additional properties are not supported"
                    ));
                }
            }
            if let Some(properties) = properties {
                for (name, property_schema) in properties {
                    if let Some(property_value) = value.get(name) {
                        validate_instance(
                            property_schema,
                            property_value,
                            &format!("{path}.{name}"),
                        )?;
                    }
                }
            }
            if let Some(required) = object.get("required").and_then(Value::as_array) {
                for name in required.iter().filter_map(Value::as_str) {
                    if value.get(name).is_none() {
                        return Err(format!("{path}.{name}: required property is missing"));
                    }
                }
            }
        }
        "array" => {
            let items = object.get("items").expect("validated array schema");
            for (index, item) in value
                .as_array()
                .expect("array type was checked")
                .iter()
                .enumerate()
            {
                validate_instance(items, item, &format!("{path}[{index}]"))?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::StructuredOutput;
    use serde_json::json;

    #[test]
    fn validates_nested_schema_and_output() {
        let output = StructuredOutput::JsonSchema(json!({
            "type":"object",
            "properties":{
                "name":{"type":"string"},
                "items":{
                    "type":"array",
                    "items":{
                        "type":"object",
                        "properties":{"kind":{"enum":["a","b"]}},
                        "required":["kind"],
                        "additionalProperties":false
                    }
                }
            },
            "required":["name","items"],
            "additionalProperties":false
        }));
        output.validate().expect("supported schema");
        output
            .validate_output(r#"{"name":"λ","items":[{"kind":"a"}]}"#)
            .expect("schema-conforming output");
        assert!(output
            .validate_output(r#"{"name":"λ","items":[{"kind":"c"}]}"#)
            .is_err());
        assert!(output
            .validate_output(r#"{"name":"λ","items":[{"kind":"a"}],"extra":1}"#)
            .is_err());
    }

    #[test]
    fn rejects_invalid_and_unsupported_schema_constructs() {
        for schema in [
            json!({"type":"object","required":["missing"]}),
            json!({"type":["string","null"]}),
            json!({"type":"array"}),
            json!({"type":"string","pattern":"^a"}),
            json!({"type":"object","additionalProperties":true}),
        ] {
            assert!(StructuredOutput::JsonSchema(schema).validate().is_err());
        }
    }

    #[test]
    fn json_object_requires_valid_object_output() {
        let output = StructuredOutput::JsonObject;
        output
            .validate_output(r#"{"ok":true}"#)
            .expect("JSON object");
        assert!(output.validate_output("[1,2]").is_err());
        assert!(output.validate_output("{invalid").is_err());
    }
}
