# Contributing

Prompt Companion aims to reduce typing while keeping the author in control. Useful feedback includes a short synthetic example, the expected behavior, and what made the interaction helpful or difficult.

## Development

Follow the [README](README.md) to install Rust, Node.js, npm, and platform prerequisites. Work from `desktop` and keep changes focused.

Before submitting code, run:

```sh
npm run build
npm test
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --features insertion-prototype --locked -- -D warnings
```

Run the appropriate Tauri package build for platform or packaging changes. Normal tests require no Codex credentials. Live tests are optional, consume the signed-in Codex account's allowance, and must never receive repository or CI secrets. Review generated outputs as well as assertions when changing prompts or model integration.

For interface changes, check the affected flow with ordinary left clicks and keyboard navigation. Preserve large targets, readable labels, draft focus after phrase acceptance, responsive refreshing, and an undoable path for generated edits. Check light and dark appearance. Do not add automatic prompt submission or carry conversation permissions into generated prompts.

Insertion changes require user-operated destination checks. Exercise cursor and selection replacement, existing and multiline text, emoji, rapid clicks, focus changes, closed windows, permission denial/revocation, unsupported fields, and destination Undo. Never retry an uncertain insertion automatically.

Include relevant validation in the pull request and state checks that could not be performed. Documentation-only changes should have commands, paths, and links reviewed.

## Reporting problems

Include operating system, architecture, Codex version, reproduction steps, expected and actual behavior, and a synthetic draft/context. Remove screenshots or text containing private task names, credentials, saved drafts, task IDs, or conversation history.

For a potential security issue, use GitHub's private vulnerability reporting option if enabled. Otherwise ask the maintainer for a private reporting route without including exploit details or sensitive data.
