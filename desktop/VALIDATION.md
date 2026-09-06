# Tauri preview validation — 6 September 2026

This is a preview alongside the working Swift app. Build success alone does not establish cross-platform live compatibility or safe global insertion.

## Verified locally on Apple Silicon

- 42 deterministic Rust tests: Unicode/UTF-16, partial words, selections, context filtering and budgets, stale results, hover freeze, clarification/expansion, Undo, task switching, copy success/failure, lifecycle detection, migration, and private atomic storage.
- Eight React interaction tests: selection action/focus, optimistic editing, IME composition, copy failure/success, clarification controls, active-task controls, long labels, and Settings.
- Both opt-in live Codex tests pass using the existing ChatGPT sign-in. Synthetic phrases included “fix the missing-user guard” and “fix the login handler crash.”
- Reviewed expansion outputs: “bigger buttons same layout” retained the vertical arrangement; “why slow” remained a question; “fix it” offered two meaningful choices and accepted one interpretation; “copy clear but no push” preserved clear-after-copy, Undo, and “Do not push the change.”
- Native Mac preview opens, migrates the selected task, connects, and generates suggestions. Real clicks verified phrase insertion restores draft focus, Copy Prompt clears the draft, and Undo restores it. Settings is readable in the current dark appearance.

## Insertion acceptance gate

| Target/method | Current result |
| --- | --- |
| Copy Prompt on macOS | Verified in the native preview; remains the supported workflow. |
| macOS native selected-text setter | Development adapter implemented; not approved for release. Requires Accessibility permission and field support. |
| macOS clipboard paste | Development adapter implemented; not approved for release. Only supported plain-text clipboard formats are eligible. |
| Windows UIA native setter | Rejected by design where no selection-replacement operation exists; no destructive whole-field fallback. |
| Windows clipboard paste | Development adapter implemented; native evaluation pending. Refuses unsupported/ambiguous fields and verifies the target. |
| Codex on either platform | No insertion approval. Agent UI tooling excludes Codex; user-assisted testing is required. |

Before enabling any destination: exercise empty and existing text, selected ranges, multiline text, emoji, rapid clicks, focus changes, closed windows, read-only/password fields, permission denial/revocation, clipboard formats, and destination Undo. Repeat each successful case and require no wrong-target writes, duplication, unrelated changes, or automatic submission. An uncertain write is never retried automatically. No universal insertion claim is made.

## Outstanding platform acceptance

Windows live Codex integration and interactive Windows app testing, Intel Mac execution, live system light/dark switching, and the insertion matrix remain subject to native validation. CI is configured for Windows x64 and universal macOS packaging, deterministic tests, frontend tests, and prototype compilation. Signing/notarization credentials have not been introduced. The Swift release remains available while acceptance is completed.
