# 1. Editing sheets (PRD milestone M4)

**Status: Next.** Next step: write the engine calls (`add_command`, `update_command`, `delete_command`, and the group equivalents) in `crates/sb-core/src/engine.rs` with tests in `tests/fragment_tests.rs`, following `promote_terminal`. Ask the author the design question under Open points before building any sheet UI.

## Goal

Create and edit projects, groups and commands in the app. Today every change means opening
`config.yaml` in an editor (Edit Config, ⌘E). Acceptance from the PRD: a project created
entirely in the UI produces a folder that `sb validate` accepts and `sb run` runs.

## Spec

[`handoff/PRD.md`](../handoff/PRD.md), sections "Sheets", "Icons" and milestone M4:

- **New / Edit Project:** name; icon well (drop or pick SVG, PNG or PDF, live preview,
  initials fallback); fallback colour.
- **New / Edit Command:** name; group picker (or none); run; folder picker showing the
  resolved path, with the inherited group folder as placeholder and a warning if missing;
  autostart and restart-on-crash toggles; expandable env table; a Test run button that runs
  the command in a temporary pane. Edit adds Duplicate and Delete.
- **New / Edit Group:** name and default folder.
- Return saves, Escape cancels, invalid fields block save with inline messages.
- Shortcuts: ⌘N new project, ⌘⇧N new command. **Conflict:** ⌘⇧N is now "Open in New
  Window" per the design spec. Pick another for new command.
- **Icons:** render any accepted image once at 512 × 512 and save as `icon.png`. SVG via an
  offscreen `WKWebView` snapshot, PDF and PNG via AppKit. The app only reads PNG at runtime.

The design handoff did not design these sheets. Use the dialog card's visual language
(`DialogCard.swift`, 460 pt card, same buttons and inputs) and the tokens in `Theme.swift`
rather than `Form(.grouped)`, which would look like stock macOS again. If they need real
design, ask for a handoff module first.

## What exists to build on

- `DialogCard.swift` already supports text inputs (`DialogInput`) and returns values through
  `DialogCenter.inputValues`. Rename Terminal and Save as Command use it. It is enough for a
  two-field prompt, not for a full form with pickers and toggles; expect a new sheet view.
- `Engine::promote_terminal` shows the write path: it appends a `CommandConfig` to a
  fragment or to `config.yaml` and reloads. Generalize that into engine calls for add,
  update, delete and reorder of commands and groups.
- `ProjectConfig` keeps unknown keys (`extra`), so a round trip through the engine does not
  drop hand-written fields. **YAML comments are lost** on rewrite (`serde_yaml`). The PRD
  asks to keep comments "where possible"; decide whether that matters before choosing an
  approach, since keeping them needs a different YAML library or text patching.
- Groups with `from:` take commands from a repo fragment. Editing such a command should
  write the fragment, or an `overrides` entry when only a per-project tweak changes.

## Suggested order

1. Engine: `add_command`, `update_command`, `delete_command`, `add_group`, `update_group`,
   `delete_group`, `create_project`, `update_project`, with tests, exposed through `sb-ffi`.
2. Command sheet (the most used), then group, then project.
3. Icon import.
4. Test run in a temporary pane.
5. Drag between groups and reorder (PRD lists it under M5; it shares the engine calls).

## Open points

- Comment preservation in YAML (above).
- Shortcut for New Command.
