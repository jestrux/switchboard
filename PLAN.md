# Plan: what's left

The MVP, the redesign, per-project windows, split panes, terminals, themes, links and repo
files are built. This is what remains, in the order to do it. Each item links to a file in
[`plans/`](plans) with the context a fresh session needs.

Read first: [`CLAUDE.md`](CLAUDE.md) and [`docs/agent-guide.md`](docs/agent-guide.md) for
how to work here, then [`plans/decisions.md`](plans/decisions.md) for choices already made
that are not obvious from the code.

## In order

1. **[Editing sheets](plans/01-editing-sheets.md)**: create and edit projects, groups and
   commands in the app instead of in YAML, with icon import. This is what makes the app
   usable without a text editor. PRD milestone M4.
2. **[Split a project into repo fragments](plans/02-split-into-fragments.md)**: move a
   project's inline commands into per-folder `.switchboard/commands.yaml` files. Without it,
   "Keep settings in the repo" versions one big file.
3. **[Remaining M5 features](plans/03-m5-remaining.md)**: find bar, quick-add from a
   folder's scripts, export, menu bar extra.
4. **[Design handoff gaps](plans/04-design-gaps.md)**: animations, pane focus by direction,
   divider readout, and smaller polish from the design spec.
5. **[Release](plans/05-release.md)**: Developer ID signing, notarization, a release build,
   and the checks before making the repo public.

## Not scheduled

- **[Open questions](plans/open-questions.md)**: product decisions still undecided. Some
  block items above; each says which.

## Reference material in `plans/`

- [`decisions.md`](plans/decisions.md): decisions made in conversation.
- [`design-handoff/`](plans/design-handoff): the full design handoff. `DESIGN.md` is the
  spec, `HANDOFF.md` the build order and acceptance criteria, `mockups/` the boards.
- [`../handoff/PRD.md`](handoff/PRD.md): the original product spec.

## When this is done

Delete `plans/` and `handoff/` once the handoff and PRD work is complete, and remove an
item's plan file when the item ships. Anything still worth keeping moves into `docs/`.
