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
  with the app. It also lists `/switchboard` in `~/.claude/CLAUDE.md`, so agents that read
  your skills list there see it.
- **Reach it:** The first launch without them offers it ("Let coding agents use
  Switchboard?"), once. Later: Switchboard › Install switchboard Command…, or from a
  terminal or agent, `/Applications/Switchboard.app/Contents/Helpers/switchboard setup`.
  A dialog (or the command's output) shows where each went. It names the folder if that folder is not on your PATH yet.
- **State:** The command goes in the first writable folder on your login shell's PATH out
  of `~/.local/bin`, `/opt/homebrew/bin`, `/usr/local/bin` and `~/bin`. If none of them
  is on your PATH, it goes in `~/.local/bin`. A file that is not already Switchboard's link
  is never replaced. In `CLAUDE.md`, the skill goes in as a row after the last row of a
  skills table (lines like `` | `/name` | … | ``), or as a short "Switchboard" section at
  the end when there is no table. A file that already names `` `/switchboard` `` is left
  alone, and a missing file is not created.
- **Limits:** No uninstall; delete the two links and the `CLAUDE.md` line by hand.

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

## Adding a command

- **What:** `switchboard add-command` adds one command to a project without editing YAML by
  hand. It goes into the group in `config.yaml`, or into the repo's
  `.switchboard/commands.yaml` when the group comes from one. A group that does not exist
  yet is created when `--group-name` and `--group-cwd` are given.
- **Reach it:** `switchboard add-command set-hero/web --id storybook --run "npm run storybook" --no-autostart`.
  Also `--name`, `--cwd`, `--env KEY=VALUE` (repeatable) and `--restart-on-crash`.
- **State:** The config is validated before it is written. The app picks the change up
  within a second, and running commands are untouched.
- **Limits:** Adds only (`edit-command` changes one); remove a command in the YAML. A
  duplicate id in the group is refused.

## Changing a command

- **What:** `switchboard edit-command` changes an existing command's fields where it's
  defined: the project's `config.yaml`, or the repo's `.switchboard/commands.yaml` for a group
  that comes from one. Only the fields passed change. When the project overrides a field that
  changes (`groups[].overrides`), the override is dropped so the edit takes effect.
- **Reach it:** `switchboard edit-command set-hero/web/dev --run "source ~/.nvm/nvm.sh && nvm use && npm run dev"`.
  Also `--cwd` and `--name` (`""` returns either to its default), `--env KEY=VALUE` and
  `--unset-env KEY` (repeatable), `--autostart`/`--no-autostart`,
  `--restart-on-crash`/`--no-restart-on-crash`. It prints what changed.
- **State:** Validated before it's written; the app picks it up within a second. A running
  command keeps its old run line and shows `needs_restart` until it's restarted.
- **Limits:** The file is rewritten, so its YAML comments are lost (as with `add-command`).
  Ad-hoc terminals aren't in the config and can't be edited. It doesn't rename a command's id.

## Validate and clean up

- **What:** `switchboard validate` checks a `config.yaml` or a repo's `project.yaml` and
  names every fragment it reads. `switchboard cleanup` stops sessions left behind by a
  crash.
- **Reach it:** `switchboard validate <folder or file>` and `switchboard cleanup`.
- **State:** None.
- **Limits:** None known.
