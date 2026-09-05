# Prompt Companion

A native Mac app that predicts chunks of your next prompt using a selected Codex task's conversation. It uses ordinary left click, large stable buttons, and the existing Codex ChatGPT sign-in.

## Use it

1. Open **Prompt Companion.app** next to this source folder.
2. Click the task you are working on. The task stays selected until you change it using the context button at the top.
3. Type in **Your prompt**, or choose a suggested starting phrase. Click a phrase to insert it. For a full prompt from shorthand, click **Expand** beside the draft.
4. Click **Copy Prompt**. After a successful copy, the draft clears and focus returns to the writing area. Paste into Codex using your usual paste method. Nothing is submitted automatically. If copying fails, your draft stays intact.

**Undo** reverses the most recent edit, including a phrase insertion, an expansion, Clear, or the clearing after Copy Prompt. Undoing a copy restores your draft without changing the clipboard. Drafts are saved separately for each task and restored after reopening. A short trailing space is added when appropriate so you can continue typing immediately.

The settings button adjusts text size and phrase-button height, pauses automatic suggestions, and controls whether the window stays on top. Session counters show typed and inserted characters and selected phrases. These are activity counts, not a measured claim about effort saved.

Suggestions freeze while the pointer is over the three-button area. If new suggestions arrive then, move the pointer out of that area to reveal them. Suggestions based on an older draft become unclickable immediately.

## Expand shorthand

Write a short note such as **“bigger buttons same layout”** and click **Expand**. The app rewrites the whole draft into an editable prompt using the selected conversation. A successful expansion restores focus to the draft and is one undoable edit; Undo restores the original text and selection. Expanded prompts are saved normally and use the same copy-and-clear behavior.

When the meaning would materially change the request, the app asks one short question using the existing large phrase buttons. Click an interpretation to expand with that meaning, or **Keep original** to cancel. The app does not ask a second clarification question; remaining uncertainty stays in the wording. New choices wait until the pointer leaves the button area before appearing.

Expansion is explicit, not automatic. Phrase generation pauses during expansion and clarification. Editing, changing tasks, clearing, copying, or receiving an updated conversation cancels the expansion and rejects late results. Errors leave the original text untouched. Afterward, the normal phrase workflow is available again.

Expansion instructions preserve questions, limits, corrections, negations, and directly applicable constraints. Details must come from the conversation; earlier permissions to commit or push do not become permissions in a new prompt. The result remains editable because generative rewriting can still miss nuance. The default is one concise paragraph, usually 2–5 sentences and shorter when sufficient.

## Current integration

This version implements the accepted **companion composer fallback**. Direct insertion into Codex and automatic detection of the visible Codex task are not implemented: the available computer-use tool blocks inspecting Codex, so those capabilities could not be verified. The app does not capture the screen, monitor global keystrokes, patch Codex, or request macOS Accessibility access.

The task picker reads local task metadata through the installed `codex app-server`. Conversation history is read without resuming or writing to the source task. It refreshes every eight seconds. For long tasks, the first three and most recent 24 turns are used; the UI labels this as partial context. Only user and assistant text is used, excluding reasoning, tool output, and attachment contents. Earlier included messages are summarized with the first prediction; recent messages remain available as text. Any conversation change invalidates that summary and existing predictions.

## Predictions and account access

Uses the existing Codex **ChatGPT** sign-in and its usage allowance. There is no API-key setup, credential copying, or automatic switch to separately billed API usage. The selected conversation excerpts and the draft are sent to OpenAI for prediction. Drafts are saved locally in `~/Library/Application Support/PromptCompanion/drafts.json` with owner-only permissions. The app does not log prompt text. Codex's own account and retention behavior still applies.

Phrase suggestions use `gpt-5.6-luna`; explicit whole-prompt expansion uses `gpt-5.6-sol` for interpretation quality. Both are checked against the installed Codex model list. A listed fast model replaces Luna if needed; expansion falls back to the phrase model if Sol is unavailable. Settings shows the active models. Each generation owns a separate ephemeral thread and transport, so cancelling an older request cannot close or overwrite a newer request. These sessions do not appear as saved tasks or modify the selected conversation.

Executable capabilities are removed using process/thread configuration: shell, connectors, plugins, browsing, computer use, image tools, hooks, agents, and related tools are disabled. A private copy of the installed model metadata removes `apply_patch`. Any remaining interactive server requests are rejected. The prediction sandbox is read-only. User Codex configuration is not edited.

The app-server and local catalog formats can change with Codex updates. Reconnect retries the connection and reloads metadata. A connection or prediction error preserves the draft. Unsupported model metadata or missing ChatGPT authentication prevents prediction instead of silently loosening these constraints.

## Build and test

Requires macOS 14+, Swift 6 / Xcode command-line tools, and an installed Codex CLI or desktop app. The supplied app is built for this Apple Silicon Mac and signed locally, not notarized for distribution.

```sh
swift test
./build.sh
```

`build.sh` puts the application next to the source folder by default. An optional first argument changes the output directory. `PROMPT_COMPANION_BUILD_DIR` can put build artifacts elsewhere.

The live integration tests are opt-in because it uses the existing Codex allowance:

```sh
PROMPT_COMPANION_LIVE_TEST=1 swift test --filter LiveIntegrationTests
```

Optionally set `PROMPT_COMPANION_TEST_TASK` to a task ID to include a read-only history check. The prediction portion uses synthetic login-crash context, regardless of the selected task. The expansion review checks the four shorthand examples and a clarification response against synthetic app context. Review the printed output for meaning as well as checking test success.

Unit tests cover partial words, replacement inside a word, selections, emoji, punctuation, malformed predictions, context filtering/budgeting, stale-result rejection, pointer freeze, Undo, task switching, persistence, copy behavior, expansion and clarification validation, cancellation, late responses, and expansion-to-copy behavior.

## First personal trial

Try composing a correction, a follow-up question, and a request to change something in a real task. Note whether each useful chunk arrives soon enough and whether selecting it feels easier than typing it. Adjust font size and button height in Settings. The next iteration should be guided by that experience, especially phrase length, button placement, and the cost of copying back into Codex.

Protocol reference: [Codex App Server](https://learn.chatgpt.com/docs/app-server).
