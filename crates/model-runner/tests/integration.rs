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
