//! Small OID-side smoke client for the provider-neutral Marina adapter.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use oid_model_client::{ChatRequest, ChatResponse, MarinaHttpClient, ModelClient, ModelDescriptor};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

const DEFAULT_BENCHMARK_RUNS: usize = 5;
const MAX_BENCHMARK_RUNS: usize = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Plain,
    Timing,
    Benchmark { runs: usize },
}

struct Arguments {
    mode: Mode,
    prompt: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SampleState {
    Warm,
    FirstAfterLoad,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Percentile {
    Median,
    P95,
}

fn classify_state(resident_before: Option<bool>, model_load_time_ms: Option<u128>) -> SampleState {
    if model_load_time_ms.is_some() {
        SampleState::FirstAfterLoad
    } else if resident_before == Some(true) {
        SampleState::Warm
    } else {
        SampleState::Unknown
    }
}

impl std::fmt::Display for SampleState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Warm => "warm",
            Self::FirstAfterLoad => "first-after-load",
            Self::Unknown => "unknown",
        })
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("OID Marina client failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let raw_arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let arguments = parse_arguments(&raw_arguments)?;
    let total_started = Instant::now();
    let client = MarinaHttpClient::from_environment()?;
    let models = client.list_models()?;
    let model = selected_model(&models)?;

    match arguments.mode {
        Mode::Plain | Mode::Timing => {
            let response = chat(&client, &model, &arguments.prompt, 0)?;
            println!("{}", response.text);
            if arguments.mode == Mode::Timing {
                print_timing(&response, total_started.elapsed());
            }
        }
        Mode::Benchmark { runs } => {
            run_benchmark(&client, &model, &arguments.prompt, runs)?;
        }
    }
    Ok(())
}

fn parse_arguments(arguments: &[String]) -> Result<Arguments, &'static str> {
    let mut mode = Mode::Plain;
    let mut index = 0;
    if let Some(first) = arguments.first() {
        match first.as_str() {
            "--timing" => {
                mode = Mode::Timing;
                index = 1;
            }
            "--benchmark" => {
                mode = Mode::Benchmark {
                    runs: DEFAULT_BENCHMARK_RUNS,
                };
                index = 1;
            }
            _ => {}
        }
    }

    let mut prompt = Vec::new();
    while index < arguments.len() {
        if arguments[index] == "--runs" {
            let Mode::Benchmark { .. } = mode else {
                return Err("--runs is only valid with --benchmark");
            };
            let Some(value) = arguments.get(index + 1) else {
                return Err("--runs requires a count from 1 to 100");
            };
            let runs = value
                .parse::<usize>()
                .ok()
                .filter(|runs| (1..=MAX_BENCHMARK_RUNS).contains(runs))
                .ok_or("--runs requires a count from 1 to 100")?;
            mode = Mode::Benchmark { runs };
            index += 2;
        } else {
            if arguments[index].starts_with("--") {
                return Err("unknown option; usage: oid-marina-chat [--timing | --benchmark [--runs N]] <prompt>");
            }
            prompt.push(arguments[index].as_str());
            index += 1;
        }
    }

    let prompt = prompt.join(" ");
    if prompt.trim().is_empty() {
        return Err("usage: oid-marina-chat [--timing | --benchmark [--runs N]] <prompt>");
    }
    Ok(Arguments { mode, prompt })
}

fn selected_model(models: &[ModelDescriptor]) -> Result<String, &'static str> {
    select_model(models, std::env::var("OID_MARINA_MODEL").ok().as_deref())
}

fn select_model(
    models: &[ModelDescriptor],
    configured_model: Option<&str>,
) -> Result<String, &'static str> {
    if let Some(model) = configured_model
        .map(str::trim)
        .filter(|model| !model.is_empty())
    {
        return Ok(model.to_owned());
    }
    models
        .iter()
        .find(|model| model.is_default == Some(true))
        .or_else(|| models.iter().find(|model| model.resident == Some(true)))
        .or_else(|| {
            models
                .iter()
                .filter(|model| {
                    model
                        .estimated_memory_mib
                        .is_some_and(|estimate| estimate > 0)
                })
                .min_by_key(|model| model.estimated_memory_mib)
        })
        .or_else(|| models.first())
        .map(|model| model.id.clone())
        .ok_or("Marina returned no models")
}

fn chat(
    client: &MarinaHttpClient,
    model: &str,
    prompt: &str,
    run: usize,
) -> Result<ChatResponse, oid_model_client::ModelClientError> {
    client.chat(
        &ChatRequest {
            model: model.to_owned(),
            prompt: prompt.to_owned(),
            request_id: format!("oid-{}-{run}", std::process::id()),
            max_tokens: 128,
            context_size: 4096,
        },
        Duration::from_secs(120),
        &AtomicBool::new(false),
    )
}

fn print_timing(response: &ChatResponse, total_elapsed: Duration) {
    println!();
    println!("MarinaOS Inference Metrics");
    println!("Model: {}", response.model);
    println!(
        "Backend: {}",
        response.backend.as_deref().unwrap_or("unavailable")
    );
    println!(
        "OID client total (client config, discovery, request): {:.3} s (OID client-measured)",
        total_elapsed.as_secs_f64()
    );
    print_milliseconds(
        "Talvik runtime latency",
        response.metrics.latency_ms,
        "Talvik-reported",
    );
    print_milliseconds(
        "Backend inference duration",
        response.metrics.inference_time_ms,
        "backend-reported",
    );
    print_milliseconds(
        "Time to first token",
        response.metrics.time_to_first_token_ms,
        "backend-reported",
    );
    print_milliseconds(
        "Prompt evaluation time",
        response.metrics.prompt_evaluation_time_ms,
        "backend-reported",
    );
    print_milliseconds(
        "Generation duration",
        response.metrics.generation_time_ms,
        "backend-reported",
    );
    print_count("Prompt tokens", response.metrics.prompt_tokens);
    print_count("Output tokens", response.metrics.generated_tokens);
    print_count("Context tokens", response.metrics.context_tokens);
    print_rate("Generation throughput", response.metrics.tokens_per_second);
    print_milliseconds(
        "Model load duration",
        response.metrics.model_load_time_ms,
        "Talvik load-boundary measured",
    );
}

fn print_milliseconds(label: &str, value: Option<u128>, source: &str) {
    match value {
        Some(milliseconds) => {
            println!("{label}: {} ({source})", format_milliseconds(milliseconds));
        }
        None => println!("{label}: unavailable"),
    }
}

fn print_count(label: &str, value: Option<u64>) {
    match value {
        Some(count) => println!("{label}: {count} (backend-reported)"),
        None => println!("{label}: unavailable"),
    }
}

fn print_rate(label: &str, value: Option<f64>) {
    match value {
        Some(rate) if rate.is_finite() => {
            println!("{label}: {rate:.2} tokens/s (backend-reported)");
        }
        _ => println!("{label}: unavailable"),
    }
}

fn run_benchmark(
    client: &MarinaHttpClient,
    model: &str,
    prompt: &str,
    runs: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut latencies = Vec::with_capacity(runs);
    let mut throughputs = Vec::with_capacity(runs);
    let mut states = Vec::with_capacity(runs);
    let mut backend = None;
    for run in 0..runs {
        let resident_before = client
            .list_models()?
            .into_iter()
            .find(|candidate| candidate.id == model)
            .and_then(|candidate| candidate.resident);
        let started = Instant::now();
        let response = chat(client, model, prompt, run + 1)?;
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        let state = classify_state(resident_before, response.metrics.model_load_time_ms);
        backend = backend.or(response.backend);
        latencies.push(elapsed);
        if let Some(rate) = response
            .metrics
            .tokens_per_second
            .filter(|rate| rate.is_finite())
        {
            throughputs.push(rate);
        }
        states.push(state);
        println!(
            "Run {}: {:.3} ms (OID client-measured), state: {}, prompt tokens: {}, output tokens: {}, TTFT: {} (backend-reported), inference: {} (backend-reported), load: {} (Talvik load-boundary), throughput: {} (backend-reported)",
            run + 1,
            elapsed,
            state,
            optional_count(response.metrics.prompt_tokens),
            optional_count(response.metrics.generated_tokens),
            optional_seconds(response.metrics.time_to_first_token_ms),
            optional_seconds(response.metrics.inference_time_ms),
            optional_seconds(response.metrics.model_load_time_ms),
            optional_rate(response.metrics.tokens_per_second)
        );
    }
    let median =
        percentile(&latencies, Percentile::Median).ok_or("benchmark produced no samples")?;
    let p95 = percentile(&latencies, Percentile::P95).ok_or("benchmark produced no samples")?;
    let warm = states
        .iter()
        .filter(|state| **state == SampleState::Warm)
        .count();
    let first_after_load = states
        .iter()
        .filter(|state| **state == SampleState::FirstAfterLoad)
        .count();
    let unknown = states
        .iter()
        .filter(|state| **state == SampleState::Unknown)
        .count();

    println!();
    println!("MarinaOS Inference Benchmark");
    println!("Model: {model}");
    println!("Backend: {}", backend.as_deref().unwrap_or("unavailable"));
    println!("Runs: {runs}");
    println!("Median latency: {median:.3} ms (OID client-measured)");
    println!("P95 latency: {p95:.3} ms (OID client-measured)");
    println!(
        "Backend throughput median ({}/{} reported): {}",
        throughputs.len(),
        runs,
        percentile(&throughputs, Percentile::Median).map_or_else(
            || "unavailable".to_owned(),
            |rate| format!("{rate:.2} tokens/s (backend-reported)")
        )
    );
    println!("Model state: {warm} warm, {first_after_load} first-after-load, {unknown} unknown");
    Ok(())
}

fn percentile(samples: &[f64], percentile: Percentile) -> Option<f64> {
    if samples.is_empty() {
        return None;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    match percentile {
        Percentile::Median if sorted.len() % 2 == 0 => {
            let upper = sorted.len() / 2;
            Some((sorted[upper - 1] + sorted[upper]) / 2.0)
        }
        Percentile::Median => Some(sorted[sorted.len() / 2]),
        Percentile::P95 => {
            let rank = sorted.len() / 100 * 95 + (sorted.len() % 100 * 95).div_ceil(100);
            Some(sorted[rank - 1])
        }
    }
}

fn format_milliseconds(milliseconds: u128) -> String {
    format!("{}.{:03} s", milliseconds / 1000, milliseconds % 1000)
}

fn optional_count(value: Option<u64>) -> String {
    value.map_or_else(|| "unavailable".to_owned(), |count| count.to_string())
}

fn optional_rate(value: Option<f64>) -> String {
    value.filter(|rate| rate.is_finite()).map_or_else(
        || "unavailable".to_owned(),
        |rate| format!("{rate:.2} tokens/s"),
    )
}

fn optional_seconds(value: Option<u128>) -> String {
    value.map_or_else(|| "unavailable".to_owned(), format_milliseconds)
}

#[cfg(test)]
mod tests {
    use super::{
        classify_state, parse_arguments, percentile, select_model, Mode, Percentile, SampleState,
    };
    use oid_model_client::ModelDescriptor;

    #[test]
    fn preserves_plain_invocation_and_parses_timing_mode() {
        assert_eq!(
            parse_arguments(&["hello".to_owned(), "world".to_owned()])
                .expect("plain arguments")
                .prompt,
            "hello world"
        );
        assert_eq!(
            parse_arguments(&["--timing".to_owned(), "hello".to_owned()])
                .expect("timing arguments")
                .mode,
            Mode::Timing
        );
    }

    #[test]
    fn benchmark_run_count_is_bounded_and_configurable() {
        assert_eq!(
            parse_arguments(&["--benchmark".to_owned(), "hello".to_owned()])
                .expect("default benchmark")
                .mode,
            Mode::Benchmark { runs: 5 }
        );
        assert_eq!(
            parse_arguments(&[
                "--benchmark".to_owned(),
                "--runs".to_owned(),
                "12".to_owned(),
                "hello".to_owned()
            ])
            .expect("configured benchmark")
            .mode,
            Mode::Benchmark { runs: 12 }
        );
        assert!(parse_arguments(&[
            "--benchmark".to_owned(),
            "--runs".to_owned(),
            "101".to_owned(),
            "hello".to_owned()
        ])
        .is_err());
    }

    #[test]
    fn calculates_median_and_nearest_rank_p95() {
        assert_eq!(percentile(&[1.0, 3.0], Percentile::Median), Some(2.0));
        assert_eq!(
            percentile(&[5.0, 1.0, 3.0, 2.0, 4.0], Percentile::Median),
            Some(3.0)
        );
        assert_eq!(
            percentile(&[1.0, 2.0, 3.0, 4.0, 5.0], Percentile::P95),
            Some(5.0)
        );
        assert_eq!(percentile(&[], Percentile::P95), None);
    }

    #[test]
    fn formats_benchmark_state_names_stably() {
        assert_eq!(SampleState::Warm.to_string(), "warm");
        assert_eq!(SampleState::FirstAfterLoad.to_string(), "first-after-load");
        assert_eq!(SampleState::Unknown.to_string(), "unknown");
    }

    #[test]
    fn benchmark_state_requires_residency_or_an_observed_load() {
        assert_eq!(classify_state(Some(true), None), SampleState::Warm);
        assert_eq!(
            classify_state(Some(false), Some(1500)),
            SampleState::FirstAfterLoad
        );
        assert_eq!(
            classify_state(None, Some(1500)),
            SampleState::FirstAfterLoad
        );
        assert_eq!(classify_state(Some(false), None), SampleState::Unknown);
        assert_eq!(classify_state(None, None), SampleState::Unknown);
    }

    #[test]
    fn model_selection_prefers_override_then_resident_then_smallest_estimate() {
        let models = [
            ModelDescriptor {
                id: "large".to_owned(),
                is_default: Some(false),
                resident: Some(false),
                estimated_memory_mib: Some(1530),
            },
            ModelDescriptor {
                id: "default".to_owned(),
                is_default: Some(true),
                resident: Some(false),
                estimated_memory_mib: Some(512),
            },
            ModelDescriptor {
                id: "smallest".to_owned(),
                is_default: Some(false),
                resident: Some(false),
                estimated_memory_mib: Some(256),
            },
            ModelDescriptor {
                id: "resident".to_owned(),
                is_default: Some(false),
                resident: Some(true),
                estimated_memory_mib: Some(1024),
            },
        ];

        assert_eq!(
            select_model(&models, None).expect("resident model"),
            "default"
        );
        let resident_fallback = [models[0].clone(), models[2].clone(), models[3].clone()];
        assert_eq!(
            select_model(&resident_fallback, None).expect("resident model fallback"),
            "resident"
        );
        let unresident = [models[0].clone(), models[2].clone()];
        assert_eq!(
            select_model(&unresident, None).expect("minimum memory model"),
            "smallest"
        );
        assert_eq!(
            select_model(&models, Some("smallest")).expect("configured model"),
            "smallest"
        );
    }
}
