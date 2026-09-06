use super::{fingerprint, TextInsertionService};
use std::time::Duration;
use windows::{
    core::PWSTR,
    Win32::{
        Foundation::{CloseHandle, HWND},
        System::{
            Com::{
                CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
            },
            DataExchange::{CloseClipboard, EnumClipboardFormats, OpenClipboard},
            Threading::{
                OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
        UI::{
            Accessibility::*,
            Input::KeyboardAndMouse::*,
            WindowsAndMessaging::{
                GetForegroundWindow, GetWindowThreadProcessId, IsWindow, SetForegroundWindow,
            },
        },
    },
};
struct Target {
    element: IUIAutomationElement,
    window: HWND,
    pid: u32,
    name: String,
    start: usize,
    end: usize,
    hash: u64,
}
#[derive(Default)]
pub struct Windows {
    target: Option<Target>,
    automation: Option<IUIAutomation>,
}
fn err(_: windows::core::Error) -> String {
    "The field does not expose the information needed for safe insertion.".into()
}
fn content(e: &IUIAutomationElement) -> Result<(String, usize, usize), String> {
    unsafe {
        if e.CurrentIsPassword().map_err(err)?.as_bool()
            || !e.CurrentIsEnabled().map_err(err)?.as_bool()
        {
            return Err("Protected or disabled field.".into());
        }
        let value: IUIAutomationValuePattern =
            e.GetCurrentPatternAs(UIA_ValuePatternId).map_err(err)?;
        if value.CurrentIsReadOnly().map_err(err)?.as_bool() {
            return Err("Read-only field.".into());
        }
        let text: IUIAutomationTextPattern =
            e.GetCurrentPatternAs(UIA_TextPatternId).map_err(err)?;
        let ranges = text.GetSelection().map_err(err)?;
        if ranges.Length().map_err(err)? != 1 {
            return Err("Ambiguous selection.".into());
        }
        let selected = ranges.GetElement(0).map_err(err)?;
        let document = text.DocumentRange().map_err(err)?;
        let prefix = document.Clone().map_err(err)?;
        prefix
            .MoveEndpointByRange(
                TextPatternRangeEndpoint_End,
                &selected,
                TextPatternRangeEndpoint_Start,
            )
            .map_err(err)?;
        let start = crate::core::utf16(&prefix.GetText(-1).map_err(err)?.to_string());
        let end = start + crate::core::utf16(&selected.GetText(-1).map_err(err)?.to_string());
        let text = document.GetText(-1).map_err(err)?.to_string();
        // Different text/value representations cannot be verified safely.
        if value.CurrentValue().map_err(err)?.to_string() != text || end > crate::core::utf16(&text)
        {
            return Err("Incompatible text and selection representations.".into());
        }
        Ok((text, start, end))
    }
}
impl Windows {
    fn automation(&mut self) -> Result<IUIAutomation, String> {
        if self.automation.is_none() {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                self.automation = Some(
                    CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).map_err(err)?,
                );
            }
        }
        Ok(self.automation.as_ref().unwrap().clone())
    }
}
impl TextInsertionService for Windows {
    fn destination(&self) -> Option<String> {
        self.target.as_ref().map(|t| t.name.clone())
    }
    fn capture(&mut self) -> Result<(), String> {
        unsafe {
            let ui = self.automation()?;
            let element = ui.GetFocusedElement().map_err(err)?;
            let pid = element.CurrentProcessId().map_err(err)? as u32;
            if pid == std::process::id() {
                return Ok(());
            }
            self.target = None;
            let process =
                OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).map_err(err)?;
            let mut buffer = vec![0u16; 32768];
            let mut length = buffer.len() as u32;
            let result = QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_WIN32,
                PWSTR(buffer.as_mut_ptr()),
                &mut length,
            );
            let _ = CloseHandle(process);
            result.map_err(err)?;
            let path = String::from_utf16_lossy(&buffer[..length as usize]).to_lowercase();
            let name = if path.ends_with("\\notepad.exe") {
                "Notepad"
            } else if path.ends_with("\\chrome.exe") {
                "Chrome"
            } else {
                return Err(
                    "This destination is outside the development test allowlist. Use Copy Prompt."
                        .into(),
                );
            };
            let window = GetForegroundWindow();
            let mut window_pid = 0;
            GetWindowThreadProcessId(window, Some(&mut window_pid));
            if window_pid != pid {
                return Err("Ambiguous destination window.".into());
            }
            let (text, start, end) = content(&element)?;
            self.target = Some(Target {
                element,
                window,
                pid,
                name: name.into(),
                start,
                end,
                hash: fingerprint(&text),
            });
            Ok(())
        }
    }
    fn insert(&mut self, text: &str, clipboard: bool) -> Result<String, String> {
        unsafe {
            if text.trim().is_empty() {
                return Err("The draft is empty.".into());
            }
            let t = self
                .target
                .take()
                .ok_or("Choose a fresh external field first.")?;
            if !clipboard {
                return Err("UI Automation provides no selected-text setter for this adapter. Whole-field ValuePattern replacement is intentionally not used.".into());
            }
            if !IsWindow(Some(t.window)).as_bool() {
                return Err("Destination window closed.".into());
            }
            let (before, start, end) = content(&t.element)?;
            if fingerprint(&before) != t.hash || start != t.start || end != t.end {
                return Err("Destination changed. Nothing was inserted.".into());
            }
            OpenClipboard(None).map_err(|_| "Clipboard is busy; nothing was changed.")?;
            let mut format = EnumClipboardFormats(0);
            let mut supported = true;
            while format != 0 {
                if ![1, 7, 13, 16].contains(&format) {
                    supported = false;
                    break;
                }
                format = EnumClipboardFormats(format);
            }
            let _ = CloseClipboard();
            if !supported {
                return Err(
                    "Clipboard contains unsupported formats; it was left untouched.".into(),
                );
            }
            if !SetForegroundWindow(t.window).as_bool() {
                return Err("Windows refused destination focus; no paste attempted.".into());
            }
            t.element.SetFocus().map_err(err)?;
            let ui = self.automation()?;
            let focused = ui.GetFocusedElement().map_err(err)?;
            if !ui
                .CompareElements(&focused, &t.element)
                .map_err(err)?
                .as_bool()
                || focused.CurrentProcessId().map_err(err)? as u32 != t.pid
            {
                return Err("Could not verify destination focus.".into());
            }
            let (again, s, e) = content(&t.element)?;
            if fingerprint(&again) != t.hash || s != start || e != end {
                return Err("Destination changed during focus restoration.".into());
            }
            arboard::Clipboard::new()
                .and_then(|mut c| c.set_text(text.to_string()))
                .map_err(|_| "Clipboard write failed; no paste attempted.")?;
            let key = |code, up| INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: code,
                        dwFlags: if up {
                            KEYEVENTF_KEYUP
                        } else {
                            KEYBD_EVENT_FLAGS(0)
                        },
                        ..Default::default()
                    },
                },
            };
            let inputs = [
                key(VK_CONTROL, false),
                key(VK_V, false),
                key(VK_V, true),
                key(VK_CONTROL, true),
            ];
            if SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) != inputs.len() as u32 {
                return Err("Paste outcome uncertain, possibly blocked by Windows privilege rules. No retry was made.".into());
            }
            let expected = format!(
                "{}{}{}",
                &before[..crate::core::byte_offset(&before, start)],
                text,
                &before[crate::core::byte_offset(&before, end)..]
            );
            for _ in 0..30 {
                if content(&t.element)?.0 == expected {
                    return Ok("Text verified at the destination. Draft kept. Check destination Undo before approving this adapter.".into());
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err("Paste outcome uncertain. Inspect the destination; no retry was made.".into())
        }
    }
}
