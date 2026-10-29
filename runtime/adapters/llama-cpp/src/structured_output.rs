use oid_model_runner::StructuredOutput;
use serde_json::Value;
use std::collections::HashSet;

const MAX_GRAMMAR_BYTES: usize = 64 * 1024;
const MAX_PROPERTIES: usize = 8;

const JSON_RULES: &str = r#"
root ::= object
value ::= object | array | string | number | ("true" | "false" | "null") ws
object ::= "{" ws (string ":" ws value ("," ws string ":" ws value)*)? "}" ws
array ::= "[" ws (value ("," ws value)*)? "]" ws
string ::= "\"" ([^"\\\x7F\x00-\x1F] | "\\" (["\\bfnrt] | "u" [0-9a-fA-F]{4}))* "\"" ws
number ::= ("-"? ([0-9] | [1-9] [0-9]{0,15})) ("." [0-9]+)? ([eE] [-+]? [0-9] [1-9]{0,15})? ws
ws ::= | " " | "\n" [ \t]{0,20}
"#;

pub(super) fn compile(output: Option<&StructuredOutput>) -> Result<Option<String>, String> {
    let Some(output) = output else {
        return Ok(None);
    };
    output.validate()?;
    match output {
        StructuredOutput::JsonObject => Ok(Some(JSON_RULES.to_owned())),
        StructuredOutput::JsonSchema(schema) => {
            let mut builder = GrammarBuilder::default();
            let schema_rule = builder.compile_schema(schema)?;
            builder
                .rules
                .insert(0, format!("root ::= ws {schema_rule} ws"));
            builder.rules.push(
                r#"json-string ::= "\"" ([^"\\\x7F\x00-\x1F] | "\\" (["\\bfnrt] | "u" [0-9a-fA-F]{4}))* "\""
number ::= ("-"? ([0-9] | [1-9] [0-9]{0,15})) ("." [0-9]+)? ([eE] [-+]? [0-9] [1-9]{0,15})?
boolean ::= "true" | "false"
null ::= "null"
ws ::= | " " | "\n" [ \t]{0,20}"#
                    .to_owned(),
            );
            let grammar = builder.rules.join("\n");
            if grammar.len() > MAX_GRAMMAR_BYTES {
                return Err(format!(
                    "JSON Schema grammar exceeds the {MAX_GRAMMAR_BYTES}-byte limit"
                ));
            }
            Ok(Some(grammar))
        }
    }
}

#[derive(Default)]
struct GrammarBuilder {
    rules: Vec<String>,
}

impl GrammarBuilder {
    fn add_rule(&mut self, body: String) -> String {
        let name = format!("schema-{}", self.rules.len());
        self.rules.push(format!("{name} ::= {body}"));
        name
    }

    fn compile_schema(&mut self, schema: &Value) -> Result<String, String> {
        let object = schema
            .as_object()
            .ok_or_else(|| "JSON Schema node must be an object".to_owned())?;
        if let Some(values) = object.get("enum").and_then(Value::as_array) {
            let alternatives = values
                .iter()
                .map(serde_json::to_string)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| format!("cannot encode JSON Schema enum: {error}"))?
                .into_iter()
                .map(|value| gbnf_literal(&value))
                .collect::<Vec<_>>();
            return Ok(self.add_rule(alternatives.join(" | ")));
        }
        match object.get("type").and_then(Value::as_str) {
            Some("object") => self.compile_object(object),
            Some("array") => {
                let items = object
                    .get("items")
                    .ok_or_else(|| "JSON Schema array is missing `items`".to_owned())?;
                let item_rule = self.compile_schema(items)?;
                Ok(self.add_rule(format!(
                    r#""[" ws ({item_rule} (ws "," ws {item_rule})*)? ws "]""#
                )))
            }
            Some("string") => Ok("json-string".to_owned()),
            Some("integer") => Ok(self.add_rule(r#""-"? ("0" | [1-9] [0-9]*)"#.to_owned())),
            Some("number") => Ok("number".to_owned()),
            Some("boolean") => Ok("boolean".to_owned()),
            Some("null") => Ok("null".to_owned()),
            Some(schema_type) => Err(format!("unsupported JSON Schema type `{schema_type}`")),
            None => Err("JSON Schema node must define `type` or `enum`".to_owned()),
        }
    }

    fn compile_object(
        &mut self,
        schema: &serde_json::Map<String, Value>,
    ) -> Result<String, String> {
        let properties = schema
            .get("properties")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        if properties.len() > MAX_PROPERTIES {
            return Err(format!(
                "JSON Schema objects support at most {MAX_PROPERTIES} properties"
            ));
        }
        let required = schema
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect::<HashSet<_>>();
        let compiled = properties
            .iter()
            .map(|(name, child_schema)| {
                self.compile_schema(child_schema)
                    .map(|rule| (name.clone(), rule))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let optional = compiled
            .iter()
            .filter(|(name, _)| !required.contains(name))
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        let combinations = 1_usize
            .checked_shl(u32::try_from(optional.len()).unwrap_or(u32::MAX))
            .ok_or_else(|| "JSON Schema object has too many optional properties".to_owned())?;
        let mut alternatives = Vec::with_capacity(combinations);
        for mask in 0..combinations {
            let included = compiled
                .iter()
                .filter(|(name, _)| {
                    required.contains(name)
                        || optional
                            .iter()
                            .position(|optional_name| optional_name == name)
                            .is_some_and(|index| mask & (1 << index) != 0)
                })
                .collect::<Vec<_>>();
            let mut body = String::from(r#""{" ws"#);
            for (index, (name, rule)) in included.iter().enumerate() {
                if index > 0 {
                    body.push_str(r#" ws "," ws"#);
                }
                let key = serde_json::to_string(name)
                    .map_err(|error| format!("cannot encode JSON Schema property: {error}"))?;
                body.push(' ');
                body.push_str(&gbnf_literal(&key));
                body.push_str(&format!(r#" ws ":" ws {rule}"#));
            }
            body.push_str(r#" ws "}""#);
            alternatives.push(body);
        }
        Ok(self.add_rule(alternatives.join(" | ")))
    }
}

fn gbnf_literal(value: &str) -> String {
    let mut literal = String::with_capacity(value.len() + 2);
    literal.push('"');
    for character in value.chars() {
        if matches!(character, '\\' | '"') {
            literal.push('\\');
        }
        literal.push(character);
    }
    literal.push('"');
    literal
}

#[cfg(test)]
mod tests {
    use super::{compile, gbnf_literal, StructuredOutput, JSON_RULES};
    use crate::LlamaCppModelRunner;
    use oid_model_runner::{
        GenerationEvent, GenerationRequest, LoadModelRequest, ModelId, ModelPath, ModelRunner,
    };
    use serde_json::json;
    use std::time::Duration;

    #[test]
    fn builds_a_complete_json_object_grammar() {
        let grammar = compile(Some(&StructuredOutput::JsonObject))
            .expect("valid JSON mode")
            .expect("grammar");
        assert_eq!(grammar, JSON_RULES);
        assert!(grammar.contains("string"));
        assert!(grammar.contains("number"));
    }

    #[test]
    fn builds_nested_array_object_and_enum_grammar() {
        let grammar = compile(Some(&StructuredOutput::JsonSchema(json!({
            "type":"object",
            "properties":{
                "items":{
                    "type":"array",
                    "items":{
                        "type":"object",
                        "properties":{"kind":{"type":"string","enum":["a","b"]}},
                        "required":["kind"],
                        "additionalProperties":false
                    }
                }
            },
            "required":["items"],
            "additionalProperties":false
        }))))
        .expect("valid schema")
        .expect("grammar");
        let value_a = gbnf_literal(&serde_json::to_string("a").expect("JSON string"));
        let value_b = gbnf_literal(&serde_json::to_string("b").expect("JSON string"));
        let items_key = gbnf_literal(&serde_json::to_string("items").expect("JSON key"));
        assert!(grammar.contains(&format!("{value_a} | {value_b}")));
        assert!(grammar.contains(&items_key));
        assert!(grammar.contains(r#"ws "," ws"#));
    }

    #[test]
    fn escapes_schema_property_names_and_enum_strings() {
        let grammar = compile(Some(&StructuredOutput::JsonSchema(json!({
            "type":"object",
            "properties":{"say\"hi":{"enum":["line\none","λ"]}},
            "required":["say\"hi"],
            "additionalProperties":false
        }))))
        .expect("valid schema")
        .expect("grammar");
        let key = gbnf_literal(&serde_json::to_string("say\"hi").expect("JSON key"));
        let line = gbnf_literal(&serde_json::to_string("line\none").expect("JSON string"));
        assert!(grammar.contains(&key));
        assert!(grammar.contains(&line));
        assert!(grammar.contains("λ"));
    }

    #[test]
    #[ignore = "requires a real GGUF in MARINA_TEST_GGUF and native llama.cpp"]
    fn native_grammar_sampler_constrains_json_and_recovers_after_cancellation() {
        let model_path = std::env::var("MARINA_TEST_GGUF")
            .expect("MARINA_TEST_GGUF must point to a local GGUF model");
        let runner = LlamaCppModelRunner::new();
        let loaded = runner
            .load(LoadModelRequest {
                model_id: ModelId::new("structured-test").expect("model id"),
                path: ModelPath::new(model_path).expect("model path"),
            })
            .expect("load native model");

        let constraints = [
            (StructuredOutput::JsonObject, "Output exactly {}."),
            (
                StructuredOutput::JsonSchema(json!({
                    "type":"object",
                    "properties":{
                        "result":{
                            "type":"object",
                            "properties":{"ok":{"enum":[true]}},
                            "required":["ok"],
                            "additionalProperties":false
                        }
                    },
                    "required":["result"],
                    "additionalProperties":false
                })),
                r#"Output exactly {"result":{"ok":true}}."#,
            ),
        ];
        for (index, (structured_output, prompt)) in constraints.iter().enumerate() {
            let request = GenerationRequest {
                model_id: loaded.model_id.clone(),
                prompt: (*prompt).to_owned(),
                max_tokens: 32,
                temperature: 0.0,
                top_p: 1.0,
                top_k: 40,
                context_size: 512,
                seed: Some(42),
                stop_sequences: Vec::new(),
                structured_output: Some(structured_output.clone()),
            };
            let stream = runner
                .generate(request)
                .expect("start constrained generation");
            let mut output = String::new();
            loop {
                match stream.recv_timeout(Duration::from_secs(60)) {
                    Ok(GenerationEvent::Token(token)) => output.push_str(&token),
                    Ok(GenerationEvent::Completed(_)) => break,
                    Ok(GenerationEvent::Cancelled(_)) => {
                        panic!("structured generation {index} was unexpectedly cancelled")
                    }
                    Ok(GenerationEvent::Error(error)) => {
                        panic!("structured generation {index} failed: {error}")
                    }
                    Err(error) => panic!("structured generation {index} timed out: {error}"),
                }
            }
            structured_output
                .validate_output(&output)
                .unwrap_or_else(|error| {
                    panic!("structured generation {index} returned {output:?}: {error}")
                });
        }

        let cancellation_request = GenerationRequest {
            model_id: loaded.model_id.clone(),
            prompt: "Generate a large JSON object.".to_owned(),
            max_tokens: 512,
            temperature: 0.0,
            top_p: 1.0,
            top_k: 40,
            context_size: 1024,
            seed: Some(42),
            stop_sequences: Vec::new(),
            structured_output: Some(StructuredOutput::JsonObject),
        };
        let cancelled = runner
            .generate(cancellation_request)
            .expect("start cancellable constrained generation");
        let mut cancellation_requested = false;
        loop {
            match cancelled.recv_timeout(Duration::from_secs(60)) {
                Ok(GenerationEvent::Token(_)) if !cancellation_requested => {
                    cancellation_requested = true;
                    cancelled.cancel();
                }
                Ok(GenerationEvent::Token(_)) => {}
                Ok(GenerationEvent::Completed(_) | GenerationEvent::Cancelled(_))
                | Ok(GenerationEvent::Error(_)) => break,
                Err(error) => panic!("cancelled structured generation timed out: {error}"),
            }
        }
        assert!(cancellation_requested);

        let recovery_constraint = StructuredOutput::JsonSchema(json!({
            "type":"object",
            "properties":{
                "result":{
                    "type":"object",
                    "properties":{"ok":{"enum":[true]}},
                    "required":["ok"],
                    "additionalProperties":false
                }
            },
            "required":["result"],
            "additionalProperties":false
        }));
        let recovery = runner
            .generate(GenerationRequest {
                model_id: loaded.model_id.clone(),
                prompt: r#"Output exactly {"result":{"ok":true}}."#.to_owned(),
                max_tokens: 32,
                temperature: 0.0,
                top_p: 1.0,
                top_k: 40,
                context_size: 512,
                seed: Some(43),
                stop_sequences: Vec::new(),
                structured_output: Some(recovery_constraint.clone()),
            })
            .expect("start recovery generation");
        let mut recovery_output = String::new();
        loop {
            match recovery.recv_timeout(Duration::from_secs(60)) {
                Ok(GenerationEvent::Token(token)) => recovery_output.push_str(&token),
                Ok(GenerationEvent::Completed(_)) => break,
                Ok(GenerationEvent::Cancelled(_)) => {
                    panic!("recovery generation was unexpectedly cancelled")
                }
                Ok(GenerationEvent::Error(error)) => {
                    panic!("recovery generation failed: {error}")
                }
                Err(error) => panic!("recovery generation timed out: {error}"),
            }
        }
        recovery_constraint
            .validate_output(&recovery_output)
            .expect("recovery generation returns valid JSON");
        runner.unload().expect("unload native model");
    }
}
