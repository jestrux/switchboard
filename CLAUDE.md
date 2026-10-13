# Switchboard

A native Mac app (SwiftUI) over a Rust engine that starts, watches and stops a project's
dev commands. See [README.md](README.md) for what it does.

**Before working here, read [docs/agent-guide.md](docs/agent-guide.md).** It has the fast
build and test loops, the UI driver for exercising the real app, how to put the app in a
known state, and the traps already hit.

**To learn what the app already does, read [docs/features/](docs/features/README.md).**
It is the feature list: every feature, how to reach it, where its state lives and its
limits. After meaningful feature work, ask the author whether to update the docs, and on a
yes update the matching feature page (and any guide it touches) as part of that work.

The essentials:

- Process logic lives in Rust (`crates/sb-core`). Swift only renders engine state.
- `cargo test -p sb-core` is the fastest check. `scripts/package.sh` builds, installs and
  relaunches the app.
- After adding a Swift file run `cd app && xcodegen generate`. After changing `sb-ffi` run
  `scripts/build-core.sh --debug`.
- Verify UI changes by driving the app with `scripts/dev/uidrive.swift` and looking at a
  screenshot, not by assuming.
- Never run `make migrate`, `make seed` or `make flush` in the sample projects unasked.

**Asked to set up a product as a Switchboard project** ("help me set up X including its
theme")? Follow [docs/setting-up-a-project.md](docs/setting-up-a-project.md).

Other guides: [docs/projects.md](docs/projects.md), [docs/theming.md](docs/theming.md).
`scripts/docs.sh` serves everything in `docs/` as a browsable site (the app is in
`docs-site/`; the Markdown files are the source).

What is left to build: [PLAN.md](PLAN.md), with context in [plans/](plans).
