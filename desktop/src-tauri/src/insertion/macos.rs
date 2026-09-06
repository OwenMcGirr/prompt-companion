use super::{fingerprint, TextInsertionService};
use block2::RcBlock;
use objc2::{rc::Retained, runtime::AnyObject};
use objc2_app_kit::{
    NSApplicationActivationOptions, NSEvent, NSEventMask, NSPasteboard, NSRunningApplication,
};
use std::cell::{Cell, RefCell};
use std::{ffi::c_void, ptr, time::Duration};
type Ref = *const c_void;
#[repr(C)]
struct Point {
    x: f64,
    y: f64,
}
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
struct Range {
    location: isize,
    length: isize,
}
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXUIElementCreateSystemWide() -> Ref;
    fn AXUIElementCopyElementAtPosition(e: Ref, x: f32, y: f32, out: *mut Ref) -> i32;
    fn CGEventGetLocation(event: Ref) -> Point;
    fn AXUIElementCopyAttributeValue(e: Ref, key: Ref, value: *mut Ref) -> i32;
    fn AXUIElementIsAttributeSettable(e: Ref, key: Ref, value: *mut bool) -> i32;
    fn AXUIElementSetAttributeValue(e: Ref, key: Ref, value: Ref) -> i32;
    fn AXUIElementGetPid(e: Ref, pid: *mut i32) -> i32;
    fn AXValueGetValue(value: Ref, kind: u32, out: *mut c_void) -> bool;
    fn AXUIElementSetMessagingTimeout(e: Ref, seconds: f32) -> i32;
    fn CGEventCreateKeyboardEvent(source: Ref, key: u16, down: bool) -> Ref;
    fn CGEventSetFlags(event: Ref, flags: u64);
    fn CGEventPost(tap: u32, event: Ref);
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(value: Ref);
    fn CFEqual(a: Ref, b: Ref) -> bool;
    fn CFGetTypeID(value: Ref) -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFStringCreateWithBytes(
        allocator: Ref,
        bytes: *const u8,
        length: isize,
        encoding: u32,
        external: bool,
    ) -> Ref;
    fn CFStringGetLength(value: Ref) -> isize;
    fn CFStringGetCString(value: Ref, buffer: *mut u8, size: isize, encoding: u32) -> bool;
}
struct Object(Ref);
impl Drop for Object {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) }
        }
    }
}
fn string(s: &str) -> Object {
    Object(unsafe {
        CFStringCreateWithBytes(ptr::null(), s.as_ptr(), s.len() as isize, 0x08000100, false)
    })
}
fn attr(e: Ref, key: &str) -> Result<Object, String> {
    let key = string(key);
    let mut out = ptr::null();
    let err = unsafe { AXUIElementCopyAttributeValue(e, key.0, &mut out) };
    if err != 0 || out.is_null() {
        return Err("This field does not expose the attributes needed for safe insertion.".into());
    }
    Ok(Object(out))
}
fn text(v: Ref) -> Result<String, String> {
    unsafe {
        if CFGetTypeID(v) != CFStringGetTypeID() {
            return Err("Unsupported field value".into());
        }
        let mut bytes = vec![0; (CFStringGetLength(v) as usize) * 4 + 1];
        if !CFStringGetCString(v, bytes.as_mut_ptr(), bytes.len() as isize, 0x08000100) {
            return Err("Cannot decode field text".into());
        }
        bytes.truncate(bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len()));
        String::from_utf8(bytes).map_err(|_| "Cannot decode field text".into())
    }
}
fn selection(e: Ref) -> Result<Range, String> {
    let value = attr(e, "AXSelectedTextRange")?;
    let mut range = Range {
        location: 0,
        length: 0,
    };
    if !unsafe { AXValueGetValue(value.0, 4, &mut range as *mut _ as *mut c_void) }
        || range.location < 0
        || range.length < 0
    {
        return Err("Selection unavailable".into());
    }
    Ok(range)
}
fn settable(e: Ref, key: &str) -> bool {
    let key = string(key);
    let mut yes = false;
    unsafe { AXUIElementIsAttributeSettable(e, key.0, &mut yes) == 0 && yes }
}
struct Target {
    element: Object,
    window: Object,
    pid: i32,
    name: String,
    range: Range,
    hash: u64,
}
#[derive(Default)]
pub struct Mac {
    target: Option<Target>,
    manual_codex: bool,
    clicked_target: bool,
}
impl Mac {
    pub fn new(manual_codex: bool) -> Self {
        Self {
            target: None,
            manual_codex,
            clicked_target: false,
        }
    }
}
fn focused() -> Result<Object, String> {
    if !unsafe { AXIsProcessTrusted() } {
        return Err(
            "Accessibility permission is not granted. Copy Prompt remains available.".into(),
        );
    }
    let system = Object(unsafe { AXUIElementCreateSystemWide() });
    unsafe {
        AXUIElementSetMessagingTimeout(system.0, 0.4);
    }
    attr(system.0, "AXFocusedUIElement")
}
impl TextInsertionService for Mac {
    fn destination(&self) -> Option<String> {
        self.target.as_ref().map(|t| {
            format!(
                "{} · cursor {} · selected {} UTF-16 units",
                t.name, t.range.location, t.range.length
            )
        })
    }
    fn capture_clicked(&mut self, x: f64, y: f64) -> Result<(), String> {
        if !x.is_finite() || !y.is_finite() {
            return Err("Click position unavailable. Nothing was inserted.".into());
        }
        self.capture()?;
        let target = self
            .target
            .as_ref()
            .ok_or("No eligible clicked field. Nothing was inserted.")?;
        if self.manual_codex {
            let bundle = NSRunningApplication::runningApplicationWithProcessIdentifier(target.pid)
                .and_then(|app| app.bundleIdentifier())
                .map(|s| s.to_string())
                .unwrap_or_default();
            if bundle != "com.openai.codex" {
                self.target = None;
                return Err(
                    "This attempt was for Codex. The click was elsewhere; nothing was inserted."
                        .into(),
                );
            }
        }
        let system = Object(unsafe { AXUIElementCreateSystemWide() });
        let mut hit = ptr::null();
        if unsafe { AXUIElementCopyElementAtPosition(system.0, x as f32, y as f32, &mut hit) } != 0
            || hit.is_null()
        {
            self.target = None;
            return Err("Cannot identify the clicked field. Nothing was inserted.".into());
        }
        let mut hit = Object(hit);
        // A click on text inside the focused editor is valid. A click on its
        // window, toolbar, another editor, or scrollbar is never sufficient.
        for _ in 0..12 {
            if unsafe { CFEqual(hit.0, target.element.0) } {
                self.clicked_target = true;
                return Ok(());
            }
            match attr(hit.0, "AXParent") {
                Ok(parent) => hit = parent,
                Err(_) => break,
            }
        }
        self.target = None;
        Err(
            "The clicked control does not match the focused editable field. Nothing was inserted."
                .into(),
        )
    }
    fn capture(&mut self) -> Result<(), String> {
        self.clicked_target = false;
        let element = focused()?;
        let mut pid = 0;
        if unsafe { AXUIElementGetPid(element.0, &mut pid) } != 0 {
            return Err("Destination unavailable".into());
        }
        if pid == std::process::id() as i32 {
            return Ok(());
        }
        self.target = None;
        let app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
            .ok_or("Destination application closed")?;
        let bundle = app
            .bundleIdentifier()
            .map(|s| s.to_string())
            .unwrap_or_default();
        // Codex is opt-in for the person operating the development UI only.
        // Agents must not use this adapter to bypass Codex UI tool restrictions.
        let allowed = ["com.apple.TextEdit", "com.google.Chrome"].contains(&bundle.as_str())
            || (self.manual_codex && bundle == "com.openai.codex");
        if !allowed {
            return Err("This destination is unsupported. Use Copy instead.".into());
        }
        let role = text(attr(element.0, "AXRole")?.0)?;
        let subrole = attr(element.0, "AXSubrole")
            .ok()
            .and_then(|v| text(v.0).ok())
            .unwrap_or_default();
        if !["AXTextField", "AXTextArea"].contains(&role.as_str()) || subrole.contains("Secure") {
            return Err("Protected or unsupported field. Use Copy Prompt.".into());
        }
        if !settable(element.0, "AXSelectedText") && !settable(element.0, "AXValue") {
            return Err("Read-only or unsupported field.".into());
        }
        let value = text(attr(element.0, "AXValue")?.0)?;
        let range = selection(element.0)?;
        if (range.location + range.length) as usize > crate::core::utf16(&value) {
            return Err("Invalid selection".into());
        }
        let window = attr(element.0, "AXWindow")?;
        self.target = Some(Target {
            element,
            window,
            pid,
            name: app.localizedName().map(|s| s.to_string()).unwrap_or(bundle),
            range,
            hash: fingerprint(&value),
        });
        Ok(())
    }
    fn insert(&mut self, value: &str, clipboard: bool) -> Result<String, String> {
        if value.trim().is_empty() {
            return Err("The draft is empty.".into());
        }
        let t = self
            .target
            .take()
            .ok_or("Choose a fresh external field first.")?;
        if !unsafe { AXIsProcessTrusted() } {
            return Err("Accessibility permission is unavailable.".into());
        }
        let before = text(attr(t.element.0, "AXValue")?.0)?;
        let range = selection(t.element.0)?;
        if fingerprint(&before) != t.hash
            || range != t.range
            || !unsafe { CFEqual(attr(t.element.0, "AXWindow")?.0, t.window.0) }
        {
            return Err("The destination changed. Nothing was inserted; choose it again.".into());
        }
        if !clipboard && !settable(t.element.0, "AXSelectedText") {
            return Err("Native selection replacement is unsupported. No whole-field replacement was attempted.".into());
        }
        if clipboard {
            let board = NSPasteboard::generalPasteboard();
            if board.types().is_some_and(|types| {
                types.iter().any(|s| {
                    ![
                        "public.utf8-plain-text",
                        "public.utf16-external-plain-text",
                        "NSStringPboardType",
                    ]
                    .contains(&s.to_string().as_str())
                })
            }) {
                return Err(
                    "Clipboard contains unsupported formats; it was left untouched.".into(),
                );
            }
        }
        let app = NSRunningApplication::runningApplicationWithProcessIdentifier(t.pid)
            .ok_or("Destination closed")?;
        #[allow(deprecated)]
        if !self.clicked_target
            && !app.activateWithOptions(NSApplicationActivationOptions::ActivateIgnoringOtherApps)
        {
            return Err("Could not restore destination focus.".into());
        }
        for _ in 0..30 {
            if focused().is_ok_and(|f| unsafe { CFEqual(f.0, t.element.0) }) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        if !focused().is_ok_and(|f| unsafe { CFEqual(f.0, t.element.0) }) {
            return Err("Focus could not be verified. Nothing was inserted.".into());
        }
        if fingerprint(&text(attr(t.element.0, "AXValue")?.0)?) != t.hash
            || selection(t.element.0)? != t.range
        {
            return Err("The field changed during focus restoration. Nothing was inserted.".into());
        }
        if clipboard {
            arboard::Clipboard::new()
                .and_then(|mut c| c.set_text(value.to_string()))
                .map_err(|_| "Clipboard write failed; no paste attempted.")?;
            if !focused().is_ok_and(|f| unsafe { CFEqual(f.0, t.element.0) })
                || selection(t.element.0)? != t.range
                || fingerprint(&text(attr(t.element.0, "AXValue")?.0)?) != t.hash
            {
                return Err("Destination changed. Clipboard contains your draft, but no paste was attempted.".into());
            }
            unsafe {
                // Allocate both events before posting either: allocation failure
                // must never leave an unmatched key-down at the destination.
                let down = Object(CGEventCreateKeyboardEvent(ptr::null(), 9, true));
                let up = Object(CGEventCreateKeyboardEvent(ptr::null(), 9, false));
                if down.0.is_null() || up.0.is_null() {
                    return Err("Could not prepare paste. No paste was attempted.".into());
                }
                for event in [&down, &up] {
                    CGEventSetFlags(event.0, 1 << 20);
                    CGEventPost(0, event.0);
                }
            }
        } else {
            let key = string("AXSelectedText");
            let value = string(value);
            if unsafe { AXUIElementSetAttributeValue(t.element.0, key.0, value.0) } != 0 {
                return Err(
                    "Native insertion outcome uncertain. Check the destination before retrying."
                        .into(),
                );
            }
        }
        let start = crate::core::byte_offset(&before, range.location as usize);
        let end = crate::core::byte_offset(&before, (range.location + range.length) as usize);
        let expected = format!("{}{}{}", &before[..start], value, &before[end..]);
        for _ in 0..30 {
            if attr(t.element.0, "AXValue")
                .and_then(|v| text(v.0))
                .is_ok_and(|v| v == expected)
            {
                return Ok("Pasted into Codex. Your draft is kept. Nothing was sent.".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Err(
            "Insertion outcome uncertain. Draft kept; inspect the destination. No retry was made."
                .into(),
        )
    }
}

thread_local! { static CLICK_MONITOR: RefCell<Option<Retained<AnyObject>>> = const { RefCell::new(None) }; }
pub fn stop_click_monitor() {
    let monitor = CLICK_MONITOR.with(|slot| slot.borrow_mut().take());
    if let Some(monitor) = monitor {
        unsafe { NSEvent::removeMonitor(&monitor) };
    }
}
pub fn start_click_monitor(app: tauri::AppHandle, token: u64) -> Result<(), String> {
    stop_click_monitor();
    if !unsafe { AXIsProcessTrusted() } {
        return Err("Accessibility permission is not granted. Nothing was armed.".into());
    }
    let click_app = app.clone();
    let seen = Cell::new(false);
    let handler = RcBlock::new(move |event: std::ptr::NonNull<NSEvent>| {
        // AppKit delivers these callbacks on the main thread. Observe only one
        // external left mouse-up; no keys, mouse moves, or persistent logging.
        if seen.replace(true) {
            // A second external click before the queued paste invalidates the first.
            super::cancel_pending();
            return;
        }
        let cg = unsafe { event.as_ref() }.CGEvent();
        let point =
            cg.map(|event| unsafe { CGEventGetLocation(std::ptr::from_ref(&*event).cast()) });
        let app = click_app.clone();
        tauri::async_runtime::spawn(async move {
            // Let the destination finish handling the actual click/selection.
            tokio::time::sleep(Duration::from_millis(80)).await;
            let callback_app = app.clone();
            let _ = app.run_on_main_thread(move || {
                if let Some(point) = point {
                    super::clicked(&callback_app, token, point.x, point.y);
                } else {
                    super::expire_click(token);
                }
            });
        });
    });
    let monitor =
        NSEvent::addGlobalMonitorForEventsMatchingMask_handler(NSEventMask::LeftMouseUp, &handler)
            .ok_or("Could not listen for the next click. Nothing was armed.")?;
    CLICK_MONITOR.with(|slot| *slot.borrow_mut() = Some(monitor));
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(30)).await;
        let _ = app.run_on_main_thread(move || super::expire_click(token));
    });
    Ok(())
}
