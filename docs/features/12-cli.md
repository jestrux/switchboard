---
id: cli
number: 12
title: The switchboard command
category: features
description: Drive the running app from a terminal or an agent, or run projects without it.
---

# The switchboard command

## Installing it

- **What:** Links the `switchboard` command onto your PATH and the agent skill into
  `~/.claude/skills/switchboard`. Both are symlinks into the app bundle, so they update
  with the app.
- **Reach it:** Switchboard › Install switchboard Command…. A dialog shows where each
  went. It names the folder if that folder is not on your PATH yet.
- **State:** The command goes in the first writable folder on your login shell's PATH out
  of `~/.local/bin`, `/opt/homebrew/bin`, `/usr/local/bin` and `~/bin`. If none of them
  is on your PATH, it goes in `~/.local/bin`. A file that is not already Switchboard's link
  is never replaced.
- **Limits:** No uninstall; delete the two links by hand.

## Driving the app

- **What:** `switchboard status`, `start`, `stop`, `restart`, `logs`, `wait` and `open`
  act on the running app. Commands run in its own terminals, where you can watch them.
  `status --json` is for scripts.
- **Reach it:** `switchboard start kinpay`, `switchboard wait kinpay/backend/api`,
  `switchboard logs kinpay/backend/api -n 50`. The full list is in
  [Projects, config and repo files](../projects.md#the-switchboard-command).
- **State:** The app serves `control.sock` in its support folder while it runs.
  `start`, `restart` and `open` launch the app first if needed, with a clean environment so
  the caller's variables do not leak into its commands.
- **Limits:** `logs` reads what the app has kept, about the last 768 KB of output.
  `wait` (ready) needs the command to print its URL; for one that never does, use
  `--for running`.

## The agent skill

- **What:** `skills/switchboard/SKILL.md` teaches an agent to use the command: start, wait,
  read logs before asking, and set a product up as a project.
- **Reach it:** Installed with the command. The source is in this repo, and
  `scripts/sync-skill.sh` refreshes its reference pages from `docs/`.
- **State:** Bundled at `Switchboard.app/Contents/Resources/skills/switchboard`.
- **Limits:** None known.

## Running without the app

- **What:** `switchboard run <project>` starts the autostart commands in the terminal it
  was called from and prints their output with a prefix per command. A group or a single
  command can be named, and a single command runs interactively.
- **Reach it:** `switchboard run kinpay`, `switchboard run kinpay backend`,
  `switchboard run kinpay backend/migrate`.
- **State:** Sessions are recorded like the app's, so `switchboard list` shows live status.
- **Limits:** Ctrl+C stops everything that run started. The app does not show these
  sessions.

## Validate and clean up

- **What:** `switchboard validate` checks a `config.yaml` or a repo's `project.yaml` and
  names every fragment it reads. `switchboard cleanup` stops sessions left behind by a
  crash.
- **Reach it:** `switchboard validate <folder or file>` and `switchboard cleanup`.
- **State:** None.
- **Limits:** None known.
