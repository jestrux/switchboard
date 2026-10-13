# Switchboard — design handoff for Claude Code

You are implementing a redesign of the Switchboard Mac app (repo: `~/Documents/web/switchboard`).
Read this file first, then `DESIGN.md` (the spec) and `Theme.swift` (the tokens).

## What Switchboard is (one paragraph)

A native Mac app (SwiftUI + AppKit, macOS 14+) that starts, watches and stops a project's dev commands.
A project is a logical group of commands across repos (not a directory), defined in
`~/Library/Application Support/Switchboard/Projects/<slug>/config.yaml`. Each command runs in a real
PTY rendered by **SwiftTerm**. The process engine is **Rust behind UniFFI**; the UI only renders engine
state and sends start/stop/input/resize. Today the UI is stock macOS chrome; this handoff replaces it
with designed chrome while keeping behaviour native.

## Hard rules

1. **Keep the information architecture**: projects › groups › commands in the sidebar, terminal on the right.
2. **Keep the six states and their meanings**: stopped · starting · running · done (exit 0) · crashed (with
   backoff, "restarting in N s") · error (e.g. folder missing). Plus the "restart to apply" variant.
3. **SwiftUI/AppKit + SwiftTerm only.** No web views. You own SwiftTerm's colours, font, padding and
   surroundings, not its text layout.
4. **No process logic in the UI.** Everything shown comes from engine events. If the design needs a value the
   engine doesn't expose (see "Engine data" below), add it to the engine API or leave it out; never derive
   it in Swift by poking at processes.
5. **Keyboard first.** Every hover action has a menu item and, where listed, a shortcut.
6. **Light + dark in every theme**, following the system by default.
7. **Don't break hand-offs**: drop a folder to add a project, Edit Config → VS Code, Reveal in Finder,
   Replace / Keep Both, leftover-sessions prompt.

## What's in this folder

| Path | What it is |
|---|---|
| `HANDOFF.md` | This file: scope, build order, file map, acceptance criteria |
| `DESIGN.md` | The design spec: themes, layout, components, states, splits, windows, dialogs, keys, tokens |
| `Theme.swift` | Generated tokens: 3 themes × light/dark palettes, ANSI palettes, style flags, metrics, type, motion, status colours, environment key |
| `icon/` | App icon (default, light, tinted) as 1024 PNG + SVG, menu-bar template 18/36 px + SVG |
| `mockups/` | Source of the design canvas boards (`.dc.html`). Reference only; read them for exact structure, copy, sizes and colours. They need the canvas runtime to render, so read the markup and the `renderVals()` data rather than opening them in a browser. |
| `brief/` | The original brief and shared context the design answers |

Canvas of record (visual reference, ask Walter for access if you can't open it):
https://claude.ai/code/artifact/3aa59f17-2194-4164-85f6-956ec5580896

### Mockup index

| File | Shows |
|---|---|
| `Shell.dc.html` | **The main reference.** One component rendering every theme × mode × state with rail on/off. Its `renderVals()` holds the full token table and the per-state header, footer and terminal content. |
| `M-*.dc.html` | Shell in each theme and mode, one state each |
| `Settings.dc.html` | Settings › Appearance (Theme, Appearance, Project rail) |
| `Tokens.dc.html` | Token sheet |
| `Pane.dc.html` | **The pane component** (bar + terminal + footer), every state, full/mid/min widths, focus/hover, empty picker, placeholder |
| `Win.dc.html` | **The window component**: board vs project window, split layouts 1–4, divider/drop/refuse overlays, maximize, empty board |
| `X-PaneSpec.dc.html` | Pane state × width matrix |
| `X-SplitCtl.dc.html` | View menu, divider states, picker, swap menu, maximize, limits |
| `X-Split*.dc.html`, `S-*.dc.html` | Split layouts in every theme |
| `X-WinMain`, `X-WinProject`, `X-WinEmpty`, `X-WinHeader`, `X-Extract`, `X-WinStates` | Project windows (extraction model) |
| `Dialog.dc.html`, `X-Dialogs*.dc.html` | The four dialogs |
| `X-Icon.dc.html` | Icon presentation |
| `Main`, `A-/B-/C-`, `X-Readback`, `X-Headers` | Early exploration and decision records; not specs |

## Decisions already made (don't relitigate)

- Three themes (Graphite default, Patch Bay, Paper), each light + dark; Theme and Appearance are separate settings.
- **Project rail** is a setting, on by default. Rail on: rail of project tiles + sidebar showing one project.
  Rail off: full tree. A 1 pt `line` divider always separates rail and sidebar.
- **Pane header option A**: one 36 pt bar for single and split panes. The old toolbar strip goes away; the
  window keeps a hidden title bar with traffic lights inside our chrome.
- **Pane bar shows a project icon only** when the command belongs to a different project than the window.
- **Windows use extraction**: a project is either on the board (main window) or in its own window, never both.
  "Move to Board" exists in the project window's ⋯ menu and the Window menu, in addition to closing the window.
- **Splits**: up to 4 panes, minimum 320 × 180 pt, layout stored per project in app state (not config.yaml).
  Picking a command that's in another pane swaps the two panes.
- **Keys**: ⌘D split right · ⌘⇧D split down · ⌘⇧W close pane (⌘W still closes the window) · ⌘⇧↩ maximize ·
  ⌘[ / ⌘] previous/next pane · ⌘⌥ arrows move focus · ⌘⇧N open project in new window. Existing: ⌘O ⌘R ⌘. ⌘K ⌘E.
- **Dialogs**: custom cards presented as sheets; destructive buttons are never default. Name conflict defaults
  to **Keep Both** (changed from Replace).

## Build order

Do these as separate branches/PRs (the repo's working agreement is one module per PR). Each step should leave
the app shippable.

1. **Tokens + theme plumbing.** Add `Theme.swift` (`app/Switchboard/Theme.swift`). Add settings storage
   (`@AppStorage`) for theme (`SBThemeID`, default `.graphite`), appearance (`SBAppearance`, default `.system`)
   and project rail (default `true`). Inject `\.sbTheme` at the root; apply `.preferredColorScheme`. Point
   `TerminalRegistry.swift` at `palette.terminal`, `terminalText`, and `ansi` per scheme, font SF Mono 12.5,
   insets 10/14. Add Settings › Appearance (`Settings.dc.html`). Replace the colour helpers in `Models.swift`.
2. **Status glyph.** Rewrite `StatusDot` as `StatusGlyph(status:)` with the six shapes, sizes and motion in
   DESIGN.md (Canvas or Shape). Breathing/spinning respect Reduce Motion. Patch Bay dark adds glow.
3. **Sidebar.** Rail (tiles, badges, selection ring, add tile, divider) and rail-off tree. Board project header,
   group rows (per-theme label style), command rows (meta, pane-number chip, restart-to-apply chip, selection
   per theme), broken-project card/tile, foot lines. Hover actions reserve their slot (no layout jump). Keep all
   context menus and the drop target. Mockups: `Shell`, `Win`.
4. **Pane component.** `PaneView` = 36 pt bar + SwiftTerm + 22 pt footer, with full/mid/min tiers from its width
   (`GeometryReader`), primary action per state, backoff bar, inline never-run and error cards, empty picker,
   "Showing in…" placeholder. Replaces the toolbar actions and the idle banner. The window title still gets
   set (command name / project) for Mission Control and the Window menu. Mockups: `Pane`, `X-PaneSpec`.
5. **Empty states.** No projects (board), board clear (all extracted), never-run. Mockups: `Win` (boardState),
   `Pane`.
6. **Dialogs.** Replace the four `NSAlert`s with a SwiftUI card presented via `NSWindow.beginSheet` (or
   `.sheet`), same behaviour and choices. Mockups: `Dialog`.
7. **App icon.** Build the asset catalog from `icon/`. Use `switchboard-icon-1024.png` as the app icon; the light
   and tinted variants are for macOS's appearance slots. Keep the menu-bar template for the later module.
8. **Project windows (extraction).** Second WindowGroup keyed by project id. Opening removes the project from the
   board; closing or Move to Board returns it. Restore open project windows on launch. Project window sidebar
   header per `X-WinHeader`. Extraction/return animation per `X-Extract`. Window states per `X-WinStates`.
   A terminal can be attached to one view at a time app-wide; the loser shows the quiet placeholder.
9. **Split panes.** Split tree (max 4), dividers (NSSplitView or custom), focus model, maximize, close, swap,
   drag a sidebar row onto a pane (centre replaces, edge splits), limits and refusal toasts, per-project layout
   persistence, View menu items. Mockups: `Win`, `X-SplitCtl`, `X-Split*`, `S-*`.

## File map (existing repo)

| File | Changes |
|---|---|
| `app/Switchboard/Theme.swift` | new (this folder) |
| `SidebarView.swift` | rail, rows, headers, foot, hover actions, per-theme styling |
| `TerminalPane.swift` | becomes `PaneView`: bar, footer, inline cards, placeholder; `StatusDot` → `StatusGlyph` |
| `TerminalRegistry.swift` | SwiftTerm theme from `SBPalette` (both schemes), font, insets; attach/detach so a terminal lives in one view |
| `ContentView.swift` | split view → rail/sidebar/pane-area layout; empty states; split tree host |
| `SwitchboardApp.swift` | settings scene, project WindowGroup, View/Window menu items and shortcuts |
| `Models.swift` | drop old colour helpers; add pane layout model (per-project, persisted) |
| new: `SettingsView.swift`, `PaneView.swift`, `SplitLayout.swift`, `ProjectWindow.swift`, `DialogCard.swift`, `StatusGlyph.swift` | as named |

## Engine data the design uses

Confirm each exists in the UniFFI surface; if not, add it to the engine or drop it from the UI.

| UI element | Needs |
|---|---|
| Uptime in rows/footer | process start time per command |
| pid in footer | current pid |
| "last pid · ran 0:11 · exit 1" after a crash | last pid, last run duration, exit code |
| "restarting in N s" + draining bar | backoff deadline (or remaining seconds) |
| "Start now" during backoff | an engine call that cancels backoff and starts immediately |
| Stop during backoff | stop must cancel the pending restart |
| "autostart off" / "restart on crash · on" | those config values per command |
| Restart to apply | existing "config changed under a running session" flag |
| Terminal size in drag readout | from SwiftTerm (UI-side, fine) |
| Leftover-sessions copy says the process "keeps its port" | confirm or reword |

## Acceptance criteria

- Every board in `mockups/` that is a spec (not exploration) can be reproduced in the app in all three
  themes, light and dark.
- Each status renders with its own shape; Reduce Motion removes motion.
- All existing shortcuts still work; new ones per "Keys" above; every hover action has a menu item.
- Drop-a-folder, Edit Config, Reveal in Finder, conflict choice and leftover prompt behave as before.
- Text contrast ≥ 4.5:1 on its surface (meta text excepted); icon buttons have labels and tooltips.
- A project is never on the board and in a window at the same time; project windows restore on launch.
- Splits refuse below 320 × 180 pt and above 4 panes, with the reason shown.
- `Theme.swift` was generated without a compiler; build it first and fix anything trivial.

## Open questions for Walter

- Engine fields in the table above that don't exist yet.
- Whether to bundle a custom font later; everything currently specs SF Pro / SF Mono.
