# macOS user-operated Codex insertion test

This is a development experiment, not an approved insertion adapter. Normal preview packages still exclude it. Agents must not operate or inspect Codex fields through the adapter to bypass UI-tool restrictions.

## Start with one native trial

1. Open the development build (`npm run tauri -- dev --features insertion-prototype`, or its locally built app). Enable **Prompt Companion Preview** in macOS Privacy & Security → Accessibility. Rebuilding may require reauthorizing the app.
2. In Codex, use a disposable, unsent draft containing `LEFT RIGHT`. Put the cursor immediately before `RIGHT`. Do not use a draft you need to keep.
3. In Prompt Companion, click **Use TEST in my draft (Undo available)**. This replaces only the companion draft with `TEST `, including the trailing space; its Undo restores the previous draft.
4. Check **Enable insertion testing** and **Include Codex — I will operate the test myself**. These switches are not persisted and do not approve the adapter for release.
5. Make the Codex draft field visible before arming. Click **Insert on next field click** in Prompt Companion. This snapshots your draft and waits up to 30 seconds for one external left click.
6. Personally click immediately before `RIGHT` in the Codex field. Capture and one native insertion attempt happen after that click, while Codex remains active. There is no return trip to Prompt Companion. A click on a toolbar, Dock icon, other app, or control that does not match the focused editable field refuses the attempt.
7. Inspect Codex yourself. Expected text: `LEFT TEST RIGHT`. Nothing should be submitted, and the companion draft must remain `TEST `.
8. Use Codex's normal Undo, using its Edit menu if available. It must restore `LEFT RIGHT` in one edit. Record whether focus and surrounding text were preserved.

**Cancel waiting insertion** disarms the request. An expired request or a changed companion draft causes no insertion. The first external left click consumes the attempt, including failures; there is no automatic rearming.

The older **Capture next field** flow remains available below as an optional capture-then-review method. It requires returning to Prompt Companion and clicking an insertion method separately.

If native replacement is unsupported, that is a valid failed method result. Do not assume an error means no write occurred: inspect the field first. After resetting the destination to the baseline, a separate clipboard trial can use **Paste on next field click**, followed by your click in the reset destination. It never runs as an automatic fallback. Clipboard paste replaces supported plain-text clipboard contents; unsupported formats are refused and no timed restoration occurs.

Every attempt consumes the capture, including errors. There is no automatic retry or switch of methods. Turn off insertion testing when finished. The test mode does not send external text to generation or persist external field contents.

## Report the first result

No screenshot or conversation text is necessary. Report:

- App's result message (such as unsupported, verified, or uncertain).
- Exact disposable field text after insertion.
- Whether one destination Undo restored `LEFT RIGHT`.
- Whether focus returned to Codex, the companion draft stayed intact, and nothing was sent.

## Remaining acceptance matrix

Passing the first trial is not release approval. Repeat native and clipboard trials separately for empty fields; cursor at start/middle/end; selected ranges; multiline text; emoji; destination text/selection changes after capture; other windows; closed/restarted destinations; protected, read-only and disabled fields; denied/revoked permission; clipboard contention/formats; rapid repeated clicks; and interrupted/uncertain attempts. Refusal cases must cause no write. Each successful case must preserve normal destination Undo.

Proposed threshold: 20 successful repetitions per insertion case plus repeated race/refusal trials, with zero wrong-target writes, duplicate insertions, unintended surrounding-text changes, or submissions. Record operating system, app version, method, case, trial count and outcome. Keep content disposable. No Codex trial has passed yet; Windows manual Codex mode is not enabled by this change.

## If permission still fails after a rebuild

Ad-hoc development builds can invalidate their previous Accessibility authorization even when Settings shows the switch on. Quit the development app, reset only its stale approval with `tccutil reset Accessibility com.owenmcgirr.prompt-companion.preview`, then add the exact rebuilt `.app` using the plus button in Privacy & Security → Accessibility. Relaunch that same bundle. Do not rebuild between granting permission and testing. No other application's permissions need to change.

## How the next-click mode works

The macOS-only prototype installs a temporary AppKit global **left mouse-up** monitor after the explicit button click. It does not monitor keystrokes or mouse movement. It removes the monitor after the attempt, cancellation, or expiry. After letting the destination handle the click, it verifies that the clicked accessibility element is the focused editor or a descendant inside it, then performs the existing field/selection checks and one insertion. When manual Codex testing is selected, next-click writes are restricted to Codex. Field contents and click coordinates remain transient and local.

[Apple's event-monitor documentation](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/MonitoringEvents/MonitoringEvents.html) describes the global monitor used here. OS focus alone never triggers next-click insertion. Synthetic tests verify cancellation, expired/changed drafts, refusal, one-attempt behavior and absence of background-triggered insertion. The physical-click and Codex Undo checks remain user-operated acceptance tests.
