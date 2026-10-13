# Agent guide: build, test and iterate fast

For an AI agent (or a person) starting a fresh session on this repo. It covers the loops
that are quick, the tools that exist for driving the app, and the traps that have already
cost time. Product behaviour is in [the README](../README.md), [projects](projects.md) and
[theming](theming.md).

## The 30-second map

| Layer | Where | Language | Owns |
|---|---|---|---|
| Engine | `crates/sb-core` | Rust | config, fragments, sessions (PTY + process group), supervisor, watcher, terminals |
| CLI | `crates/sb-cli` | Rust | `sb` |
| Bridge | `crates/sb-ffi` | Rust (UniFFI proc-macros) | records and the `Engine` object exposed to Swift |
| App | `app/Switchboard` | SwiftUI + AppKit | rendering, windows, panes, SwiftTerm views |

**Rule that shapes everything:** process logic lives in Rust. Swift never spawns a process
or derives state by inspecting one. If the UI needs a value, add it to the engine and
expose it through `sb-ffi`.

## Loops, fastest first

```sh
cargo test -p sb-core                       # ~10 s, 25 tests, spawns real processes
cargo test -p sb-core --test fragment_tests # one suite
cargo build -p sb-cli && target/debug/sb validate samples/kinpay

scripts/build-core.sh --debug               # Rust → XCFramework + Swift bindings (after any sb-ffi change)
scripts/package.sh                          # full build, sign, install to /Applications, relaunch
scripts/package.sh --no-build               # reinstall dist/ and relaunch only
```

Swift-only change, quickest compile check (skips signing and install):

```sh
cd app && xcodebuild -project Switchboard.xcodeproj -scheme Switchboard -configuration Debug \
  -destination 'platform=macOS' -derivedDataPath build/DerivedData \
  -skipPackagePluginValidation -skipMacroValidation build 2>&1 \
  | grep -E "^/Users.*error:|BUILD \w+" | grep -v buildCommands | sort -u
```

Prefer proving behaviour in a Rust test or with `sb` before touching the app. The engine is
fully testable without a UI, and the CLI exercises the same code the app does.

### Things that will bite

- **Added a Swift file?** Run `cd app && xcodegen generate` first. The Xcode project is
  generated and git-ignored, so a new file is invisible until you do.
- **Changed `sb-ffi`?** Run `scripts/build-core.sh --debug` before building the app, or
  Swift compiles against stale bindings. `scripts/package.sh` does this for you.
- **Do not `import SwitchboardCore`.** The generated bindings compile into the app target.
- **`xcodebuild` output is enormous.** Always filter it as above. The `buildCommands`
  exclusion drops a multi-kilobyte SwiftTerm plugin line.
- **`-skipPackagePluginValidation` is required** for SwiftTerm's build plugin.

## Driving the app for real

`scripts/dev/uidrive.swift` is a small UI driver. Compile it once and chain commands in one
call so the app stays frontmost between steps:

```sh
swiftc -O -o /tmp/uidrive scripts/dev/uidrive.swift
/tmp/uidrive activate resize 1280 800 sleep 500 click 266 238 sleep 800 shot /tmp/a.png
```

| Command | Use |
|---|---|
| `activate`, `resize W H`, `window` | bring to front, size and place, print frame |
| `move X Y`, `click X Y`, `drag X1 Y1 X2 Y2` | pointer, in screen points from the top-left |
| `cmdmove`, `cmdclick` | the same with ⌘ held (link hover and click) |
| `key CODE [cmd shift ctrl alt]`, `type TEXT` | keys by virtual key code |
| `shot PATH` | the frontmost Switchboard window only |
| `shotapp PATH` | every Switchboard window composited (sheets, dialogs, child windows) |
| `region PATH` | the screen rectangle of the window, as the user sees it |

`resize 1280 800` puts the window at screen origin (116, 60) on a 1512 pt wide display, so
a window point (x, y) is screen (116 + x, 60 + y). Take a screenshot, read a coordinate off
it, scale by the image's downsizing factor, and click. **Look at the screenshot before you
trust a click.** Layouts shift when rows are added.

Menus are the most reliable way to trigger actions:

```sh
osascript -e 'tell application "System Events" to tell process "Switchboard" to click menu item "Start All in Project" of menu "Command" of menu bar 1'
```

### Automation limits found the hard way

- **SwiftUI context menus cannot be opened** by synthetic right-clicks or `AXShowMenu`.
  Every context-menu action also has a menu-bar or hover equivalent; use that.
- **Synthetic text into a terminal:** `uidrive type` does not reach SwiftTerm reliably. Use
  `osascript -e 'tell application "System Events" to keystroke "…"'`.
- **The open panel's Go to Folder is flaky.** To add a project, run
  `open -a /Applications/Switchboard.app <folder>` instead.
- **A real ⌘ must be held for link hover:** `osascript … key down command`, then `cmdmove`.
- Screen capture and control need Screen Recording and Accessibility permission for the
  terminal running the script.

### Putting the app in a known state

Settings live in `defaults` under `dev.waky.switchboard`. Set them before launching.

| Key | Values |
|---|---|
| `appearance.theme` | `graphite` · `patchBay` · `paper` |
| `appearance.mode` | `system` · `light` · `dark` |
| `appearance.rail` | bool; rail of project tiles vs one tree |
| `rail.main` | slug shown in the sidebar when the rail is on |
| `layout.<scope>`, `focus.<scope>` | pane tree and focused pane; delete to reset. Scope is `main` or `project.<slug>` |
| `reopenProjectWindows` | project windows to reopen at launch; delete for a clean board |
| `homeOffer.dismissed.<slug>` | hides the "keep settings in the repo" offer |
| `traceLinks` | bool; logs link hover hits |

Data on disk, all under `~/Library/Application Support/Switchboard/`:

- `Projects/<slug>/config.yaml`, `icon.png`, `terminals.yaml`
- `sessions.json`: live sessions, for crash cleanup and `sb list`

Logs: `~/Library/Logs/Switchboard/links.log` records every URL the app opens, which makes
link behaviour assertable.

For isolated runs, point the engine elsewhere: `SWITCHBOARD_HOME=/tmp/x` moves app support,
`SB_PROJECTS_DIR=/tmp/x/Projects` moves the CLI's projects directory.

## Fixtures

- `samples/kinpay`, `samples/citybuild`: real projects on the author's Mac, with custom
  themes. CityBuild's ports (3006, 8005) are usually free. Kinpay's (3000, 3001, 8000) are
  often already taken by servers running outside Switchboard, so "port in use" there is not
  a Switchboard bug.
- `samples/states-demo`: one command per status (running, crash loop, done, missing folder,
  never run). Use it for anything about status display.
- Throwaway projects: write a `config.yaml` with `echo`/`sleep` commands into a temp folder.
  It is faster and safer than real servers.

**Never run, without being asked:** `make migrate`, `make seed`, `make flush`,
`make flush-loans` in the sample projects. They change the author's real databases.
**Never show in screenshots or video:** a directory listing of `citybuild/backend`, which
contains credential filenames.

## Recording media

```sh
scripts/walkthrough/record.py --dry     # run the beats, screenshot each into target/walkthrough/
scripts/walkthrough/record.py           # narrated video → docs/media/walkthrough.mp4 + .srt
scripts/walkthrough/theme_shots.py      # docs/media/themes/*.png
```

Narration is local Kokoro (`~/.kokoro`, voice `am_michael`), the same setup as the author's
other projects. After recording, build a contact sheet and read it for dead air:

```sh
ffmpeg -v error -y -i docs/media/walkthrough.mp4 -vf "fps=1/4,scale=480:-1,tile=5x4" -frames:v 1 /tmp/sheet.png
```

Recording has repeatedly exposed real bugs. Treat an odd frame as a product defect until
shown otherwise.

## SwiftTerm notes

Pinned by commit in `app/project.yml`. Read the checkout in
`app/build/DerivedData/SourcePackages/checkouts/SwiftTerm` rather than guessing its API.

- It renders with a **Metal layer**. A subview placed over a terminal does not show; the
  link underline is a click-through child window for that reason.
- `TerminalViewDelegate` is `@MainActor` and several methods have no default
  implementation. A SwiftTerm bump can break `TerminalHost` conformance.
- It learns ⌘ is down from `flagsChanged`, which AppKit sends only to the first responder.
  `LinkHandling` forwards it to the terminal under the pointer.
- It restores the cursor on every `ESC[?1049l`. Never emit that unconditionally. The engine
  sends mode resets on exit only for modes the program left on.
- `feed(byteArray:)` is safe from any thread. Output goes straight from engine threads to
  the view, bypassing the main-actor store.
- One `TerminalView` per command lives in `TerminalRegistry` for the app's lifetime, and an
  `NSView` can be in one place only. That is why a command shows in one pane at a time, and
  why panes claim terminals with a per-view token.
- An overlay above a terminal must not hit-test, or the terminal stops receiving clicks.

## Engine notes

- A session is `$SHELL -lc "<run>"` in its own PTY and process group. Terminals are
  `$SHELL -l` and are stopped with SIGHUP, because interactive shells ignore SIGINT and
  SIGTERM.
- A user-requested stop reports `Stopped`, however the process died. `Crashed` means it
  died on its own.
- `Project.config` is the file as written. `Project.effective` has `from:` fragments
  expanded and terminals merged in, and is what runs. Read `effective` for behaviour and
  write `config` back.
- The watcher follows the Projects directory, each fragment folder and each home
  `.switchboard` folder. `reload()` never touches running sessions.
- Relative `from:` paths resolve from the **home folder**: `.` is the home, `../api` a
  sibling repo. Resolved paths are normalized, and seeds compare by resolved path.

## Signing and permissions

`scripts/build-app.sh` re-signs the app with the first "Apple Development" certificate in
the keychain. That keeps the app's identity stable, so macOS remembers folder permissions
across rebuilds. Without a certificate it signs ad hoc and macOS re-asks after every build.
Do not pass a signing identity to `xcodebuild` on the command line: it also applies to the
SwiftTerm package's build tools and fails.

## Conventions

- Commit after each coherent step, with a message that says what changed and why.
- Keep generated things out of git: `target/`, `dist/`, `app/build/`, `app/Generated/`,
  `app/Frameworks/`, `app/Switchboard.xcodeproj`.
- Run a secret scan before pushing, and check media for private names.
- Design reference: the handoff in `~/Downloads/switchboard-design-handoff` (`DESIGN.md`,
  `Theme.swift`, mockups). Tokens in `app/Switchboard/Theme.swift` came from it.

## Not built yet

Editing sheets for projects and commands, icon import, find bar, quick-add from a folder's
scripts, export, the menu bar extra, extraction and return animations, ⌘⌥-arrow pane focus,
a divider drag readout, and splitting an existing project's inline commands into per-folder
`commands.yaml` files.
