# Shared context — Switchboard design handoffs

> Read this first, then `01-app-shell/BRIEF.md`. This file is the shared product +
> design-system + direction + guardrail context; the module-specific work and its 16
> screenshots (window shots of the real app, 1280×800 window, 2× PNGs) live in the
> module folder next to its `BRIEF.md`. Module 01 is the whole MVP surface: sidebar,
> terminal pane, every state, light + dark, the dialogs, and the app icon (none exists yet).

## The product
**Switchboard** — a native **Mac** app (macOS 14+) that starts, watches and stops a
project's dev commands from one window. The one idea that makes it different from Solo
and from a terminal full of tabs: **a project is a logical grouping of commands, not a
directory.** One product usually spans several repos (API, web frontend, e2e suite); each
command carries its own folder, and the project just groups them under a name and an icon.

- **Project** → optional **groups** (Frontend / Backend / E2E…) → **commands** (`run` +
  `cwd` + env, `autostart`, `restart_on_crash`). Ungrouped commands sit above the groups.
- Every command runs in a **real PTY** with colour and full interactivity — `pnpm dev`,
  `make dev`, `vim`, `htop`, **`claude`** (Claude Code) all work in the pane.
- **Status** per command: stopped · starting · running · exited-0 (“done”, a ✓) · crashed
  (with crash-restart backoff) · error (e.g. folder missing).
- The window is a **sidebar + one terminal pane**, like tmux. Hidden panes keep running;
  switching back reattaches with scrollback intact. Quit stops everything cleanly.
- Projects are **YAML files on disk** (`~/Library/Application Support/Switchboard/
  Projects/<slug>/config.yaml` + `icon.png`). In the MVP you **edit the file** (Edit Config
  opens it in VS Code) and the sidebar live-reloads. Editing sheets are the next module.
- There is also an `sb` CLI on the same engine (`sb run <project>`), not in scope here.

**Audience:** one developer at their own Mac, running 3–12 long-lived dev processes a day
and context-switching between two or three products. They live in VS Code, Claude Code,
a terminal, Spotify.

## Stack (what you'd be restyling)
- **SwiftUI + AppKit**, macOS 14+. `NavigationSplitView` with a `List(.sidebar)` on the
  left, a `TerminalView` from **SwiftTerm** on the right, a SwiftUI footer under it.
  Toolbar is the standard unified toolbar. Dialogs are `NSAlert`s.
- The engine (processes, config, statuses) is **Rust** behind UniFFI. The UI never spawns
  processes; it only renders state and sends start/stop/input/resize.
- Files you'd touch: `app/Switchboard/SidebarView.swift` (rows, dots, hover controls,
  context menus, drop target), `TerminalPane.swift` (pane, idle banner, footer,
  `StatusDot`), `TerminalRegistry.swift` (SwiftTerm theme + ANSI palettes), `ContentView.swift`
  (split view, toolbar, empty state), `SwitchboardApp.swift` (menus, shortcuts),
  `Models.swift` (colour helpers). Repo: `~/Documents/web/switchboard`.
- Run it: `scripts/package.sh` builds and installs `/Applications/Switchboard.app` and
  relaunches it. `File › Install Sample Project (Kinpay)` adds the reference project;
  `samples/citybuild` is a second one (drop the folder on the sidebar).

## Design direction (the ask, in one paragraph)
**Make it feel less like a stock Apple app and more like a modern cross-platform
product — think Spotify, VS Code, Framer, Linear, Raycast.** Clean, dense-but-calm,
opinionated chrome rather than system-default chrome: a designed sidebar surface, designed
rows and status language, a real header for the pane instead of the unified toolbar's
grey strip, a footer/status bar that reads as part of the product, dialogs that are not
`NSAlert`. Keep it native in *behaviour* (keyboard, menus, Finder/VS Code hand-offs,
light/dark), not necessarily in *look*. Dark mode is the primary mode for this audience;
light must still be first-class.

## Current de-facto design system (there is no DESIGN.md yet)
- **Type:** system font (SF) everywhere; terminal is **SF Mono 12**.
- **Icons:** SF Symbols (`play.fill`, `stop.fill`, `arrow.clockwise`, `folder`,
  `exclamationmark.triangle.fill`, `checkmark.circle`, `terminal`, `switch.2`).
- **Colour:** no brand colour. Status is the only colour language — **gray** stopped,
  **yellow** starting, **green** running (+ green count pill on the project row), **red**
  crashed, **gray ✓** done, **orange ⚠** error/missing folder. Project icons are the
  projects' own `icon.png` (22 pt, rounded) or initials on the project's `color`.
- **Surfaces:** standard sidebar material; terminal background `#1C1C20` dark /
  `#FDFDFB` light with a custom 16-colour ANSI palette (`TerminalRegistry.swift`);
  footer and idle banner on `.bar` material.
- **Layout:** sidebar 220–420 pt (ideal 260); window min 820×480; pane = banner (when
  never run) + terminal + 1-line footer.

## Global guardrails (apply to every screen)
1. **Keep the information architecture.** Sidebar of projects › groups › commands on the
   left, one terminal pane on the right, footer with the command's vitals. Splits, sheets,
   and a menu-bar extra come later as their own modules — don't design them in here.
2. **Keep the status semantics** (the six states and what they mean, the ✓ for exit 0,
   "restarting in Ns" during backoff, "restart to apply" when the config changed under a
   running session). Restyle the language, not the meaning.
3. **SwiftUI/AppKit + SwiftTerm stay.** Anything you propose must be buildable with them
   (custom views are fine; a web view is not). The terminal content itself is rendered by
   SwiftTerm — you own its colours, font, padding and chrome, not its text layout.
4. **Light + dark**, both real. Follow the system by default.
5. **Keyboard first.** Every hover action has a menu/shortcut equivalent (⌘R restart,
   ⌘. stop, ⌘K clear, ⌘E edit config, ⌘O add project). Keep that true.
6. **Don't break the hand-offs:** drop a folder to add a project, Edit Config → VS Code,
   Reveal in Finder, the Replace / Keep Both conflict choice, the leftover-sessions prompt.
7. **No process logic in the UI.** Everything shown comes from engine events.

## Working agreement (per-module handoff)
- One module per folder; work within its scope.
- Shared things you touch (the `StatusDot`, the sidebar row, the footer, colours) are used
  across modules — note any shared-component change in your return.
- Deliver as a branch/PR per module so modules review and merge independently.
