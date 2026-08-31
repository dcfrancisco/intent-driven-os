//! Process-signal and shutdown coordination for the console.

use std::collections::VecDeque;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Signals relevant to the console lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessSignal {
    /// Interrupt the active child or input line.
    Interrupt,
    /// Request graceful OID shutdown.
    Terminate,
    /// Request graceful shutdown when the controlling session disappears.
    Hangup,
}

/// Thread-safe signal event source shared by the application and child supervisor.
#[derive(Clone, Debug)]
pub struct SignalController {
    events: Arc<Mutex<VecDeque<ProcessSignal>>>,
    shutdown_requested: Arc<AtomicBool>,
}

impl SignalController {
    /// Install the Unix signal bridge where supported.
    pub fn new() -> io::Result<Self> {
        let controller = Self::default();
        #[cfg(unix)]
        {
            let mut signals = signal_hook::iterator::Signals::new([
                signal_hook::consts::SIGINT,
                signal_hook::consts::SIGTERM,
                signal_hook::consts::SIGHUP,
            ])?;
            let events = Arc::clone(&controller.events);
            let shutdown_requested = Arc::clone(&controller.shutdown_requested);
            std::thread::Builder::new()
                .name("oid-signal-listener".to_owned())
                .spawn(move || {
                    for signal in signals.forever() {
                        let event = match signal {
                            signal_hook::consts::SIGINT => ProcessSignal::Interrupt,
                            signal_hook::consts::SIGTERM => ProcessSignal::Terminate,
                            signal_hook::consts::SIGHUP => ProcessSignal::Hangup,
                            _ => continue,
                        };
                        if matches!(event, ProcessSignal::Terminate | ProcessSignal::Hangup) {
                            shutdown_requested.store(true, Ordering::Release);
                        }
                        if let Ok(mut events) = events.lock() {
                            events.push_back(event);
                        }
                    }
                })
                .map_err(|error| io::Error::other(format!("signal listener: {error}")))?;
        }
        Ok(controller)
    }

    /// Construct a controller without installing process handlers.
    #[must_use]
    pub fn test() -> Self {
        Self::default()
    }

    /// Inject a signal for deterministic tests.
    #[allow(dead_code)]
    pub fn inject(&self, signal: ProcessSignal) {
        if matches!(signal, ProcessSignal::Terminate | ProcessSignal::Hangup) {
            self.shutdown_requested.store(true, Ordering::Release);
        }
        self.events
            .lock()
            .expect("signal controller lock")
            .push_back(signal);
    }

    /// Return whether shutdown was requested, preserving unrelated events.
    pub fn take_shutdown(&self) -> bool {
        let mut shutdown = false;
        if let Ok(mut events) = self.events.lock() {
            let mut retained = VecDeque::new();
            while let Some(event) = events.pop_front() {
                if matches!(event, ProcessSignal::Terminate | ProcessSignal::Hangup) {
                    shutdown = true;
                } else {
                    retained.push_back(event);
                }
            }
            *events = retained;
        }
        shutdown || self.is_shutdown_requested()
    }

    /// Return whether graceful shutdown has been requested.
    #[must_use]
    pub fn is_shutdown_requested(&self) -> bool {
        self.shutdown_requested.load(Ordering::Acquire)
    }

    /// Return whether an interrupt was requested.
    pub fn take_interrupt(&self) -> bool {
        let mut interrupted = false;
        if let Ok(mut events) = self.events.lock() {
            let mut retained = VecDeque::new();
            while let Some(event) = events.pop_front() {
                if event == ProcessSignal::Interrupt {
                    interrupted = true;
                } else {
                    retained.push_back(event);
                }
            }
            *events = retained;
        }
        interrupted
    }
}

impl Default for SignalController {
    fn default() -> Self {
        Self {
            events: Arc::new(Mutex::new(VecDeque::new())),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ProcessSignal, SignalController};

    #[test]
    fn injected_signals_are_deterministic_and_distinct() {
        let controller = SignalController::test();
        controller.inject(ProcessSignal::Interrupt);
        controller.inject(ProcessSignal::Terminate);
        assert!(controller.take_interrupt());
        assert!(controller.take_shutdown());
        assert!(!controller.take_interrupt());
    }

    #[test]
    fn repeated_interrupts_are_idempotently_consumed() {
        let controller = SignalController::test();
        controller.inject(ProcessSignal::Interrupt);
        controller.inject(ProcessSignal::Interrupt);
        assert!(controller.take_interrupt());
        assert!(!controller.take_interrupt());
    }
}
