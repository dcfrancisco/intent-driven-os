//! MarinaOS user-session desktop shell.
//!
//! The desktop is a client of the unprivileged foundation service. It never
//! invokes privileged commands, Talvik, or model output directly.

#![cfg_attr(target_os = "linux", forbid(unsafe_code))]
#![cfg_attr(target_os = "linux", warn(missing_docs))]

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("marina-desktop is currently supported on Linux only");
}

#[cfg(target_os = "linux")]
mod linux {
    use gtk4::prelude::*;
    use gtk4::{
        glib, Box as GtkBox, Button, Entry, Label, ListBox, ListBoxRow, Orientation,
    };
    use libadwaita as adw;
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::sync::mpsc;
    use std::time::Duration;

    const SOCKET: &str = "/run/marinaos/marinaos.sock";

    #[derive(Clone, Debug, Default)]
    struct FoundationSnapshot {
        status: BTreeMap<String, String>,
        hardware: BTreeMap<String, String>,
        talvik: String,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum IntentState {
        Pending,
        Unavailable(String),
    }

    #[derive(Clone, Debug)]
    struct MarinaApi {
        socket: PathBuf,
    }

    impl MarinaApi {
        fn new() -> Self {
            Self {
                socket: std::env::var_os("MARINAOS_SOCKET")
                    .map_or_else(|| SOCKET.into(), PathBuf::from),
            }
        }

        fn request(&self, method: &str) -> Result<BTreeMap<String, String>, String> {
            let mut stream = UnixStream::connect(&self.socket)
                .map_err(|e| format!("foundation unavailable at {}: {e}", self.socket.display()))?;
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .map_err(|e| e.to_string())?;
            writeln!(stream, "{{\"version\":1,\"method\":\"{method}\"}}")
                .map_err(|e| e.to_string())?;
            let mut response = String::new();
            BufReader::new(stream)
                .read_line(&mut response)
                .map_err(|e| e.to_string())?;
            let values = parse_flat_json(&response)?;
            if values.contains_key("error") {
                return Err(values
                    .get("detail")
                    .cloned()
                    .unwrap_or_else(|| "foundation rejected request".to_owned()));
            }
            Ok(values)
        }

        fn snapshot(&self) -> Result<FoundationSnapshot, String> {
            Ok(FoundationSnapshot {
                status: self.request("status")?,
                hardware: self.request("hardware")?,
                talvik: "optional / not required".to_owned(),
            })
        }

        fn submit_intent(&self, _request: &str) -> IntentState {
            IntentState::Unavailable("No governed intent-submission endpoint is exposed by marinaosd yet. Nothing was executed.".to_owned())
        }
    }

    fn parse_flat_json(input: &str) -> Result<BTreeMap<String, String>, String> {
        let body = input
            .trim()
            .strip_prefix('{')
            .and_then(|v| v.strip_suffix('}'))
            .ok_or_else(|| "invalid foundation response".to_owned())?;
        let mut result = BTreeMap::new();
        for field in body.split(',') {
            let Some((key, value)) = field.split_once(':') else {
                continue;
            };
            let key = key.trim().trim_matches('"');
            let value = value.trim().trim_matches('"');
            if !key.is_empty() {
                result.insert(key.to_owned(), value.replace("\\\"", "\""));
            }
        }
        Ok(result)
    }

    fn labelled(text: &str) -> Label {
        let label = Label::new(Some(text));
        label.set_xalign(0.0);
        label.set_wrap(true);
        label.set_selectable(true);
        label
    }

    fn status_row(name: &str, value: &str) -> ListBoxRow {
        let row = ListBoxRow::new();
        let box_ = GtkBox::new(Orientation::Horizontal, 12);
        box_.set_margin_top(8);
        box_.set_margin_bottom(8);
        box_.set_margin_start(12);
        box_.set_margin_end(12);
        let key = labelled(name);
        key.set_hexpand(true);
        let val = labelled(value);
        val.add_css_class(if value.contains("unavailable") {
            "error"
        } else {
            "success"
        });
        box_.append(&key);
        box_.append(&val);
        row.set_child(Some(&box_));
        row
    }

    fn build_window(app: &adw::Application) {
        let window = adw::ApplicationWindow::new(app);
        window.set_title(Some("MarinaOS"));
        window.set_default_size(1120, 720);
        window.set_size_request(760, 520);
        let root = GtkBox::new(Orientation::Vertical, 0);
        let header = adw::HeaderBar::new();
        let title = adw::WindowTitle::new("MarinaOS", Some("Trustworthy system operations"));
        header.set_title_widget(Some(&title));
        root.append(&header);
        let content = GtkBox::new(Orientation::Horizontal, 0);
        let sidebar = GtkBox::new(Orientation::Vertical, 10);
        sidebar.set_width_request(250);
        sidebar.set_margin_top(18);
        sidebar.set_margin_bottom(18);
        sidebar.set_margin_start(18);
        sidebar.set_margin_end(12);
        let brand = labelled("MARINAOS\n\nYour computer, with receipts.");
        brand.add_css_class("title-2");
        sidebar.append(&brand);
        sidebar.append(&labelled("REQUESTS"));
        let requests = ListBox::new();
        requests.set_selection_mode(gtk4::SelectionMode::Single);
        requests.append(&status_row("System overview", "selected"));
        sidebar.append(&requests);
        sidebar.append(&labelled(
            "The desktop remains usable when Talvik is absent.",
        ));
        content.append(&sidebar);
        let panel = GtkBox::new(Orientation::Vertical, 18);
        panel.set_margin_top(24);
        panel.set_margin_bottom(24);
        panel.set_margin_start(12);
        panel.set_margin_end(24);
        let overview = labelled("System overview");
        overview.add_css_class("title-1");
        panel.append(&overview);
        panel.append(&labelled(
            "Authoritative foundation state is shown below. Model text never determines success.",
        ));
        let health = ListBox::new();
        health.set_vexpand(true);
        health.append(&status_row("Foundation service", "checking…"));
        health.append(&status_row("Talvik inference", "optional / checking…"));
        health.append(&status_row("Hardware", "checking…"));
        panel.append(&health);
        let detail = labelled("Timeline\n  • foundation connection pending\n  • no intent has been submitted\n  • no operation or evidence exists");
        detail.add_css_class("card");
        panel.append(&detail);
        let command_box = GtkBox::new(Orientation::Horizontal, 8);
        let entry = Entry::new();
        entry.set_placeholder_text(Some("Ask MarinaOS what is happening…"));
        entry.set_hexpand(true);
        entry.set_tooltip_text(Some("Natural-language request; Enter submits"));
        let submit = Button::with_label("Submit request");
        submit.set_tooltip_text(Some("Submit a governed request"));
        command_box.append(&entry);
        command_box.append(&submit);
        panel.append(&command_box);
        let api = MarinaApi::new();
        let state = Rc::new(RefCell::new(IntentState::Pending));
        let detail_for_submit = detail.clone();
        let entry_for_submit = entry.clone();
        let api_for_submit = api.clone();
        let state_for_submit = state.clone();
        submit.connect_clicked(move |_| {
            let request = entry_for_submit.text().to_string();
            if request.trim().is_empty() {
                detail_for_submit.set_text("Enter a request first. No operation was created.");
                return;
            }
            let result = api_for_submit.submit_intent(&request);
            *state_for_submit.borrow_mut() = result.clone();
            match result {
                IntentState::Pending => detail_for_submit.set_text("Request pending."),
                IntentState::Unavailable(message) => {
                    detail_for_submit.set_text(&format!("Request not submitted\n\n{message}"))
                }
            }
        });
        entry.connect_activate(move |_| submit.emit_clicked());
        content.append(&panel);
        root.append(&content);
        window.set_content(Some(&root));
        let (sender, receiver) = mpsc::channel::<Result<FoundationSnapshot, String>>();
        let api_for_thread = api.clone();
        std::thread::spawn(move || {
            let _ = sender.send(api_for_thread.snapshot());
        });
        let health_for_poll = health.clone();
        let detail_for_poll = detail.clone();
        glib::timeout_add_local(Duration::from_millis(50), move || {
            match receiver.try_recv() {
                Ok(Ok(snapshot)) => {
                    health_for_poll.remove_all();
                    health_for_poll.append(&status_row(
                        "Foundation service",
                        snapshot
                            .status
                            .get("service")
                            .map_or("ready", String::as_str),
                    ));
                    health_for_poll.append(&status_row("Talvik inference", &snapshot.talvik));
                    let hardware = snapshot.hardware.get("cpu").map_or_else(
                        || "connected; details unavailable".to_owned(),
                        |cpu| {
                            format!(
                                "{} · {} cores",
                                cpu,
                                snapshot
                                    .hardware
                                    .get("logical_cores")
                                    .map_or("?", String::as_str)
                            )
                        },
                    );
                    health_for_poll.append(&status_row("Hardware", &hardware));
                    detail_for_poll.set_text("Timeline\n  ✓ foundation status received over local IPC\n  ✓ hardware snapshot received over local IPC\n  • governed intent API: unavailable\n  • operation evidence: unavailable");
                    glib::ControlFlow::Break
                }
                Ok(Err(error)) => {
                    health_for_poll.remove_all();
                    health_for_poll.append(&status_row(
                        "Foundation service",
                        &format!("unavailable: {error}"),
                    ));
                    health_for_poll
                        .append(&status_row("Talvik inference", "optional / unavailable"));
                    health_for_poll.append(&status_row(
                        "Hardware",
                        "unavailable without foundation service",
                    ));
                    detail_for_poll.set_text("Offline mode\n\nMarinaOS is still running as a desktop client. Start marinaosd to reconnect. No diagnostic result is being claimed.");
                    glib::ControlFlow::Break
                }
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
            }
        });
        window.present();
    }

    pub fn run() {
        eprintln!("marina-desktop: starting user-session UI");
        let app = adw::Application::builder()
            .application_id("org.marinaos.Desktop")
            .build();
        app.connect_activate(build_window);
        app.run();
    }

    #[cfg(test)]
    mod tests {
        use super::parse_flat_json;

        #[test]
        fn parses_foundation_scalars_without_treating_text_as_authority() {
            let values = parse_flat_json(r#"{"version":1,"service":"ready","talvik":"optional"}"#)
                .expect("valid response");
            assert_eq!(values.get("service").map(String::as_str), Some("ready"));
            assert_eq!(values.get("talvik").map(String::as_str), Some("optional"));
        }

        #[test]
        fn rejects_non_object_responses() {
            assert!(parse_flat_json("offline").is_err());
        }
    }
}

#[cfg(target_os = "linux")]
fn main() {
    linux::run();
}
