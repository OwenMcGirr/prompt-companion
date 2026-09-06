//! Development-only native experiment. No adapter has passed Codex acceptance.
//! No keystroke logging, screen capture, telemetry, automatic sending, or retries.
//! macOS observes one external left-click only while explicitly armed.
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    hash::{Hash, Hasher},
    time::{Duration, Instant},
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
    pub click_armed: bool,
    pub click_available: bool,
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
    ArmClick { text: String, clipboard: bool },
    Cancel,
    Capture,
    Native { text: String, token: u64 },
    Paste { text: String, token: u64 },
}
pub trait TextInsertionService {
    fn capture(&mut self) -> Result<(), String>;
    fn capture_clicked(&mut self, _x: f64, _y: f64) -> Result<(), String> {
        Err("Click insertion is unsupported on this platform.".into())
    }
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
struct PendingClick {
    text: String,
    clipboard: bool,
    deadline: Instant,
}
struct Probe {
    enabled: bool,
    armed: bool,
    pending_click: Option<PendingClick>,
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
            pending_click: None,
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
        self.pending_click = None;
        self.ready = false;
        self.token += 1;
        self.service = platform(self.manual_codex);
    }
    fn apply(&mut self, request: Request) -> Status {
        if self
            .pending_click
            .as_ref()
            .is_some_and(|p| Instant::now() >= p.deadline)
        {
            self.reset();
            self.message = "Click insertion expired. Nothing was inserted.".into();
        }
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
            Request::Cancel => {
                self.reset();
                self.message = "Insertion cancelled. Nothing was inserted.".into();
            }
            Request::ArmClick { text, clipboard } if self.enabled && cfg!(target_os = "macos") => {
                self.reset();
                if text.trim().is_empty() {
                    self.message = "The draft is empty. Nothing was armed.".into();
                } else {
                    self.pending_click = Some(PendingClick {
                        text,
                        clipboard,
                        deadline: Instant::now() + Duration::from_secs(30),
                    });
                    self.message = if self.manual_codex {
                        "Click the Codex draft field within 30 seconds. One insertion attempt will follow that click; there is no need to return here."
                    } else {
                        "Click a disposable TextEdit or Chrome field within 30 seconds. One insertion attempt will follow that click."
                    }.into();
                }
            }
            Request::Capture if self.enabled && self.armed && self.pending_click.is_none() => {
                match self.service.capture() {
                    Ok(()) if self.service.destination().is_some() => {
                        self.ready = true;
                        self.armed = false;
                        self.message = "Target captured and frozen. Check the destination below. Click one insertion method once.".into();
                    }
                    Ok(()) => {}
                    Err(error) => self.message = error,
                }
            }
            Request::Native { text, token } => self.insert(&text, token, false),
            Request::Paste { text, token } => self.insert(&text, token, true),
            _ => {}
        }
        Status {
            available: true,
            enabled: self.enabled,
            armed: self.armed,
            click_armed: self.pending_click.is_some(),
            click_available: cfg!(target_os = "macos"),
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
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    fn clicked(&mut self, token: u64, x: f64, y: f64, current_draft: &str) {
        if token != self.token || !self.enabled {
            return;
        }
        let Some(pending) = self.pending_click.take() else {
            return;
        };
        self.armed = false;
        self.ready = false;
        self.token += 1;
        if Instant::now() >= pending.deadline || pending.text != current_draft {
            self.message =
                "Click insertion cancelled: the draft changed or the request expired.".into();
            return;
        }
        // This is a single attempt. Any capture/validation error consumes it too.
        self.message = match self.service.capture_clicked(x, y) {
            Ok(()) if self.service.destination().is_some() => self
                .service
                .insert(&pending.text, pending.clipboard)
                .unwrap_or_else(|e| e),
            Ok(()) => "The click did not identify an eligible field. Nothing was inserted.".into(),
            Err(e) => e,
        };
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
pub fn execute(request: Request, app: tauri::AppHandle) -> Status {
    let start_click = matches!(request, Request::ArmClick { .. });
    let status = PROBE.with(|state| state.borrow_mut().apply(request));
    #[cfg(target_os = "macos")]
    {
        if !status.click_armed {
            macos::stop_click_monitor();
        }
        if start_click && status.click_armed {
            if let Err(error) = macos::start_click_monitor(app, status.token) {
                return PROBE.with(|state| {
                    let mut state = state.borrow_mut();
                    state.reset();
                    state.message = error;
                    state.apply(Request::Status)
                });
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, start_click);
    status
}
#[cfg(target_os = "macos")]
fn clicked(app: &tauri::AppHandle, token: u64, x: f64, y: f64) {
    use tauri::Manager;
    let draft = app
        .state::<crate::runtime::Service>()
        .view
        .lock()
        .ok()
        .map(|v| v.draft.text.clone());
    PROBE.with(|state| {
        let mut state = state.borrow_mut();
        if token != state.token {
            return;
        }
        macos::stop_click_monitor();
        if let Some(draft) = draft {
            state.clicked(token, x, y, &draft);
        } else {
            state.reset();
            state.message = "Draft unavailable; no insertion attempted.".into();
        }
    });
}
#[cfg(target_os = "macos")]
fn expire_click(token: u64) {
    PROBE.with(|state| {
        let mut state = state.borrow_mut();
        if state.token == token && state.pending_click.is_some() {
            state.reset();
            state.message = "Click insertion expired. Nothing was inserted.".into();
            macos::stop_click_monitor();
        }
    });
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
        fn capture_clicked(&mut self, _: f64, _: f64) -> Result<(), String> {
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
    fn click_probe(calls: Rc<Cell<usize>>) -> Probe {
        Probe {
            enabled: true,
            pending_click: Some(PendingClick {
                text: "TEST ".into(),
                clipboard: false,
                deadline: Instant::now() + Duration::from_secs(30),
            }),
            service: Box::new(Fake { calls }),
            ..Probe::default()
        }
    }
    #[test]
    fn click_inserts_once_without_another_request() {
        let calls = Rc::new(Cell::new(0));
        let mut probe = click_probe(calls.clone());
        probe.clicked(0, 10., 20., "TEST ");
        probe.clicked(0, 10., 20., "TEST ");
        assert_eq!(calls.get(), 1);
        assert!(probe.pending_click.is_none());
        assert!(probe.message.contains("Uncertain"));
    }
    #[test]
    fn changed_draft_cancel_and_expiry_prevent_click_insertion() {
        let calls = Rc::new(Cell::new(0));
        let mut probe = click_probe(calls.clone());
        probe.clicked(0, 10., 20., "changed");
        let mut probe = click_probe(calls.clone());
        probe.apply(Request::Cancel);
        probe.clicked(0, 10., 20., "TEST ");
        let mut probe = click_probe(calls.clone());
        probe.pending_click.as_mut().unwrap().deadline = Instant::now() - Duration::from_secs(1);
        probe.clicked(0, 10., 20., "TEST ");
        assert_eq!(calls.get(), 0);
    }
    #[test]
    fn background_capture_cannot_trigger_click_insertion() {
        let calls = Rc::new(Cell::new(0));
        let mut probe = click_probe(calls.clone());
        probe.apply(Request::Capture);
        probe.apply(Request::Status);
        assert_eq!(calls.get(), 0);
        assert!(probe.pending_click.is_some());
        assert!(!probe.ready);
    }
    #[test]
    fn rejected_clicked_control_consumes_attempt_without_writing() {
        struct Reject(Fake);
        impl TextInsertionService for Reject {
            fn capture(&mut self) -> Result<(), String> {
                Ok(())
            }
            fn destination(&self) -> Option<String> {
                self.0.destination()
            }
            fn insert(&mut self, text: &str, clipboard: bool) -> Result<String, String> {
                self.0.insert(text, clipboard)
            }
        }
        let calls = Rc::new(Cell::new(0));
        let mut probe = click_probe(calls.clone());
        probe.service = Box::new(Reject(Fake {
            calls: calls.clone(),
        }));
        probe.clicked(0, 10., 20., "TEST ");
        assert_eq!(calls.get(), 0);
        assert!(probe.pending_click.is_none());
    }
}
