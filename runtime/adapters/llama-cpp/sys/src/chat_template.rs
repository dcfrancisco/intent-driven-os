//! Execute embedded templates with the pinned native Jinja engine.
use serde_json::{json, Value};
use std::ffi::{c_char, c_void, CStr, CString};
use std::sync::atomic::{AtomicU64, Ordering};

extern "C" {
    fn oid_chat_template_abi_version() -> u32;
    fn oid_chat_template_render(
        source: *const c_char,
        context: *const c_char,
        ok: *mut bool,
        length: *mut usize,
    ) -> *mut c_char;
    fn oid_chat_template_free(data: *mut c_char);
}
struct NativeText(*mut c_char);
impl Drop for NativeText {
    fn drop(&mut self) {
        unsafe { oid_chat_template_free(self.0) };
    }
}
fn render(template: &str, input: &Value) -> Result<String, String> {
    if unsafe { oid_chat_template_abi_version() } != 1 {
        return Err("unsupported native chat-template ABI".to_owned());
    }
    let source = CString::new(template).map_err(|_| "chat template contains NUL")?;
    let context = CString::new(input.to_string()).map_err(|_| "chat context contains NUL")?;
    let mut ok = false;
    let mut length = 0;
    let text = NativeText(unsafe {
        oid_chat_template_render(
            source.as_ptr(),
            context.as_ptr(),
            &raw mut ok,
            &raw mut length,
        )
    });
    if text.0.is_null() {
        return Err("native chat template allocation failed".to_owned());
    }
    let bytes = unsafe { std::slice::from_raw_parts(text.0.cast::<u8>(), length) };
    let result = std::str::from_utf8(bytes)
        .map_err(|_| "chat template returned invalid UTF-8")?
        .to_owned();
    if ok {
        Ok(result)
    } else {
        Err(format!("embedded chat template: {result}"))
    }
}
fn model_context(pointer: *const c_void) -> Result<Value, String> {
    let vocab = unsafe { super::ffi::llama_model_get_vocab(pointer) };
    let token = |id| -> Result<String, String> {
        if id < 0 {
            return Ok(String::new());
        }
        String::from_utf8(unsafe { super::NativeModel::token_piece_with_special(vocab, id, true)? })
            .map_err(|_| "special token is not UTF-8".to_owned())
    };
    Ok(
        json!({"bos_token":token(unsafe {super::ffi::llama_vocab_bos(vocab)})?,"eos_token":token(unsafe {super::ffi::llama_vocab_eos(vocab)})?,"add_generation_prompt":true}),
    )
}
pub(super) fn probe(pointer: *const c_void, template: &str) -> Result<String, String> {
    let mut input = model_context(pointer)?;
    input["messages"] = json!([{"role":"user","content":"Hello"}]);
    render(template, &input)
}
pub(super) fn probe_tools(pointer: *const c_void, template: &str) -> Result<String, String> {
    let mut input = model_context(pointer)?;
    input["messages"] = json!([{"role":"user","content":"Hello"}]);
    input["tools"] = json!([{"type":"function","function":{"name":"__talvik_probe","description":"Probe template rendering only.","parameters":{"type":"object","properties":{}}}}]);
    let text = render(template, &input)?;
    if text.contains("__talvik_probe") {
        Ok(text)
    } else {
        Err("template omitted supplied tools".to_owned())
    }
}

fn normalize_messages(mut messages: Vec<Value>, template: &str, choices: &[String]) -> Vec<Value> {
    let developer = template.contains("'developer'") || template.contains("\"developer\"");
    for message in &mut messages {
        if message["role"] == "developer" && !developer {
            message["role"] = json!("system");
        }
    }
    let count = messages
        .iter()
        .take_while(|m| m["role"] == "system")
        .count();
    let mut system = messages[..count]
        .iter()
        .filter_map(|m| m["content"].as_str())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    system.extend_from_slice(choices);
    let mut normalized = Vec::new();
    if !system.is_empty() {
        normalized.push(json!({"role":"system","content":system.join("\n\n")}));
    }
    normalized.extend(messages.into_iter().skip(count));
    normalized
}
struct Conversation {
    messages: Vec<Value>,
    tools: Vec<Value>,
    thinking: Option<bool>,
}
#[allow(clippy::too_many_lines)]
fn decode(prompt: &str, template: &str) -> Result<Conversation, String> {
    let Some(records) = prompt.strip_prefix('\u{001e}') else {
        return Ok(Conversation {
            messages: vec![json!({"role":"user","content":prompt})],
            tools: Vec::new(),
            thinking: None,
        });
    };
    let mut messages = Vec::new();
    let mut tools = Vec::new();
    let mut choices = Vec::new();
    let mut thinking = None;
    for record in records.split('\u{001e}').filter(|r| !r.is_empty()) {
        let (role, content) = record
            .split_once('\n')
            .ok_or("invalid structured chat record")?;
        match role {
            "__talvik_tools" => tools = serde_json::from_str(content).map_err(|e|format!("invalid tools: {e}"))?,
            "__talvik_template" => thinking = Some(content.strip_prefix("enable_thinking=").ok_or("invalid template option")?.parse::<bool>().map_err(|_|"invalid thinking option")?),
            "__talvik_tool_choice" => choices.push(match content {
                "single" => "Make at most one tool call in this response.".to_owned(),
                "required" => "A tool call is required for this response. Call one of the available tools instead of answering in prose.".to_owned(),
                value if value.starts_with("function:") => format!("Call the function {:?} exactly once in this response and no other function.", &value[9..]),
                _ => return Err("invalid tool-choice directive".to_owned()),
            }),
            "__talvik_tool_calls" => {
                let mut history: Value = serde_json::from_str(content).map_err(|e|format!("invalid tool history: {e}"))?;
                let calls = history["calls"].as_array_mut().ok_or("tool history requires calls")?;
                for call in calls.iter_mut() {
                    if let Some(arguments) = call.pointer("/function/arguments").and_then(Value::as_str) {
                        call["function"]["arguments"] = serde_json::from_str(arguments).map_err(|e|format!("invalid historical arguments: {e}"))?;
                    }
                }
                messages.push(json!({"role":"assistant","content":history["content"].as_str().unwrap_or(""),"tool_calls":history["calls"]}));
            }
            "__talvik_tool_result" => {
                let mut result: Value = serde_json::from_str(content).map_err(|e|format!("invalid tool result: {e}"))?;
                if !result.is_object() { return Err("tool result must be an object".to_owned()); }
                result["role"] = json!("tool");
                messages.push(result);
            }
            "system" | "developer" | "user" | "assistant" | "tool" => messages.push(json!({"role":role,"content":content})),
            _ => return Err(format!("unsupported native chat role: {role}")),
        }
    }
    Ok(Conversation {
        messages: normalize_messages(messages, template, &choices),
        tools,
        thinking,
    })
}
static BOUNDARY: AtomicU64 = AtomicU64::new(0);
pub(super) fn format(
    model: &super::NativeModel,
    prompt: &str,
    template: &CStr,
) -> Result<super::FormattedPrompt, String> {
    let source = template
        .to_str()
        .map_err(|_| "chat template is not UTF-8")?;
    let Conversation {
        messages,
        tools,
        thinking,
    } = decode(prompt, source)?;
    let mut input = model_context(model.pointer.cast_const())?;
    input["messages"] = json!(messages);
    if !tools.is_empty() {
        input["tools"] = json!(tools);
    }
    if let Some(enabled) = thinking {
        input["enable_thinking"] = json!(enabled);
    }
    let text = super::apply_thinking_prefill(render(source, &input)?, source, thinking);
    // Render a structurally identical conversation with a unique last-user
    // marker. This works even when a template rejects a system-only render.
    // A prefix is retained only if the actual full output starts with it.
    let cache_prefix = messages
        .iter()
        .rposition(|m| m["role"] == "user")
        .filter(|i| *i > 0)
        .and_then(|index| {
            let mut alternate = input.clone();
            let marker = loop {
                let marker = format!(
                    "__talvik_cache_boundary_{}__",
                    BOUNDARY.fetch_add(1, Ordering::Relaxed)
                );
                if !prompt.contains(&marker) && !source.contains(&marker) {
                    break marker;
                }
            };
            alternate["messages"][index]["content"] = json!(marker);
            let rendered = render(source, &alternate).ok()?;
            let offset = rendered.find(&marker)?;
            let prefix = &rendered[..offset];
            (!prefix.is_empty() && text.starts_with(prefix)).then(|| prefix.to_owned())
        });
    let reasoning_disabled = model.profile.reasoning_format.is_none()
        || (model.profile.thinking_control && thinking == Some(false));
    Ok(super::FormattedPrompt {
        text,
        cache_prefix,
        constrain_json_tools: !tools.is_empty()
            && reasoning_disabled
            && model.profile.tool_format == Some(oid_model_runner::ToolFormat::XmlJson),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_renderer_preserves_unicode_nul_and_reports_errors() {
        assert_eq!(
            render("{{ text }}", &json!({"text":"hi 日本\u{0}end"})).unwrap(),
            "hi 日本\u{0}end"
        );
        assert!(render("{% broken %}", &json!({})).is_err());
        assert!(render("{{ raise_exception('invalid role') }}", &json!({}))
            .unwrap_err()
            .contains("invalid role"));
    }
    #[test]
    fn history_keeps_tool_arguments_and_results_structured() {
        let prompt = "\u{001e}developer\nScope\u{001e}__talvik_tool_calls\n{\"content\":\"\",\"calls\":[{\"id\":\"a\",\"type\":\"function\",\"function\":{\"name\":\"build\",\"arguments\":\"{\\\"command\\\":\\\"mvn\\\"}\"}}]}\u{001e}__talvik_tool_result\n{\"tool_call_id\":\"a\",\"name\":\"build\",\"content\":\"BUILD SUCCESS\"}";
        let messages = decode(prompt, "").unwrap().messages;
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(
            messages[1]["tool_calls"][0]["function"]["arguments"]["command"],
            "mvn"
        );
        assert_eq!(messages[2]["content"], "BUILD SUCCESS");
        assert_eq!(messages[2]["tool_call_id"], "a");
    }
}
