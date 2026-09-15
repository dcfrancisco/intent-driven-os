//! System-wide audio output controls for Linux desktops.

use std::io;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const DEFAULT_WPCTL_SINK: &str = "@DEFAULT_AUDIO_SINK@";
const DEFAULT_PACTL_SINK: &str = "@DEFAULT_SINK@";
const BLACKHOLE_DEVICE: &str = "BlackHole 16ch";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AudioRequest {
    Status,
    Gain(u8),
    Mute(MuteAction),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MuteAction {
    Mute,
    Unmute,
    Toggle,
}

trait CommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<String, String>;
}

#[derive(Clone, Copy, Debug, Default)]
struct ProcessCommandRunner;

impl CommandRunner for ProcessCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
        let output = Command::new(program).args(args).output().map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                format!("{program} is not installed")
            } else {
                format!("could not run {program}: {error}")
            }
        })?;

        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned());
        }

        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(if detail.is_empty() {
            format!("{program} exited with {}", output.status)
        } else {
            format!("{program}: {detail}")
        })
    }
}

/// Execute an `audio` console command against the default system output.
#[must_use]
pub fn execute(command: &str) -> Vec<String> {
    if cfg!(target_os = "macos") {
        return execute_macos(command, &ProcessCommandRunner);
    }
    execute_with_runner(command, &ProcessCommandRunner)
}

fn execute_macos(command: &str, runner: &dyn CommandRunner) -> Vec<String> {
    let request = match parse(command) {
        Ok(request) => request,
        Err(message) => return vec![message, usage()],
    };

    match request {
        AudioRequest::Gain(multiplier) if multiplier > 1 => start_macos_relay(multiplier),
        AudioRequest::Gain(_) => reset_macos(runner),
        AudioRequest::Status => run_macos(
            runner,
            "output volume of (get volume settings)",
            "System audio status via macOS: ",
        ),
        AudioRequest::Mute(action) => {
            let script = match action {
                MuteAction::Mute => "set volume output muted true",
                MuteAction::Unmute => "set volume output muted false",
                MuteAction::Toggle => {
                    "set volume output muted not (output muted of (get volume settings))"
                }
            };
            run_macos(
                runner,
                script,
                &format!("System audio {} via macOS.", action.label()),
            )
        }
    }
}

fn start_macos_relay(multiplier: u8) -> Vec<String> {
    let runner = ProcessCommandRunner;
    let _ = stop_macos_relay(&runner);
    let original = match runner.run("SwitchAudioSource", &["-c", "-t", "output", "-f", "json"]) {
        Ok(output) => output,
        Err(error) => return audio_error(&error),
    };
    let original_name = json_field(&original, "name").unwrap_or_default();
    if original_name.is_empty() || original_name == BLACKHOLE_DEVICE {
        return vec![
            "Could not identify a physical macOS output device.".to_owned(),
            "Reboot macOS so BlackHole 16ch appears, then select your speakers or headphones and retry.".to_owned(),
        ];
    }
    let output_devices = match list_macos_output_devices() {
        Ok(devices) => devices,
        Err(error) => return audio_error(&error),
    };
    let Some(output_index) = output_devices
        .iter()
        .find_map(|(index, name)| (name == &original_name).then_some(*index))
    else {
        return vec![format!(
            "FFmpeg could not find the physical output device {original_name:?}."
        )];
    };
    if let Err(error) = runner.run("SwitchAudioSource", &["-s", BLACKHOLE_DEVICE]) {
        return vec![
            format!("Could not route macOS audio through {BLACKHOLE_DEVICE}: {error}"),
            "Reboot macOS first if the BlackHole device is not listed.".to_owned(),
        ];
    }
    let gain = format!("{multiplier}.0");
    let child = match Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "avfoundation",
            "-i",
            ":BlackHole 16ch",
            "-af",
            &format!("volume={gain}"),
            "-ac",
            "2",
            "-f",
            "audiotoolbox",
            "-audio_device_index",
            &output_index.to_string(),
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            let _ = runner.run("SwitchAudioSource", &["-s", &original_name]);
            return audio_error(&format!("could not start ffmpeg: {error}"));
        }
    };
    let state = format!("{}\n{}\n{}\n", child.id(), original_name, output_index);
    if let Err(error) = std::fs::write(macos_relay_state_path(), state) {
        let _ = runner.run("kill", &["-TERM", &child.id().to_string()]);
        let _ = runner.run("SwitchAudioSource", &["-s", &original_name]);
        return audio_error(&format!("could not save relay state: {error}"));
    }
    vec![format!(
        "System audio relay started at {multiplier}x through {original_name}."
    )]
}

fn reset_macos(runner: &dyn CommandRunner) -> Vec<String> {
    let mut output = stop_macos_relay(runner);
    output.extend(run_macos(
        runner,
        "set volume output volume 100",
        "System audio reset to 1x (100%) via macOS.",
    ));
    output
}

fn stop_macos_relay(runner: &dyn CommandRunner) -> Vec<String> {
    let path = macos_relay_state_path();
    let Ok(state) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let mut lines = state.lines();
    let Some(pid) = lines.next() else {
        return Vec::new();
    };
    let original_name = lines.next().unwrap_or_default();
    let mut output = Vec::new();
    if let Err(error) = runner.run("kill", &["-TERM", pid]) {
        output.push(format!("Audio relay stop warning: {error}"));
    }
    if !original_name.is_empty() {
        if let Err(error) = runner.run("SwitchAudioSource", &["-s", original_name]) {
            output.push(format!("Audio output restore warning: {error}"));
        }
    }
    let _ = std::fs::remove_file(path);
    output
}

fn macos_relay_state_path() -> PathBuf {
    std::env::temp_dir().join("oid-audio-relay.state")
}

fn list_macos_output_devices() -> Result<Vec<(usize, String)>, String> {
    let output = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-f",
            "audiotoolbox",
            "-list_devices",
            "true",
            "-i",
            "-",
        ])
        .output()
        .map_err(|error| format!("could not run ffmpeg: {error}"))?;
    let text = String::from_utf8_lossy(&output.stderr);
    let devices = text
        .lines()
        .filter_map(|line| {
            let start = line
                .match_indices('[')
                .map(|(index, _)| index)
                .next_back()?;
            let end = line[start + 1..].find(']')? + start + 1;
            let index = line[start + 1..end].parse().ok()?;
            let name = line[end + 1..].trim().to_owned();
            (!name.is_empty()).then_some((index, name))
        })
        .collect::<Vec<_>>();
    if devices.is_empty() {
        Err("FFmpeg found no macOS AudioToolbox output devices; reboot after installing BlackHole and retry.".to_owned())
    } else {
        Ok(devices)
    }
}

fn json_field(json: &str, field: &str) -> Option<String> {
    let marker = format!("\"{field}\": \"");
    let value = json.split_once(&marker)?.1.split_once('"')?.0;
    (!value.is_empty()).then_some(value.to_owned())
}

fn run_macos(runner: &dyn CommandRunner, script: &str, success: &str) -> Vec<String> {
    runner.run("osascript", &["-e", script]).map_or_else(
        |error| audio_error(&error),
        |output| {
            if success.ends_with(": ") {
                vec![format!("{success}{}", one_line(&output))]
            } else {
                vec![success.to_owned()]
            }
        },
    )
}

fn execute_with_runner(command: &str, runner: &dyn CommandRunner) -> Vec<String> {
    let request = match parse(command) {
        Ok(request) => request,
        Err(message) => return vec![message, usage()],
    };

    match request {
        AudioRequest::Status => run_with_fallback(
            runner,
            &["get-volume", DEFAULT_WPCTL_SINK],
            &["get-sink-volume", DEFAULT_PACTL_SINK],
        )
        .map_or_else(
            |error| audio_error(&error),
            |(backend, output)| vec![format!("System audio ({backend}): {}", one_line(&output))],
        ),
        AudioRequest::Gain(multiplier) => {
            let wpctl_gain = format!("{multiplier}.0");
            let pactl_gain = format!("{}%", u16::from(multiplier) * 100);
            run_with_fallback(
                runner,
                &[
                    "set-volume",
                    DEFAULT_WPCTL_SINK,
                    &wpctl_gain,
                    "--limit",
                    "3.0",
                ],
                &["set-sink-volume", DEFAULT_PACTL_SINK, &pactl_gain],
            )
            .map_or_else(
                |error| audio_error(&error),
                |(backend, _)| {
                    let mut lines = vec![format!(
                        "System audio set to {multiplier}x ({}%) via {backend}.",
                        u16::from(multiplier) * 100
                    )];
                    if multiplier > 1 {
                        lines.push(
                            "Warning: amplification above 100% can clip or distort audio."
                                .to_owned(),
                        );
                    }
                    lines
                },
            )
        }
        AudioRequest::Mute(action) => {
            let value = match action {
                MuteAction::Mute => "1",
                MuteAction::Unmute => "0",
                MuteAction::Toggle => "toggle",
            };
            run_with_fallback(
                runner,
                &["set-mute", DEFAULT_WPCTL_SINK, value],
                &["set-sink-mute", DEFAULT_PACTL_SINK, value],
            )
            .map_or_else(
                |error| audio_error(&error),
                |(backend, _)| vec![format!("System audio {} via {backend}.", action.label())],
            )
        }
    }
}

fn parse(command: &str) -> Result<AudioRequest, String> {
    let command = command.trim();
    let arguments = command
        .strip_prefix("audio")
        .or_else(|| command.strip_prefix("aidio"))
        .ok_or_else(|| "Expected an audio command.".to_owned())?
        .trim();

    match arguments {
        "" | "status" => Ok(AudioRequest::Status),
        "1x" | "100%" | "reset" => Ok(AudioRequest::Gain(1)),
        "2x" | "200%" => Ok(AudioRequest::Gain(2)),
        "3x" | "300%" => Ok(AudioRequest::Gain(3)),
        "mute" => Ok(AudioRequest::Mute(MuteAction::Mute)),
        "unmute" => Ok(AudioRequest::Mute(MuteAction::Unmute)),
        "toggle-mute" => Ok(AudioRequest::Mute(MuteAction::Toggle)),
        _ => Err(format!("Unsupported audio control: {arguments}")),
    }
}

fn run_with_fallback(
    runner: &dyn CommandRunner,
    wpctl_args: &[&str],
    pactl_args: &[&str],
) -> Result<(&'static str, String), String> {
    match runner.run("wpctl", wpctl_args) {
        Ok(output) => Ok(("PipeWire", output)),
        Err(wpctl_error) => runner
            .run("pactl", pactl_args)
            .map(|output| ("PulseAudio", output))
            .map_err(|pactl_error| format!("{wpctl_error}; {pactl_error}")),
    }
}

fn audio_error(detail: &str) -> Vec<String> {
    vec![
        format!("System audio control failed: {detail}"),
        platform_install_hint().to_owned(),
    ]
}

fn platform_install_hint() -> &'static str {
    if cfg!(target_os = "linux") {
        "Linux: install WirePlumber (wpctl) or PulseAudio utilities (pactl), and ensure your user audio session is running."
    } else if cfg!(target_os = "macos") {
        "macOS: install BlackHole 16ch, FFmpeg, and switchaudio-osx; reboot after installing BlackHole so its driver appears."
    } else {
        "System audio control currently supports Linux and macOS only."
    }
}

fn usage() -> String {
    "Usage: :audio [status|1x|2x|3x|mute|unmute|toggle-mute]".to_owned()
}

fn one_line(output: &str) -> String {
    if output.is_empty() {
        "command completed".to_owned()
    } else {
        output.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

impl MuteAction {
    const fn label(self) -> &'static str {
        match self {
            Self::Mute => "muted",
            Self::Unmute => "unmuted",
            Self::Toggle => "mute toggled",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{execute_with_runner, parse, AudioRequest, CommandRunner, MuteAction};
    use std::sync::Mutex;

    #[derive(Debug)]
    struct FakeRunner {
        calls: Mutex<Vec<(String, Vec<String>)>>,
        wpctl_fails: bool,
    }

    impl FakeRunner {
        fn new(wpctl_fails: bool) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                wpctl_fails,
            }
        }
    }

    impl CommandRunner for FakeRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
            self.calls.lock().expect("calls lock").push((
                program.to_owned(),
                args.iter().map(|argument| (*argument).to_owned()).collect(),
            ));
            if program == "wpctl" && self.wpctl_fails {
                Err("wpctl unavailable".to_owned())
            } else {
                Ok("Volume: 2.00".to_owned())
            }
        }
    }

    #[test]
    fn parses_supported_gain_and_mute_controls() {
        assert_eq!(parse("audio"), Ok(AudioRequest::Status));
        assert_eq!(parse("aidio 2x"), Ok(AudioRequest::Gain(2)));
        assert_eq!(parse("audio 2x"), Ok(AudioRequest::Gain(2)));
        assert_eq!(parse("audio 300%"), Ok(AudioRequest::Gain(3)));
        assert_eq!(
            parse("audio toggle-mute"),
            Ok(AudioRequest::Mute(MuteAction::Toggle))
        );
        assert!(parse("audio 4x").is_err());
    }

    #[test]
    fn sets_pipewire_gain_with_a_three_x_limit() {
        let runner = FakeRunner::new(false);
        let output = execute_with_runner("audio 2x", &runner);
        assert!(output[0].contains("200%) via PipeWire"));
        assert_eq!(
            runner.calls.into_inner().expect("calls"),
            vec![(
                "wpctl".to_owned(),
                vec![
                    "set-volume".to_owned(),
                    "@DEFAULT_AUDIO_SINK@".to_owned(),
                    "2.0".to_owned(),
                    "--limit".to_owned(),
                    "3.0".to_owned(),
                ],
            )]
        );
    }

    #[test]
    fn falls_back_to_pulseaudio() {
        let runner = FakeRunner::new(true);
        let output = execute_with_runner("audio 3x", &runner);
        assert!(output[0].contains("300%) via PulseAudio"));
        let calls = runner.calls.into_inner().expect("calls");
        assert_eq!(calls[0].0, "wpctl");
        assert_eq!(calls[1].0, "pactl");
        assert_eq!(calls[1].1.last().map(String::as_str), Some("300%"));
    }
}
