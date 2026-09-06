# Tauri preview validation — 6 September 2026

This is a preview alongside the working Swift app. Build success alone does not establish cross-platform live compatibility or safe global insertion.

## Verified locally on Apple Silicon

- 44 deterministic Rust tests: Unicode/UTF-16, partial words, selections, context filtering and budgets, stale results, hover freeze, clarification/expansion, Undo, task switching, copy success/failure, lifecycle detection, migration, private atomic storage, edits while reconnecting, and recovery from context-read failures.
- Eight React interaction tests: selection action/focus, optimistic editing, IME composition, copy failure/success, clarification controls, active-task controls, long labels, and Settings.
- Both opt-in live Codex tests pass using the existing ChatGPT sign-in. Synthetic phrases included “fix the missing-user guard” and “fix the login handler crash.”
- Reviewed expansion outputs: “bigger buttons same layout” retained the vertical arrangement; “why slow” remained a question; “fix it” offered two meaningful choices and accepted one interpretation; “copy clear but no push” preserved clear-after-copy, Undo, and “Do not push the change.”
- Native Mac preview opens, migrates the selected task, connects, and generates suggestions. Real clicks verified phrase insertion restores draft focus, Copy Prompt clears the draft, and Undo restores it. A real expansion and single-step Undo, active-task pause, per-task draft restoration, and live light/dark switching were also checked. Settings and 32-point composer text remained readable; the original appearance and preferences were restored.

## Windows protocol smoke check

The official Codex 0.153.4 x64 executable was run under Windows 11 ARM64 x64 emulation with a fresh, isolated Codex home. `initialize` returned the Windows platform and home path, `model/list` returned 11 models, and `account/read` correctly returned no account and required authentication. The subprocess was stopped after the check. This verifies basic JSON-RPC/catalog responses only: no credentials were copied, no generation was attempted, and it is not native x64 acceptance.

## Insertion acceptance gate

| Target/method | Current result |
| --- | --- |
| Copy Prompt on macOS | Verified in the native preview; remains the supported workflow. |
| macOS native selected-text setter | Development adapter compiled. Actual permission-denied test refused capture and kept both insertion buttons disabled. No write was attempted; permission was not granted. Not approved for release. |
| macOS clipboard paste | Development adapter implemented; not approved for release. Only supported plain-text clipboard formats are eligible. |
| Windows UIA native setter | Rejected by design where no selection-replacement operation exists; no destructive whole-field fallback. |
| Windows clipboard paste | Development adapter implemented; native evaluation pending. Refuses unsupported/ambiguous fields and verifies the target. |
| Codex on either platform | No insertion approval. Agent UI tooling excludes Codex; user-assisted testing is required. |

Before enabling any destination: exercise empty and existing text, selected ranges, multiline text, emoji, rapid clicks, focus changes, closed windows, read-only/password fields, permission denial/revocation, clipboard formats, and destination Undo. Repeat each successful case and require no wrong-target writes, duplication, unrelated changes, or automatic submission. An uncertain write is never retried automatically. No universal insertion claim is made.

## Outstanding platform acceptance

Windows live Codex integration and interactive Windows app testing, Intel Mac execution, and the insertion matrix remain subject to native validation. The available local Windows VM is ARM64 and has no Codex installation/sign-in, so it cannot establish Windows 11 x64 live acceptance. CI is configured for Windows x64 and universal macOS packaging, deterministic tests, frontend tests, and prototype compilation. Signing/notarization credentials have not been introduced. The Swift release remains available while acceptance is completed.

## User-operated Codex test mode

Accessibility permission was subsequently granted by the user and recognized by the Mac development build. A session-only, explicit Codex opt-in is now available in the development UI. Capture is armed by a click, freezes at one eligible field, and each attempt consumes a Rust-owned token before calling the adapter. Queued captures or clicks cannot automatically repeat an uncertain attempt or switch methods. The UI also suppresses concurrent requests and clears its target after an IPC failure.

Three Rust controller tests and four additional frontend tests cover default-off behavior, mode changes, stale/disabled attempts, uncertain results, rapid clicks, IPC failure, and the platform gate. These use fake adapters/IPC and make no external-field writes. [Manual instructions](MANUAL_INSERTION_TESTS.md) prepare a disposable Codex trial. No successful Codex insertion or Undo result has been reported yet; permission and test-mode readiness do not constitute acceptance. The default build still excludes native insertion.
