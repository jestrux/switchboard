# 6. The board: where an agent hands a project to its author

**Status: In progress (the stand-in track).** A1–A4 are done: `switchboard start switchboard/board/board`
serves this repo's board on http://localhost:5198, the agent writes it with `scripts/board`, answers
come straight back, and changes reach the page within a fraction of a second. Next step: P, the
phone (Fuchi's NOTIFY_PLAN), then A5.

## What it is

The board is where a coding agent hands a project to its author. The agent posts **cards**: a
decision with its recommendation and why, a "look closer" where it's unsure, a question
answered, something that changed. The author answers, and the answers go straight back to
the agent. It has two uses:

- **The big picture at any time:** where the project is, what moved since you last looked,
  what's owed.
- **Getting in the zone:** working through what needs you, one card at a time, from the
  keyboard.

It extends Switchboard because Switchboard already knows the projects, runs their servers,
finds their Claude sessions and has its skill and CLI in every agent. The PRD lists
"AI-agent orchestration" as a v1 non-goal (`handoff/PRD.md:22`). This item changes that
on purpose: the PRD needs a line saying so when the item ships.

**Where the board lives (decided 2026-10-20, on the board, lc-commit):** it follows the project.
With a home, it's `.switchboard/board/` in the home folder, beside `project.yaml`: cards, board
and log committed; `state/` (the author's sends, the agent's read marker) and `private/` (images
that must never enter the repo) ignored. With no home (the default), it's
`Projects/<slug>/board/` in Switchboard's app folder, not in git, and its first card offers Keep
Settings in Repo. The CLI finds it the same way: the nearest `.switchboard/board/` above where it
runs, else the Switchboard project whose commands run in that folder. Switchboard's settings
watcher watches `.switchboard/` non-recursively and compares settings, so card writes don't
raise "Settings changed in the repo".

**Names (decided 2026-10-20, on the board):**

- The feature is the **board**. Its inbox is **Needs you**. The CLI is `switchboard board …`.
- Switchboard's main window, which the code and docs called "the board", becomes **All
  Projects**: Move to Board becomes Move to All Projects, and "The board is clear" becomes
  "Every project has its own window".
- The window that lists every project's Needs you is the **Boards** window.

## Where the design comes from

- **The design review**, `~/Downloads/board-product-review/review/index.html`: the first
  version of this board, before it moved here. Its cards are now in `.switchboard/board/cards/`.
- **Flex's review prototype**, `~/Downloads/flex-review-prototype/` (source of truth in the
  Flex repo, `review/`). It is a working build of the Claude Design "Screening room" design:
  one HTML page, a data contract (`window.FLEX_REVIEW`), and a CLI the agent writes with
  (`flex-review add | settle | working | log | serve | answers`). The board starts from it.
- **Most of the board's recommendations are still open.** Treat them as proposals until the
  author answers them on the board.
- **Privacy:** both Downloads folders contain photos of a real person and a real song.
  **Neither goes into this repo**, which may become public. Cards that show those images read
  them from `.switchboard/board/private/media/`, which is ignored.

## The approach: two tracks

We use the workflow while we build it.

**Track A, the stand-in.** The board runs now as a Vite dev server (`board/`), with a small
local API in a Vite plugin and, from A2, a small CLI. The agent posts cards through the CLI,
and the author's answers come straight back through the server. We run the loop on it every
day and refine the contract, card types and keys as we go. It is both the plan and the
working reference.

**Track B, the real board.** Built properly in Switchboard: the engine, the socket, the
`switchboard board` CLI, SwiftUI chrome, the menu bar, notifications and windows, with card
bodies in a web view.

**They meet at one contract** (the data format and the endpoints). Whatever Track A teaches
becomes Track B's spec. As each Track B part lands it replaces its stand-in, until the Vite
board retires or becomes the card-body web view:

| Stand-in (Track A) | Replaced by (Track B) |
|---|---|
| The `board` CLI | `switchboard board …` in `sb-cli`, over the control socket |
| Vite plugin API (cards, answers, live updates) | A board store in `sb-core`, served by Switchboard |
| The page in a browser tab | The board in SwiftUI, card bodies in `WKWebView` |
| In-page notifications | System notifications, menu-bar count and popover |

The live board has no simulator: real cards arriving produce the in-page notifications, and
the page stops reading the real board while a simulation plays. The scripted events are kept
in `board/fixtures/demo-events.json` for tests and demo films.

**Blocking, and what's new on top.** `board ask <id> --blocking` (or `add --blocking`) marks a
card the agent can't go on without: it gets a Blocking tag (first in the Tags filter, always)
and a red pill, and the server clears it when the card is answered, from any device. New,
asked and blocking cards sit in a **New** group at the top of Needs you, blocking first, until
handled; review mode starts with them. **New means not opened yet** (per browser): a card stays
there until you open it and move on, however often the page reloads, and asking for it again
brings it back up. (Flex's "since your last visit" dropped cards on every reload.)

**Asking for a card now.** A card's `askedAt` (milliseconds) means "the agent needs this one
now". When it moves forward, the page shows a Needs you notification for that card. It stands
in for B4.

## One board per project, one inbox across them

Each project keeps its own board in its own repo (`.switchboard/board/`), written by that project's
agent. Switchboard reads every project's board and adds them up: a count per project tile,
one menu-bar number, and the Boards window, which lists every project's Needs you. While the
stand-in runs, one Vite server serves one board; `BOARD_DIR` points it at another project's
`.switchboard/board/`. prod-app's board is the second one to move over.

## Track A steps

1. **A1 · Serve the board.** *Done.* `board/` is Flex's page served by Vite; the plugin in
   `board/vite.config.ts` and `board/server/board.ts` builds `data.js` from `.switchboard/board/`
   (`board.json`, `cards/<id>.json`, `log.json`) on every request, serves card images, and
   takes Send into `.switchboard/board/state/answers.json`. It runs as `switchboard/board/board`. The
   first round trip worked on 2026-10-20: two cards asked, two answers received.
2. **A2 · The agent writes through a CLI.** *Done:* `scripts/board` (`board/bin/board.mjs`, no
   dependencies): `add | update | ask | remove | settle | working | log | list | answers`. One
   file per card in `.switchboard/board/cards/`, written atomically. It refuses unknown stages, duplicate
   ids and unsafe ids; `update --changed "…"` makes a new version. The agent never edits the
   page or the card files by hand. The verbs mirror `flex-review`'s, plus `ask`.
3. **A3 · Answers come straight back.** *Done:* Send writes `.switchboard/board/state/answers.json` and
   the page shows "The agent has your answers"; `board answers` prints sends not yet read and
   remembers them (`.switchboard/board/state/read.json`); `--wait` blocks until one lands (`--timeout`
   exits 124). An agent keeps `board answers --wait` running in the background.
4. **A4 · Live arrival.** *Done:* the plugin watches `.switchboard/board/` and sends `change` on
   `/events`; the page re-reads then, and polls every 15 s only as a fallback (every 3 s when
   opened from a file). About 150 ms from a CLI write to the push.
5. **P · The phone, next (moved up 2026-10-20).** When the agent asks for a card, the phone and
   iPad get a push; long-pressing it opens that one card in a web view, without launching
   Fuchi, and you answer there. Built as Fuchi platform parts (push, a custom notification view,
   a way back) that any Fuchi extension can combine; the board is the first user. Plan:
   `~/Documents/swift-apps/Fuchi/NOTIFY_PLAN.md`. This comes before A5 and stands in for B6. *Working 2026-10-20* on the iPhone and iPad: `board ask` pushes, long-press answers inline, a tap opens the card in a Fuchi page.
6. **A5 · Close the gaps found in the audit.** Reason labels on cards (decided 2026-10-20: six reasons, each a filter: decision, look closer, your question answered, changed, from the wishlist, for someone else); "for you to see" cards
   out of the Needs you count; decided cards into a searchable Record; the overview, **as part of The plan in the sidebar, not a page of its own** (decided
   2026-10-20, `v-sidebar`: the sidebar keeps its shape; the plan is the high-level view, and
   the overview adds since you last looked, health and what's owed under it). The overview and Record get a
   Claude Design handoff before they're built.

## Track B steps (order proposed on the board's "Build order" card)

0. **B0 · Homes from the CLI (next in Track B, 2026-10-20).** The board follows the project's
   home, and most projects pick one by hand today; this makes the agent able to suggest and,
   on the author's yes, adopt one.
   - **The suggestion rule, in `sb-core`** (one place for the app and the CLI), from how the
     author's projects are already set up (career-path, citybuild and otesha keep theirs in
     `e2e`; kinpay and set-hero in the repo root): (1) an `e2e` or `*-e2e` folder among the
     project's command folders, since it exercises the whole product; (2) else a repo root
     that contains the other folders; (3) else the single folder's repo root; (4) else no
     suggestion, just the list.
   - **`switchboard keep <project> [folder]`**: adopts the home through the running app over
     the control socket (`engine.adopt_home`, which exists), like `start`; with no folder it
     prints the suggestion and the alternatives and changes nothing.
   - **The app's Keep Settings in Repo… panel** opens on the suggested folder.
   - **The skill** gains a short section: a project with no home gets a board card suggesting
     one (the folder above as the pick, the others as options); run `switchboard keep` only on
     the author's yes. Switchboard still never writes into a repo unasked: the answer is the ask.
   - **The stand-in's first card** ("Keep this project's board in the repo?") uses the same rule
     now, naming the folder.
   - **A consequence:** the board lives where the project's home is. prod-app belongs to the
     set-hero project, whose home is `~/Documents/web/set-hero`, so its board would live there.
1. **B1 · Engine and CLI.** A board store in `sb-core` (cards, answers, agent state), socket
   ops, and `switchboard board …` in `sb-cli`. A2 and A3 switch over to it. The board verbs join the skill here, not before: until then the stand-in only
   serves this repo's board, so `scripts/board` stays this repo's tool.
2. **B2 · The board in a project window.** SwiftUI list, rail and review mode; card bodies in
   a web view built from Track A's page; the board as a pane beside a terminal.
3. **B3 · Menu bar.** The `MenuBarExtra` from [item 3](03-m5-remaining.md#menu-bar-extra), with
   the Needs you count and a popover to answer the next card.
4. **B4 · Notifications** only when an agent is blocked, with the recommended answer and a
   reply field.
5. **B5 · Every project.** The Boards window across projects; tear-off; agent state on tiles.
6. **B6 · Away from the Mac.** A Switchboard app on the phone on the same contract. Later; see
   the board's "Away from the Mac" section.

## Open on the board

Answer these on the board before the step they affect:

- **The contract: Flex's core plus optional fields, or a fork** (`a-contract`). Shapes A2
  and B1.
- **Where cards live** (`a-data`) is now covered by `lc-commit` below; close it on the board.
- **Native SwiftUI, a web page, or both** (`lc-home`). Shapes B2.
- **The three window shapes** (`w-modes`). Shapes B2 and B5.

## Verifiables

- **Track A:** posting a card from the CLI shows it on the open page within a second; Send
  writes `answers.json` and `board answers --wait` returns it; the page works with the
  plugin's API or with a static data file.
- **Track B:** `cargo test -p sb-core` covers the store and socket ops; the app is checked by
  driving it with `scripts/dev/uidrive.swift` and looking at screenshots.
