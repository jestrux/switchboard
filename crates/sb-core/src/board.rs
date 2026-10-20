//! The board (plans/06-board.md): where a coding agent hands a project to its author. A board is
//! a folder of JSON files, in Flex's review contract plus the board's optional fields:
//!
//!   board.json         project, steps (stages with state, label, doc), going-ahead calls
//!   cards/<id>.json    one card each
//!   log.json           what the agent did
//!   record.json        every decision the author sent (written by the board server)
//!   kept.json          decisions whose work is done
//!   state/             the author's sends and the agent's read marker (not committed)
//!   private/           media that must never enter a repo (not committed)
//!
//! It follows the project: `<home>/.switchboard/board/` when the project keeps its settings in a
//! repo, else `Projects/<slug>/board/` in the app folder, whose first card offers Keep. The agent
//! writes it only through `switchboard board …`; the page (`board/` in this repo) reads the files.

use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const DIR: &str = "board";
pub const STATES: [&str; 4] = ["done", "now", "next", "later"];

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// A board's folder, and the app project it lives in when it has no home yet.
#[derive(Debug, Clone)]
pub struct Board {
    pub dir: PathBuf,
    pub app_slug: Option<String>,
}

/// Find the board for `start` (a working folder): the nearest `.switchboard/board/` above it, else
/// the board of the Switchboard project whose commands run in it (its home's, or its app folder's).
pub fn locate(start: &Path, projects_dir: &Path) -> Result<Board, String> {
    for d in start.ancestors() {
        let b = d.join(crate::fragment::DOT_DIR).join(DIR);
        if b.join("board.json").is_file() {
            return Ok(Board { dir: b, app_slug: None });
        }
    }
    let start = std::fs::canonicalize(start).unwrap_or_else(|_| start.to_path_buf());
    let mut best: Option<(usize, crate::project::Project)> = None;
    for p in crate::project::load_projects(projects_dir) {
        let Ok(cfg) = &p.effective else { continue };
        for c in cfg.resolve_commands() {
            let Some(cwd) = c.cwd else { continue };
            let cwd = std::fs::canonicalize(&cwd).unwrap_or(cwd);
            if start.starts_with(&cwd) && best.as_ref().map_or(true, |(n, _)| cwd.components().count() > *n) {
                best = Some((cwd.components().count(), p.clone()));
            }
        }
    }
    let Some((_, p)) = best else {
        return Err("no board here: no .switchboard/board/ above this folder, and no Switchboard project runs commands in it".into());
    };
    Ok(for_project(&p))
}

/// A project's board: in its home when it has one, else in its app folder.
pub fn for_project(p: &crate::project::Project) -> Board {
    match p.home() {
        Some(home) => Board { dir: home.join(crate::fragment::DOT_DIR).join(DIR), app_slug: None },
        None => Board { dir: p.dir.join(DIR), app_slug: Some(p.slug.clone()) },
    }
}

pub fn valid_id(id: &str) -> bool {
    let mut cs = id.chars();
    matches!(cs.next(), Some(c) if c.is_ascii_alphanumeric()) && cs.all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
}

/// The text of HTML, for log lines.
pub fn plain(h: &str) -> String {
    let mut out = String::new();
    let mut tag = false;
    for c in h.chars() {
        match c {
            '<' => tag = true,
            '>' => tag = false,
            _ if !tag => out.push(c),
            _ => {}
        }
    }
    out
}

fn read(path: &Path, fallback: Value) -> Value {
    std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(fallback)
}

/// Write to a temp file and rename, so a reader never sees half a file.
fn write(path: &Path, v: &Value) -> Result<(), String> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(v).map_err(|e| e.to_string())? + "\n";
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn obj(v: &mut Value) -> &mut Map<String, Value> {
    if !v.is_object() {
        *v = json!({});
    }
    v.as_object_mut().unwrap()
}

impl Board {
    fn p(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }
    fn card_path(&self, id: &str) -> Result<PathBuf, String> {
        if !valid_id(id) {
            return Err(format!("bad card id \"{id}\""));
        }
        Ok(self.dir.join("cards").join(format!("{id}.json")))
    }

    pub fn board(&self) -> Value {
        read(&self.p("board.json"), json!({}))
    }
    fn save_board(&self, b: &Value) -> Result<(), String> {
        write(&self.p("board.json"), b)
    }
    pub fn step_ids(&self) -> Vec<String> {
        self.board()["stages"].as_array().into_iter().flatten().filter_map(|s| s["id"].as_str().map(String::from)).collect()
    }
    pub fn card(&self, id: &str) -> Option<Value> {
        let p = self.card_path(id).ok()?;
        p.is_file().then(|| read(&p, Value::Null)).filter(|v| v.is_object())
    }
    pub fn cards(&self) -> Vec<Value> {
        let Ok(rd) = std::fs::read_dir(self.dir.join("cards")) else { return vec![] };
        let mut v: Vec<Value> = rd
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .map(|e| read(&e.path(), Value::Null))
            .filter(|c| c["id"].is_string())
            .collect();
        let order = self.step_ids();
        let rank = |c: &Value| order.iter().position(|s| Some(s.as_str()) == c["stage"].as_str()).unwrap_or(99);
        v.sort_by(|a, b| rank(a).cmp(&rank(b)).then(a["order"].as_i64().unwrap_or(0).cmp(&b["order"].as_i64().unwrap_or(0))));
        v
    }
    fn save_card(&self, c: &Value) -> Result<(), String> {
        let id = c["id"].as_str().ok_or("a card needs an id")?;
        write(&self.card_path(id)?, c)
    }

    pub fn log(&self, text: &str, item: Option<&str>) -> Result<(), String> {
        let mut l = read(&self.p("log.json"), json!([]));
        l.as_array_mut().ok_or("log.json isn't a list")?.push(json!({ "t": now(), "text": text, "item": item }));
        write(&self.p("log.json"), &l)
    }

    /// A project with no home starts its board in the app folder. Its first card is Switchboard's
    /// own Keep, naming the suggested folder (sb_core::home), asked where the author will see it.
    pub fn start_app_board(&self, project: Option<&crate::project::Project>) -> Result<(), String> {
        let (Some(slug), false) = (&self.app_slug, self.p("board.json").is_file()) else { return Ok(()) };
        self.save_board(&json!({
            "project": { "name": slug, "meta": "Switchboard · not in a repo yet", "dir": slug },
            "stages": [{ "id": "now", "title": "Now", "blurb": "What needs you" }],
        }))?;
        let sug = project.map(crate::home::suggest);
        let short = |p: &Path| crate::config::collapse_tilde(p);
        let mut options = vec![];
        if let Some(s) = sug.as_ref().and_then(|s| s.suggested.clone().map(|p| (p, s))) {
            let (p, s) = s;
            options.push(json!({ "o": "keep", "label": format!("Keep it in {}", short(&p)), "sub": s.reason.clone().unwrap_or_default(), "rec": true,
                                 "then": format!("Run switchboard keep {slug} {}, then move the board into the home.", p.display()) }));
            for (i, c) in s.candidates.iter().filter(|c| **c != p).take(3).enumerate() {
                options.push(json!({ "o": format!("other{i}"), "label": format!("Keep it in {}", short(c)),
                                     "then": format!("Run switchboard keep {slug} {}, then move the board into the home.", c.display()) }));
            }
        } else {
            options.push(json!({ "o": "keep", "label": "Yes: Keep Settings in Repo", "sub": "pick the folder in Switchboard", "rec": true,
                                 "then": "Ask you to choose Project › Keep Settings in Repo in Switchboard, then move the board into the home." }));
        }
        options.push(json!({ "o": "here", "label": "No, leave it on this Mac" }));
        self.save_card(&json!({
            "id": "keep-board", "stage": "now", "order": 0, "updated": now(), "version": 1, "tags": ["Setup"],
            "title": "Keep this project's board in the repo?",
            "summary": "This board lives in Switchboard's app folder, so it isn't in git or on your other Macs. Keeping the project's settings in a repo gives it a home, and the board moves into <code>.switchboard/board/</code> there.",
            "decision": { "q": "Keep it in the repo?", "basis": "taste",
                          "why": ["A board in the repo travels with the code and its history; the app folder keeps it on this Mac only."],
                          "options": options },
        }))
    }

    /// Post a card. It needs id, stage and title; the stage must be one of the board's steps.
    pub fn add(&self, mut card: Value, ask: bool, blocking: bool) -> Result<Value, String> {
        let (id, stage, title) = (card["id"].as_str(), card["stage"].as_str(), card["title"].as_str());
        let (Some(id), Some(stage), Some(_)) = (id, stage, title) else { return Err("a card needs id, stage and title".into()) };
        let (id, stage) = (id.to_string(), stage.to_string());
        let steps = self.step_ids();
        if !steps.contains(&stage) {
            return Err(format!("no step \"{stage}\"; steps: {}", steps.join(", ")));
        }
        if self.card_path(&id)?.is_file() {
            return Err(format!("card {id} exists; use update"));
        }
        let last = self.cards().iter().filter(|c| c["stage"] == stage.as_str()).filter_map(|c| c["order"].as_i64()).max().unwrap_or(0);
        let o = obj(&mut card);
        o.entry("order").or_insert(json!(last + 10));
        o.insert("updated".into(), json!(now()));
        o.insert("version".into(), json!(1));
        if ask {
            o.insert("askedAt".into(), json!(now()));
        }
        if blocking {
            o.insert("blocking".into(), json!(true));
        }
        self.save_card(&card)?;
        Ok(card)
    }

    /// Merge fields into a card. `changed` makes a new version (the author sees it come back);
    /// moving a card (step, tags, order, blocking) or a `quiet` fix isn't news.
    pub fn update(&self, id: &str, patch: Value, changed: Option<&str>, quiet: bool) -> Result<Value, String> {
        let mut c = self.card(id).ok_or(format!("no card {id}"))?;
        let mut patch = patch;
        let p = obj(&mut patch);
        p.remove("id");
        if let Some(st) = p.get("stage").and_then(|v| v.as_str()) {
            let steps = self.step_ids();
            if !steps.iter().any(|s| s == st) {
                return Err(format!("no step \"{st}\"; steps: {}", steps.join(", ")));
            }
        }
        let news = !quiet && p.keys().any(|k| !["stage", "tags", "order", "blocking"].contains(&k.as_str()));
        let co = obj(&mut c);
        for (k, v) in p.iter() {
            co.insert(k.clone(), v.clone());
        }
        if news || changed.is_some() {
            co.insert("updated".into(), json!(now()));
        }
        if let Some(what) = changed {
            let v = co.get("version").and_then(|v| v.as_i64()).unwrap_or(1) + 1;
            co.insert("version".into(), json!(v));
            co.insert("changed".into(), json!(what));
        }
        self.save_card(&c)?;
        Ok(c)
    }

    /// "I need this one now": the page and the phone say so. `blocking`: the agent can't go on.
    pub fn ask(&self, id: &str, blocking: bool) -> Result<Value, String> {
        let mut c = self.card(id).ok_or(format!("no card {id}"))?;
        let o = obj(&mut c);
        o.insert("askedAt".into(), json!(now()));
        if blocking {
            o.insert("blocking".into(), json!(true));
        }
        self.save_card(&c)?;
        Ok(c)
    }

    pub fn remove(&self, id: &str) -> Result<(), String> {
        let p = self.card_path(id)?;
        if !p.is_file() {
            return Err(format!("no card {id}"));
        }
        std::fs::remove_file(p).map_err(|e| e.to_string())
    }

    /// A call going ahead unless the author objects (`text`), or off the list (`None`).
    pub fn settle(&self, id: &str, text: Option<&str>, stage: Option<&str>) -> Result<String, String> {
        let key = if id.starts_with("settled-") { id.to_string() } else { format!("settled-{id}") };
        let mut b = self.board();
        let proj = obj(obj(&mut b).entry("project").or_insert(json!({})));
        let list = proj.entry("settled").or_insert(json!([])).as_array_mut().ok_or("settled isn't a list")?;
        let i = list.iter().position(|x| x["id"] == key.as_str());
        match (text, i) {
            (None, Some(i)) => {
                list.remove(i);
            }
            (None, None) => return Err(format!("no going-ahead call {id}")),
            (Some(t), i) => {
                let e = json!({ "id": key, "stage": stage, "text": t });
                match i {
                    Some(i) => list[i] = e,
                    None => list.push(e),
                }
            }
        }
        self.save_board(&b)?;
        Ok(key)
    }

    /// What the agent is doing now (`None` clears it).
    pub fn working(&self, text: Option<&str>, stage: Option<&str>, item: Option<&str>, progress: Option<f64>) -> Result<(), String> {
        let mut b = self.board();
        let proj = obj(obj(&mut b).entry("project").or_insert(json!({})));
        match text {
            None => {
                proj.insert("working".into(), Value::Null);
            }
            Some(t) => {
                let prev = proj.get("working").cloned().unwrap_or(Value::Null);
                let mut w = if prev["text"] == t { prev } else { json!({ "since": now() }) };
                let wo = obj(&mut w);
                wo.insert("text".into(), json!(t));
                if let Some(s) = stage.map(String::from).or_else(|| wo.get("stage").and_then(|v| v.as_str()).map(String::from)) {
                    wo.insert("stage".into(), json!(s));
                }
                if let Some(i) = item.map(String::from).or_else(|| wo.get("item").and_then(|v| v.as_str()).map(String::from)) {
                    wo.insert("item".into(), json!(i));
                }
                if let Some(p) = progress {
                    wo.insert("progress".into(), json!(p));
                }
                proj.insert("working".into(), w);
            }
        }
        self.save_board(&b)
    }

    /// A step of the plan: its state (done, now, next, later), label, title, blurb and `doc`
    /// (the plan section its panel shows). New steps go at the end, or after `after`.
    pub fn step(&self, id: &str, state: Option<&str>, fields: &[(&str, &str)], after: Option<&str>) -> Result<Value, String> {
        if let Some(s) = state {
            if !STATES.contains(&s) {
                return Err("a step is done, now, next or later".into());
            }
        }
        let mut b = self.board();
        let stages = obj(&mut b).entry("stages").or_insert(json!([])).as_array_mut().ok_or("stages isn't a list")?;
        let i = stages.iter().position(|s| s["id"] == id);
        let mut e = i.map(|i| stages[i].clone()).unwrap_or_else(|| json!({ "id": id, "title": id }));
        let eo = obj(&mut e);
        for (k, v) in fields {
            eo.insert((*k).into(), json!(v));
        }
        if let Some(s) = state {
            eo.insert("state".into(), json!(s));
        }
        match i {
            Some(i) => stages[i] = e.clone(),
            None => {
                let at = after.and_then(|a| stages.iter().position(|s| s["id"] == a)).map(|p| p + 1).unwrap_or(stages.len());
                stages.insert(at, e.clone());
            }
        }
        self.save_board(&b)?;
        Ok(e)
    }

    pub fn remove_step(&self, id: &str) -> Result<(), String> {
        if self.cards().iter().any(|c| c["stage"] == id) {
            return Err(format!("step {id} still has cards; move them first"));
        }
        let mut b = self.board();
        let stages = obj(&mut b).entry("stages").or_insert(json!([])).as_array_mut().ok_or("stages isn't a list")?;
        let n = stages.len();
        stages.retain(|s| s["id"] != id);
        if stages.len() == n {
            return Err(format!("no step {id}"));
        }
        self.save_board(&b)
    }

    /// A decision's work is done (`commit` kept it), or owed again (`undo`).
    pub fn kept(&self, id: &str, commit: Option<&str>, undo: bool) -> Result<(), String> {
        let path = self.p("kept.json");
        let mut k = read(&path, json!({}));
        if undo {
            obj(&mut k).remove(id);
        } else {
            let rec = read(&self.p("record.json"), json!([]));
            if !rec.as_array().into_iter().flatten().any(|r| r["id"] == id && !r["pick"].is_null()) {
                return Err(format!("{id} has no decision in the Record"));
            }
            obj(&mut k).insert(id.into(), json!({ "at": now(), "commit": commit }));
        }
        write(&path, &k)
    }

    /// The author's sends the agent hasn't read (or all of them), and the total.
    pub fn sends(&self, all: bool) -> (Vec<Value>, usize) {
        let sends = read(&self.p("state/answers.json"), json!([]));
        let sends = sends.as_array().cloned().unwrap_or_default();
        let seen = read(&self.p("state/read.json"), json!({ "count": 0 }))["count"].as_u64().unwrap_or(0) as usize;
        let fresh = if all { sends.clone() } else { sends.iter().skip(seen).cloned().collect() };
        (fresh, sends.len())
    }
    pub fn mark_read(&self, count: usize) -> Result<(), String> {
        write(&self.p("state/read.json"), &json!({ "count": count, "at": now() }))
    }

    /// The whole board, as the page and the app read it (docs/board-contract.md): project, steps,
    /// cards (in step order), log, Record and kept.
    pub fn snapshot(&self) -> Value {
        let b = self.board();
        json!({
            "project": b["project"].clone(), "stages": b["stages"].clone(), "items": self.cards(),
            "log": read(&self.p("log.json"), json!([])), "record": read(&self.p("record.json"), json!([])),
            "kept": read(&self.p("kept.json"), json!({})),
        })
    }

    /// Record a send from the author (the page, the phone, a notification, the app): keep it in
    /// `state/answers.json` for the agent, write each decision to the Record, and clear Blocking
    /// on every card with a pick (a note alone doesn't unblock).
    pub fn record_send(&self, send: Value) -> Result<(), String> {
        let mut send = send;
        obj(&mut send).insert("receivedAt".into(), json!(now()));
        let path = self.p("state/answers.json");
        let mut sends = read(&path, json!([]));
        sends.as_array_mut().ok_or("answers.json isn't a list")?.push(send.clone());
        write(&path, &sends)?;
        let mut record = read(&self.p("record.json"), json!([]));
        let rec = record.as_array_mut().ok_or("record.json isn't a list")?;
        let at = send["sentAt"].as_i64().unwrap_or_else(now);
        let from = send["from"].as_str().unwrap_or("board").to_string();
        for a in send["answers"].as_array().into_iter().flatten() {
            let Some(id) = a["id"].as_str() else { continue };
            let notes: Vec<Value> = a["notes"].as_array().into_iter().flatten().filter_map(|n| n["text"].as_str()).filter(|t| !t.is_empty()).map(|t| json!(t)).collect();
            let picked = !(a["pick"].is_null() || a["pick"].as_array().is_some_and(|p| p.is_empty()));
            if !picked && notes.is_empty() {
                continue;
            }
            rec.push(json!({ "id": id, "title": a["title"].as_str().unwrap_or(id), "step": a["stage"].as_str().unwrap_or(""),
                             "version": a["version"].as_i64().unwrap_or(1), "pick": if picked { a["pick"].clone() } else { Value::Null },
                             "labels": a["labels"].clone(), "rec": a["rec"].clone(), "way": a["way"].as_str().unwrap_or(""),
                             "notes": notes, "at": at, "from": from }));
            if picked {
                if let Some(mut c) = self.card(id) {
                    if c["blocking"] == true {
                        obj(&mut c).insert("blocking".into(), json!(false));
                        self.save_card(&c)?;
                    }
                }
            }
        }
        write(&self.p("record.json"), &record)
    }

    /// Has the author sent a pick for this card (any version)?
    pub fn answered(&self) -> std::collections::HashSet<String> {
        let rec = read(&self.p("record.json"), json!([]));
        rec.as_array().into_iter().flatten().filter(|r| !r["pick"].is_null()).filter_map(|r| r["id"].as_str().map(String::from)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board() -> (tempfile::TempDir, Board) {
        let t = tempfile::tempdir().unwrap();
        let b = Board { dir: t.path().join("board"), app_slug: None };
        b.step("a1", Some("now"), &[("title", "First")], None).unwrap();
        (t, b)
    }

    #[test]
    fn add_validates_and_numbers() {
        let (_t, b) = board();
        assert!(b.add(json!({ "id": "x", "stage": "nope", "title": "X" }), false, false).unwrap_err().contains("no step"));
        assert!(b.add(json!({ "id": "../x", "stage": "a1", "title": "X" }), false, false).unwrap_err().contains("bad card id"));
        let c = b.add(json!({ "id": "x", "stage": "a1", "title": "X" }), true, true).unwrap();
        assert_eq!(c["version"], 1);
        assert_eq!(c["order"], 10);
        assert_eq!(c["blocking"], true);
        assert!(c["askedAt"].is_i64());
        assert!(b.add(json!({ "id": "x", "stage": "a1", "title": "X" }), false, false).unwrap_err().contains("exists"));
        assert_eq!(b.add(json!({ "id": "y", "stage": "a1", "title": "Y" }), false, false).unwrap()["order"], 20);
    }

    #[test]
    fn update_versions_and_moves_quietly() {
        let (_t, b) = board();
        b.step("b1", Some("next"), &[], None).unwrap();
        let c = b.add(json!({ "id": "x", "stage": "a1", "title": "X" }), false, false).unwrap();
        let moved = b.update("x", json!({ "stage": "b1" }), None, false).unwrap();
        assert_eq!(moved["updated"], c["updated"]);
        let v2 = b.update("x", json!({ "summary": "S" }), Some("added a summary"), false).unwrap();
        assert_eq!(v2["version"], 2);
        assert_eq!(v2["changed"], "added a summary");
        assert!(b.update("x", json!({ "stage": "zz" }), None, false).is_err());
    }

    #[test]
    fn a_send_is_recorded_and_unblocks() {
        let (_t, b) = board();
        b.add(json!({ "id": "x", "stage": "a1", "title": "X" }), true, true).unwrap();
        b.record_send(json!({ "from": "app", "sentAt": 5, "answers": [{ "id": "x", "title": "X", "pick": null, "notes": [{ "text": "hm" }] }] })).unwrap();
        assert_eq!(b.card("x").unwrap()["blocking"], true);
        b.record_send(json!({ "from": "app", "answers": [{ "id": "x", "title": "X", "pick": "yes", "labels": ["Yes"] }] })).unwrap();
        assert_eq!(b.card("x").unwrap()["blocking"], false);
        let snap = b.snapshot();
        assert_eq!(snap["record"].as_array().unwrap().len(), 2);
        assert_eq!(snap["record"][0]["notes"][0], "hm");
        assert_eq!(b.sends(false).0.len(), 2);
        assert!(b.answered().contains("x"));
    }

    #[test]
    fn steps_settle_working_and_reads() {
        let (_t, b) = board();
        b.step("b0", Some("done"), &[("label", "B0")], Some("a1")).unwrap();
        assert_eq!(b.step_ids(), vec!["a1", "b0"]);
        assert!(b.step("a1", Some("soon"), &[], None).is_err());
        b.add(json!({ "id": "x", "stage": "b0", "title": "X" }), false, false).unwrap();
        assert!(b.remove_step("b0").is_err());
        assert_eq!(b.settle("s1", Some("A call"), Some("a1")).unwrap(), "settled-s1");
        b.settle("s1", None, None).unwrap();
        assert!(b.settle("s1", None, None).is_err());
        b.working(Some("Doing"), Some("a1"), None, Some(0.5)).unwrap();
        assert_eq!(b.board()["project"]["working"]["progress"], 0.5);
        std::fs::create_dir_all(b.dir.join("state")).unwrap();
        std::fs::write(b.dir.join("state/answers.json"), r#"[{"text":"one"},{"text":"two"}]"#).unwrap();
        let (fresh, n) = b.sends(false);
        assert_eq!((fresh.len(), n), (2, 2));
        b.mark_read(n).unwrap();
        assert_eq!(b.sends(false).0.len(), 0);
        assert!(b.kept("x", None, false).is_err());
    }
}
