# Validation — 5 September 2026

Recorded development environment: Apple Swift 6.3.3 for Apple Silicon, with macOS 14 as the deployment minimum.

- 32 deterministic tests pass: text insertion, partial words, selections, combining accents, emoji, output validation, bounded context, preservation of the opening goal, late-result rejection, pointer freeze, draft isolation, instant phrase reuse, persistence, Clear, Undo, copy success, copy failure, empty-copy behavior, expansion parsing, whole-draft replacement, clarification, cancellation, late responses, expansion Undo/persistence/focus, and expansion-to-copy behavior.
- The opt-in live integration test passes against the installed Codex 0.153.1 with the existing ChatGPT sign-in. It reads local task metadata and this task's history without resuming the source task.
- A real prediction request using synthetic login-crash context and `fix` returned three usable completions in 3.5 seconds: “fix the missing-user guard”, “fix the login crash with a guard”, and “fix this and add a regression test”. This is one observed request, not a latency guarantee.
- A local mock-provider probe using both the Luna phrase model and Sol expansion model verified that the outgoing prediction request offers **no tools**. Disabling executable features plus removing `apply_patch` from a private model-catalog copy also eliminates executable tools for the tested Mini fallback; its remaining `request_user_input` tool has no implementation in this client and server requests are rejected. No original configuration or model-catalog file was modified.
- The application bundle is signed locally and passes strict code-signature and property-list validation.

## Native interface checks

The app opened and displayed contextual suggestions. A real left click on “Verify focus returns to the text box” appended that phrase to the existing draft. The accessibility tree confirmed that the Prompt draft text area was the focused element immediately afterward. Clicking Undo restored the original draft and kept focus in that text area. The original draft was preserved. The updated Copy Prompt button was also tested: it clears the draft and restores focus, and pasting back into the draft confirms that the clipboard text is intact. That copy check left a cleared draft and the copied prompt on the clipboard. Pasting into Codex itself and the remaining Settings controls still need a manual check; longer personal accessibility trials remain outstanding.

Direct composition in Codex and automatic tracking of the visible Codex task are not included. This build uses a separate composer and an explicitly selected task for context.

## Expansion review

The current expansion review passes against Sol using synthetic app context. Earlier authorization to commit and push was deliberately included in that context; it was not carried into the new prompts.

| Shorthand | Reviewed output |
| --- | --- |
| bigger buttons same layout | “Make the three phrase buttons bigger while keeping the same vertical layout.” |
| why slow | “Why are the phrase suggestions in Prompt Companion arriving slowly? Identify and explain the cause of the delay without making changes.” |
| fix it | Asked which issue to fix, with choices for enlarging buttons, speeding up suggestions, or both. Selecting buttons produced an expansion about those buttons. |
| copy clear but no push | “Update Copy Prompt so that an ordinary left click copies the editable draft and then clears it. Ensure Undo can restore the cleared text. Do not push the change.” |

Observed initial expansion requests took approximately 3.7–6.3 seconds. These samples support this review, not a guarantee of latency or semantic equivalence on every prompt. The initial lighter-model review exposed ambiguity and verb-sequence errors; Sol and clearer interpretation rules produced the reviewed outputs above.

In the running app, a real click on Expand replaced “bigger buttons same layout” with a fuller prompt and returned keyboard focus to the draft. One click on Undo restored the shorthand. “bigger text or buttons” displayed a clear question with three stable buttons; clicking “The buttons” expanded only that interpretation. Keep original cancelled another attempt without changing the shorthand. The default layout was visually inspected, and the pre-test draft was restored after testing. The updated application remains running.

## Settings dark-mode fix

Rebuilt the release app and checked Settings with actual clicks in macOS dark appearance. Adaptive AppKit label and window-background colors replace the inherited fixed ink color and translucent surface. All labels and multiline descriptions are visible; the popover keeps its full content height. Toggled “Keep this window above other apps” off and on, verified both accessibility values, and restored the original enabled preference. The draft was untouched. Light appearance and live system appearance switching were not manually tested. This presentation-only change does not alter generation or draft behavior.

## Public repository preparation

A clean Swift scratch-directory run passed 32 deterministic tests with two live tests skipped. Release packaging was checked separately. Added a macOS GitHub Actions workflow for deterministic tests, packaging, and signature/plist validation; no account credentials or live generation are configured in CI. Reviewed tracked file names and commit author metadata, and scanned all 30 unique tracked blobs across the four pre-preparation commits for common API/GitHub/AWS credential patterns, private-key headers, and personal absolute paths. No matches were found. This limited pattern scan is not a comprehensive security audit.
