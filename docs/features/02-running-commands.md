---
id: running-commands
number: 2
title: Running commands
category: features
description: Starting and stopping commands, what each status means, and what happens on a crash, on quit and at the next launch.
---

# Running commands

## Start, stop, restart

- **What:** Each command runs through your login shell in its own real terminal and its own
  process group. Stop sends SIGINT, then SIGTERM after 3 seconds, then SIGKILL after 3 more.
- **Reach it:** The row's hover buttons, the pane bar, the context menu, or the Command
  menu. Stop is ⌘. and Restart is ⌘R.
- **State:** Live sessions are recorded in `sessions.json` in Application Support.
- **Limits:** A command whose folder is missing cannot start and shows "no folder".

## Start all, stop all, restart all

- **What:** Start all starts every command with `autostart` on. Stop all stops everything
  alive. Restart all restarts what is alive and starts the autostart commands.
- **Reach it:** The project header, a group row on hover, or Command › Start All in
  Project and Stop All in Project. Command › Stop Everything covers every project.
- **State:** `autostart` is per command in the config and defaults to on.
- **Limits:** Nothing starts when the app opens. `autostart` only decides what Start all includes.

## Statuses

- **What:** Every command shows one of six states: stopped, starting, running, done (exit
  0, a tick), crashed, or error such as a missing folder.
- **Reach it:** The marker at the start of each row and in each pane bar. The row tooltip
  has the full name and a one-line summary.
- **State:** Reported by the engine. The app only renders it.
- **Limits:** None known.

## Crash restarts

- **What:** A command with `restart_on_crash` is restarted with a growing delay of 1, 2, 4
  and up to 30 seconds. It gives up after 5 crashes in 2 minutes.
- **Reach it:** Set `restart_on_crash: true` on the command. The row counts down to the
  next attempt and the pane shows a backoff bar.
- **State:** In the config. It defaults to off.
- **Limits:** Stop cancels a pending restart.

## Restart to apply

- **What:** When a running command's `run`, `cwd` or `env` changes in the config, the
  command keeps running the old version and is marked "↻ apply".
- **Reach it:** Restart the command to pick up the change.
- **State:** Derived by comparing the running session with the config.
- **Limits:** Other edits, such as a rename, apply at once without a restart.

## Run separators

- **What:** Each time a command starts, the pane prints a "started HH:MM:SS" line, so runs
  are easy to tell apart in the scrollback.
- **Reach it:** Automatic.
- **State:** Part of the terminal's scrollback only.
- **Limits:** None known.

## Quitting and leftovers

- **What:** Quitting stops every session and waits for whole process groups to exit. If the
  app crashed and left sessions behind, the next launch offers to stop them.
- **Reach it:** Quit as usual. `sb cleanup` does the same clean-up from a terminal.
- **State:** `sessions.json` is how leftovers are found.
- **Limits:** Closing the last window does not quit the app, so commands keep running.

## Starting again at the next launch

- **What:** Switchboard remembers which commands and terminals were running. At the next
  launch it asks "Start the command that was running?" and lists them. Start It starts
  each one fresh. Not Now starts nothing and drops the offer. Start It is ⌘Return, not
  Return, so a stray keypress at launch cannot start anything. Esc is Not Now.
- **Reach it:** Automatic at launch, after the leftovers prompt when there is one. It also
  covers a crash, since the list is kept up to date while commands run.
- **State:** `runningAtQuit` in the app's `defaults`, as `project<tab>address` entries.
- **Limits:** A command starts from scratch, so a session inside it, such as a Claude
  conversation, does not come back. Entries that are no longer in the config are skipped.
  Quitting while the prompt is still open keeps the offer for the launch after.
