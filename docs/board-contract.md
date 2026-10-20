---
id: board-contract
title: The board's contract
category: reference
description: The files a project's board is made of, and every field an agent can write.
---

# The board's contract

The board is where a coding agent hands a project to its author (`plans/06-board.md`). It's a
folder of JSON files; the agent writes it only with `switchboard board …`, and the page reads
it as `window.BOARD`. The contract started from Flex's review prototype and is now the board's
own (decided on the board, `a-contract`, 2026-10-20): Flex's film-only fields aren't part of it,
though the page still tolerates them.

## Where it lives

It follows the project: `<home>/.switchboard/board/` when the project keeps its settings in a
repo, else `Projects/<slug>/board/` in Switchboard's app folder.

| File | Holds | In git |
|---|---|---|
| `board.json` | `project`, `stages` (the plan's steps) | yes |
| `cards/<id>.json` | one card each | yes |
| `log.json` | what the agent did: `[{ t, text, item }]` | yes |
| `record.json` | every decision the author sent | yes |
| `kept.json` | decisions whose work is done: `{ <id>: { at, commit } }` | yes |
| `state/answers.json`, `state/read.json` | the author's sends; the agent's read marker | no |

Times are milliseconds since 1970. HTML is allowed in titles, summaries and bodies.

## `board.json`

```json
{
  "project": {
    "name": "Switchboard", "meta": "one line under the name", "dir": "a stable key",
    "intro": "shown on first visit",
    "working": { "text": "Building B1", "stage": "b1", "item": "card-id", "since": 0, "progress": 0.4 },
    "settled": [{ "id": "settled-x", "stage": "b1", "text": "a call going ahead unless the author objects" }]
  },
  "stages": [
    { "id": "b1", "label": "B1", "title": "Engine and CLI", "blurb": "one line",
      "state": "now", "doc": "plans/06-board.md#B1" }
  ]
}
```

- A stage is a step of the plan. `state` is `done`, `now`, `next` or `later` (deferred, not
  counted). `doc` names the plan section the step's panel shows (path from the repo, `#` the
  heading that starts with that label).

## A card

```json
{
  "id": "lc-commit", "stage": "b0", "title": "Where does the board live?", "order": 10,
  "summary": "one paragraph", "tags": ["Data"],
  "updated": 0, "version": 1, "changed": "what changed in this version",
  "askedAt": 0, "blocking": true, "reason": "answer", "for": "Luke",
  "body": { "html": "", "checks": [{ "state": "ok", "text": "" }], "beats": [], "gallery": [],
            "image": "", "video": "", "compare": [], "fields": [] },
  "decision": {
    "q": "the question", "basis": "evidence", "multi": false,
    "why": ["why I recommend it"],
    "unsure": { "why": "why I'm unsure", "where": "where to look" },
    "options": [{ "o": "home", "label": "Follow the project", "sub": "", "rec": true,
                  "then": "what the agent will do", "out": [["Label", "what changes", false]] }]
  }
}
```

- **Required:** `id` (letters, digits, `.`, `_`, `-`), `stage` (a step's id), `title`.
- **Set by the CLI:** `order`, `updated`, `version`; `changed` and a new version with
  `update --changed`; `askedAt` with `ask`; `blocking` with `ask --blocking`.
- **Why it's here:** worked out (a decision, look closer when `unsure`, changed, for you to see
  with no decision) unless `reason` says `answer` (your question, answered) or `others` (with
  `for`, a name).
- **A decision** is answered by picking options (`multi` for several) and/or a note. `basis` is
  `evidence` or `taste`; only an evidence-based single recommendation is one tap away on the
  phone ("Go with my pick").
- **Bodies** are any of: `html`, `checks`, `beats` (`{ at, title, text }`), `gallery`
  (`{ src, at, caption }`), `image`, `video`, `compare` (`{ label, image, caption }`), `fields`
  (`{ label, text }`, each copyable).
- **Images and videos stay where they are.** A card names the file in place, `~/Downloads/shot.png`
  or an absolute path, in any of the fields above or an `<img src>` in its HTML; nothing is copied
  into the board, so there's nothing to clean up when a card goes. The page shows a file only
  when a card on its board names it, it's an image, video, audio or PDF, and it's inside the home
  folder. A web URL works as usual. The phone's card page (a push) loads from the Mac's board
  server, so it shows them too while it's on the same network; away from it, nothing on the card
  loads (B6).

## A send, and the Record

The page posts a send to the board server: `{ project, sentAt, from, text, answers: [{ id,
stage, title, version, pick, labels, rec, way, notes: [{ text }] }], objections }`. `from` is
`board`, `card` (the phone's card page) or `notification`. The server keeps it in
`state/answers.json` and writes each decision to `record.json`: `{ id, title, step, version,
pick, labels, rec, way, notes, at, from }`. `way` is `agreed`, `way` (against the
recommendation) or `note`.
