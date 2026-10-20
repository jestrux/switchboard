# Plan: what's left

The MVP, the redesign, per-project windows, split panes, terminals, themes, links and repo
files are built. This is what remains, in the order to do it. Each item links to a file in
[`plans/`](plans) with the context a fresh session needs.

Read first: [`CLAUDE.md`](CLAUDE.md) and [`docs/agent-guide.md`](docs/agent-guide.md) for
how to work here, then [`plans/decisions.md`](plans/decisions.md) for choices already made
that are not obvious from the code.

## Start here

**Next step:** item 1, step 1. Add the engine calls for adding, updating and deleting
commands and groups, with tests, and expose them through `sb-ffi`
([details](plans/01-editing-sheets.md#suggested-order)). Everything in items 1 to 3 that
writes a config depends on them.

Two questions to put to the author before going far, because they change the work:

- Should the editing sheets get a design pass from Claude Design, or follow the existing
  dialog card?
- How should two groups that share one folder be stored as repo fragments? This blocks
  item 2.

Status key: **Next** is the item to pick up now, **Ready** can start any time,
**Blocked** waits on the thing named.

| # | Item | Status |
|---|---|---|
| 1 | Editing sheets | **Next** |
| 2 | Split into repo fragments | Blocked on the shared-folder question |
| 3 | Remaining M5 features | Find bar and menu bar extra are Ready; quick add waits on item 1; export needs a yes from the author |
| 4 | Design handoff gaps | Ready, independent of the rest |
| 5 | Release | Signed, notarized builds work via `scripts/release.sh`; updates and versioning undecided |
| 6 | The board: where an agent hands a project to its author | **In progress**: stand-in A1–A4 done, A5 next |

Update this table and the "Next step" line in each plan file as work lands.

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
6. **[The board](plans/06-board.md)**: where a coding agent hands a project to its author:
   cards with a recommendation, answers straight back to the agent, the big picture and a
   one-card-at-a-time review mode. Built in two tracks: a stand-in board on Vite that we use
   while building, and the real thing in the engine, CLI and app.

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
