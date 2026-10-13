# Switchboard

A native Mac app that starts, watches and stops a project's dev commands from one
window, with each command running in its own folder. A project is a logical grouping
of commands, not a directory. See `handoff/PRD.md` for the full spec.

```
crates/sb-core      Rust engine: config, supervisor, PTY sessions, file watcher
crates/sb-cli       `sb` command-line front end
crates/sb-ffi       UniFFI bridge used by the Mac app
app/                SwiftUI app (Switchboard.app)
samples/kinpay      Reference project (config.yaml + icon.png)
samples/citybuild   Second sample: frontend + backend + e2e across three folders
```

Projects live in `~/Library/Application Support/Switchboard/Projects/<slug>/config.yaml`.

## Build and run the CLI (M1)

Requires a Rust toolchain (`curl https://sh.rustup.rs | sh`).

```sh
cargo build --release -p sb-cli          # binary at target/release/sb
cargo test -p sb-core                    # config + process-tree tests

sb validate samples/kinpay               # check a config.yaml
sb install samples/kinpay                # copy it into the Projects directory
sb list                                  # projects, commands, live status
sb run kinpay                            # autostart commands, prefixed output; Ctrl+C stops all
sb run kinpay backend                    # one group
sb run kinpay backend/migrate            # one command in the foreground (interactive)
sb run samples/kinpay                    # a project folder that is not installed
sb cleanup                               # kill sessions left behind by a crash
```

Commands run through `$SHELL -lc "<run>"` in their own PTY and process group. Stop
sends SIGINT to the group, SIGTERM after 3 s, SIGKILL after a further 3 s.

## FFI bridge (M2)

```sh
scripts/build-core.sh          # release XCFramework + Swift bindings (add --debug for fast builds)
scripts/swift-smoke.sh         # starts a command from Swift and checks output/status events
```

`crates/sb-ffi` exposes the engine through UniFFI proc-macros. The build script writes
`app/Frameworks/SwitchboardCore.xcframework` and `app/Generated/SwitchboardCore.swift`,
both of which the app target consumes (they are build outputs, not checked in).

## Mac app (M3, the MVP)

Requires Xcode 15+ and [xcodegen](https://github.com/yonaskolb/XcodeGen) (`brew install xcodegen`).

```sh
scripts/build-app.sh             # builds everything and writes dist/Switchboard.app
open dist/Switchboard.app
scripts/package.sh               # build, install to /Applications, relaunch (add Release or --no-build)
```

`scripts/dev/uidrive.swift` is a small helper used for automated screenshots (window
sizing, synthetic hover/click/keys, composite captures of menus and alerts).

Or open `app/Switchboard.xcodeproj` in Xcode after `scripts/build-core.sh --debug` and
`cd app && xcodegen generate`. The project file and the generated framework are build
outputs and are not checked in.

What the app does:

- **Add a project**: drop a folder containing `config.yaml` onto the sidebar, or
  File > Add Project… (⌘O). Invalid configs show their errors and nothing is copied;
  name clashes ask Replace / Keep Both. Debug builds also have File > Install Sample
  Project, which adds `samples/kinpay`.
- **Run things**: hover a command, group or project for start / stop / restart; the
  toolbar and the Command menu act on the selected command (⌘R restart, ⌘. stop,
  ⌘K clear). Status dots: gray stopped, yellow starting, green running, red crashed,
  check mark when a one-off exits 0.
- **Edit by editing the file**: Project > Edit Config (⌘E) opens `config.yaml` in VS Code
  (falls back to the default app for .yaml); Reveal in Finder sits next to it. Saving the
  file updates the sidebar within about a second without touching running sessions; a
  changed `run`/`cwd`/`env` shows a "restart to apply" hint.
- **Terminals**: each command has its own SwiftTerm pane kept alive while hidden, so
  switching back shows scrollback and the alternate screen intact. Interactive programs
  (`claude`, `vim`, `htop`) work; Option is Meta.
- **Windows**: right-click a project (or Project > Open in New Window, ⌥⌘N) to give it
  its own window showing only that project, with its own selection. A command's terminal
  lives in one window at a time; selecting it elsewhere moves it there and the previous
  window offers "Show Here" to take it back. ⌘0 brings back the all-projects window.
- **Themes**: Settings (⌘,) has Graphite, Patch Bay and Paper, each with a light and a dark
  palette, and an Appearance control (System · Light · Dark). A project can pin its own look
  for its own window with `theme: patch-bay` and `appearance: dark` in `config.yaml`
  (values: `graphite` · `patch-bay` · `paper`, `system` · `light` · `dark`); the board keeps
  the global setting. A project can also define a **custom theme**: a `theme:` block with
  an optional `name`, a `base` built-in to inherit from, and `dark:` / `light:` maps of
  token overrides (`accent`, `window`, `sidebar`, `terminal`, `running`, … plus `ansi`, a
  list of 16 colours). Unknown tokens and non-`#RRGGBB` values are config errors.
  `samples/citybuild` and `samples/kinpay` carry palettes lifted from those products'
  CSS variables. The Project rail setting switches between a rail of project tiles with
  one project in the sidebar and a full tree.
- **Links**: ⌘-hover underlines a link and ⌘-click opens it. Plain URLs, Markdown links
  (`[text](url)`, the way Claude Code prints them; clicking the text or the URL both work)
  and OSC 8 terminal hyperlinks are all supported.
- **Terminals**: ⌘T, a group's hover button or context menu, or the empty-pane picker
  opens a login shell in that group's folder. Terminals are per-device working state:
  they persist across relaunches in the project's `terminals.yaml` in Application Support
  and go away with the project. Rename one, or Save as Command… to turn it into a real
  command in its group.
- **Split panes**: Pane > Split Right (⌘D) or Split Down (⇧⌘D) shows up to four
  terminals in one window, each with a header that picks its command (or pick from the
  sidebar: selection always targets the focused pane). Drag a sidebar row onto a pane: the centre replaces that pane's command, an edge splits
  on that side. Drag the divider to resize, ⌘] cycles focus, ⇧⌘W closes a pane. The layout is remembered per window.
- **Quit** stops every session (SIGINT → SIGTERM → SIGKILL per process group) before
  exiting. On the next launch, sessions left behind by a crash are offered for cleanup.

## Versioning a project with its repos (`.switchboard/`)

A project spans folders, so its settings are split the same way and composed.

```
~/code/acme-app/.switchboard/
  commands.yaml     # this folder's commands (cwd is the folder)
  project.yaml      # the project: name, icon, theme, and its member folders
  icon.png
~/code/acme-api/.switchboard/
  commands.yaml
```

```yaml
# acme-app/.switchboard/commands.yaml
name: Web
commands:
  - { id: dev, run: npm run dev }
  - { id: storybook, run: npm run storybook, autostart: false }
```

```yaml
# acme-app/.switchboard/project.yaml   (the "home" folder)
version: 1
name: Acme
groups:
  - id: web
    from: .                 # this folder's commands.yaml
  - id: api
    from: ../acme-api       # a sibling repo; relative to the home folder
    overrides:
      migrate: { hidden: true }
      dev: { autostart: true, env: { PORT: "8080" } }
```

- **A group with `from:`** takes its commands from that folder's `commands.yaml`. `overrides`
  tweak them per command (`name`, `autostart`, `restart_on_crash`, `env`, `hidden`) without
  touching the other repo. Inline `commands:` still work and are appended. A missing folder
  or fragment marks that one group unavailable; the rest of the project loads.
- **New device**: drop any member folder on the sidebar or the Dock icon (or
  `sb import <folder>`). Switchboard finds the `project.yaml`, in that folder or in a sibling
  repo that lists it, shows which folders resolved, and installs the project. If several
  copies exist, the newest is proposed and the others can be chosen.
- **Write-back**: the live config stays in Application Support. Once a project has a home
  folder, every change is mirrored to that folder's `project.yaml`, so git always has the
  current settings. If the repo's copy changes underneath (a pull), the sidebar shows
  "Settings changed in the repo · Apply" instead of overwriting anything.
- **Existing projects** get a one-time offer to keep their settings in a repo folder
  (Project › Keep Settings in Repo… does it later).
- Fragments are watched: editing a repo's `commands.yaml` updates the sidebar live.
- `sb validate <repo folder>` checks a `project.yaml` and names every fragment it reads.
