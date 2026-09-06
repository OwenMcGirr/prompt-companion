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
    pub destination: Option<String>,
    pub message: String,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Request {
    Status,
    Enable { value: bool },
    Capture,
    Native { text: String },
    Paste { text: String },
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
fn platform() -> Box<dyn TextInsertionService> {
    #[cfg(target_os = "macos")]
    return Box::new(macos::Mac::default());
    #[cfg(windows)]
    return Box::new(windows::Windows::default());
    #[cfg(not(any(target_os = "macos", windows)))]
    compile_error!("Insertion experiment supports macOS and Windows only");
}
thread_local! {static PROBE:RefCell<(bool,Box<dyn TextInsertionService>,String)>=RefCell::new((false,platform(),"Development experiment. No destination is validated for release.".into()));}
/// Always executed on the Tauri main thread, including COM/UIA access.
pub fn execute(request: Request) -> Status {
    PROBE.with(|state|{let mut state=state.borrow_mut();match request{
    Request::Enable{value}=>{state.0=value;state.1=platform();state.2="Focus a field in TextEdit/Notepad or Chrome, then return here. Accessibility permission is required on macOS.".into();},
    Request::Capture if state.0=>{if let Err(e)=state.1.capture(){state.2=e;}},
    Request::Native{text}|Request::Paste{text} if !state.0=>{let _=text;state.2="Enable the development experiment first.".into();},
    Request::Native{text}=>state.2=state.1.insert(&text,false).unwrap_or_else(|e|e),
    Request::Paste{text}=>state.2=state.1.insert(&text,true).unwrap_or_else(|e|e),_=>{}
}Status{available:true,enabled:state.0,destination:state.1.destination(),message:state.2.clone()}})
}
