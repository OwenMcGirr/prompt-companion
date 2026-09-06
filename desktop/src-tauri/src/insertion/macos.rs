use super::{fingerprint, TextInsertionService};
use objc2_app_kit::{NSApplicationActivationOptions, NSPasteboard, NSRunningApplication};
use std::{ffi::c_void, ptr, time::Duration};
type Ref = *const c_void;
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
        self.target.as_ref().map(|t| t.name.clone())
    }
    fn capture(&mut self) -> Result<(), String> {
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
        // Restrict agent-driven evaluation to disposable editors. Codex requires
        // user-assisted validation; do not bypass the automation tool's exclusion.
        if !["com.apple.TextEdit", "com.google.Chrome"].contains(&bundle.as_str()) {
            return Err(
                "This destination is outside the development test allowlist. Use Copy Prompt."
                    .into(),
            );
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
        if !app.activateWithOptions(NSApplicationActivationOptions::ActivateIgnoringOtherApps) {
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
            unsafe {
                for down in [true, false] {
                    let event = Object(CGEventCreateKeyboardEvent(ptr::null(), 9, down));
                    if event.0.is_null() {
                        return Err("Paste outcome uncertain. Do not retry automatically.".into());
                    }
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
            if text(attr(t.element.0, "AXValue")?.0)? == expected {
                return Ok("Text verified at the destination. Draft kept. Check destination Undo before approving this adapter.".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Err(
            "Insertion outcome uncertain. Draft kept; inspect the destination. No retry was made."
                .into(),
        )
    }
}
