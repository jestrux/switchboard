---
id: linux
number: 13
title: Switchboard on Linux
category: features
description: The same projects and command on Linux (Omarchy) — a background host, the window in a terminal, and platform overlays.
---

# Switchboard on Linux

There is no app on Linux. The engine runs as a background host, the window is a terminal
app, and every `switchboard` command works as it does on the Mac. The plan and decisions
behind it are in [plans/07-linux.md](../../plans/07-linux.md).

## Installing

- **What:** The `switchboard` command, built from this repo, and the agent skill linked from it.
- **Reach it:** `cargo install --path crates/sb-cli --locked` (it lands in `~/.cargo/bin`), then
  `ln -s ~/Work/switchboard/skills/switchboard ~/.claude/skills/switchboard`.
- **State:** Projects live in `~/.local/share/Switchboard/Projects/<slug>/`.
- **Limits:** No `switchboard setup` on Linux; it looks for the Mac app bundle.

## The host

- **What:** `switchboard serve` holds the engine and the control socket that the other commands
  talk to, as the Mac app does. It kills sessions a dead host left behind, watches the Projects
  folder, and stops every command's process group on Ctrl+C, SIGTERM or SIGHUP.
- **Reach it:** Automatic: `start`, `restart`, `wait` and the window launch it when nothing serves,
  through the systemd user unit when it's enabled
  (`systemctl --user enable ~/Work/switchboard/scripts/linux/switchboard.service`), else in the
  background. `switchboard serve` runs it in the foreground.
- **State:** Logs to `~/.local/share/Switchboard/serve.log` when launched in the background.
- **Limits:** Stopping the host stops every command. Without the systemd unit nothing restarts
  it if it dies, and nothing starts the commands that ran before at the next login.

## The window

- **What:** `switchboard ui`, the Mac window in a terminal: a project rail with running counts,
  the grouped sidebar (collapsed groups with counts, every command with its status, uptime,
  port and pane number, hover actions), and up to four live terminals from any project, each
  with its pane bar and footer, in its project's colours. Quitting stops nothing.
- **Reach it:** Super+B, the Omarchy menu's Switchboard entry, or `switchboard ui`. Keys: ↑↓
  select, Enter types into the command (Ctrl+] back), s start/stop, r restart, a/X start/stop
  all, `|`/`-` split, x close pane, Tab next pane, [ ] or 1–9 project, PgUp/PgDn scroll back,
  t terminal, R rename it, D close it, c clear, o open the port, ? the rest, q or Ctrl+W quit.
  Clicks work on tiles, rows, buttons, ports and links; drag the divider to resize.
- **State:** Panes, the focused pane, open groups and the sidebar width in
  `~/.local/share/Switchboard/ui-layout.json`, written as they change.
- **Limits:** Monospace, symbols for icons, dark palettes only. No drag to reorder, image
  previews, OSC 8 links, repo notices, Settings, or a window per project. A "started"
  separator shows only to a viewer attached when the command starts.

## A command's terminal in yours

- **What:** `switchboard attach <slug>/<group>/<command>` turns the current terminal into that
  command's: its output so far, then live output, keystrokes and resizes, across restarts.
  Enter starts a stopped command.
- **Reach it:** `switchboard attach`; Ctrl+] detaches and leaves it running.
- **State:** None.
- **Limits:** Anything typed goes to the command.

## Platform overlays

- **What:** `.switchboard/project.linux.yaml` beside a shared `project.yaml` holds only what
  differs on Linux (paths, `mise` instead of `nvm`, hidden Xcode builds); `project.local.yaml`
  does the same for one machine. See [Projects](../projects.md#platform-and-machine-overlays).
- **Reach it:** Write the file; `switchboard validate <repo folder>` lists the overlays applied.
- **State:** In the repo (or, for `project.local.yaml`, kept out of git).
- **Limits:** Applied when a project loads and never written back. Scripts in a repo that
  hard-code Mac paths still need fixing in the repo.
