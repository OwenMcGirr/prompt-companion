# Prompt Companion

Prompt Companion reduces the amount of typing needed to write useful Codex prompts. Select a Codex task for context, type a few words, then use phrase suggestions or expand the shorthand into an editable prompt.

The interface uses large controls and works with ordinary left clicks. It also supports arrow-key suggestion navigation. Speech is not required.

Prompt Companion is an independent, early-stage community project built with Tauri 2, Rust, React, and TypeScript. It targets macOS 14+ on Apple Silicon and Intel, plus Windows 11 x64. Linux and Windows ARM are not currently supported.

[Setup](#setup) · [Build and run](#build-and-run) · [Usage](#usage) · [Tests](#tests) · [Privacy](#context-privacy-and-storage) · [Contributing](CONTRIBUTING.md)

## Requirements

- A current stable Rust toolchain.
- Node.js 24 and npm.
- The [native Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform. macOS needs Xcode Command Line Tools. Windows needs Microsoft C++ Build Tools with the desktop C++ workload and WebView2.
- Codex installed and signed in with ChatGPT, with available usage and at least one local task. Prompt Companion does not accept API-key authentication.
- Internet access for phrase generation and prompt expansion.

The Mac app searches installed Codex/ChatGPT app bundles and standard CLI paths. Windows searches `PATH` and common native locations for `codex.exe`; it does not execute shell `.cmd` launchers. Open Codex once before connecting so its local model catalog exists.

## Setup

```sh
git clone https://github.com/OwenMcGirr/prompt-companion.git
cd prompt-companion/desktop
npm ci
```

Prompt Companion uses the existing Codex ChatGPT session. Do not copy account credentials into this repository.

## Build and run

Start a development build from `desktop`:

```sh
npm run tauri -- dev
```

Build a macOS application bundle:

```sh
npm run tauri -- build --bundles app
```

For a universal Mac bundle:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run tauri -- build --target universal-apple-darwin --bundles app
```

Build a Windows x64 installer from Windows:

```powershell
npm run tauri -- build --bundles nsis
```

Bundles are written below `desktop/src-tauri/target/release/bundle`, or the corresponding target directory for universal builds. Current local artifacts are unsigned and are not notarized or Authenticode-signed. CI builds both platforms without publishing a release.

## Usage

1. Select a Codex task with the context button. This is the conversation used to generate relevant wording.
2. Type in the draft or left-click a suggested phrase. A phrase inserts at the cursor or replaces the selection, then focus returns to the draft.
3. Click **Expand** to turn shorthand into a fuller prompt. Review the result, resolve one clarification if offered, or use **Keep original**.
4. On macOS, click **Paste on next field click**, then click the Codex draft within 30 seconds. Prompt Companion never presses Enter or sends the prompt. A confirmed paste clears the unchanged companion draft; Undo restores it. Failed or uncertain pastes keep it.
5. On Windows, use **Copy Prompt** and paste into Codex yourself. On macOS, the same fallback is under **Copy instead**. Successful copying clears the draft and Undo restores it.

Drafts are saved separately for each selected task. Settings controls text size, suggestion-button height, automatic suggestions, and whether the window stays above other apps. Both appearances follow the system theme.

When the selected task is active, phrase generation and expansion pause. The draft remains editable. Pending generation is cancelled, stale results are rejected, and suggestions resume after the task becomes inactive when automatic suggestions are enabled.

Mouse movement does not pause suggestion refreshing. To use the keyboard, press Down at the end of the draft or Up at its start, then use the arrow keys to move between enabled suggestions. Enter accepts the highlighted phrase or clarification choice; Escape returns to the draft.

### Paste on next field click

The macOS paste action requires Accessibility permission in **System Settings → Privacy & Security → Accessibility**. It listens for one external left click only while armed, verifies that the clicked control is the Codex editable field, and performs one clipboard paste. It expires after 30 seconds and never retries automatically.

The clipboard must contain only supported plain-text formats; unsupported formats are left unchanged. Prompt Companion checks the field, selection, surrounding text, and focus before pasting. It clears the companion draft only after the resulting destination text is verified and the original draft, revision, and selected task still match. If verification is uncertain, inspect Codex before trying again.

Unsigned Mac rebuilds can invalidate Accessibility authorization even if an old Settings toggle remains visible. Re-authorize the exact rebuilt bundle when this happens.

## Context, privacy, and storage

- Selected conversation excerpts and the draft go to OpenAI through the existing Codex account for generation.
- Prompt Companion reads task metadata and history through `codex app-server`; it does not resume or write to the selected task.
- Only user and assistant text enters generation context. Tool output, hidden reasoning, and attachment contents are excluded.
- Each generation uses an isolated temporary transport with executable features and MCP servers disabled, a read-only sandbox, and a private tool-restricted model catalog. Original Codex configuration is not edited.
- Prompt text is not logged. The app does not capture the screen or monitor global keystrokes.
- Drafts and settings use the platform application-data directory for `com.owenmcgirr.prompt-companion.preview`. Writes are atomic; Mac files are owner-restricted and Windows files inherit the user profile ACL. Data is local but is not encrypted by the application.

## Tests

Run the deterministic checks from `desktop`:

```sh
npm run build
npm test
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --features insertion-prototype --locked -- -D warnings
```

The two live tests are ignored by default. They use the installed Codex account and consume allowance:

```sh
PROMPT_COMPANION_LIVE_TEST=1 cargo test --manifest-path src-tauri/Cargo.toml --test live -- --ignored --nocapture
```

On Windows PowerShell, set and later remove `PROMPT_COMPANION_LIVE_TEST` through `$env:`. Optionally set `PROMPT_COMPANION_TEST_TASK` to an existing task ID for a read-only context/activity check. Generation tests use synthetic conversation data; review their wording as well as assertions.

See [desktop/VALIDATION.md](desktop/VALIDATION.md) for measured platform and interaction coverage and [desktop/MANUAL_INSERTION_TESTS.md](desktop/MANUAL_INSERTION_TESTS.md) for the user-operated insertion matrix.

## Troubleshooting

| Problem | What to try |
| --- | --- |
| Codex cannot be found | Install Codex in a standard location or put its native executable on `PATH`, then reopen Prompt Companion. |
| ChatGPT sign-in is required | Sign in through Codex, then click **Reconnect**. |
| Model catalog unavailable | Open Codex once, update it if needed, then reconnect. |
| Wrong context | Open the task picker and explicitly select the intended local task. |
| Suggestions are paused | Check whether the selected task is active and whether automatic suggestions are enabled. |
| Paste cannot arm on macOS | Grant Accessibility permission to the exact running application bundle. |
| Paste outcome is uncertain | Inspect the Codex draft before taking another action; Prompt Companion intentionally does not retry. |

Report reproducible problems through [GitHub Issues](https://github.com/OwenMcGirr/prompt-companion/issues). Include platform, architecture, Codex version, steps, expected behavior, and actual behavior. Use synthetic examples and remove account details, private task IDs, drafts, and conversation history.

## Project layout

- `desktop/src`: React and TypeScript interface and interaction tests.
- `desktop/src-tauri/src`: Rust state, persistence, generation, activity detection, and platform integration.
- `desktop/src-tauri/tests`: deterministic parity and opt-in live tests.
- `.github/workflows/tauri.yml`: macOS and Windows build, test, lint, and packaging jobs.

## License

Licensed under the [MIT License](LICENSE). Copyright © 2026 Owen McGirr.
