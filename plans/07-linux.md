# 7. Switchboard on Linux (Omarchy)

**Status: In progress.** Overlays, `serve`, `attach`, `herd` and a first `switchboard ui` are
built (uncommitted), with split panes; Herdr is closed. **Next step:** the author's look,
then the rest of the Mac window (step 4) and a keybind.

## What it is

The same projects, config and agent CLI on the author's Omarchy laptop (Arch + Hyprland), next
to the Mac app. The `.switchboard/` configs in the product repos are shared by both machines, so
nothing done for Linux may change what the Mac reads or what gets pushed.

## What we learned (2026-10-29)

- **The engine needs no port.** `cargo test -p sb-core` passes unchanged on Linux, including the
  PTY and process-group tests. `dirs::data_dir()` puts the app folder at
  `~/.local/share/Switchboard`. Only the SwiftUI app (about 7.5k lines) is Mac-only.
- **The CLI assumes the app.** `start`, `stop`, `restart`, `logs`, `wait` and `status` talk to
  the control socket the Mac app hosts. On Linux nothing hosts it; only `switchboard run`
  (foreground, in the current terminal) works without it. Nothing streams a command's live
  terminal to another process.
- **Herdr ships with Omarchy 4** (`/usr/bin/herdr`, Apache-2.0, Rust). It's a terminal workspace
  manager: workspaces, tabs and panes in a background server, a CLI and socket API, agent status
  (working, blocked, idle), and its own plugins. Omarchy binds it to `Super+Ctrl+Enter` and
  scripts layouts with it (`/usr/share/omarchy/default/bash/fns/herdr`). It has no notion of
  autostart, crash restarts, or a dev server's running, done or crashed state.
  [yordanbuilds/rig](https://github.com/yordanbuilds/rig) already turns a JSON stack into Herdr
  tabs and panes, which proves the layout half.
- **The real configs are richer than the samples.** CityBuild, Kinpay and Otesha (in
  `~/Work`) use autostart, `restart_on_crash`, `env`, per-command folders, groups and custom
  themes. Their Mac-only parts are absolute `~/Documents/web/...` and `/Users/waky/...` paths,
  `source ~/.nvm/nvm.sh` on Node commands, Xcode builds and an Android Studio `JAVA_HOME`.
- **Folder names follow the configs.** Local folders were named after each config's relative
  `cwd:` paths (and the Mac's layout), not the GitHub repo names, so shared configs and e2e
  scripts work unchanged. For example: `otesha/admin-panel` is `otesha-admin`, and
  `kinpay-circles/backend` is `circles-engine`.
- **Scripts hard-code Mac paths too.** `citybuild/e2e/start.sh` sets
  `ROOT=/Users/waky/Documents/web/citybuild` and uses `osascript`. Overlays can't reach scripts;
  those need a portable fix in the repo (`ROOT="$(cd "$(dirname "$0")/.." && pwd)"`), which is the
  author's call.

## Decisions

- **No GTK or Qt app (2026-10-29).** Herdr is the window and pane layer; `sb-core` keeps
  supervision, config and the CLI.
- **The engine owns the processes, not Herdr (2026-10-29, "Option B").** Herdr panes show
  commands the engine runs, so crash restarts, status and the agent CLI keep working. The
  rejected "Option A" had `switchboard herd` run each command straight in a Herdr pane: less
  work, but it loses the supervision and the agent side that make Switchboard worth having.
- **Linux differences go in overlays, never in the shared `project.yaml` (2026-10-29).** The
  Linux overlays are committed to the product repos (2026-10-30: `citybuild/e2e`, `kinpay`,
  `otesha/e2e`), so any Linux machine with this folder layout gets them.
- **The window is our own terminal app, not Herdr (2026-10-30).** Seen in Herdr, the author
  missed the Mac's grouping. Herdr can't show it: its sidebar lists workspaces, tabs and agents
  (rows customizable, content not), stopped commands can't appear (no pane, no row), and plugins
  only run commands ("native non-terminal plugin UI are not part of plugin v1"). Of the three
  ways forward (a sidebar program inside Herdr, a full-screen terminal app, a GTK4 + VTE
  window), the author chose the terminal app. `herd` was removed on 2026-10-30.

## Done

### Platform overlays

`crates/sb-core/src/overlay.rs`, applied in `ProjectConfig::compose`
(`crates/sb-core/src/fragment.rs`). Tests: `crates/sb-core/tests/overlay_tests.rs`.

- `.switchboard/project.<os>.yaml` (`linux`, `macos`), then `.switchboard/project.local.yaml`,
  over `project.yaml`, keyed by group and command id. Later wins.
- A patch can set `run`, `cwd`, `name`, `autostart`, `restart_on_crash` and `env` (merged), hide
  a command or group (`hidden: true`), repoint a group's `cwd` or `from`, and add commands or
  groups. Relative paths resolve from the home folder.
- Applied at compose time and never saved: write-back and engine edits work from the raw
  config, so overlays can't reach `project.yaml` (tested).
- A broken overlay is a problem on the project, not a failure; unknown keys are rejected, so
  typos surface.
- `switchboard validate` lists the overlays applied, and now resolves a seed's relative paths
  from its home folder rather than the shell's current folder (it reported false missing
  folders before).
- Linux overlays written for CityBuild (`citybuild/e2e`), Kinpay (`kinpay`) and
  Otesha (`otesha/e2e`). Otesha validates clean; the others only miss repos not cloned yet.
  Node commands use `mise x node@$(cat .nvmrc) -- …`, which relates to "Honour `.nvmrc`" in
  [PLAN.md](../PLAN.md#not-scheduled).

### Machine setup

Repos cloned to `~/Work`, `.env` files and Firebase credentials copied from the Mac (all
git-ignored or outside a repo), Rust, uv and magic-wormhole installed through mise.

**Databases without Docker or sudo (2026-10-29).** `mise use -g conda:postgis@3.4` brings
PostgreSQL 16.10 with PostGIS 3.4.3 in one conda prefix (the version pair Otesha's README asks
for; pacman only has 18 + 3.6). mise exposes only PostGIS's own binaries, so `postgres`,
`initdb`, `pg_ctl`, `pg_isready`, `createdb` and `createuser` are linked into `~/.local/bin`.
Data is in `~/.local/share/postgres/16` (port 5432, localhost, trust auth, sockets in `/tmp`),
with the `otesha` role and database from the backend's `.env` and `postgis` enabled. Redis 7
comes from `mise use -g redis@7`.

**A local Services project** (`~/.local/share/Switchboard/Projects/services/config.yaml`, not in
any repo) runs both, with crash restarts. One Postgres and one Redis serve every project, so
they live outside the product configs where two projects would fight over the ports.
`switchboard run services` starts both and Ctrl+C stops both cleanly.

**Otesha is installed:** backend `.venv` (Python 3.12 via `uv sync --all-extras`), and
`npm ci` in admin-panel, customer-app, corporate-portal, e2e and website (Node 25). The local
database is migrated (`alembic upgrade head`, to 0081) and seeded (`seed_reference.py`,
`seed_auth.py`); the API starts on :8030 and `/health` answers. Local databases need no sign-off:
migrate, seed and flush them freely in every project (the author, 2026-10-29).

**CityBuild and Kinpay are installed and herded (2026-10-29).** Venvs with uv (Python 3.12),
`npm ci` and `bun install` everywhere, databases `citybuild_db` and `kinpay` (owned by a
`kinpay` role; the monolith and the circles engine share it by design, circles in its own
schemas with its own `alembic_version`), all migrated, CityBuild and circles seeded, the Kinpay
superadmin created. CityBuild `mobile` is cloned to `citybuild/mobile/app` (the repo root is
the app, as on the Mac); `Kinimals` to `~/Work/kinimals`, pointed at by Kinpay's overlay
rather than nested inside the kinpay repo where it would be untracked. Kinpay's overlay runs
`npm run dev:app`/`dev:dashboard` (no yarn here; the repo is an npm workspace). All 13
autostart commands across the three projects run; backends answer `/health`.

**Traps hit:** CityBuild's `requirements.txt` lists the backend itself as an editable git URL
(a `pip freeze` artifact) that uv refuses: install the rest, then `uv pip install -e .`.
Parallel `npm ci` runs crash npm ("Exit handler never called"); run them one at a time. Vite
dev servers listen on `[::1]` only, so `curl 127.0.0.1:<port>` fails while browsers on
`localhost` work.

**Trap: `sharp` and the system libvips.** Omarchy ships `libvips`, so sharp's install check
tries to build from source and `npm ci` fails ("Please add node-addon-api"). The prebuilt binary
works; `SHARP_IGNORE_GLOBAL_LIBVIPS=1` is set in the global mise `[env]` so every repo installs.

## The steps

### 1. `switchboard serve` (done 2026-10-29)

A headless host for the engine and its control socket: what the Mac app does, without the UI.

- `switchboard serve` (`crates/sb-cli/src/main.rs`) builds the engine, kills sessions a dead host
  left behind (unattended, there's no one to ask as the app does), starts the file watcher and
  serves the control socket, in the foreground. Ctrl+C, SIGTERM or SIGHUP stop answering, then
  stop everything it started, process group by process group.
- On Linux, commands that launched the app (`start`, `restart`, `wait`…) launch the host
  instead (`launch_host` in `crates/sb-cli/src/app.rs`): `systemctl --user start
  switchboard.service` when that unit is installed, else `switchboard serve` in its own process
  group with a clean environment, logging to `~/.local/share/Switchboard/serve.log`.
- The binary is installed with `cargo install --path crates/sb-cli --locked` (to
  `~/.cargo/bin`, on the login shell's PATH).
- **Proved on Otesha:** `start services` launched the host by itself; `start otesha` ran the six
  autostart commands; `wait otesha/backend/api` reported ready at :8030; `status` showed every
  URL; `logs` read the worker and website; `kill -9` on uvicorn crash-restarted it; `stop otesha`
  left no process and no open port, including the Node servers behind `mise x`. SIGTERM to
  `serve` stopped Postgres and Redis cleanly before it exited.

Found and fixed on the way (tests for each):

- **No `/bin/zsh` on Linux.** `session::login_shell` fell back to it when `$SHELL` is unset, which
  it is under systemd or a cleaned environment, so every command would have failed. It now uses
  the account's shell from the user database first (`crates/sb-core/tests/shell_tests.rs`).
- **`import` then `start` raced the watcher.** A request naming a project the engine doesn't know
  now reloads the Projects folder once before answering "unknown project". This affects the Mac
  app too (`control_tests.rs`).
- **`wait` gave up during a crash restart.** It saw "crashed, restarting in 1 s" and reported
  failure. The status reply now carries `restarting`, and `wait` for ready or running waits
  through a scheduled restart (`crates/sb-cli/tests/restart_tests.rs`).

**Not done, the author's call:** a systemd user unit to start `serve` at login. Writing and
enabling it was blocked as a persistent change. Without it, the first `switchboard start` after
login launches the host itself, and nothing restarts it if it dies. A unit would be:

```ini
# ~/.config/systemd/user/switchboard.service
[Unit]
Description=Switchboard engine (switchboard serve)

[Service]
ExecStart=%h/.cargo/bin/switchboard serve
# serve stops every command's process group itself on SIGTERM; only then kill what is left.
KillMode=mixed
TimeoutStopSec=30
Restart=on-failure

[Install]
WantedBy=default.target
```

### 2. `switchboard attach <command>` (built 2026-10-29; feel check pending)

A command's live terminal in the current terminal: output, keystrokes and resize, through the
engine.

- Protocol (`crates/sb-core/src/control.rs`): an `Attach { project, address, cols, rows }`
  request takes over its connection. After the reply line, the server sends the command's
  `raw_tail` (what it printed so far, as the Mac app replays it), then live output as raw bytes.
  The client sends frames: a kind byte (`FRAME_INPUT`, `FRAME_RESIZE`), a big-endian `u32`
  length, then the payload. The engine's listener only queues output to a writer thread, so a
  slow viewer can't hold up the engine.
- It follows the command across restarts (matched by command, not session) and refits each new
  session to the viewer's size. Between runs it prints a dim note ("crashed, restarting in
  1 s", "stopped · Enter starts it"). Enter on a stopped command starts it.
- Client (`crates/sb-cli/src/app.rs`): raw mode, keystrokes become input frames, a 200 ms
  size poll sends resize frames, the window title becomes `project · command`, and Ctrl+]
  detaches (the command keeps running).
- Test: `attach_streams_a_terminal_both_ways_and_survives_a_detach` in `control_tests.rs`.
- **Gate, still open:** the author uses it in Herdr panes (colours, resize, Ctrl+C, scrollback,
  an interactive prompt) and says whether it feels native.
- Known: anything typed into an attached pane goes to the command, including text pasted or
  typed by mistake. (Seen once when a `herdr pane run` was sent twice to one pane.)

### 3. `switchboard herd <project>` (built 2026-10-29, removed 2026-10-30)

Removed once the window became our own terminal app (step 4): nothing used it. It lived in
`crates/sb-cli/src/herd.rs` (see git history). It started the project's autostart commands, then built a Herdr
workspace named after the project: a tab per group that has autostart commands, panes in a
grid (ceil(sqrt n) columns, even shares, Omarchy's `hsl` rule), each running
`switchboard attach`. If Herdr's server isn't running, it opens `omarchy-launch-terminal-herdr`.
Re-running it focuses the existing workspace. Groups with no autostart commands get no tab.

- **Seen working:** `switchboard herd otesha` made 5 tabs (Backend with uvicorn and the
  worker, Console, Customer, Corporate, Website) and 6 attached panes.
- `herdr pane run` prints nothing on success; every other call returns JSON with `result`.
- **Seen working for all three:** CityBuild (Frontend, Backend, Mobile/Metro) and Kinpay
  (Backend, Frontend with app and dashboard, Kinimals) next to Otesha.
- **Left:** a keybind or Omarchy menu entry (possibly an Omarchy plugin), and what to do with
  groups that have no autostart commands (e2e, the circles groups, Claude).


### 4. `switchboard ui` (first screen built 2026-10-30)

The Mac window in a terminal (`crates/sb-cli/src/ui/`): ratatui over crossterm, a client of
the host, so quitting stops nothing.

- **Rail:** a tile per project (initials on the project's `color`), the current one marked,
  a running count under each. **Sidebar:** name, Start all, "N of M running · 1 pane",
  ungrouped commands, collapsible group headers, every command with its status glyph
  (● running, ◐ starting or restarting, ● red crashed, ▲ error, ✓ finished, ○ stopped) and
  uptime, the selection drawn as the accent bar; footer with the running count and Stop all.
  **Pane:** bar with group / command, status and uptime, ■/▶ and ↻; the live terminal; footer
  with folder, pid, uptime and key hints.
- **Colours:** Graphite dark from `Theme.swift`, the project's custom dark tokens and ANSI
  palette on top (`ui/theme.rs`), so each project looks like its Mac window.
- **Terminal:** an `Attach` connection per shown command feeding a client-side `vt100` parser
  (5000 lines of scrollback) drawn cell by cell; keys re-encoded to xterm bytes; paste;
  resize follows the pane.
- **Input:** ↑↓/jk select (selection shows that command), Enter or Tab types into it, Ctrl+]
  back to the list, s start/stop, r restart, a Start all, X Stop all, [ ] or 1–9 switch project,
  PgUp/PgDn or the wheel scroll back, q quits; clicks on tiles, rows, headers, buttons.
- **Checked headless:** rendered through a PTY into `pyte` at the window's size: layout,
  stopped-command note, project switch with its theme, first visit landing on a running
  command. Fixed on the way: attaching before the first draw sized the command's terminal to a
  20×5 placeholder; footer text overlapping the hints.
- **Split panes (2026-10-30):** up to four, from any project, laid out as one, two (side by
  side or stacked), three (one left, two right) or a 2×2 grid. `|` splits right and `-` down
  with the selected command, `x` or the pane's ✕ closes (the command keeps running), Tab
  cycles, a click focuses. Selecting a command already in a pane focuses that pane; otherwise
  it replaces the focused pane's command. Pane bars name the project, sidebar rows carry their
  pane number, and each pane wears its own project's colours. The layout is saved in
  `ui-layout.json` in the app folder and reopened at launch.
- **Ported from the Mac feature list (2026-10-30):**
  - Terminals: `t` opens a login shell in the selected group's folder (kept per machine in
    the project's `terminals.yaml`, as the Mac does), `R` renames it in a prompt in the
    sidebar's foot, `D` closes it. New control requests `NewTerminal`, `CloseTerminal`,
    `RenameTerminal` (test: `terminals_open_rename_and_close_through_the_socket`).
  - Groups start collapsed, show "N running" or "N idle", open when one of their commands is
    selected or shown, and are remembered in `ui-layout.json`. A project never seen opens the
    groups with something running. ← closes a group or goes up to it.
  - Rows show the listening port (click opens `http://localhost:<port>`, `o` too), an
    "apply" badge when a running command's config changed, the crash-restart countdown, and,
    on hover, ▶ or ■ ↻ (and ▶ ■ for a group).
  - Never-run card: run line, folder, autostart and a Start button in place of an empty pane.
  - Clear (`c`) wipes the focused pane's screen and scrollback; the command is untouched.
  - Links: clicking an `http(s)://` URL in a pane opens it with `xdg-open`.
  - Footer exit summary ("exited 1 after 0:42", "stopped after 3:10").
  - A "started HH:MM:SS" separator when a command starts (server side, in the attach
    stream). Only viewers attached at that moment see it; it isn't in the replayed history.
  - Broken configs: the rail tile turns red with ▲ and the sidebar lists the errors.
  - Resizable sidebar (drag the divider, 28 to 72 columns, remembered).
  - Live reload: a saved `config.yaml` or terminal list shows within about a second.
  - `?` lists the less common keys in the footer.
- **Checked headless:** counts and ports on rows and in the pane bar, a collapsed idle
  group, the never-run card, `t` opening a shell in the group's folder and typing into it,
  `D` closing it and the window falling back to a running command.
- **Not ported:** drag to reorder, image previews, OSC 8 hyperlinks, repo notices (Keep
  Settings in Repo, "settings changed · Apply"), adding and deleting projects from the
  window, group icon picking, save terminal as command, light themes, a project in its own
  window (a second `switchboard ui` shows everything), and Settings. The Omarchy menu entry and Super+B
  (`omarchy-launch-or-focus-tui ~/.cargo/bin/switchboard ui`, in `~/.config/omarchy/extensions/`
  and `~/.config/hypr/bindings.lua`) open it, sized by a window rule in `~/.config/hypr/hyprland.lua`.

## Open

- Enable the systemd user unit for `serve`: the author agreed (2026-10-30), but enabling a
  login service is blocked for the agent. The unit is `scripts/linux/switchboard.service`;
  `systemctl --user enable ~/Work/switchboard/scripts/linux/switchboard.service`.
- Make `citybuild/e2e/start.sh` portable (relative `ROOT`, `osascript` behind a Darwin check)?
- Theming: map a project's custom theme onto Herdr, or follow Omarchy's current theme?
- The Mac build is untested after these changes (shared engine code and the CLI's macOS
  launch path moved behind `cfg`). Run `scripts/package.sh` on the Mac before relying on it.
- Repos not cloned yet that configs reference: CityBuild `mobile`, Kinpay `Kinimals`,
  `kinpay-circles/mobile` and `mobile-designs`, `kinpay/frame-studio-simple`.
