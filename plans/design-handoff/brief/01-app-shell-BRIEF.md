# Handoff 01 — App shell (sidebar · terminal pane · states · dialogs · app icon)

**Screenshots** (window shots, 1280×800 window, 2× PNGs; light unless noted; the projects
shown are real — CityBuild and Kinpay are the author's products — plus a throwaway
"States Demo" and a deliberately broken "broken" project to show every state):
- `sb-1-empty-state.png` — **first launch, no projects**: the sidebar's inline empty
  state ("Drop a folder containing config.yaml here, or Add Project… ⌘O") and the pane's
  `ContentUnavailableView` with an **Add Project…** button.
- `sb-2-command-idle.png` — a command selected that has **never run** (CityBuild ›
  Frontend › dev): the **idle banner** above an empty terminal (folder icon · `npm run dev`
  · `~/Documents/web/citybuild/frontend` · **Start**), the **footer** (gray dot · name ·
  folder · "Stopped" · ▶), the toolbar (＋ add project · ▶ start · Edit Config · Reveal).
- `sb-3-running.png` — **Start All** on CityBuild: two green dots, the green **"2"**
  running pill on the project row, the terminal showing `react-router dev` output with
  colour, the footer with **pid + uptime** and ■ / ↻, the toolbar swapped to **Stop /
  Restart**.
- `sb-4-hover-command.png` — **hover on a command row**: the selected (blue) row reveals a
  ▶ at the trailing edge (■ ↻ when running).
- `sb-5-hover-project.png` — **hover on the project row**: ▶ ■ ↻ (start all / stop all /
  restart all) appear at the trailing edge.
- `sb-6-interactive-claude.png` — **Claude Code running in the pane** (the ungrouped
  "Claude Code" command): a full-screen TUI with its own prompt, status line and
  bottom bar — the pane must stay a real terminal for this. Footer shows the program's
  own title ("Claude Code") on the right.
- `sb-7-states-overview.png` — the **States Demo** project with every dot in one list:
  **never started** (gray), **worker** (green, selected, ticking output), **flaky api**
  (red, crash-looping with backoff), **build** (✓ done), **missing folder** (orange ⚠ dot
  + an orange folder-badge icon after the name). Also shows the **"started HH:MM:SS"
  separator** the pane prints between runs.
- `sb-8-crashed.png` — the **crashed footer**: red dot · "Exited with code 1 · crashed,
  restarting in 16 s" in red · ▶.
- `sb-9-done.png` — the **done footer** for a one-off task that exited 0: ✓ · "Done ·
  exit 0".
- `sb-10-missing-folder.png` — the **error footer**: orange ⚠ · "Folder not found:
  ~/Documents/does-not-exist"; the folder label in the footer turns orange; Start is
  disabled.
- `sb-11-restart-to-apply.png` — the config was edited while **dev** ran: a small ↻ badge
  after the row name and a blue **"Config changed · restart to apply"** link in the footer.
- `sb-12-dark-running.png` — `sb-3` in **dark**.
- `sb-13-dark-states.png` — `sb-7/8` in **dark** (crashed command selected).
- `sb-14-add-invalid-alert.png` — **Add Project… with a folder that isn't a project**:
  the `NSAlert` ("This folder is not a valid Switchboard project … Nothing was copied.").
  Note the **blank app icon** in the alert.
- `sb-15-add-conflict-alert.png` — adding a project whose name already exists: **Replace /
  Keep Both / Cancel**.
- `sb-16-leftover-sessions-alert.png` — on launch after a crash: "Switchboard left 1
  session running … Stop Them / Leave Running".

Not captured (the automation couldn't open them), described instead:
- **Project context menu:** Start All · Stop All · Restart All · — · Edit Config · Reveal in
  Finder · — · Delete Project…
- **Group context menu:** Start All · Stop All · Restart All. **Group rows** are small
  uppercase gray labels (FRONTEND / BACKEND / E2E) with disclosure chevrons; they show the
  same ▶ ■ ↻ on hover.
- **Command context menu:** Start (or Stop · Restart) · — · the `run` line in monospace ·
  Reveal Folder in Finder.
- **Delete confirmation:** an `NSAlert` ("Delete “States Demo”? This stops its commands and
  moves the project folder to the Trash…", Delete / Cancel).
- **Menus:** File (Add Project… ⌘O, Install Sample Project), Command (Start · Stop ⌘. ·
  Restart ⌘R · Clear Terminal ⌘K · Start/Stop All in Project · Stop Everything), Project
  (Edit Config ⌘E · Reveal in Finder · Reload Projects · Open Projects Folder).
- **Broken project row** (top of the sidebar in most shots): orange ⚠ · name · an
  **Edit Config** link · the validation messages in small gray text.

## What this is
The whole MVP surface. One window:
1. **Sidebar** — projects (icon + name + running-count pill) › groups (uppercase label) ›
   commands (status dot + name, hover ▶ ■ ↻). A broken project shows an error row instead.
   The sidebar is also the **drop target** for adding a project folder.
2. **Pane** — the selected command's terminal (SwiftTerm), with an **idle banner** when it
   has never run, and a one-line **footer**: dot · prefixed name ("Frontend / dev") · folder
   · pid · uptime (or the exit summary) · "restart to apply" when relevant · ▶ or ■ ↻.
3. **Toolbar** — ＋ (add project) on the sidebar side; ▶ or ■ ↻, Edit Config, Reveal in
   Finder on the pane side. Window title = command name, subtitle = project.
4. **Dialogs** — invalid folder, name conflict, delete, leftover sessions (all `NSAlert`).

## How to reach it
- `scripts/package.sh` in the repo installs and launches `/Applications/Switchboard.app`.
- `File › Install Sample Project (Kinpay)` adds a project; drop
  `~/Documents/web/switchboard/samples/citybuild` on the sidebar for a second one. A
  "states demo" config to reproduce `sb-7` is in the repo at `samples/states-demo`
  (put it in a folder and drop it).
- Theme follows the system: System Settings › Appearance.

## Current state (what's on screen)
- **Chrome is 100% stock macOS:** sidebar material, unified toolbar, `List(.sidebar)`
  rows and disclosure chevrons, SF Symbols buttons, `.bar` material footer, `NSAlert`s with
  the **generic blank app icon**. It looks like a Finder-adjacent utility.
- **Status language** works but is tiny and purely colour-coded: 9 pt dots; the ✓ and ⚠
  are 9 pt symbols; the running pill is a green capsule with a number.
- **Hover controls** are three 10 pt symbol buttons that appear at the trailing edge
  (row and project row). No hover background of their own.
- **The pane** is a bare terminal with 8 pt inset, a 1-line footer, and the idle banner
  (monospace `run` line + folder + Start) when the command has never run. The terminal
  theme is a custom near-black (`#1C1C20`) / near-white and a custom ANSI palette; the
  font is SF Mono 12.
- **Light mode** is the default system light: white terminal, light-gray sidebar. **Dark**
  is the system dark with the near-black terminal. Both are implemented.
- **App icon:** none. The app shows the blank macOS placeholder in the Dock, Finder, and
  in every alert.

## Design critique — opportunities
1. **It reads as an Apple utility, not a product.** Every surface is a system default.
   The direction (see `_SHARED-CONTEXT.md`) is Spotify / VS Code / Framer / Linear:
   designed chrome, a point of view on type, colour and spacing, dark-first.
2. **The pane has no header of its own.** The command's identity lives in the window
   title bar (small, gray, easy to miss) and again in the footer. A pane header — identity
   (project icon · group / command), status, folder, the primary actions — would make the
   toolbar's grey strip unnecessary and the footer lighter.
3. **Status is too quiet.** 9 pt dots carry the most important information in the app.
   Running vs crashed vs done needs to be legible at a glance from across the sidebar,
   and starting/restarting/backoff need motion or a distinct treatment, not just yellow.
4. **Hover-only actions have no resting affordance.** Nothing suggests a row is
   actionable until you hover; the icons are tiny when they do appear. Consider a
   persistent compact action, a designed hover state, or a row layout that reserves space.
5. **The long sidebar doesn't scale.** Two real projects already fill 800 px (`sb-3`).
   Group headers are low-contrast uppercase labels; projects don't visually separate from
   each other; there is no collapsed/overview mode. Think about density, project
   separation, collapsed groups with counts, and where a 5th project goes.
6. **The idle banner and the empty states are placeholders.** The never-run banner is a
   plain bar; the no-projects state is a stock `ContentUnavailableView`. These are the first
   things a new user sees.
7. **Footer is a strip of gray text.** pid · uptime · folder · exit code is the right
   content; it needs hierarchy (what matters when running vs when crashed).
8. **Dialogs are `NSAlert`s** with the blank icon. Conflict (Replace / Keep Both), invalid
   folder, delete and leftover-sessions each deserve a designed treatment — they can stay
   AppKit-presented but should feel like the product.
9. **No app icon.** See the ask below.

**Keep (working well):** the sidebar › pane model; the prefixed naming ("Backend /
make:dev"); the ✓-for-exit-0 idea; the "started HH:MM:SS" run separator; the restart-to-
apply hint; the green running-count pill; the custom terminal palette (readable in both
modes); the drop-a-folder add flow; the keyboard shortcuts.

## Asks for the designer
- **A visual language for the shell** in the cross-platform direction: sidebar surface,
  row anatomy (project / group / command), status treatment (six states + the backoff and
  restart-to-apply variants), hover/selected/focused states, the running-count pill.
- **A pane header + footer system** for the terminal: identity, status, folder, actions,
  and what the footer shows in each state (running / done / crashed / error / idle).
- **Idle and empty states** (never-run banner; no projects; invalid project row).
- **Dialogs**: invalid folder, name conflict, delete, leftover sessions.
- **Light + dark** for all of the above, dark-first.
- **Type + colour tokens** we can put into a `DESIGN.md` (and into SwiftUI as a small
  token set): fonts/sizes, the status colours, surfaces, radii, spacing scale, the ANSI
  16-colour palette for the terminal in both modes.
- **The app icon.** Switchboard has none. Brief: a telephone-switchboard / patch-bay idea
  — jacks, patch cables, a row of lit indicators — or a simpler "toggles" mark (the
  current in-app placeholder uses SF Symbol `switch.2`). It should sit comfortably in a
  Dock between VS Code, Spotify and Framer: a bold, simple glyph on a solid or softly
  graded tile, legible at 16 px (sidebar/menu bar) and 1024 px. Deliver a 1024×1024 PNG
  (plus a dark-mode/tinted variant if the glyph needs one for macOS Tahoe's icon styles);
  we'll generate the `.icns` / Asset Catalog from it. It will also be used in the alerts
  and a future menu-bar extra (a monochrome template version would help there).

## Guardrails (in addition to the global ones)
- Keep the sidebar › single-pane layout, the footer's content and the toolbar's actions
  (they can move into a pane header). Splits, sheets and the menu bar extra are later
  modules.
- Keep the states and their meanings; keep "done" distinct from "stopped".
- The terminal area is SwiftTerm: you set its colours, font, padding and surroundings; the
  text layout and cursor are its own.
- Everything must be buildable in SwiftUI/AppKit on macOS 14+.
