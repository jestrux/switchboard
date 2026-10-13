---
id: settings-and-startup
number: 11
title: Settings and startup
category: features
description: The Settings window, opening at login, and what the app remembers per Mac.
---

# Settings and startup

## Settings window

- **What:** Theme, appearance, the project rail and open at login, with a live preview of the shell.
- **Reach it:** Switchboard › Settings… (⌘,).
- **State:** Preferences on this Mac, under the `dev.waky.switchboard` domain.
- **Limits:** None known.

## Open at login

- **What:** Switchboard opens when you log in. The installed app turns this on once, at its
  first launch from `/Applications`. After that your choice stands.
- **Reach it:** Settings › Startup › Open at login, or System Settings › General › Login Items.
- **State:** Held by macOS as a login item. The app keeps only a flag that it has set the default.
- **Limits:** No commands start at login. A build run from outside `/Applications` does not
  register itself.

## What is remembered per Mac

- **What:** Window state that is yours and not the project's: pane layouts, the focused
  pane, sidebar widths, open groups, project order, which projects have their own windows,
  and terminals.
- **Reach it:** Automatic.
- **State:** The app's preferences, plus `terminals.yaml` per project. None of it is
  written to a config or a repo. The keys are listed in the [Agent guide](../agent-guide.md).
- **Limits:** State for a deleted project is not cleaned out of the preferences.

## Downloadable build

- **What:** A universal build, signed with Developer ID and notarized by Apple, that opens
  on any Mac without a Gatekeeper warning.
- **Reach it:** The latest GitHub release. `scripts/release.sh patch --publish` (or minor,
  major, or an exact version) bumps the version, builds, and creates the release.
- **State:** `dist/Switchboard-<version>.zip`. The script writes the version to
  `app/project.yml` and `Cargo.toml`.
- **Limits:** There is no auto-update. The script needs Xcode signed in to the developer account.

## Folder access

- **What:** The app is signed with a stable identity, so macOS asks once for access to
  folders such as Documents and remembers the answer across rebuilds.
- **Reach it:** Automatic with `scripts/build-app.sh`.
- **State:** Held by macOS.
- **Limits:** It needs an Apple Development certificate on the Mac that builds it.
