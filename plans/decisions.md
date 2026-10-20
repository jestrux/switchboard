# Decisions already made

Choices settled in conversation with the author. They are not all visible in the code, and
a new session should not reopen them without a reason.

## Product model

- **A project is a logical group, not a directory.** Every command carries its own folder.
  This is the difference from Solo and the reason for most of the design.
- **Windows use extraction.** The main window is **All Projects** (it was called "the board" until 2026-10-20, when that
  name went to the [agent's board](06-board.md)). Opening a project in its own window pulls it
  out of All Projects until that window closes or Move to All Projects is chosen. A project
  is in All Projects or in its own window, never both. A project window has no project
  row: its sidebar is a header plus the groups at the top level. Project windows reopen on
  launch.
- **One terminal view per command, app-wide.** Showing a command in a pane moves its
  terminal there; whoever had it shows a quiet "Showing in… · Show Here" placeholder.
  Picking a command that is in another pane of the same window swaps the two panes.
- **Splits:** up to four panes, minimum 320 × 180 pt, layout stored per window in app state,
  not in config.
- **A requested stop is "Stopped", not "Crashed"**, however the process died.

## Terminals

- Ad hoc, but they **persist across relaunches**. They are device state, kept in
  `Projects/<slug>/terminals.yaml` in Application Support, so they go away when the project
  is deleted and never enter a repo.
- A restored terminal starts a fresh shell when first shown. Scrollback is not kept.
- "Save as Command" promotes one into its group: into the group's `commands.yaml` when the
  group has `from:`, otherwise into `config.yaml`.

## Repo files (`.switchboard/`)

- `commands.yaml` in any repo folder holds that folder's commands. `project.yaml` in one
  folder, the **home**, holds project settings and the member folders.
- The live config stays in Application Support. Repo files seed a new device and are what
  gets versioned. Write-back mirrors config changes to the home's `project.yaml`.
- A repo-side change (a pull) is **never applied silently**: the sidebar shows "Settings
  changed in the repo · Apply".
- Adopting a home is **offered, not automatic**. The app does not write into a repo unasked.
  The author's real repos (kinpay, citybuild, kinpay-circles) have no `.switchboard` folders
  yet; that is their choice to make by clicking Keep.
- Relative `from:` paths resolve from the home folder (`.` is the home, `../api` a sibling).
- Importing works from any member folder. With several `project.yaml` copies the newest is
  proposed and the others can be chosen.

## Themes

- **Every theme is system-aware by default.** Each has a light and a dark palette, and
  Appearance defaults to System. A custom theme should define both `dark:` and `light:`.
- A project's `theme:` and `appearance:` apply **only in its own window**. All Projects keeps
  the app-wide setting.
- Custom themes inherit layout style from a `base` built-in and override colours only.

## Design direction

- Less like a stock Apple app, more like a cross-platform product: Spotify, VS Code, Framer,
  Linear, Raycast. Dark-first, light first-class. Native in behaviour, not in look.
- The design spec in [`design-handoff/DESIGN.md`](design-handoff/DESIGN.md) lists decisions
  not to relitigate (three themes, project rail on by default, one 36 pt pane bar, dialog
  cards with Keep Both as the conflict default, the key bindings).

## Engineering

- All process logic is in Rust. Swift never spawns a process.
- The app is signed with the author's Apple Development certificate so macOS remembers
  folder permissions across rebuilds.
- The repo `jestrux/switchboard` is private for now.
- Narration for videos uses local Kokoro, voice `am_michael`, no music (the author's music
  tracks are CC BY and would need attribution).

## Things to leave alone

- Do not run `make migrate`, `make seed`, `make flush` or `make flush-loans` in the sample
  projects. They touch real databases.
- `make flush` is deliberately absent from the sample configs.
- The old `kinpay-circles/e2e` folder is a stale stub; `journeys-e2e` is the current suite.
  It was left in place, not deleted.
