# 3. Remaining M5 features

**Status: partly Ready.** Next step: the find bar, which depends on nothing else. Start by reading what SwiftTerm's search API exposes publicly. Quick add waits on item 1's engine calls. Ask the author whether Export is still wanted before building it.

From [`handoff/PRD.md`](../handoff/PRD.md) milestone M5. Split panes and crash-leftover
cleanup are done. These are not.

## Find bar (⌘F)

Search a pane's scrollback. SwiftTerm ships a find bar (`MacFindBarView.swift`,
`TerminalViewSearch.swift`, `SearchService.swift` in the checkout under
`app/build/DerivedData/SourcePackages/checkouts/SwiftTerm`). Check what is public before
building one. The engine also keeps a parsed screen per session (`vt100`, 10,000 lines) that
could back a custom search. The find bar should be themed like the pane bar.

## Quick add from a folder

Dropping a folder onto a project, or "Add from folder…", scans it for `package.json`
scripts, `composer.json` scripts, Makefile targets and a Procfile, and shows a sheet of
found commands with checkboxes and editable names. Selected ones are added with that folder
as `cwd`. PRD acceptance: dropping a Laravel or Node repo offers its scripts.

- The scan belongs in the engine so `sb` can use it too.
- Depends on the engine's add-command calls from [item 1](01-editing-sheets.md).
- Today a dropped folder without `config.yaml` or `.switchboard/project.yaml` shows "This
  folder isn't a Switchboard project". Quick add is the better answer for that case.
- Dropping onto a pane is taken by drag-to-split, and onto the sidebar by add-project.
  Decide the target for "add to this project": the project header or a group row.

## Export

Zip a project folder. The PRD pairs it with importing a zip. Largely superseded by repo
files (`.switchboard/`, `sb import`). Ask the author whether it is still wanted before
building it.

## Menu bar extra

A menu bar icon listing projects with running counts and start/stop all. Marked "nice to
have" in the PRD and reserved for a later module by the design handoff. The template icon
already exists: `plans/design-handoff/icon/switchboard-menubarTemplate.png` and `@2x`.
SwiftUI's `MenuBarExtra` scene fits; the store's `totalRunning` and per-project counts are
already published.

## Smaller PRD items not yet done

- Keyboard: ⌘1…9 to select a command in the current project.
- Option-as-Meta as a setting (it is always on today).
- Font and size as a setting (fixed at SF Mono 12.5).
