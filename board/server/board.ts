// The stand-in for the board's engine (plans/06-board.md, Track A). Reads a board from a
// `.switchboard/board/` folder and records the author's answers there. Switchboard's own board store
// replaces this in Track B; the page and the folder layout stay the same.
//
//   .switchboard/board/board.json        project, stages, going-ahead calls
//   .switchboard/board/cards/<id>.json   one card each; `order` sorts cards within their stage
//   .switchboard/board/log.json          what the agent did, oldest first
//   .switchboard/board/private/media/    images the cards show that never enter the repo (ignored)
//   .switchboard/board/state/            the author's sends (ignored)
import fs from "node:fs";
import path from "node:path";

export type Card = { id: string; stage: string; order?: number; [k: string]: unknown };

function readJson<T>(file: string, fallback: T): T {
  try {
    return JSON.parse(fs.readFileSync(file, "utf8")) as T;
  } catch {
    return fallback;
  }
}

/** The board as the page's data contract (`window.FLEX_REVIEW`). */
export function loadBoard(dir: string) {
  const board = readJson<{ project?: object; stages?: { id: string }[] }>(path.join(dir, "board.json"), {});
  const stages = board.stages ?? [];
  const rank = new Map(stages.map((s, i) => [s.id, i]));
  const cardsDir = path.join(dir, "cards");
  const cards: Card[] = fs.existsSync(cardsDir)
    ? fs
        .readdirSync(cardsDir)
        .filter((f) => f.endsWith(".json"))
        .map((f) => readJson<Card | null>(path.join(cardsDir, f), null))
        .filter((c): c is Card => !!c && typeof c.id === "string")
    : [];
  cards.sort(
    (a, b) =>
      (rank.get(a.stage) ?? 99) - (rank.get(b.stage) ?? 99) ||
      (a.order ?? 0) - (b.order ?? 0) ||
      a.id.localeCompare(b.id),
  );
  return { project: board.project ?? {}, stages, items: cards, log: readJson(path.join(dir, "log.json"), []) };
}

/** Appends one send from the page to `.switchboard/board/state/answers.json`, newest last. */
export function recordSend(dir: string, send: unknown) {
  const file = path.join(dir, "state", "answers.json");
  fs.mkdirSync(path.dirname(file), { recursive: true });
  const sends = readJson<unknown[]>(file, []);
  sends.push({ receivedAt: Date.now(), ...(send as object) });
  fs.writeFileSync(file, JSON.stringify(sends, null, 2) + "\n");
  // A card with a pick no longer blocks the agent (a note alone doesn't unblock it). The path
  // check matches the CLI's id rule.
  for (const a of ((send as { answers?: { id?: string; pick?: unknown }[] }).answers ?? [])) {
    if (a.pick == null || (Array.isArray(a.pick) && !a.pick.length)) continue;
    const card = path.join(dir, "cards", String(a.id) + ".json");
    const c = readJson<Card | null>(card, null);
    if (/^[a-z0-9][a-z0-9._-]*$/i.test(String(a.id)) && c && c.blocking) {
      c.blocking = false;
      fs.writeFileSync(card + ".tmp", JSON.stringify(c, null, 2) + "\n");
      fs.renameSync(card + ".tmp", card);
    }
  }
}
