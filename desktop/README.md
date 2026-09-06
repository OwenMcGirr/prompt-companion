# Prompt Companion — Rust/Tauri application

Prompt Companion is built with **Tauri 2, Rust, React and TypeScript**. This directory contains the complete application implementation.

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
2. Type in the draft or left-click a suggested phrase. Text is inserted at the cursor or replaces the selection; focus returns to the draft. Mouse movement and hovering do not pause suggestion refreshing.
3. Click **Expand** for a fuller prompt based on the selected conversation. Resolve one clarification if offered, or click **Keep original**. Read the result before using it.
4. On macOS, click **Paste on next field click**, then click the Codex draft within 30 seconds. Your companion draft clears only after the destination text is verified. Failed or uncertain pastes keep it. Undo restores a cleared draft. **Cancel waiting paste** cancels it. On Windows, use **Copy Prompt**, then paste yourself. On macOS, **Copy instead** provides the same fallback; successful copying clears the draft and Undo restores it. Nothing is submitted automatically.
5. Use **Undo** to restore a phrase edit, expansion, Clear, or the clearing after copying. Settings adjusts text size, phrase-button height, automatic suggestions, and always-on-top behavior. Both Settings and the composer follow system appearance.

When the selected task is active, generation pauses. Pending requests are cancelled and stale results are ignored. Activity is polled approximately every two seconds and checked before generation; changes are not instantaneous. The backend checks task/turn metadata plus local lifecycle markers because a separate app-server may label another process's unfinished turn as interrupted. An unclosed task-start marker after a crash can conservatively leave generation paused. Completing/stopping a subsequent turn records a terminal marker. If activity cannot be read, the composer pauses generation and keeps the draft editable.

The UI can wrap long phrases to additional lines rather than hiding text behind ellipses. At larger text sizes or smaller windows it scrolls to keep all controls accessible. The small session activity counters are unchanged in scope; persistent usage statistics are not part of this rewrite.

## Storage and migration

The preview identifier is `com.owenmcgirr.prompt-companion.preview`. Its `state.json` and private model-catalog copy are stored in the OS application-data directory for that identifier (on macOS, `~/Library/Application Support/com.owenmcgirr.prompt-companion.preview`; on Windows, the corresponding AppData/Roaming directory).

If `state.json` is unreadable or uses an unsupported future version, it is left untouched and the app displays an error. Editing and copying remain available in memory, while saving stays disabled to avoid destroying the file.

Writes use temporary files and atomic replacement. Mac directories/files are restricted to the owner; Windows files inherit the user-profile directory's ACL. Only one preview instance runs per user. On exit, the backend saves state and stops its Codex processes. Drafts are local, not encrypted by this application. Prompt text is not logged by the app; Codex's own retention behavior still applies.

The selected conversation excerpts and draft go to OpenAI through the existing Codex account. Tool output, hidden reasoning, and attachments are excluded. Long context retains opening requirements and recent turns. Generation uses isolated ephemeral transports with executable features/MCP servers disabled, a read-only sandbox, a private tool-restricted model catalog, and rejection of interactive server requests. No original Codex configuration is edited.

## Keyboard navigation

Press Down at the end of the draft (or Up at its start) to focus a suggestion. Use any arrow key to move between enabled suggestions, wrapping at either end. Enter accepts the highlighted phrase or clarification choice and returns focus to the draft. Escape returns without accepting. Suggestions stay stable while keyboard-focused. Mouse hovering and movement do not pause refreshing. Modified arrows, IME composition, and vertical movement within multiline draft text retain normal editing behavior. Left-click selection remains available.

## Paste into Codex on macOS

**Paste on next field click** is included in normal macOS builds. No development switches are required. Grant the running app Accessibility permission in **System Settings → Privacy & Security → Accessibility**. It observes one external left click only while armed, validates the clicked editable field, then performs one clipboard paste. It never presses Enter. Native selected-text insertion is no longer offered in the normal interface.

The destination must be Codex. A click elsewhere, missing permission, an unsupported/protected field, a changed draft or task, or a 30-second timeout prevents pasting. Editing, clearing, copying, expanding, or changing tasks cancels a waiting paste. A second external click before the attempt cancels it. The app checks the destination's field, selection, surrounding text and focus before pasting, and checks the resulting text afterward. It never retries automatically, including when verification is uncertain. Inspect an uncertain result before trying again.

Paste replaces plain-text clipboard contents with your draft. Unsupported clipboard formats are left untouched; no timed clipboard restoration is used. A confirmed paste clears and saves the unchanged companion draft. A newer edit or task switch prevents clearing. Failed or uncertain pastes keep the draft. The destination's Undo operates on the paste; Prompt Companion's Undo operates on its own draft.

A user-operated Codex clipboard trial succeeded; native Accessibility replacement returned an uncertain result and the user reported no visible change. Repeated trials, destination Undo, multiline and emoji selections, and permission-revocation checks remain to be completed. This is not a claim of universal insertion reliability. Windows retains Copy Prompt until its paste workflow has live validation. See [validation status](VALIDATION.md).

Unsigned macOS builds can lose Accessibility authorization after rebuilding even if the Settings toggle remains on. Re-authorize the exact rebuilt bundle. Stable production signing remains an outstanding distribution task; no signing credentials are bundled.

Additional native adapter code and controller tests remain behind `insertion-prototype` on Windows and in source for diagnosis. The normal interface exposes only the macOS clipboard workflow. Agents must not operate Codex through this adapter to bypass automation-tool restrictions; Codex verification is user-operated.
