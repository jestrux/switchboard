# 6. The board: where an agent hands a project to its author

**Status: In progress.** A1–A4, P and B0 are done: `switchboard start switchboard/board/board`
serves this repo's board on http://localhost:5198, the agent writes it with `scripts/board`,
answers come straight back from the Mac or the phone, and `switchboard keep` suggests and adopts
a project's home (this repo is the Switchboard project's home since 2026-10-20). Now: A5, the
audit's gaps. Each step's brief, parts, verifiables and gate are under **The steps**.

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

**The plan is the sidebar (2026-10-20).** The board's stages are this plan's steps, each with a
`state` (done, now, next) and a `label` (A1, P, B0…), set with `board step <id> <state>`; the
sidebar shows "The plan · 5 of 13 done", a tick, a spinner or an empty ring, and each step's
open cards. Cards sit under the step they shape (`board update <id>` with a new `stage`); the
design review's old topics (Precedent, Look closer, …) are tags. Moving a card doesn't change its
`updated` time, so a move isn't news. Both fields are optional additions beside Flex's core.

**Each step's panel is its section of this plan.** A step's `doc` (`board step b0 --doc
plans/06-board.md#B0`) names its section; the board server renders it (brief, parts ticked or
not, verifiables, gate) into the step's panel, above its open cards, and counts its parts
("3 of 6 parts done"). The plan is written once, here, and an edit reaches an open panel live.

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

## The steps

In the order they're done, as the board's sidebar shows them. Each step's heading starts with
its label (A1, P, B0…): the board's step panel shows that section, so the plan is written
once, here. Parts are ticked as they land.

### A1 · Serve the board

**Brief.** Run this repo's board as a Vite dev server, so the workflow can be used while it's
built. The page is Flex's review page; a small plugin builds its data from `.switchboard/board/`.

**Parts**

- [x] `board/`: Flex's page with the board's wording, served by Vite on port 5198
- [x] A plugin (`board/vite.config.ts`, `board/server/board.ts`) that builds `data.js` from
  `board.json`, `cards/<id>.json` and `log.json` on every request
- [x] Card images from `.switchboard/board/media/` or the ignored `private/media/`
- [x] Send posts to `/answers`, written to `state/answers.json`
- [x] Run by Switchboard itself as `switchboard/board/board`
- [x] The design review's content moved in as cards, private media left out

**Verifiables**

- The page loads every card from the folder, with no generated `data.js`
- A path outside the media folders isn't served
- A send arrives in `state/answers.json`

**Gate.** Done 2026-10-20: the first round trip, two cards asked and two answers received.

### A2 · The board CLI

**Brief.** The agent writes the board only through a CLI, never by editing the page or the
card files. The verbs mirror `flex-review`'s.

**Parts**

- [x] `scripts/board` (`board/bin/board.mjs`, no dependencies): `add`, `update`, `ask`,
  `remove`, `settle`, `working`, `log`, `list`, `answers`, `step`
- [x] One file per card, written atomically (temp file, then rename)
- [x] Refuses unknown steps, duplicate ids and unsafe ids
- [x] `update --changed "…"` makes a new version; moving a card isn't news
- [x] `ask --blocking`, and `step <id> done|now|next` for the plan

**Verifiables**

- Every verb tried on a throwaway copy of the board, its refusals included

**Gate.** Done 2026-10-20.

### A3 · Answers come straight back

**Brief.** What the author sends reaches the agent without copy and paste.

**Parts**

- [x] Send writes `state/answers.json`; the page shows "The agent has your answers"
- [x] `board answers` prints sends not yet read and remembers them (`state/read.json`)
- [x] `--wait` blocks until one lands; `--timeout` exits 124
- [x] Answering a card with a pick clears its Blocking mark, from any device

**Verifiables**

- `board answers --wait` returns within a second of a send

**Gate.** Done 2026-10-20. The agent keeps `board answers --wait` running; it never reads
with the output hidden, which would mark a send read unseen.

### A4 · Live arrival

**Brief.** A card the agent writes reaches the open page at once.

**Parts**

- [x] The plugin watches `.switchboard/board/` and sends `change` on `/events`
- [x] The page re-reads then, and polls every 15 s only as a fallback
- [x] New, asked and blocking cards at the top of Needs you, until opened

**Verifiables**

- About 150 ms from a CLI write to the push

**Gate.** Done 2026-10-20.

### P · The phone

**Brief.** When the agent asks for a card, the phone and iPad get a push, and the author
answers there. Built as Fuchi platform parts any Fuchi extension can combine; the board is the
first user. Moved up ahead of A5 on 2026-10-20; it stands in for B6. Plan:
`~/Documents/swift-apps/Fuchi/NOTIFY_PLAN.md`.

**Parts**

- [x] The card page, `card.html?id=…`, on the board server, on the local network
- [x] Pushes through the crotchet worker, with a notification category
- [x] Fuchi's notification view: long-press answers inline, without launching Fuchi
- [x] A tap goes in: a Fuchi page of the card
- [x] Go with my pick and Reply, from the notification
- [x] The notification says what the agent would do

**Verifiables**

- Checked on the iPhone and the iPad: a push arrives, each way of answering reaches
  `board answers --wait`, and the notification clears

**Gate.** Done 2026-10-20.

### B0 · Homes from the CLI

**Brief.** The board follows the project's home, and most projects pick one by hand today.
This lets the agent suggest a home and, on the author's yes, adopt it. Switchboard still never
writes into a repo unasked: the answer is the ask.

**Parts**

- [x] The suggestion rule in `sb-core` (`home.rs`), from how the author's projects are set up:
  (1) a repo holding more of the project's folders than any other → its root (kinpay,
  set-hero); (2) else, with the folders in separate repos, an `e2e` or `*-e2e` folder
  (career-path, citybuild, otesha); (3) else a folder holding the others, or a single folder's
  repo; (4) else no suggestion, just the list
- [x] The `keep` operation on the control socket (`engine.adopt_home`, which exists)
- [x] `switchboard keep <project> [folder] [--json]`: no folder shows the home or a suggestion
  and changes nothing (it reads the config, so the app needn't run); a folder adopts it through
  the running app, refusing to move an existing home
- [x] The app's Keep Settings in Repo… panel opens on the suggested folder, and names it
  (`suggest_home` in `sb-ffi`); the Keep menu lists it first, marked "(suggested)"; the
  sidebar's one-click Keep offer uses it too (it used the first group's folder)
- [x] The skill: "A project's home": propose the suggested folder, and run `switchboard keep`
  only on the author's yes
- [x] The stand-in's first card ("Keep this project's board in the repo?") names the folder,
  using `switchboard keep --json`, the others as options

**Verifiables**

- `cargo test -p sb-core home::` covers each rule
- `switchboard keep <project>` on the author's real projects suggests what they chose by hand:
  checked 2026-10-20 on all seven (career-path, citybuild and otesha → `e2e`; kinpay,
  set-hero → the repo root; thrive → `e2e` and switchboard → its root, both with no home yet)
- The Keep panel opens on the suggestion: checked 2026-10-20 on Thrive by driving the app
  (menu by name, `uidrive shotapp`, Escape to cancel)

**Gate.** A project with no home gets a card that names the right folder, and one yes makes
its home. Done 2026-10-20: this repo became the Switchboard project's home with
`switchboard keep switchboard ~/Documents/web/switchboard`.

**A consequence.** The board lives where the project's home is: prod-app belongs to the
set-hero project, whose home is `~/Documents/web/set-hero`, so its board would live there.

### A5 · The audit's gaps

**Brief.** What the board still lacks against the plan's own design, found in the audit.

**Parts**

- [ ] Reason labels on cards, each a filter (decided: decision, look closer, your question
  answered, changed, from the wishlist, for someone else)
- [ ] "For you to see" cards out of the Needs you count
- [ ] Decided cards into a searchable Record
- [ ] The overview as part of The plan in the sidebar (decided, `v-sidebar`): since you last
  looked, health, what's owed

**Verifiables**

- Screenshots of each, light and dark

**Design.** Built in the page's own design language (Flex's prototype, the Screening-room
design), like everything else on the board; screenshots for the author before the gate.

**Gate.** The board can be read at a glance: what needs you, what happened, what's owed.

### B1 · Engine and CLI

**Brief.** The board moves into Switchboard: a board store in `sb-core`, socket operations, and
`switchboard board …` in `sb-cli`. The stand-in's CLI and API switch over to it.

**Parts**

- [ ] A board store in `sb-core`: cards, answers, steps, agent state
- [ ] Socket operations and `switchboard board …`, with the stand-in's verbs
- [ ] The board verbs join the skill, now that every project can have a board

**Verifiables**

- `cargo test -p sb-core` covers the store and the socket operations

**Gate.** `scripts/board` retires; the stand-in page reads from Switchboard.

### B2 · The board in a project window

**Brief.** The board in SwiftUI in a project's window: list, rail and review mode, with card
bodies in a web view built from Track A's page, and the board as a pane beside a terminal.

**Parts**

- [ ] The board as a pane kind beside terminals
- [ ] List, rail and review mode in SwiftUI
- [ ] Card bodies in a `WKWebView`

**Verifiables**

- Driven with `scripts/dev/uidrive.swift`, and screenshots

**Gate.** A day's work done in the app, not the browser.

### B3 · Menu bar

**Brief.** The `MenuBarExtra` from [item 3](03-m5-remaining.md#menu-bar-extra), with the Needs
you count and a popover to answer the next card.

**Parts**

- [ ] The count across projects
- [ ] A popover with the next card and its buttons

**Verifiables**

- Screenshots of the popover, light and dark

**Gate.** A card answered from the menu bar.

### B4 · Notifications

**Brief.** Mac notifications only when an agent is blocked, with the recommended answer and a
reply field, like the phone's.

**Parts**

- [ ] A notification on `ask --blocking`
- [ ] Go with my pick and Reply

**Verifiables**

- A blocking card answered from a notification

**Gate.** No notification for anything that isn't blocking.

### B5 · Every project

**Brief.** The Boards window across projects; tear-off; agent state on project tiles.

**Parts**

- [ ] The Boards window
- [ ] Tear-off into its own window
- [ ] Agent state on tiles

**Verifiables**

- Two projects' boards in one window

**Gate.** Every project's Needs you in one place.

### B6 · Away from the Mac

**Brief.** A Switchboard app on the phone, on the same contract. Later; P covers asking and
answering until then.

**Parts**

- [ ] Switchboard Mobile on the board's contract

**Verifiables**

- The same board on the phone as on the Mac

**Gate.** A day's decisions made away from the desk.

## Open on the board

Answer these on the board before the step they affect:

- **The contract: Flex's core plus optional fields, or a fork** (`a-contract`). Shapes A2
  and B1.
- **Where cards live** (`a-data`) is now covered by `lc-commit` below; close it on the board.
- **Native SwiftUI, a web page, or both** (`lc-home`). Shapes B2.
- **The three window shapes** (`w-modes`). Shapes B2 and B5.
