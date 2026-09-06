//! Development-only native experiment. No adapter has passed Codex acceptance.
//! No global hooks, screen capture, telemetry, automatic sending, or retries.
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    hash::{Hash, Hasher},
};
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;
#[derive(Serialize, Clone)]
pub struct Status {
    pub available: bool,
    pub enabled: bool,
    pub armed: bool,
    pub manual_codex: bool,
    pub manual_codex_available: bool,
    pub token: u64,
    pub destination: Option<String>,
    pub message: String,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Request {
    Status,
    Enable { value: bool },
    ManualCodex { value: bool },
    Arm,
    Capture,
    Native { text: String, token: u64 },
    Paste { text: String, token: u64 },
}
pub trait TextInsertionService {
    fn capture(&mut self) -> Result<(), String>;
    fn destination(&self) -> Option<String>;
    fn insert(&mut self, text: &str, clipboard: bool) -> Result<String, String>;
}
pub fn fingerprint(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}
fn platform(manual_codex: bool) -> Box<dyn TextInsertionService> {
    #[cfg(target_os = "macos")]
    return Box::new(macos::Mac::new(manual_codex));
    #[cfg(windows)]
    {
        let _ = manual_codex;
        Box::new(windows::Windows::default())
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    compile_error!("Insertion experiment supports macOS and Windows only");
}
struct Probe {
    enabled: bool,
    armed: bool,
    manual_codex: bool,
    token: u64,
    ready: bool,
    service: Box<dyn TextInsertionService>,
    message: String,
}
impl Default for Probe {
    fn default() -> Self {
        Self {
            enabled: false,
            armed: false,
            manual_codex: false,
            token: 0,
            ready: false,
            service: platform(false),
            message: "Development experiment. No destination is validated for release.".into(),
        }
    }
}
impl Probe {
    fn reset(&mut self) {
        self.armed = false;
        self.ready = false;
        self.token += 1;
        self.service = platform(self.manual_codex);
    }
    fn apply(&mut self, request: Request) -> Status {
        match request {
            Request::Enable { value } => {
                self.enabled = value;
                self.reset();
                self.message =
                    "Click Capture next field, focus the test field yourself, then return here."
                        .into();
            }
            Request::ManualCodex { value } => {
                self.manual_codex = value && cfg!(target_os = "macos");
                self.reset();
                self.message = "Manual Codex testing is macOS-only and is not release approval. Operate Codex yourself; never send the test text.".into();
            }
            Request::Arm if self.enabled => {
                self.reset();
                self.armed = true;
                self.message = "Waiting for an eligible external field. Click its cursor or selection yourself, then return here. Capture stops after one field.".into();
            }
            Request::Capture if self.enabled && self.armed => match self.service.capture() {
                Ok(()) if self.service.destination().is_some() => {
                    self.ready = true;
                    self.armed = false;
                    self.message = "Target captured and frozen. Check the destination below. Click one insertion method once.".into();
                }
                Ok(()) => {}
                Err(error) => self.message = error,
            },
            Request::Native { text, token } => self.insert(&text, token, false),
            Request::Paste { text, token } => self.insert(&text, token, true),
            _ => {}
        }
        Status {
            available: true,
            enabled: self.enabled,
            armed: self.armed,
            manual_codex: self.manual_codex,
            manual_codex_available: cfg!(target_os = "macos"),
            token: self.token,
            destination: if self.ready {
                self.service.destination()
            } else {
                None
            },
            message: self.message.clone(),
        }
    }
    fn insert(&mut self, text: &str, token: u64, clipboard: bool) {
        if !self.enabled || !self.ready || token != self.token {
            self.message = "No fresh target for this attempt. Nothing was inserted.".into();
            return;
        }
        // Consume the capture before any native call, including errors. Queued
        // clicks/captures cannot retry, switch methods, or reuse an uncertain write.
        self.ready = false;
        self.armed = false;
        self.token += 1;
        self.message = self
            .service
            .insert(text, clipboard)
            .unwrap_or_else(|error| error);
    }
}
thread_local! {static PROBE:RefCell<Probe> = RefCell::new(Probe::default());}
/// Always executed on the Tauri main thread, including COM/UIA access.
pub fn execute(request: Request) -> Status {
    PROBE.with(|state| state.borrow_mut().apply(request))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};
    struct Fake {
        calls: Rc<Cell<usize>>,
    }
    impl TextInsertionService for Fake {
        fn capture(&mut self) -> Result<(), String> {
            Ok(())
        }
        fn destination(&self) -> Option<String> {
            Some("Disposable field".into())
        }
        fn insert(&mut self, _: &str, _: bool) -> Result<String, String> {
            self.calls.set(self.calls.get() + 1);
            Err("Uncertain outcome; no retry".into())
        }
    }
    #[test]
    fn uncertain_attempt_consumes_token_and_ignores_queued_capture() {
        let calls = Rc::new(Cell::new(0));
        let mut probe = Probe {
            enabled: true,
            armed: true,
            service: Box::new(Fake {
                calls: calls.clone(),
            }),
            ..Probe::default()
        };
        let target = probe.apply(Request::Capture);
        assert!(!target.armed);
        assert!(target.destination.is_some());
        probe.apply(Request::Native {
            text: "test".into(),
            token: target.token,
        });
        probe.apply(Request::Capture);
        let after = probe.apply(Request::Paste {
            text: "test".into(),
            token: target.token,
        });
        assert_eq!(calls.get(), 1);
        assert!(after.destination.is_none());
    }
    #[test]
    fn disabled_and_stale_attempts_never_reach_adapter() {
        let calls = Rc::new(Cell::new(0));
        let mut probe = Probe {
            ready: true,
            service: Box::new(Fake {
                calls: calls.clone(),
            }),
            ..Probe::default()
        };
        probe.apply(Request::Native {
            text: "test".into(),
            token: 0,
        });
        probe.enabled = true;
        probe.apply(Request::Native {
            text: "test".into(),
            token: 999,
        });
        assert_eq!(calls.get(), 0);
    }
    #[test]
    fn changing_mode_discards_target_and_defaults_are_off() {
        let mut probe = Probe::default();
        assert!(!probe.enabled && !probe.manual_codex && !probe.armed);
        probe.ready = true;
        probe.apply(Request::ManualCodex { value: true });
        assert!(!probe.ready && !probe.armed);
        probe.apply(Request::Enable { value: false });
        assert!(!probe.enabled && !probe.ready);
    }
}
