# Validation status — 6 September 2026

Prompt Companion targets macOS 14+ on Apple Silicon and Intel, and Windows 11 x64. Build success alone does not establish live Codex compatibility or universal insertion support.

## Local Apple Silicon checks

- Deterministic Rust coverage includes UTF-16 editing, Unicode and selections, context limits, stale-result rejection, activity pausing, phrase generation, expansion and clarification, Undo, task isolation, atomic private storage, copy failure, paste completion, and process cleanup.
- React interaction coverage includes optimistic editing, IME composition, left-click selection, draft focus, keyboard navigation, mouse-responsive refreshing, clarification, active-task controls, large labels, Settings, and paste-action state.
- Frontend build, Rust formatting, Rust tests, and feature-enabled Clippy have passed locally. The macOS application bundle builds and opens.
- Live generation has been checked with synthetic context through an existing Codex ChatGPT session. Generated wording still requires human review.
- The composer has been inspected in light and dark appearance with large text and button settings.

## Platform status

The CI workflow runs frontend checks, Rust tests, formatting, Clippy, universal macOS packaging, and a Windows x64 installer build. macOS Intel execution and signed/notarized distribution have not been validated. Windows protocol startup has been smoke-tested, but live generation and interactive Windows acceptance still require a signed-in native Windows 11 x64 environment.

## Codex paste status

The user reported a successful **Paste on next field click** trial in the Codex draft. The app validates the clicked editable field, selection, surrounding text, focus, draft revision, and selected task; every arm is single-use and expires after 30 seconds. It never presses Enter or retries automatically.

Native Accessibility selected-text replacement returned success without visible text and is not exposed in the normal interface. Clipboard paste is the macOS action. Codex can accept a paste while its accessibility value fails post-write verification, so such attempts remain “uncertain” and retain the companion draft. A verified paste clears only the unchanged matching draft; Undo restores it.

Repeated trials are still required for empty/existing text, selections, multiline text, emoji, rapid clicks, focus changes, closed windows, protected/read-only fields, permission denial/revocation, clipboard formats, and destination Undo. Acceptance requires zero wrong-target writes, duplicates, surrounding-text changes, or submissions.

Windows retains Copy Prompt because its insertion workflow has not completed live validation. Development-only adapters remain feature-gated for diagnostic work.
