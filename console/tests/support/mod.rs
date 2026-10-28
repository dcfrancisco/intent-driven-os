#![allow(missing_docs)]

use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[cfg(windows)]
use std::net::{TcpListener, TcpStream};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};

#[cfg(unix)]
type Listener = UnixListener;
#[cfg(windows)]
type Listener = TcpListener;

#[cfg(unix)]
type Stream = UnixStream;
#[cfg(windows)]
type Stream = TcpStream;

pub struct MockTalvik {
    endpoint: PathBuf,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl MockTalvik {
    pub fn start(state_root: &std::path::Path) -> Self {
        static NEXT_ENDPOINT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let requested_endpoint = state_root.join(format!(
            "mock-talvik-{}-{}.sock",
            std::process::id(),
            NEXT_ENDPOINT.fetch_add(1, Ordering::Relaxed)
        ));
        let (endpoint, listener) = bind_listener(&requested_endpoint);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = thread::spawn(move || {
            while !worker_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        if let Err(error) = respond(stream) {
                            if error.kind() != io::ErrorKind::ConnectionReset {
                                eprintln!("mock Talvik request failed: {error}");
                            }
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => {
                        eprintln!("mock Talvik listener failed: {error}");
                        break;
                    }
                }
            }
        });
        Self {
            endpoint,
            stop,
            worker: Some(worker),
        }
    }

    pub fn configure(&self, command: &mut Command) {
        #[cfg(unix)]
        command.env("TALVIK_SOCKET", &self.endpoint);
        #[cfg(windows)]
        command.env(
            "TALVIK_TCP_ADDR",
            self.endpoint
                .to_str()
                .expect("mock endpoint should be valid UTF-8"),
        );
    }
}

impl Drop for MockTalvik {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        #[cfg(unix)]
        {
            let _ = std::fs::remove_file(&self.endpoint);
        }
    }
}

#[cfg(unix)]
fn bind_listener(endpoint: &std::path::Path) -> (PathBuf, Listener) {
    if let Some(parent) = endpoint.parent() {
        std::fs::create_dir_all(parent).expect("mock socket parent");
    }
    let listener = UnixListener::bind(endpoint).expect("mock Talvik socket");
    listener
        .set_nonblocking(true)
        .expect("mock socket nonblocking");
    (endpoint.to_path_buf(), listener)
}

#[cfg(windows)]
fn bind_listener(_endpoint: &std::path::Path) -> (PathBuf, Listener) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("mock Talvik listener");
    let endpoint = PathBuf::from(
        listener
            .local_addr()
            .expect("mock listener address")
            .to_string(),
    );
    listener
        .set_nonblocking(true)
        .expect("mock listener nonblocking");
    (endpoint, listener)
}

fn respond(mut stream: Stream) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut request = String::new();
    BufReader::new(stream.try_clone()?).read_line(&mut request)?;
    match request.trim_end_matches(['\r', '\n']).split('\t').next() {
        Some("hello") => writeln!(stream, "OK protocol\t2"),
        Some("status") => writeln!(
            stream,
            "OK status\tHealthy\tNone\t\t\t0.1\tmock\tfalse\ttest\ttest\tfalse\t0 B\t0\t0\t8\tfalse\t0\t0\t\t\tfalse\t\t"
        ),
        Some("model_list") => writeln!(stream, "END"),
        Some("model_load" | "model_unload") => writeln!(stream, "OK"),
        Some("model_pull") => writeln!(stream, "ERROR\tmock daemon does not pull models"),
        Some("tokenize") => writeln!(stream, "OK tokenize\t0"),
        Some("cancel") => writeln!(stream, "OK cancelled"),
        Some("generate_v2") => {
            writeln!(stream, "TOKEN\tmock response")?;
            writeln!(
                stream,
                "DONE\tcompleted\tmock-generation\t2\t1\t3\t20\t18\t17\t2\t3\t15\t100.0\ttrue\tstop\tfalse\t0\tcpu\tCPU\t0\t0\t"
            )
        }
        _ => writeln!(stream, "ERROR\tunsupported mock request"),
    }
}
