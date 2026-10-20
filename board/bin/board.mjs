#!/usr/bin/env node
// The board CLI, Track A's stand-in for `switchboard board …` (plans/06-board.md, step A2).
// The agent writes the board only through this; it never edits the page or the card files.
//
//   board add <file|->            post a card (JSON); --ask asks for it now, --blocking marks it blocking
//   board update <id> <file|->    merge fields into a card; --changed "what" makes it a new version
//   board ask <id>                "I need this one now": a Needs you notification on the page and a push to
//                                 the phone (Fuchi's NOTIFY_PLAN); --blocking: I can't go on without it; --no-push
//   board remove <id>             take a card off the board
//   board settle <id> <text>      a call going ahead unless the author objects; --stage, --remove
//   board working <text>          what the agent is doing now; --stage, --item, --progress, --clear
//   board log <text>              add to the activity log; --item
//   board step <id> [done|now|next]  a step of the plan, shown in the sidebar with its state;
//                                 --title, --label ("A1"), --blurb, --doc <plan.md#A1>, --after <id>,
//                                 --remove (only when empty)
//   board list                    the cards, with whether each is still open
//   board answers                 sends not read yet (then marks them read); --wait, --timeout, --json, --all
//
// Where the board is (plans/06-board.md, decided on the board as lc-commit): it follows the project.
//   1. --dir, or BOARD_DIR
//   2. the nearest .switchboard/board/ above here: the project's home, committed with it
//   3. else the Switchboard project whose commands run in this folder: its board lives in the
//      app folder (Projects/<slug>/board/), not in git, and its first card offers Keep
import fs from "node:fs";
import { execFileSync } from "node:child_process";
import os from "node:os";
import path from "node:path";

const argv = process.argv.slice(2);
const flags = {};
const args = [];
for (let i = 0; i < argv.length; i++) {
  const a = argv[i];
  if (!a.startsWith("--")) { args.push(a); continue; }
  const [k, v] = a.slice(2).split("=", 2);
  if (v !== undefined) flags[k] = v;
  else if (i + 1 < argv.length && !argv[i + 1].startsWith("--") && !["ask", "clear", "wait", "json", "all", "remove", "blocking", "no-push"].includes(k)) flags[k] = argv[++i];
  else flags[k] = true;
}
const verb = args.shift();

function fail(msg) { console.error("board: " + msg); process.exit(1); }
const PROJECTS = process.env.SB_PROJECTS_DIR || path.join(os.homedir(), "Library", "Application Support", "Switchboard", "Projects");
let APP_SLUG = null;  // set when the board is in the app folder (no home yet)
function findDir() {
  if (flags.dir) return path.resolve(flags.dir);
  if (process.env.BOARD_DIR) return path.resolve(process.env.BOARD_DIR);
  for (let d = process.cwd(); ; d = path.dirname(d)) {
    if (fs.existsSync(path.join(d, ".switchboard", "board", "board.json"))) return path.join(d, ".switchboard", "board");
    if (path.dirname(d) === d) break;
  }
  // No home with a board: the Switchboard project whose commands run here (the longest match).
  const here = process.cwd(), expand = (p) => p.replace(/^~(?=$|\/)/, os.homedir());
  let best = null;
  for (const slug of fs.existsSync(PROJECTS) ? fs.readdirSync(PROJECTS) : []) {
    let yaml = ""; try { yaml = fs.readFileSync(path.join(PROJECTS, slug, "config.yaml"), "utf8"); } catch { continue; }
    for (const m of yaml.matchAll(/^\s*cwd:\s*"?([^"\n]+?)"?\s*$/gm)) {
      const c = path.resolve(expand(m[1]));
      if ((here === c || here.startsWith(c + path.sep)) && (!best || c.length > best.len)) best = { slug, len: c.length };
    }
  }
  if (!best) fail("no board here: no .switchboard/board/ above this folder, and no Switchboard project runs commands in it; pass --dir or set BOARD_DIR");
  APP_SLUG = best.slug;
  return path.join(PROJECTS, best.slug, "board");
}
const DIR = findDir();
const P = { board: path.join(DIR, "board.json"), cards: path.join(DIR, "cards"), log: path.join(DIR, "log.json"),
            answers: path.join(DIR, "state", "answers.json"), read: path.join(DIR, "state", "read.json") };
const read = (f, fb) => { try { return JSON.parse(fs.readFileSync(f, "utf8")); } catch { return fb; } };
// Write to a temp file and rename, so the page never reads half a file.
const write = (f, v) => { fs.mkdirSync(path.dirname(f), { recursive: true }); const t = f + ".tmp"; fs.writeFileSync(t, JSON.stringify(v, null, 2) + "\n"); fs.renameSync(t, f); };
const cardPath = (id) => { if (!/^[a-z0-9][a-z0-9._-]*$/i.test(id)) fail(`bad card id "${id}"`); return path.join(P.cards, id + ".json"); };
const cards = () => (fs.existsSync(P.cards) ? fs.readdirSync(P.cards).filter((f) => f.endsWith(".json")).map((f) => read(path.join(P.cards, f), null)).filter(Boolean) : []);
const now = () => Date.now();
function input(src) {
  if (!src) fail("give a JSON file, or - for stdin");
  const text = src === "-" ? fs.readFileSync(0, "utf8") : fs.readFileSync(src, "utf8");
  try { return JSON.parse(text); } catch (e) { fail("not JSON: " + e.message); }
}
function log(text, item) { const l = read(P.log, []); l.push({ t: now(), text, item: item || null }); write(P.log, l); }
function stageIds() { return (read(P.board, {}).stages || []).map((s) => s.id); }
// The phone and iPad hear about an asked card through the crotchet worker's push (Fuchi's
// NOTIFY_PLAN): category fuchi.view opens Fuchi's notification view on this card's page, which
// the board server serves on the local network.
function lanAddress() {
  const nets = os.networkInterfaces();
  const all = Object.entries(nets).flatMap(([name, a]) => (a || []).map((x) => ({ name, ...x })));
  const v4 = all.filter((x) => x.family === "IPv4" && !x.internal && !x.address.startsWith("169.254."));
  return (v4.find((x) => x.name === "en0") || v4[0] || {}).address || "localhost";
}
async function push(c) {
  const backend = process.env.FUCHI_BACKEND || "https://backend.wakyj07.workers.dev";
  const base = process.env.BOARD_URL || `http://${lanAddress()}:5198`;
  const project = read(P.board, {}).project || {};
  const d = c.decision || null, opts = (d && d.options) || [];
  const recs = opts.filter((o) => o.rec);
  // Sure: one recommendation, resting on evidence, not a pick-any. Only then is my pick one tap
  // away, like the board's "Enter takes my pick only when I'm sure".
  const sure = !!d && !d.unsure && !d.multi && recs.length === 1 && d.basis === "evidence";
  const lower = (t) => plain(t).charAt(0).toLowerCase() + plain(t).slice(1);
  const body = !d ? "Something to look at. Hold to see it."
    : d.unsure ? "I'm not sure: hold to look."
    : sure ? `I'd go with “${plain(recs[0].label)}”.` + (recs[0].then ? ` Then I'll ${lower(recs[0].then)}` : "")
    : recs.length ? `I'd lean “${plain(recs.map((o) => o.label).join("”, “"))}”: hold to see why.`
    : `${plain(d.q)} Hold to answer.`;
  // What each button posts: the same send the card page makes, so `board answers` reads it alike.
  const send = (pick, labels, way, note) => ({
    project: project.dir, from: "notification", objections: [],
    text: [`${project.name || "Board"}: from the notification`, plain(c.title) + ` [${c.id}]`,
           pick ? `    → ${labels.join(", ")}  (recommended)` : null, note ? `    note: ${note}` : null].filter(Boolean).join("\n"),
    answers: [{ id: c.id, stage: c.stage, title: plain(c.title), version: c.version || 1, pick, labels,
                rec: recs[0] ? recs[0].o : null, way, notes: note ? [{ text: note }] : [] }],
  });
  const actions = {};
  if (sure) actions.accept = { post: `${base}/answers`, body: send(recs[0].o, [plain(recs[0].label)], "agreed", null) };
  if (d) actions.reply = { post: `${base}/answers`, body: send(null, [], "note", "{{text}}") };
  const q = new URLSearchParams({
    topic: process.env.BOARD_PUSH_TOPIC || "fuchi",
    title: (c.blocking ? "Blocking · " : "") + plain(c.title),
    body,
    category: sure ? "fuchi.view.accept" : d ? "fuchi.view.reply" : "fuchi.view",
    data: JSON.stringify({ fuchi: JSON.stringify({ kind: "web", ext: "board", id: c.id, url: `${base}/card.html?id=${encodeURIComponent(c.id)}`, actions }) }),
  });
  try {
    const r = await fetch(`${backend}/firebase/notify?${q}`);
    const j = await r.json().catch(() => null);
    console.log(r.ok && j && j.success ? `pushed to the phone${sure ? " (with Go with my pick)" : ""}` : `push didn't go out (${r.status}${j ? "" : ", no reply"})`);
  } catch (e) {
    console.log("push didn't go out: " + e.message);
  }
}
function plain(h) { return String(h || "").replace(/<[^>]+>/g, ""); }

// A project with no home starts its board in the app folder. Its first card is Switchboard's
// own Keep, asked where the author will see it.
function startAppBoard() {
  if (!APP_SLUG || fs.existsSync(P.board)) return;
  // Switchboard suggests the folder (`switchboard keep <slug> --json`, plans/06-board.md B0).
  let sug = { suggested: null, reason: null, candidates: [] };
  try { sug = JSON.parse(execFileSync("switchboard", ["keep", APP_SLUG, "--json"], { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] })).suggestion || sug; } catch {}
  const short = (p) => String(p).replace(os.homedir(), "~");
  const others = (sug.candidates || []).filter((c) => c !== sug.suggested).slice(0, 3);
  write(P.board, { project: { name: APP_SLUG, meta: "Switchboard · not in a repo yet", dir: APP_SLUG }, stages: [{ id: "now", title: "Now", blurb: "What needs you" }] });
  write(cardPath("keep-board"), { id: "keep-board", stage: "now", order: 0, updated: now(), version: 1, tags: ["Setup"],
    title: "Keep this project's board in the repo?",
    summary: "This board lives in Switchboard's app folder, so it isn't in git or on your other Macs. Keeping the project's settings in a repo gives it a home, and the board moves into <code>.switchboard/board/</code> there.",
    decision: { q: "Keep it in the repo?", basis: "taste", why: ["A board in the repo travels with the code and its history; the app folder keeps it on this Mac only."],
      options: sug.suggested
        ? [{ o: "keep", label: `Keep it in ${short(sug.suggested)}`, sub: sug.reason || "", rec: true, then: `Run switchboard keep ${APP_SLUG} ${sug.suggested}, then move the board into the home.` },
           ...others.map((c, i) => ({ o: "other" + i, label: `Keep it in ${short(c)}`, then: `Run switchboard keep ${APP_SLUG} ${c}, then move the board into the home.` })),
           { o: "here", label: "No, leave it on this Mac" }]
        : [{ o: "keep", label: "Yes: Keep Settings in Repo", rec: true, sub: "pick the folder in Switchboard", then: "Ask you to choose Project › Keep Settings in Repo in Switchboard, then move the board into the home." },
           { o: "here", label: "No, leave it on this Mac" }] } });
}

switch (verb) {
  case "add": {
    startAppBoard();
    const c = input(args[0]);
    if (!c.id || !c.stage || !c.title) fail("a card needs id, stage and title");
    if (!stageIds().includes(c.stage)) fail(`no stage "${c.stage}"; stages: ${stageIds().join(", ")}`);
    const f = cardPath(c.id);
    if (fs.existsSync(f)) fail(`card ${c.id} exists; use update`);
    const same = cards().filter((x) => x.stage === c.stage);
    c.order ??= same.reduce((m, x) => Math.max(m, x.order || 0), 0) + 10;
    c.updated = now(); c.version = 1;
    if (flags.ask) c.askedAt = now();
    if (flags.blocking) c.blocking = true;
    write(f, c);
    if (flags.log !== false) log(flags.log || "Posted a card: " + plain(c.title), c.id);
    console.log(`added ${c.id}${flags.ask ? " (asked)" : ""}`);
    break;
  }
  case "update": {
    const id = args[0], f = cardPath(id), c = read(f, null);
    if (!c) fail(`no card ${id}`);
    const patch = input(args[1]);
    delete patch.id;
    if (patch.stage && !stageIds().includes(patch.stage)) fail(`no step "${patch.stage}"; steps: ${stageIds().join(", ")}`);
    Object.assign(c, patch);
    // Moving a card (its step, tags, order or blocking) isn't news; changing what it says is.
    if (Object.keys(patch).some((k) => !["stage", "tags", "order", "blocking"].includes(k))) c.updated = now();
    if (typeof flags.changed === "string") { c.version = (c.version || 1) + 1; c.changed = flags.changed; log("Changed: " + plain(c.title) + " (v" + c.version + ")", id); }
    write(f, c);
    console.log(`updated ${id}${c.changed && flags.changed ? " to v" + c.version : ""}`);
    break;
  }
  case "ask": {
    const id = args[0], f = cardPath(id), c = read(f, null);
    if (!c) fail(`no card ${id}`);
    c.askedAt = now(); if (flags.blocking) c.blocking = true;
    write(f, c); log((c.blocking ? "Blocked on you: " : "Asked: ") + plain(c.title), id);
    console.log(`asked ${id}${c.blocking ? " (blocking)" : ""}`);
    if (!flags["no-push"] && process.env.BOARD_PUSH !== "off") await push(c);
    break;
  }
  case "remove": {
    const id = args[0], f = cardPath(id);
    if (!fs.existsSync(f)) fail(`no card ${id}`);
    fs.unlinkSync(f); console.log(`removed ${id}`);
    break;
  }
  case "settle": {
    const b = read(P.board, {}); b.project ??= {}; const s = (b.project.settled ??= []);
    const id = args[0]; if (!id) fail("give an id");
    const key = id.startsWith("settled-") ? id : "settled-" + id;
    const i = s.findIndex((x) => x.id === key);
    if (flags.remove) { if (i < 0) fail(`no going-ahead call ${id}`); s.splice(i, 1); }
    else { const e = { id: key, stage: flags.stage || null, text: args.slice(1).join(" ") }; if (!e.text) fail("give the call's text"); if (i >= 0) s[i] = e; else s.push(e); }
    write(P.board, b); console.log(flags.remove ? `removed ${key}` : `going ahead: ${key}`);
    break;
  }
  case "working": {
    const b = read(P.board, {}); b.project ??= {};
    if (flags.clear) b.project.working = null;
    else {
      const text = args.join(" "); if (!text) fail("say what you're working on, or --clear");
      const w = b.project.working && b.project.working.text === text ? b.project.working : { since: now() };
      Object.assign(w, { text, stage: flags.stage || w.stage || null, item: flags.item || w.item || null });
      if (flags.progress !== undefined) w.progress = Number(flags.progress);
      b.project.working = w;
    }
    write(P.board, b); console.log(flags.clear ? "cleared" : "working: " + b.project.working.text);
    break;
  }
  case "log": {
    const text = args.join(" "); if (!text) fail("give the text");
    log(text, flags.item); console.log("logged");
    break;
  }
  case "step": {
    const b = read(P.board, {}); const st = (b.stages ??= []);
    const id = args[0]; if (!id) fail("give the step's id");
    const i = st.findIndex((x) => x.id === id);
    if (flags.remove) {
      if (i < 0) fail(`no step ${id}`);
      if (cards().some((c) => c.stage === id)) fail(`step ${id} still has cards; move them first`);
      st.splice(i, 1); write(P.board, b); console.log(`removed step ${id}`); break;
    }
    const state = args[1];
    if (state && !["done", "now", "next"].includes(state)) fail("a step is done, now or next");
    const e = i >= 0 ? st[i] : { id, title: id };
    for (const k of ["title", "label", "blurb", "doc"]) if (typeof flags[k] === "string") e[k] = flags[k];
    if (state) e.state = state;
    if (i < 0) {
      const at = typeof flags.after === "string" ? st.findIndex((x) => x.id === flags.after) : -1;
      st.splice(at >= 0 ? at + 1 : st.length, 0, e);
    }
    write(P.board, b);
    if (state === "now") log(`Now: ${e.label ? e.label + " · " : ""}${plain(e.title)}`, null);
    if (state === "done") log(`Done: ${e.label ? e.label + " · " : ""}${plain(e.title)}`, null);
    console.log(`${e.id}${e.state ? " · " + e.state : ""}`);
    break;
  }
  case "list": {
    const sends = read(P.answers, []);
    const answered = new Set(sends.flatMap((s) => (s.answers || []).map((a) => a.id)));
    for (const c of cards().sort((a, b) => stageIds().indexOf(a.stage) - stageIds().indexOf(b.stage) || (a.order || 0) - (b.order || 0)))
      console.log(`${answered.has(c.id) ? "✓" : c.decision ? "○" : "·"} ${c.id.padEnd(16)} ${c.stage.padEnd(12)} ${plain(c.title)}`);
    break;
  }
  case "answers": {
    const unread = () => { const all = read(P.answers, []); const r = read(P.read, { count: 0 }); return { all, r, fresh: flags.all ? all : all.slice(r.count) }; };
    const deadline = flags.timeout ? now() + Number(flags.timeout) * 1000 : Infinity;
    let u = unread();
    while (flags.wait && !u.fresh.length) {
      if (now() > deadline) { console.error("board: no answers yet"); process.exit(124); }
      await new Promise((res) => setTimeout(res, 1000));
      u = unread();
    }
    if (!u.fresh.length) { console.log("No new answers."); break; }
    if (flags.json) console.log(JSON.stringify(u.fresh, null, 2));
    else console.log(u.fresh.map((s) => s.text).join("\n\n"));
    if (!flags.all) write(P.read, { count: u.all.length, at: now() });
    break;
  }
  default:
    console.log(fs.readFileSync(new URL(import.meta.url), "utf8").split("\n").slice(1, 16).map((l) => l.replace(/^\/\/ ?/, "")).join("\n"));
    if (verb && verb !== "help") process.exit(1);
}
