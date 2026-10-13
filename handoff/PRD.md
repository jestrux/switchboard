# Switchboard — PRD

Oct 13, 2026 · Walter Kimaro

## Overview

Switchboard is a native Mac app that starts, watches and stops a project's dev commands from one window, with each command running in its own folder. It is inspired by Solo, with one key difference: a project is a logical grouping of commands, not a directory.

The problem: a single product often spans several repos or folders (API, web frontend, e2e suite). Today that means juggling terminal tabs, remembering which command runs where, and hunting orphaned processes holding ports.

**Goals**

- Group any set of commands from any folders under one named project with an icon.
- Run each command in a real terminal, with full color and interactivity, and switch between them in a sidebar-plus-pane layout like tmux.
- Create and edit projects and commands entirely through native sheets, while keeping configs as readable YAML on disk.
- Stop everything cleanly: no orphaned child processes.
- Keep the engine in Rust as a standalone library and CLI, with SwiftUI as the UI shell.

**Non-goals for v1**

- Cross-platform support (Mac only; the Rust core should not block it later).
- Remote or SSH sessions, AI-agent orchestration, team sync or cloud accounts.
- Copying Solo's assets or exact config schema.

## MVP scope

The MVP is done when the Kinpay sample in `samples/kinpay/` runs properly in the app: every command starts, shows a live terminal, stops cleanly and reports the right status. There is no project-editing UI in the MVP.

**In the MVP**

- **Add a project by dropping a folder.** Dropping a folder that contains `config.yaml` onto the sidebar (or choosing Add Project… and picking one) validates it and copies it into the Projects directory. An invalid config shows its errors and nothing is copied. If a project folder with that name exists, ask to replace it or keep both.
- **Edit by editing the file.** Each project has an Edit Config action (context menu and toolbar) that opens its `config.yaml` in VS Code via `open -a "Visual Studio Code" <path>`. If VS Code is not installed, it falls back to the default app for `.yaml` files. A Reveal in Finder action sits next to it.
- **Live reload does the rest.** Saving the file updates the sidebar within about a second through the existing file watcher, without killing running sessions. A project whose config no longer parses shows an error row with the message and an Edit Config button.
- Sidebar with projects, groups and commands; one terminal pane; start, stop and restart for commands, groups and projects; the `sb` CLI.

**After the MVP**

- New and Edit sheets for projects, groups and commands, and icon import.
- Split panes, quick add from a folder's scripts, drag between groups, export, and the menu bar extra.

## Core concepts

A project owns no directory; every command carries its own working folder.

| Concept | What it is | Key fields |
| --- | --- | --- |
| Project | A named, logical grouping of commands, stored as its own folder in app support | name, icon, color, groups, commands |
| Group | An optional, ordered section inside a project, such as Frontend or Backend; gives commands a shared default folder and a display prefix | id, name, cwd (default), commands |
| Command | One long-running or one-off process | id, name, run, cwd, env, autostart, restart on crash |
| Session | A live run of a command: its PTY, process group, status and scrollback | status, pid, exit code, started at |

A command is addressed as `group/command` (for example `backend/migrate`), or just `command` when ungrouped, and is displayed as `Backend / make:migrate` in panes, search and window titles. Command status is one of: stopped, starting, running, crashed, exited. A command that exits with code 0 shows as done (a checkmark), so one-off tasks like migrations read as finished, not crashed. Each command belongs to exactly one project and at most one group in v1; shared commands across projects are deferred (see Open questions).

## Architecture

A Rust engine owns config, processes and terminal state; a SwiftUI app renders it through UniFFI, and a CLI shares the same core.

```
┌──────────────── Switchboard.app (SwiftUI + AppKit) ────────────────┐
│  Sidebar and sheets  │  Terminal panes (SwiftTerm)  │  Icon import │
└─────────────────────────────────┬──────────────────────────────────┘
                                  │  commands in / status + output bytes out
                       UniFFI bridge (sb-ffi)
                                  │
┌──────────── sb-core (Rust library) ────────────┐      ┌─────────┐
│  Config loader (config.yaml, FSEvents watch)   │ ◄─── │ sb CLI  │
│  Supervisor (start, stop, restart, backoff)    │      └─────────┘
│  PTY sessions (portable-pty, login shell)      │
│  Terminal parser (scrollback, search)          │
└───────────┬───────────────────────┬────────────┘
            │                       │
   Projects folder           Child processes
   (config.yaml + icon.png)  (one PTY + process group each)
```

The app and the CLI are two front ends on one library, so everything except drawing can be built and tested from the terminal first.

```
switchboard/
  Cargo.toml            # workspace
  crates/
    sb-core/            # config, supervisor, PTYs, parser
    sb-cli/             # `sb` binary
    sb-ffi/             # UniFFI exports + XCFramework build
  app/
    Switchboard.xcodeproj
    Switchboard/        # SwiftUI sources
  samples/
    kinpay/             # config.yaml + icon.png (reference fixture)
```

## Storage and config format

Each project is a self-contained folder under `~/Library/Application Support/Switchboard/Projects/`, holding `config.yaml` and an optional `icon.png`.

```
Projects/
  kinpay/
    config.yaml
    icon.png
  sethero/
    config.yaml
    icon.png
```

**config.yaml schema (v1)**

```yaml
version: 1
name: Kinpay
icon: icon.png              # optional; relative to this folder, or absolute
color: "#9B3FE6"            # optional; used for the initials fallback

commands: []                # optional; ungrouped commands, shown above groups

groups:
  - id: frontend            # unique within the project
    name: Frontend
    cwd: ~/Documents/web/kinpay   # default folder for this group's commands
    commands:
      - id: dev-app         # unique within its group; address: frontend/dev-app
        name: dev:app       # display name; any characters
        run: yarn dev:app
        autostart: true     # default true

      - id: storybook
        name: Storybook
        cwd: ~/Documents/web/kinpay/app   # overrides the group folder
        run: npm run storybook
        autostart: false
        restart_on_crash: false           # default false
        env:                              # optional; merged over the inherited env
          PORT: "6006"

  - id: backend
    name: Backend
    cwd: ~/Documents/web/kinpay-backend
    commands:
      - id: migrate
        name: make:migrate
        run: make migrate
        autostart: false
```

**Rules**

- Folder name is a slug of the project name; renaming the project in the UI does not rename the folder.
- `cwd` supports `~` and is stored `~`-relative when under the home folder.
- Unknown keys are preserved on save, so hand edits and future fields survive the UI.
- Missing `autostart` defaults to true; missing `restart_on_crash` defaults to false.
- Invalid YAML shows the project in an error state with the parse message; it never crashes the app or drops the file.

**Group rules**

- `groups` and top-level `commands` are both optional and can be mixed; ungrouped commands appear above the groups with no prefix.
- Group and command order in the file is the sidebar order; drag-reordering in the UI rewrites it.
- A command's `cwd` resolves in this order: its own `cwd`, then its group's `cwd`. Neither set is a validation error.
- Group ids are unique within a project; command ids are unique within their group (or among ungrouped commands).

**Icons**

- Resolution order: the `icon` field, then `icon.png` in the folder, then a generated fallback (initials on `color`).
- The UI accepts SVG, PNG or PDF. It renders the image once at 512×512 and saves it as `icon.png`. SVG is rendered via an offscreen `WKWebView` snapshot; PDF and PNG via AppKit.
- The app only ever reads PNG at runtime.

**Live reload**

- Watch the Projects directory with FSEvents. Adding a folder, editing YAML or replacing the icon updates the sidebar without a restart.
- Config changes never kill running sessions; a changed `run` or `cwd` applies on the next start, and the UI shows a "restart to apply" hint.

**Import and export**

- Export zips a project folder; import accepts a zip or a folder dragged onto the sidebar.

**Sample project in the repo**

The repo ships a real sample at `samples/kinpay/` containing `config.yaml` (nine commands in three groups, Frontend, Backend and Tunnels, across four folders) and `icon.png` (512×512). It is the reference for the format and the fixture for tests.

- The config loader's tests parse `samples/kinpay/config.yaml` and assert all three groups and nine commands, their order, ids, resolved folders (including group defaults and overrides) and autostart flags.
- `sb validate samples/kinpay` must pass from M1 onward.
- The app's debug build offers "Install sample project", which copies the folder into the Projects directory.
- The sample's folders exist only on the author's Mac, so a missing `cwd` must show the folder-missing state rather than fail the load. This doubles as the test for that state.
- Solo-style keys (`processes`, `working_dir`, `auto_start`) are not supported; v1 reads only this format.

## Process engine requirements

The Rust engine must make every command behave exactly as it would in the user's own terminal, and must never leave processes behind.

**Spawning**

- Each command runs in its own PTY via `portable-pty`, never through plain pipes, so colors, progress bars and interactive prompts work.
- Commands launch through the user's login shell: `$SHELL -lc "<run>"`, falling back to `/bin/zsh`. This picks up PATH from nvm, pnpm, Herd, Homebrew and similar tools.
- `cwd` is expanded and checked before spawn. A missing folder sets the command to an error state with a clear message; nothing is spawned.
- `env` entries are merged over the inherited environment. `TERM=xterm-256color` and `COLORTERM=truecolor` are set by default.
- PTY size follows the visible pane; resizes are forwarded (SIGWINCH).

**Stopping**

- Each command starts its own process group (`setsid`). Stop sends SIGINT to the group, then SIGTERM after 3 s, then SIGKILL after a further 3 s.
- Quitting the app stops all sessions the same way and waits for them before exiting.
- On launch, the engine checks a pid file for sessions from a previous crash and offers to kill leftovers.

**Supervision**

- Status transitions: stopped → starting → running → exited (code 0) or crashed (non-zero or signal).
- `restart_on_crash` restarts with backoff (1 s, 2 s, 4 s, capped at 30 s), resetting after 60 s of healthy running. Give up after 5 crashes in 2 minutes and show crashed.
- Actions per command: start, stop, restart. Per group and per project: start all (autostart commands), stop all, restart all.

**Output**

- The engine parses PTY output into a terminal grid with `alacritty_terminal` (or `vt100` if simpler), keeping 10,000 lines of scrollback per session.
- Raw bytes are also streamed to the UI so the terminal view can render directly; the parsed grid powers search and the CLI.
- Input from the UI (keystrokes, paste) is written to the PTY.

**CLI (`sb`)**

- `sb list` shows projects and command status.
- `sb run <project> [group | group/command]` runs commands in the foreground with prefixed, colored output, like a Procfile runner.
- `sb validate <path>` checks a `config.yaml` and reports errors.
- The CLI uses the same core library as the app and is the first deliverable.

## UI requirements

The window is a sidebar of projects and commands beside a terminal area, built in SwiftUI with AppKit where needed. Target macOS 14+.

**Layout**

- Sidebar: projects with their icon, each expanding to its commands. Each command shows a status dot (gray stopped, yellow starting, green running, red crashed, hollow exited) and hover buttons for start, stop and restart.
- Project row actions: start all, stop all, restart all, edit, export, delete. Inside a project, groups are collapsible sections with their own start, stop and restart all; ungrouped commands sit above them, and commands can be dragged between groups.
- Main area: the selected command's terminal, rendered with SwiftTerm's `TerminalView` fed the raw byte stream from the engine (SwiftTerm does not spawn processes itself).
- Splits: the user can split the main area horizontally or vertically to show two to four terminals at once. Each pane picks its command from a menu. The layout is remembered per project.
- Pane titles and the window title use the prefixed name (Backend / make:migrate). A footer per pane shows command name, cwd, pid, uptime and exit code when stopped.

**Terminal behavior**

- Full keyboard input when the pane is focused; Cmd+C/Cmd+V copy and paste; clickable URLs; mouse reporting for apps that request it.
- Cmd+F find bar over the scrollback; Cmd+K clears the pane.
- Font and theme follow a single app setting (default SF Mono 12, system light/dark).

**Interactive apps are first-class**

A command can be a full-screen interactive program, such as `claude`, `vim`, `htop` or `lazygit`, and the pane must behave like a real terminal for it.

- Support the alternate screen buffer, bracketed paste, mouse reporting, focus events, 256-color and truecolor, and OSC 8 hyperlinks.
- Forward every key correctly, including Option as Meta (a setting), Ctrl combos, Escape, and arrow and function keys.
- Keep app shortcuts (Cmd+…) for the app; everything else goes to the process when the pane is focused.
- Sessions keep running when their pane is hidden or the window is closed; switching back reattaches with the screen intact.
- Commands with `autostart: false` are the usual home for these, for example `run: claude` with `cwd` set to the repo.

**Sheets (after the MVP; the MVP edits config files directly)**

- **New / Edit Project**: name; icon well (drop or pick SVG/PNG/PDF, live preview, initials fallback); fallback color. Save creates or updates the project folder.
- **New / Edit Command**: name; group picker (or none); run; folder picker showing the resolved path, with the inherited group folder as a placeholder until overridden and a warning if missing; autostart and restart-on-crash toggles; expandable env key/value table; a Test run button that runs the command in a temporary pane. Edit adds Duplicate and Delete.
- **New / Edit Group**: name and default folder.
- Use `Form` with `.formStyle(.grouped)` inside `.sheet`; Return saves, Escape cancels, invalid fields block save with inline messages.

**Quick add from a folder**

- Dropping a folder onto a project (or choosing "Add from folder…") scans it for `package.json` scripts, `composer.json` scripts, Makefile targets and a Procfile.
- A sheet lists the found commands with checkboxes and editable names; selected ones are added with that folder as `cwd`.

**Keyboard shortcuts**

- Cmd+N new project, Cmd+Shift+N new command, Cmd+1…9 select command in current project, Cmd+R restart selected, Cmd+. stop selected, Cmd+D split pane.

**Menu bar extra (nice to have)**

- A menu bar icon listing projects with running counts and start/stop all.

## Milestones and acceptance criteria

Build in five milestones, each ending in something runnable; do not start a milestone until the previous one's criteria pass. M1 to M3 are the MVP; stop after M3 for review before starting M4.

1. **M1: Rust core and CLI**
   - Cargo workspace with `sb-core` (library) and `sb-cli` (binary).
   - `sb validate`, `sb list` and `sb run` work against the folder layout above.
   - Accepts: `sb run kinpay` starts dev:app, dev:dashboard and make:dev from two groups in different folders, with colored, prefixed output; `sb run kinpay backend/migrate` runs a one-off task and exits cleanly; Ctrl+C leaves no process in `ps` for any of them; a crashing command with `restart_on_crash` restarts with backoff.
   - Unit tests for config parsing (including group `cwd` resolution and unknown-key preservation) and an integration test that spawns and stops a process tree.
2. **M2: FFI bridge**
   - `sb-ffi` crate exposing the engine through UniFFI: load projects, start/stop/restart (command, group, project), subscribe to status changes and output bytes, write input, resize.
   - Build script producing an XCFramework plus generated Swift bindings.
   - Accepts: a minimal Swift test target starts a command and receives its output and status events.
3. **M3: MVP app (end of MVP)**
   - SwiftUI app with sidebar (projects, groups, commands), status dots, a single terminal pane via SwiftTerm, start/stop/restart for commands, groups and projects, and FSEvents live reload.
   - Add Project by dropping a folder or picking one; Edit Config opens `config.yaml` in VS Code; Reveal in Finder; error rows for invalid configs. No editing sheets.
   - Accepts: dropping `samples/kinpay` adds the project with its icon and three groups; launching starts the autostart commands; every one of the nine commands can be started and stopped from the UI; make:migrate shows done when it exits 0; `pnpm dev` shows full color; a Claude Code session works end to end (typing, pasting, scrolling, resizing); editing `config.yaml` in VS Code and saving updates the sidebar without stopping running commands; quitting the app leaves no processes behind.
4. **M4: Sheets and icons (post-MVP)**
   - New/Edit Project, Group and Command sheets, icon import with SVG/PDF to PNG conversion, YAML written back without losing unknown keys or comments where possible.
   - Accepts: a project created entirely in the UI produces a valid folder that `sb validate` accepts and `sb run` runs.
5. **M5: Splits and polish (post-MVP)**
   - Split panes, find bar, keyboard shortcuts, quick add from a folder's scripts, drag between groups, export, crash-leftover cleanup, menu bar extra.
   - Accepts: three panes show three live commands; dropping a Laravel or Node repo offers its scripts.

**Engineering notes for Claude Code**

- Keep all process logic in Rust; Swift never spawns processes.
- Prefer small, well-known crates: `portable-pty`, `alacritty_terminal` or `vt100`, `serde_yaml`, `notify`, `nix`, `uniffi`, `tokio`.
- Commit after each milestone with a short README section on how to build and run it.

## Open questions and out of scope

**Open questions**

- [ ] Bundle ID (app name is Switchboard; CLI binary is `sb`).
- [ ] Sandboxing: ship unsandboxed (simplest for spawning arbitrary commands) or sandboxed with security-scoped bookmarks per folder? Recommendation: unsandboxed, distributed outside the Mac App Store.
- [ ] Shared commands across projects (one Postgres used by several projects): defer, or support references in v1?
- [ ] Should the CLI also control the running app (for example `sb start kinpay` talking to it over a local socket)?
- [ ] Per-project `.env` file support alongside inline `env`.

**Out of scope for v1**

- Windows and Linux builds.
- Remote/SSH sessions and AI-agent orchestration.
- Health checks (waiting on a port before starting dependents), command dependencies, and restarting on file changes (Solo's `restart_when_changed`).
- Log persistence to disk beyond the in-memory scrollback.
