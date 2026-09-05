# Prompt Companion

A native macOS app that helps you write Codex prompts with less typing. Choose a task, type a few words, and click useful phrase suggestions—or expand shorthand into a fuller, editable prompt using that task’s conversation.

Designed around large buttons and ordinary left clicks, including for people who use head-controlled pointing. Speech is not required.

**Status:** early-stage community project. This is an independent companion app, not an official OpenAI product. It reads selected task context and helps compose text; you review, copy, and paste into Codex yourself. Nothing is sent to a Codex task automatically.

[Setup](#setup) · [Build and run](#build-and-run) · [Usage](#usage) · [Tests](#tests) · [Troubleshooting](#troubleshooting) · [Contributing](CONTRIBUTING.md)

## Requirements

- **macOS 14 or newer.** The interface uses SwiftUI and AppKit; Windows and Linux are not supported.
- **Swift 6 or newer** and the macOS SDK, provided by a compatible Xcode or Xcode Command Line Tools installation.
- **Codex installed and signed in with ChatGPT**, with available usage allowance and at least one local task. API-key authentication is not supported by this app.
- Internet access for phrase generation and expansion. Building and deterministic tests do not need a Codex account.

Development has been checked on Apple Silicon with Swift 6.3.3 and Codex 0.153.1. Other versions and Intel Macs are not yet verified. The build targets the current Mac’s architecture; it does not produce a universal binary.

## Setup

1. Install Apple’s command-line developer tools if needed:

   ```sh
   xcode-select --install
   ```

   Complete the installer, then check `swift --version`. If it is older than Swift 6, update your developer tools. If using full Xcode, open it once to complete setup.

2. Install and open Codex, sign in using **ChatGPT**, and create or open a task. CLI users can follow the [official Codex CLI setup instructions](https://learn.chatgpt.com/docs/codex/cli). Open Codex once before starting Prompt Companion so its local model catalog is available.

3. Clone the repository:

   ```sh
   git clone https://github.com/OwenMcGirr/prompt-companion.git
   cd prompt-companion
   ```

No third-party Swift package dependencies or new API keys are required. Prompt Companion uses your existing Codex sign-in; do not copy credentials into this repository.

## Build and run

Run these commands from the repository root:

```sh
./build.sh "$PWD/dist"
open "dist/Prompt Companion.app"
```

Alternatively, open `dist` in Finder and double-click **Prompt Companion.app**. You can copy the bundle to your Applications folder. Quit any running copy before rebuilding or replacing it.

The script makes a release build, includes the icon and app metadata, and applies a local ad-hoc signature. It does **not** notarize the app or sign it with a Developer ID for distribution. This repository’s documented installation path is building from source.

Without an output argument, `./build.sh` places the bundle in the repository’s parent directory. Set `PROMPT_COMPANION_BUILD_DIR` to change the Swift build cache location:

```sh
PROMPT_COMPANION_BUILD_DIR="$PWD/.build-release" ./build.sh "$PWD/dist"
```

For a quick development build and run without packaging:

```sh
swift run PromptCompanion
```

Use the packaged app when checking the icon, window behavior, and Finder launch. Open `Package.swift` in Xcode if you prefer an IDE.

## Usage

1. **Choose a task.** Click the context button at the top and select the Codex conversation you want to use. The app keeps this selection until you change it; it does not detect the task currently visible in Codex.
2. **Write your prompt.** Type into the draft, or click one of the three large phrase buttons. A phrase is inserted at the cursor or replaces the selection, and focus returns to the draft so you can keep typing.
3. **Expand shorthand when helpful.** Click **Expand** to rewrite the entire draft into a fuller prompt using the selected conversation. Review and edit the result.
4. **Copy and use it.** Click **Copy Prompt**, then paste into Codex with your usual paste method. A successful copy clears the draft and returns focus to it. A failed copy keeps the text. You decide when to submit in Codex.

**Undo** reverses the last edit, including a phrase insertion, expansion, Clear, or clearing after Copy Prompt. Undoing a copy restores the draft without changing the clipboard. Drafts are saved separately for each task and restored after reopening the app.

Suggestions stay stable while the pointer is over the phrase area. Move the pointer out to reveal waiting suggestions. Results based on an older draft become unclickable immediately. Use **Refresh phrases** to request another set.

### Expand shorthand

Examples to try in a relevant task:

| Short draft | Intended behavior |
| --- | --- |
| `bigger buttons same layout` | Ask for larger buttons while preserving the established layout. |
| `why slow` | Ask for an explanation of the relevant delay; keep it a question. |
| `fix it` | Offer clickable interpretations if the conversation leaves more than one meaningful issue unresolved. |
| `copy clear but no push` | Preserve the requested copy-and-clear behavior and the instruction not to push. |

If clarification is needed, choose one of 2–3 interpretations or click **Keep original**. There is at most one clarification round. Expansion pauses phrase predictions. Editing, changing tasks, clearing, copying, or a conversation update cancels it; failures keep the original draft. A successful replacement is one undoable edit.

The generation instructions preserve intent, limits, uncertainty, and questions, and use details already established in context. Earlier permission to commit, push, or deploy is not supposed to become permission in a new prompt. **Review generated wording:** these are intended behaviors, not guarantees that a model will always interpret your meaning correctly.

### Settings

Click the sliders icon to adjust text size, phrase-button height, automatic suggestions, and whether the window stays above other apps. Settings follows macOS light/dark appearance; the main composer currently uses a light palette. Session counters report typing and phrase activity, not a measured amount of effort saved.

## Context, privacy, and account usage

- The selected conversation excerpts and your draft go to OpenAI for generation using your existing Codex ChatGPT sign-in and allowance. This is not an offline text predictor and does not switch to separately billed API access.
- The app reads local task metadata/history through `codex app-server`, without resuming or writing to the selected task. Context refreshes every eight seconds.
- Long conversations use the first three and most recent 24 turns, with partial context indicated in the interface. Only user and assistant text is included; tool output, reasoning, and attachment contents are excluded. Earlier included text may be summarized for prediction.
- Drafts and preferences are stored in `~/Library/Application Support/PromptCompanion/drafts.json`, with owner-only file permissions. This is local storage, not encryption. To reset saved drafts and preferences, quit the app and move this file to a safe backup location.
- Prompt Companion does not log prompt text, capture the screen, monitor global keystrokes, or request macOS Accessibility access. Codex’s own account and retention behavior still applies.
- Each generation uses an isolated ephemeral session. Executable capabilities are disabled, interactive server requests are rejected, and the generation sandbox is read-only. A private model-catalog copy removes file-editing tools without changing your Codex configuration.

Phrase generation prefers `gpt-5.6-luna`; expansion prefers `gpt-5.6-sol`. The installed model list is checked, with a supported fast-model fallback for phrases and the phrase model as the expansion fallback. Settings shows the models actually in use. Codex protocol and catalog changes can break compatibility; unsupported metadata causes an error instead of relaxing tool restrictions.

## Tests

Run the deterministic suite from the repository root:

```sh
swift test
```

The two live tests are skipped by default. Deterministic tests cover text insertion, Unicode and selections, context limits, stale results, pointer freeze, draft persistence, Undo, copy-and-clear, expansion, clarification, cancellation, and failure recovery. GitHub Actions is configured to run these tests and package the app on macOS without account credentials.

To run live integration tests locally **using your Codex usage allowance**:

```sh
PROMPT_COMPANION_LIVE_TEST=1 swift test --filter LiveIntegrationTests
```

Sign in and have at least one local Codex task first. Optionally set `PROMPT_COMPANION_TEST_TASK` to a task ID for a read-only history check. Generated test requests use synthetic context. Inspect the printed expansion examples for meaning; automated assertions cannot fully establish preserved intent.

Before proposing a UI change, check actual clicks: phrase insertion and focus, Expand, clarification, Keep original, Undo, Copy Prompt clearing, and Settings appearance. See [VALIDATION.md](VALIDATION.md) for recorded checks and remaining limitations.

## Troubleshooting

| Problem | What to try |
| --- | --- |
| `swift` is missing or too old | Install/update Xcode or Command Line Tools and check `swift --version`. |
| Codex cannot be found | Install Codex in `/Applications` or `~/Applications`, or the CLI in `/opt/homebrew/bin`, `/usr/local/bin`, or your launch environment’s `PATH`. Finder launches may not inherit shell PATH customizations. |
| ChatGPT sign-in is required | Sign into Codex with ChatGPT, then click **Reconnect**. An API-key login is not accepted. |
| Model catalog unavailable | Open Codex once, then reconnect. Update Codex if its metadata is unsupported. |
| No task or wrong context | Create/open a local Codex task, refresh the task list, and explicitly select the right task. |
| Suggestions appear stuck | Move the pointer away from the phrase area. Check automatic suggestions in Settings or click **Refresh phrases**. |
| Timeout or connection error | Check network connectivity and Codex allowance, then retry or reconnect. Your draft remains available. |
| An old version opens | Quit all running copies and open the exact bundle you just built. |

For a reproducible problem, [open an issue](https://github.com/OwenMcGirr/prompt-companion/issues) with macOS, Swift, and Codex versions, steps, expected behavior, and actual behavior. Use synthetic examples and redact private conversation text and account information.

## Project layout and contributions

- `Sources/CompanionCore`: draft editing, context processing, and expansion response validation.
- `Sources/PromptCompanion`: SwiftUI/AppKit interface, state and persistence, generation service, and app-server transport.
- `Tests/CompanionCoreTests`: deterministic tests and opt-in live integration tests.
- `Resources`: bundle metadata and icon.
- `build.sh`: release app packaging and local signing.

Accessibility feedback, bug reports, and focused improvements are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) before submitting changes.

## License

A license has not been selected yet. No open-source license is granted by this repository at present; a license decision is pending before public release.
