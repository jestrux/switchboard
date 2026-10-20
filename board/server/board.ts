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
import { marked } from "marked";

export type Card = { id: string; stage: string; order?: number; [k: string]: unknown };

function readJson<T>(file: string, fallback: T): T {
  try {
    return JSON.parse(fs.readFileSync(file, "utf8")) as T;
  } catch {
    return fallback;
  }
}

type Step = { id: string; label?: string; doc?: string; [k: string]: unknown };

/** A step's `doc` ("plans/06-board.md#A1") is its section of the plan: the board shows it in the
 *  step's panel, so the plan is written once. The path is from the repo (two folders above
 *  `.switchboard/board/`) unless absolute; `#A1` is the heading that starts with that label. */
function withDoc(dir: string, step: Step): Step {
  if (!step.doc) return step;
  const [file, anchor] = step.doc.split("#");
  const full = path.isAbsolute(file) ? file : path.resolve(dir, "..", "..", file);
  let md = "";
  try { md = fs.readFileSync(full, "utf8"); } catch { return { ...step, docError: `can't read ${step.doc}` }; }
  if (anchor) {
    const lines = md.split("\n");
    const esc = anchor.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const start = lines.findIndex((l) => new RegExp(`^#{2,4}\\s+${esc}(\\s|$)`).test(l));
    if (start < 0) return { ...step, docError: `no section ${anchor} in ${file}` };
    const level = lines[start].match(/^#+/)![0].length;
    const end = lines.findIndex((l, i) => i > start && /^#+\s/.test(l) && l.match(/^#+/)![0].length <= level);
    md = lines.slice(start + 1, end < 0 ? undefined : end).join("\n");
  }
  const done = (md.match(/^\s*- \[x\]/gim) || []).length, open = (md.match(/^\s*- \[ \]/gm) || []).length;
  return { ...step, docHtml: marked.parse(md, { async: false, gfm: true }) as string, parts: { done, total: done + open } };
}

/** The board as the page's data contract (`window.FLEX_REVIEW`). */
export function loadBoard(dir: string) {
  const board = readJson<{ project?: object; stages?: Step[] }>(path.join(dir, "board.json"), {});
  const stages = (board.stages ?? []).map((s) => withDoc(dir, s));
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
  return { project: board.project ?? {}, stages, items: cards, log: readJson(path.join(dir, "log.json"), []), record: readJson(path.join(dir, "record.json"), []) };
}

/** One decision, as the Record keeps it (A5): what was asked, what was picked or said, when
 *  and from where. Kept in `record.json`, committed, so a card answered anywhere is handled
 *  everywhere and the decision outlives the card. */
type Decision = { id: string; title: string; step: string; version: number; pick: unknown; labels: string[];
                  rec: unknown; way: string; notes: string[]; at: number; from: string };
type Answer = { id?: string; stage?: string; title?: string; version?: number; pick?: unknown; labels?: string[];
                rec?: unknown; way?: string; notes?: { text?: string }[] };

export function recordDecisions(dir: string, send: { answers?: Answer[]; from?: string; sentAt?: number }) {
  const file = path.join(dir, "record.json");
  const record = readJson<Decision[]>(file, []);
  for (const a of send.answers ?? []) {
    if (!a.id) continue;
    const notes = (a.notes ?? []).map((n) => String(n.text ?? "")).filter(Boolean);
    const picked = a.pick != null && !(Array.isArray(a.pick) && !a.pick.length);
    if (!picked && !notes.length) continue;
    record.push({ id: a.id, title: String(a.title ?? a.id), step: String(a.stage ?? ""), version: a.version ?? 1,
                  pick: picked ? a.pick : null, labels: a.labels ?? [], rec: a.rec ?? null, way: String(a.way ?? ""),
                  notes, at: send.sentAt ?? Date.now(), from: send.from ?? "board" });
  }
  fs.writeFileSync(file + ".tmp", JSON.stringify(record, null, 2) + "\n");
  fs.renameSync(file + ".tmp", file);
}

/** Appends one send from the page to `.switchboard/board/state/answers.json`, newest last. */
export function recordSend(dir: string, send: unknown) {
  const file = path.join(dir, "state", "answers.json");
  fs.mkdirSync(path.dirname(file), { recursive: true });
  const sends = readJson<unknown[]>(file, []);
  sends.push({ receivedAt: Date.now(), ...(send as object) });
  fs.writeFileSync(file, JSON.stringify(sends, null, 2) + "\n");
  recordDecisions(dir, send as { answers?: Answer[] });
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
