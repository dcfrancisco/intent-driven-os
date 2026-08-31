#![allow(missing_docs)]

#[test]
fn crate_boundary_is_reachable() {
    assert!(!oid_model_runner::boundary_name().is_empty());
}

#[test]
fn model_paths_are_limited_to_gguf() {
    use oid_model_runner::{ModelPath, ModelRunnerError};

    assert!(ModelPath::new("model.gguf").is_ok());
    assert_eq!(
        ModelPath::new("model.safetensors"),
        Err(ModelRunnerError::UnsupportedFormat(
            "model.safetensors".to_owned()
        ))
    );
}

#[test]
fn stream_can_cancel_without_backend_types() {
    use oid_model_runner::{CancellationToken, GenerationEvent, GenerationStats, GenerationStream};
    use std::sync::mpsc;

    let (sender, receiver) = mpsc::channel();
    let token = CancellationToken::default();
    let stream = GenerationStream::new(receiver, token.clone());
    sender
        .send(GenerationEvent::Token("partial".to_owned()))
        .expect("stream remains open");
    assert_eq!(
        stream.recv().expect("token arrives"),
        GenerationEvent::Token("partial".to_owned())
    );
    stream.cancel();
    assert!(token.is_cancelled());
    sender
        .send(GenerationEvent::Cancelled(GenerationStats::default()))
        .expect("terminal event arrives");
    assert!(matches!(stream.recv(), Ok(GenerationEvent::Cancelled(_))));
}

#[test]
fn generation_request_rejects_invalid_controls() {
    use oid_model_runner::{GenerationRequest, ModelRunnerError};

    let request = GenerationRequest {
        prompt: "hello".to_owned(),
        max_tokens: 0,
        temperature: 0.8,
        top_p: 0.95,
        top_k: 40,
        context_size: 4096,
        seed: None,
    };
    assert_eq!(
        request.validate(),
        Err(ModelRunnerError::InvalidRequest(
            "max tokens must be greater than zero".to_owned()
        ))
    );
}

#[test]
fn generation_request_accepts_default_controls() {
    use oid_model_runner::GenerationRequest;

    let request = GenerationRequest {
        prompt: "hello".to_owned(),
        max_tokens: 128,
        temperature: 0.8,
        top_p: 0.95,
        top_k: 40,
        context_size: 4096,
        seed: Some(7),
    };
    assert!(request.validate().is_ok());
}

#[test]
fn generation_request_rejects_empty_prompts() {
    use oid_model_runner::{GenerationRequest, ModelRunnerError};

    let request = GenerationRequest {
        prompt: "  ".to_owned(),
        max_tokens: 1,
        temperature: 0.0,
        top_p: 1.0,
        top_k: 1,
        context_size: 1,
        seed: None,
    };
    assert_eq!(
        request.validate(),
        Err(ModelRunnerError::InvalidRequest(
            "prompt is empty".to_owned()
        ))
    );
}
