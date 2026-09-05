# Validation — 5 September 2026

Built on this Mac using Apple Swift 6.3.3 for Apple Silicon, with macOS 14 as the deployment minimum.

- 16 deterministic tests pass: text insertion, partial words, selections, combining accents, emoji, output validation, bounded context, preservation of the opening goal, late-result rejection, pointer freeze, draft isolation, instant phrase reuse, persistence, Clear, and Undo.
- The opt-in live integration test passes against the installed Codex 0.153.1 with the existing ChatGPT sign-in. It reads local task metadata and this task's history without resuming the source task.
- A real prediction request using synthetic login-crash context and `fix` returned three usable completions in 3.5 seconds: “fix the missing-user guard”, “fix the login crash with a guard”, and “fix this and add a regression test”. This is one observed request, not a latency guarantee.
- A local mock-provider probe using the default Luna model verified that the outgoing prediction request offers **no tools**. Disabling executable features plus removing `apply_patch` from a private model-catalog copy also eliminates executable tools for the tested Mini fallback; its remaining `request_user_input` tool has no implementation in this client and server requests are rejected. No original configuration or model-catalog file was modified.
- The application bundle is signed locally and passes strict code-signature and property-list validation.

## Outstanding manual checks

After the Mac was unlocked, the app opened and displayed contextual suggestions. A real left click on “Verify focus returns to the text box” appended that phrase to the existing draft. The accessibility tree confirmed that the Prompt draft text area was the focused element immediately afterward. Clicking Undo restored the original draft and kept focus in that text area. The original draft was preserved. Clipboard handoff and Settings controls still need a manual check; longer personal accessibility trials remain outstanding.

Direct composition in Codex and automatic tracking of the visible Codex task are not included. The computer-use tool explicitly blocks inspecting Codex itself. This build implements the accepted separate-composer fallback and uses a clearly labeled task picker.
