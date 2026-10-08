//! Small OID-side smoke client for the provider-neutral Marina adapter.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use oid_model_client::{ChatRequest, MarinaHttpClient, ModelClient};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

fn main() {
    if let Err(error) = run() {
        eprintln!("OID Marina client failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let prompt = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    if prompt.trim().is_empty() {
        return Err("usage: oid-marina-chat <prompt>".into());
    }
    let client = MarinaHttpClient::from_environment()?;
    let models = client.list_models()?;
    let model = std::env::var("OID_MARINA_MODEL")
        .ok()
        .or_else(|| models.first().map(|model| model.id.clone()))
        .ok_or("Marina returned no models")?;
    let response = client.chat(
        &ChatRequest {
            model,
            prompt,
            request_id: format!("oid-{}", std::process::id()),
            max_tokens: 128,
            context_size: 4096,
        },
        Duration::from_secs(120),
        &AtomicBool::new(false),
    )?;
    println!("{}", response.text);
    Ok(())
}
