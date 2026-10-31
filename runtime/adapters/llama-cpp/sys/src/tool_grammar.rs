//! JSON syntax constraints activated only when a model begins a tool call.

use std::ffi::{c_void, CString};

// Keep ordinary prose unrestricted. Once a call is recognized, enforce JSON
// syntax and the matching outer delimiter through the end of the call. Tool
// names, arguments, schema checks and execution remain owned by the HTTP edge.
const GRAMMAR: &str = r#"
root ::= ws (wrapped | fenced | object) ws
wrapped ::= "<tool_call>" ws object "</tool_call>" ws wrapped?
fenced ::= "```" "json"? ws object "```" ws
value ::= object | array | string | number | ("true" | "false" | "null") ws
object ::= "{" ws (string ":" ws value ("," ws string ":" ws value)*)? "}" ws
array ::= "[" ws (value ("," ws value)*)? "]" ws
string ::= "\"" ([^"\\\x7F\x00-\x1F] | "\\" (["\\bfnrt] | "u" [0-9a-fA-F]{4}))* "\"" ws
number ::= "-"? ("0" | [1-9] [0-9]*) ("." [0-9]+)? ([eE] [-+]? [0-9]+)? ws
ws ::= [ \t\n\r]{0,20}
"#;

const TRIGGERS: [&str; 3] = [
    r"^[\s\S]*?(<tool_call>[\s\S]*)",
    r"^[ \t\r\n]*(```(?:json)?[ \t\r\n]*\{[\s\S]*)",
    r"^[ \t\r\n]*(\{[\s\S]*)",
];

pub(super) unsafe fn init(vocab: *const c_void) -> *mut c_void {
    let grammar = CString::new(GRAMMAR).expect("static grammar has no NUL");
    let root = c"root";
    let patterns = TRIGGERS.map(|value| CString::new(value).expect("static trigger has no NUL"));
    let pointers = patterns.each_ref().map(|value| value.as_ptr());
    // The pinned engine copies the grammar rules and trigger strings.
    unsafe {
        super::ffi::llama_sampler_init_grammar_lazy_patterns(
            vocab,
            grammar.as_ptr(),
            root.as_ptr(),
            pointers.as_ptr(),
            pointers.len(),
            std::ptr::null(),
            0,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::c_void;

    #[repr(C)]
    struct TokenData {
        id: i32,
        logit: f32,
        probability: f32,
    }

    #[repr(C)]
    struct TokenDataArray {
        data: *mut TokenData,
        size: usize,
        selected: i64,
        sorted: bool,
    }

    extern "C" {
        fn llama_sampler_accept(sampler: *mut c_void, token: i32);
        fn llama_sampler_apply(sampler: *mut c_void, candidates: *mut TokenDataArray);
        fn llama_vocab_eos(vocab: *const c_void) -> i32;
    }

    unsafe fn allowed(sampler: *mut c_void, tokens: &[i32]) -> Vec<bool> {
        let mut data = tokens
            .iter()
            .map(|id| TokenData {
                id: *id,
                logit: 0.0,
                probability: 0.0,
            })
            .collect::<Vec<_>>();
        let mut array = TokenDataArray {
            data: data.as_mut_ptr(),
            size: data.len(),
            selected: -1,
            sorted: false,
        };
        unsafe {
            llama_sampler_apply(sampler, std::ptr::from_mut(&mut array));
        }
        data.iter().map(|token| token.logit.is_finite()).collect()
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    #[ignore = "requires MARINA_TEST_GGUF for real tokenizer/grammar qualification"]
    fn lazy_tool_grammar_blocks_missing_commas_and_incomplete_wrappers() {
        let path = std::env::var("MARINA_TEST_GGUF").expect("GGUF fixture");
        crate::initialize().expect("initialize");
        let model = crate::load_model(&path, None, crate::cpu_load_options()).expect("load");
        let vocabulary = unsafe { crate::ffi::llama_model_get_vocab(model.pointer.cast_const()) };
        let comma = model.tokenize(",").unwrap();
        let quote = model.tokenize("\"").unwrap();
        assert_eq!(comma.len(), 1);
        assert_eq!(quote.len(), 1);
        let eos = unsafe { llama_vocab_eos(vocabulary) };
        let penalties = unsafe {
            crate::ffi::llama_sampler_init_penalties(
                crate::ffi::llama_vocab_n_tokens(vocabulary),
                64,
                1.1,
                0.0,
                0.0,
            )
        };
        assert!(!penalties.is_null());
        unsafe {
            llama_sampler_accept(penalties, comma[0]);
        }
        let mut data = [
            TokenData {
                id: comma[0],
                logit: 2.0,
                probability: 0.0,
            },
            TokenData {
                id: quote[0],
                logit: 2.0,
                probability: 0.0,
            },
        ];
        let mut array = TokenDataArray {
            data: data.as_mut_ptr(),
            size: 2,
            selected: -1,
            sorted: false,
        };
        unsafe {
            llama_sampler_apply(penalties, std::ptr::from_mut(&mut array));
        }
        assert!((data[0].logit - 2.0 / 1.1).abs() < 0.00001);
        assert!((data[1].logit - 2.0).abs() < f32::EPSILON);
        // The pinned C API takes vocabulary size before the 64-token window.
        // This eviction assertion also detects an older/misaligned ABI that
        // accidentally uses vocabulary size as the history window.
        for _ in 0..64 {
            unsafe {
                llama_sampler_accept(penalties, quote[0]);
            }
        }
        data[0].logit = 2.0;
        data[1].logit = 2.0;
        unsafe {
            llama_sampler_apply(penalties, std::ptr::from_mut(&mut array));
        }
        assert!((data[0].logit - 2.0).abs() < f32::EPSILON);
        assert!(data[1].logit < 2.0);
        unsafe {
            crate::ffi::llama_sampler_free(penalties);
        }
        let prose = unsafe { super::init(vocabulary) };
        for token in model.tokenize("An example: {not JSON").unwrap() {
            unsafe {
                llama_sampler_accept(prose, token);
            }
        }
        assert_eq!(
            unsafe { allowed(prose, &[comma[0], quote[0]]) },
            [true, true]
        );
        unsafe {
            crate::ffi::llama_sampler_free(prose);
        }

        for (start, close) in [
            ("<tool_call>", "</tool_call>"),
            ("Let me inspect.\n<tool_call>", "</tool_call>"),
            ("```json\n", "```"),
            ("", ""),
        ] {
            let sampler = unsafe { super::init(vocabulary) };
            assert!(!sampler.is_null(), "grammar must compile");
            // Before any trigger, neither punctuation choice is constrained.
            assert_eq!(
                unsafe { allowed(sampler, &[comma[0], quote[0]]) },
                [true, true]
            );
            let prefix = format!("{start}{{\"name\":\"read_file\",\"arguments\":{{\"path\":\"README.md\",\"startLine\":1");
            for token in model.tokenize(&prefix).unwrap() {
                unsafe {
                    llama_sampler_accept(sampler, token);
                }
            }
            assert_eq!(
                unsafe { allowed(sampler, &[comma[0], quote[0]]) },
                [true, false],
                "missing comma must be impossible for {start:?}"
            );
            for token in model.tokenize(",\"endLine\":3}}").unwrap() {
                unsafe {
                    llama_sampler_accept(sampler, token);
                }
            }
            if !close.is_empty() {
                assert_eq!(
                    unsafe { allowed(sampler, &[eos]) },
                    [false],
                    "closing wrapper is required"
                );
                for token in model.tokenize(close).unwrap() {
                    unsafe {
                        llama_sampler_accept(sampler, token);
                    }
                }
            }
            assert_eq!(unsafe { allowed(sampler, &[eos]) }, [true]);
            unsafe {
                crate::ffi::llama_sampler_free(sampler);
            }
        }
        drop(model);
        crate::shutdown();
    }
}
