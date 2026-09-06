# Prompt Companion — Rust/Tauri preview

This preview rebuilds the composer in **Tauri 2, Rust, React and TypeScript**. The Swift application and its build remain available in the repository root while platform acceptance is completed. The preview uses a separate identity and does not replace the Swift installation.

## Platforms and prerequisites

Targets: macOS 14+ (Apple Silicon and Intel), Windows 11 x64. Windows ARM and Linux are not release targets. A successful cross-platform build is not a claim of live Codex compatibility; see [validation](VALIDATION.md).

Install a current stable Rust toolchain, Node.js 24/npm, and the native build prerequisites from [Tauri's documentation](https://v2.tauri.app/start/prerequisites/). macOS needs Xcode Command Line Tools. Windows needs Microsoft C++ Build Tools with the desktop C++ workload and WebView2.

Generation requires a locally installed Codex and an existing **ChatGPT** sign-in. No API key or account credentials belong in this project. Open Codex once before connecting so its model catalog exists. The Mac app searches the installed Codex/ChatGPT app bundles and standard CLI paths. Windows searches PATH for `codex.exe` and common native binary locations in the npm Codex package; shell `.cmd` launchers are not executed. If necessary put the directory containing the native `codex.exe` on PATH, then reopen the preview. Store-installed desktop bundles are not assumed to have a stable filesystem location.

## Build, run and test

From this directory (`desktop`):

```sh
npm ci
npm run tauri -- dev
```

Production frontend and deterministic checks:

```sh
npm run build
npm test
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --features insertion-prototype --locked -- -D warnings
```

Package on macOS:

```sh
npm run tauri -- build --bundles app
```

For both Mac architectures, install both Rust targets and build universally:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run tauri -- build --target universal-apple-darwin --bundles app
```

Package on Windows x64:

```sh
npm run tauri -- build --bundles nsis
```

Bundles appear beneath `src-tauri/target/release/bundle`, or `src-tauri/target/universal-apple-darwin/release/bundle` for a universal build. Open **Prompt Companion Preview.app** on macOS, or install the generated NSIS `.exe` on Windows. These are unsigned previews, not notarized/Developer-ID or Authenticode releases. CI builds and uploads artifacts without publishing a release or using signing credentials.

The two live tests are ignored by default and consume your Codex allowance. On macOS:

```sh
PROMPT_COMPANION_LIVE_TEST=1 cargo test --manifest-path src-tauri/Cargo.toml --test live -- --ignored --nocapture
```

On Windows PowerShell:

```powershell
$env:PROMPT_COMPANION_LIVE_TEST = '1'
cargo test --manifest-path src-tauri/Cargo.toml --test live -- --ignored --nocapture
Remove-Item Env:PROMPT_COMPANION_LIVE_TEST
```

Optionally set `PROMPT_COMPANION_TEST_TASK` to an existing task ID for a read-only context/activity check. Generation tests use synthetic conversation data. Review printed wording as well as assertions.

## Use the composer

1. Choose a local Codex task using the context button. Search and Load more are available.
2. Type in the draft or left-click a suggested phrase. Text is inserted at the cursor or replaces the selection; focus returns to the draft. Hovering over the phrase area keeps waiting results from appearing beneath your pointer.
3. Click **Expand** for a fuller prompt based on the selected conversation. Resolve one clarification if offered, or click **Keep original**. Read the result before using it.
4. Click **Copy Prompt**, then paste into Codex yourself. Successful copying clears the draft; failure preserves it. Nothing is submitted automatically.
5. Use **Undo** to restore a phrase edit, expansion, Clear, or the clearing after copying. Settings adjusts text size, phrase-button height, automatic suggestions, and always-on-top behavior. Both Settings and the composer follow system appearance.

When the selected task is active, generation pauses. Pending requests are cancelled and stale results are ignored. Activity is polled approximately every two seconds and checked before generation; changes are not instantaneous. The backend checks task/turn metadata plus local lifecycle markers because a separate app-server may label another process's unfinished turn as interrupted. An unclosed task-start marker after a crash can conservatively leave generation paused. Completing/stopping a subsequent turn records a terminal marker. If activity cannot be read, the composer pauses generation and keeps the draft editable.

The UI can wrap long phrases to additional lines rather than hiding text behind ellipses. At larger text sizes or smaller windows it scrolls to keep all controls accessible. The small session activity counters are unchanged in scope; persistent usage statistics are not part of this rewrite.

## Storage and migration

The preview identifier is `com.owenmcgirr.prompt-companion.preview`. Its `state.json` and private model-catalog copy are stored in the OS application-data directory for that identifier (on macOS, `~/Library/Application Support/com.owenmcgirr.prompt-companion.preview`; on Windows, the corresponding AppData/Roaming directory).

On the first Mac launch only, existing Swift drafts/preferences are imported from `~/Library/Application Support/PromptCompanion/drafts.json`. A `swift-drafts-backup.json` copy is retained in the preview directory. The original file is never overwritten. Cursor offsets are validated during import. Later preview launches use only preview state. Unreadable/future-version preview storage is left untouched and an error is displayed; editing/copying remain available, but saving stays disabled to avoid destroying that file.

Writes use temporary files and atomic replacement. Mac directories/files are restricted to the owner; Windows files inherit the user-profile directory's ACL. Only one preview instance runs per user. On exit, the backend saves state and stops its Codex processes. Drafts are local, not encrypted by this application. Prompt text is not logged by the app; Codex's own retention behavior still applies.

The selected conversation excerpts and draft go to OpenAI through the existing Codex account. Tool output, hidden reasoning, and attachments are excluded. Long context retains opening requirements and recent turns. Generation uses isolated ephemeral transports with executable features/MCP servers disabled, a read-only sandbox, a private tool-restricted model catalog, and rejection of interactive server requests. No original Codex configuration is edited.

## Direct insertion experiment

Direct insertion is **excluded from the default build**. Copy Prompt is the supported fallback. To build the isolated development prototype:

```sh
npm run tauri -- dev --features insertion-prototype
```

The development panel starts disabled. Enable insertion testing, click **Capture next field**, focus a disposable text field in TextEdit/Notepad or Chrome, and return to the composer. Capture freezes after one eligible field and each attempt consumes its target. It displays the remembered destination and explicit native/clipboard insertion buttons. It does not log global keystrokes or provide external text to the model.

macOS uses Accessibility selected-text replacement where the field advertises support. Windows UI Automation has no universal selected-text setter; this prototype explicitly refuses whole-field ValuePattern replacement and evaluates clipboard paste separately. The clipboard path accepts only supported plain-text formats, explains that the clipboard will be replaced, and does not restore it with a timer. It verifies field identity, selection and content before inserting, verifies the resulting text, consumes the target after an attempt, never presses Enter, never retries an uncertain result, and keeps the draft.

No destination adapter has been approved for release. Codex is excluded by default. On macOS, a separate **Include Codex — I will operate the test myself** checkbox permits a session-only manual trial. The person testing must operate and inspect Codex themselves; agents must not use this adapter to bypass Codex automation-tool restrictions. See [the manual test guide](MANUAL_INSERTION_TESTS.md). Missing accessibility permission, incompatible fields, ambiguous selection, or unverified focus leaves direct insertion unavailable. See [the measured acceptance matrix](VALIDATION.md).

## Architecture

Rust owns revisions, drafts, Undo, task/context state and generation orchestration. React sends typed actions through restricted Tauri IPC and renders snapshots. Acknowledgements prevent older snapshots from overwriting newer local typing; IME composition is not sent until committed. UTF-16 offsets cross IPC explicitly and are converted to valid Rust string boundaries.

`src-tauri/tests/parity.rs` ports the Swift editing/model cases and tests storage/activity behavior. `src/App.test.tsx` checks the frontend boundary and interactions; it mocks IPC, not the Rust algorithms. Keep real clicks and live model checks in the acceptance process.

[MIT License](../LICENSE).
