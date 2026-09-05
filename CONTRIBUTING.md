# Contributing

Prompt Companion aims to reduce typing while keeping the author in control of their words. Useful feedback includes a short example of what you typed, what you expected, and what made a suggestion helpful or difficult to select. Use invented context instead of sharing private task history.

## Development

Follow [README.md](README.md) to install prerequisites, build, and run. There are no third-party Swift package dependencies. Make focused changes on a branch and describe the problem and resulting behavior in your pull request.

Run `swift test` and `./build.sh "$PWD/dist"` before submitting code changes. The normal test suite requires no Codex credentials. Live tests are optional, use your account allowance, and must never receive repository or CI secrets. Review model outputs as well as assertions when changing prompts or model integration.

For interface changes, check the affected flow using real clicks. Preserve large targets, stable suggestions, readable labels, draft focus after insertion, and an undoable path for generated edits. Check Settings in light and dark appearance. Do not introduce automatic sending or carry old conversation permissions into newly composed prompts.

Include relevant validation in the pull request. State checks you could not perform. Documentation-only changes should have their commands, paths, and links reviewed; avoid unnecessary live generation runs.

## Reporting problems

Include macOS, Swift, and Codex versions, reproduction steps, expected and actual behavior, and a synthetic draft/context if relevant. Redact screenshots. Never post account credentials, `auth.json`, saved drafts, private task IDs, or full conversation histories in issues or pull requests.

For a potential security issue, avoid posting exploit details or private data publicly. Use GitHub’s private vulnerability reporting option if it is enabled; otherwise ask the maintainer for a private reporting route without including sensitive details.
