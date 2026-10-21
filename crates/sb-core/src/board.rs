//! The board (plans/06-board.md): where a coding agent hands a project to its author. A board is
//! a folder of JSON files, in Flex's review contract plus the board's optional fields:
//!
//!   board.json         project, steps (stages with state, label, doc), going-ahead calls
//!   cards/<id>.json    one card each
//!   log.json           what the agent did
//!   record.json        every decision the author sent (written by the board server)
//!   kept.json          decisions whose work is done
//!   state/             the author's sends and the agent's read marker (not committed)
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
        check_card(&card)?;
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
        check_card(&c)?;
        let co = obj(&mut c);
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
    /// A step of the plan: its state and its fields (`title`, `label`, `blurb`, `doc`). Only what's
    /// passed changes, and `""` returns a field to its default. An unknown id is an error unless
    /// `new` (so a typo doesn't add a step); `after` places a new step, or moves an existing one.
    /// Returns the step and what changed.
    pub fn step(&self, id: &str, state: Option<&str>, fields: &[(&str, &str)], after: Option<&str>, new: bool) -> Result<(Value, Vec<String>), String> {
        if let Some(s) = state {
            if !STATES.contains(&s) {
                return Err("a step is done, now, next or later".into());
            }
        }
        if !id.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)) || id.is_empty() {
            return Err(format!("bad step id {id:?}: letters, digits, . _ -"));
        }
        for (k, v) in fields {
            if *k == "doc" && !v.is_empty() {
                self.check_doc(v)?;
            }
        }
        let mut b = self.board();
        let stages = obj(&mut b).entry("stages").or_insert(json!([])).as_array_mut().ok_or("stages isn't a list")?;
        let ids = |st: &Vec<Value>| st.iter().filter_map(|s| s["id"].as_str().map(String::from)).collect::<Vec<_>>().join(", ");
        let i = stages.iter().position(|s| s["id"] == id);
        match (i, new) {
            (None, false) => return Err(format!("no step {id:?}; steps: {} (--new adds one)", ids(stages))),
            (Some(_), true) => return Err(format!("step {id:?} exists; leave out --new to change it")),
            _ => {}
        }
        if let Some(a) = after {
            if a == id || !stages.iter().any(|s| s["id"] == a) {
                return Err(format!("no step {a:?} to go after; steps: {}", ids(stages)));
            }
        }
        let mut e = i.map(|i| stages[i].clone()).unwrap_or_else(|| json!({ "id": id, "title": id }));
        let mut changed = vec![];
        let eo = obj(&mut e);
        for (k, v) in fields {
            let was = eo.get(*k).and_then(|x| x.as_str()).map(String::from);
            if v.is_empty() {
                if eo.remove(*k).is_some() { changed.push(format!("{k} reset")); }
            } else if was.as_deref() != Some(*v) {
                eo.insert((*k).into(), json!(v));
                changed.push((*k).to_string());
            }
        }
        if let Some(s) = state {
            if eo.get("state").and_then(|x| x.as_str()) != Some(s) {
                eo.insert("state".into(), json!(s));
                changed.push(format!("state {s}"));
            }
        }
        if let Some(i) = i {
            stages.remove(i);
        }
        let at = match after {
            Some(a) => stages.iter().position(|s| s["id"] == a).map(|p| p + 1).unwrap_or(stages.len()),
            None => i.unwrap_or(stages.len()),
        };
        if i.is_some() && after.is_some() && Some(at) != i {
            changed.push(format!("moved after {}", after.unwrap_or("")));
        }
        stages.insert(at, e.clone());
        if new {
            changed.insert(0, "added".into());
        }
        self.save_board(&b)?;
        Ok((e, changed))
    }

    /// A step's `doc` must name a real plan section: the file, from the project's home (or absolute),
    /// and a heading starting with the anchor. A board with no home can't check a relative path.
    pub fn check_doc(&self, doc: &str) -> Result<(), String> {
        let (file, anchor) = doc.split_once('#').map(|(f, a)| (f, Some(a))).unwrap_or((doc, None));
        if anchor.is_some_and(|a| a.is_empty() || a.contains(char::is_whitespace)) {
            return Err(format!("--doc {doc:?}: the part after # is a heading's first word, e.g. plans/x.md#M3"));
        }
        let home = self.dir.parent().filter(|p| p.ends_with(".switchboard")).and_then(|p| p.parent());
        let path = if Path::new(file).is_absolute() { PathBuf::from(file) } else {
            match home { Some(h) => h.join(file), None => return Ok(()) }
        };
        if !path.is_file() {
            return Err(format!("--doc: no file {}", path.display()));
        }
        if let Some(a) = anchor {
            if self.doc_section(doc).is_none() {
                return Err(format!("--doc: {file} has no heading starting with {a:?}"));
            }
        }
        Ok(())
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
        let stages: Vec<Value> = b["stages"].as_array().cloned().unwrap_or_default().into_iter().map(|mut st| {
            if let Some(md) = st["doc"].as_str().and_then(|d| self.doc_section(d)) {
                obj(&mut st).insert("docMd".into(), json!(md));
            }
            st
        }).collect();
        json!({
            "project": b["project"].clone(), "stages": stages, "items": self.cards(), "health": self.health(),
            "log": read(&self.p("log.json"), json!([])), "record": read(&self.p("record.json"), json!([])),
            "kept": read(&self.p("kept.json"), json!({})),
        })
    }

    /// The repo the board lives in (two folders above `.switchboard/board/`), if it is one.
    fn repo(&self) -> Option<PathBuf> {
        let r = self.dir.parent()?.parent()?.to_path_buf();
        r.join(".git").exists().then_some(r)
    }

    /// A step's `doc` ("plans/06-board.md#B1"): that file's section whose heading starts with the
    /// label, up to the next heading of the same level or higher, as markdown.
    pub fn doc_section(&self, doc: &str) -> Option<String> {
        let (file, anchor) = doc.split_once('#').map(|(f, a)| (f, Some(a))).unwrap_or((doc, None));
        let path = if Path::new(file).is_absolute() { PathBuf::from(file) } else { self.dir.parent()?.parent()?.join(file) };
        let md = std::fs::read_to_string(path).ok()?;
        let Some(anchor) = anchor else { return Some(md) };
        let lines: Vec<&str> = md.lines().collect();
        let level = |l: &str| l.chars().take_while(|c| *c == '#').count();
        let start = lines.iter().position(|l| {
            let n = level(l);
            (2..=4).contains(&n) && l[n..].trim_start().split_whitespace().next() == Some(anchor)
        })?;
        let lv = level(lines[start]);
        let end = lines.iter().enumerate().skip(start + 1).find(|(_, l)| { let n = level(l); n > 0 && n <= lv && l[n..].starts_with(' ') }).map(|(i, _)| i).unwrap_or(lines.len());
        Some(lines[start + 1..end].join("\n"))
    }

    /// Uncommitted, unpushed and recent commits in the board's repo (the overview's health),
    /// cached for 15 s: git is too slow to run on every read.
    pub fn health(&self) -> Value {
        use std::sync::Mutex;
        static CACHE: Mutex<Vec<(PathBuf, i64, Value)>> = Mutex::new(Vec::new());
        if let Some((_, at, v)) = CACHE.lock().unwrap().iter().find(|(d, _, _)| *d == self.dir) {
            if now() - at < 15_000 {
                return v.clone();
            }
        }
        let v = match self.repo() {
            None => json!({ "repo": false, "commits": [] }),
            Some(repo) => {
                let git = |args: &[&str]| std::process::Command::new("git").args(args).current_dir(&repo).output().ok()
                    .filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
                let status = git(&["status", "--porcelain"]);
                let ahead = git(&["rev-list", "--count", "@{u}..HEAD"]);
                let log = git(&["log", "--since=14.days", "--format=%ct%x09%h%x09%s"]).unwrap_or_default();
                let commits: Vec<Value> = log.lines().filter_map(|l| {
                    let mut p = l.splitn(3, '\t');
                    Some(json!({ "t": p.next()?.parse::<i64>().ok()? * 1000, "hash": p.next()?, "subject": p.next().unwrap_or("") }))
                }).collect();
                json!({ "repo": true,
                        "uncommitted": status.map(|s| if s.is_empty() { 0 } else { s.lines().count() }),
                        "unpushed": ahead.and_then(|a| a.parse::<i64>().ok()),
                        "commits": commits })
            }
        };
        let mut c = CACHE.lock().unwrap();
        c.retain(|(d, _, _)| *d != self.dir);
        c.push((self.dir.clone(), now(), v.clone()));
        v
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

    /// The author decided in conversation, not on the board: record it as theirs (`from: chat`),
    /// with the options named in `picks`, or the recommended ones. Unblocks the card.
    pub fn decide(&self, id: &str, picks: &[String], note: Option<&str>) -> Result<Value, String> {
        let mut c = self.card(id).ok_or(format!("no card {id}"))?;
        let d = c["decision"].clone();
        let opts = d["options"].as_array().ok_or(format!("{id} has no decision"))?;
        let is_rec = |o: &Value| o["rec"] == true || o["rec"].as_i64().is_some_and(|n| n != 0);
        let rec: Vec<String> = opts.iter().filter(|o| is_rec(o)).filter_map(|o| o["o"].as_str().map(String::from)).collect();
        let pick: Vec<String> = if picks.is_empty() { rec.clone() } else { picks.to_vec() };
        if pick.is_empty() {
            return Err(format!("{id} has no recommended option; name one with --pick"));
        }
        let mut labels = vec![];
        for p in &pick {
            let o = opts.iter().find(|o| o["o"].as_str() == Some(p.as_str())).ok_or(format!("{id} has no option {p}"))?;
            labels.push(json!(plain(o["label"].as_str().unwrap_or(p))));
        }
        let one = |v: &[String]| if v.len() == 1 { json!(v[0]) } else { json!(v) };
        let entry = json!({ "id": id, "title": c["title"].as_str().unwrap_or(id), "step": c["stage"].as_str().unwrap_or(""),
                            "version": c["version"].as_i64().unwrap_or(1), "pick": one(&pick), "labels": labels,
                            "rec": one(&rec), "way": if pick == rec { "agreed" } else { "chose" },
                            "notes": note.map(|n| vec![json!(n)]).unwrap_or_default(), "at": now(), "from": "chat" });
        let mut record = read(&self.p("record.json"), json!([]));
        record.as_array_mut().ok_or("record.json isn't a list")?.push(entry.clone());
        write(&self.p("record.json"), &record)?;
        if c["blocking"] == true {
            obj(&mut c).insert("blocking".into(), json!(false));
            self.save_card(&c)?;
        }
        Ok(entry)
    }

    /// The author has seen a card that asks nothing (said in chat): a Record entry with no pick and
    /// `way: seen`, so every device treats it as handled, not only the browser it was marked in.
    pub fn seen(&self, id: &str) -> Result<(), String> {
        let c = self.card(id).ok_or(format!("no card {id}"))?;
        if c["decision"].is_object() {
            return Err(format!("{id} asks a question; answer it (decide) instead"));
        }
        let mut record = read(&self.p("record.json"), json!([]));
        record.as_array_mut().ok_or("record.json isn't a list")?.push(json!({
            "id": id, "title": c["title"].as_str().unwrap_or(id), "step": c["stage"].as_str().unwrap_or(""),
            "version": c["version"].as_i64().unwrap_or(1), "pick": null, "labels": [], "rec": null, "way": "seen",
            "notes": [], "at": now(), "from": "chat" }));
        write(&self.p("record.json"), &record)
    }

    /// Has the author sent a pick for this card (any version)?
    pub fn answered(&self) -> std::collections::HashSet<String> {
        let rec = read(&self.p("record.json"), json!([]));
        rec.as_array().into_iter().flatten().filter(|r| !r["pick"].is_null()).filter_map(|r| r["id"].as_str().map(String::from)).collect()
    }
}

/// A card in the contract's shape (docs/board-contract.md), or every way it isn't, with where each
/// field goes. A card the page can't read shows only its title: no question, no picture.
pub fn check_card(c: &Value) -> Result<(), String> {
    const TOP: &[&str] = &["id", "stage", "title", "sub", "summary", "tags", "order", "version", "updated", "askedAt", "blocking",
                           "reason", "for", "decision", "body", "notePrompt", "visual", "wide", "changed", "make"];
    const DECISION: &[&str] = &["q", "options", "why", "basis", "multi", "unsure"];
    const BODY: &[&str] = &["html", "checks", "beats", "gallery", "image", "video", "compare", "fields", "audio", "poster", "takes"];
    const VIDEO: &[&str] = &["mp4", "mov", "m4v", "webm"];
    let mut bad = vec![];
    let Some(o) = c.as_object() else { return Err("a card is a JSON object".into()) };
    for k in o.keys() {
        if DECISION.contains(&k.as_str()) {
            bad.push(format!("`{k}` goes inside `decision`, not at the top of the card"));
        } else if BODY.contains(&k.as_str()) {
            bad.push(format!("`{k}` goes inside `body`, not at the top of the card"));
        } else if !TOP.contains(&k.as_str()) {
            bad.push(format!("`{k}` isn't a card field"));
        }
    }
    if let Some(d) = o.get("decision").filter(|d| !d.is_null()) {
        if !d["q"].is_string() {
            bad.push("`decision.q` (the question) is missing".into());
        }
        match d["options"].as_array() {
            None => bad.push("`decision.options` is missing".into()),
            Some(opts) => {
                for (i, op) in opts.iter().enumerate() {
                    if !(op["o"].is_string() || op["o"].is_number()) {
                        bad.push(format!("option {} needs an `o` (a short id, e.g. \"yes\")", i + 1));
                    }
                    if !op["label"].is_string() {
                        bad.push(format!("option {} needs a `label`", i + 1));
                    }
                }
            }
        }
        if !(d["why"].is_null() || d["why"].is_array()) {
            bad.push("`decision.why` is a list of paragraphs: [\"…\"]".into());
        }
    }
    let b = &c["body"];
    for (k, v) in [("image", &b["image"]), ("top-level image", &c["image"])] {
        if let Some(src) = v.as_str() {
            let ext = src.rsplit('.').next().unwrap_or("").to_lowercase();
            if VIDEO.contains(&ext.as_str()) {
                bad.push(format!("{src} is a video: use `body.video`, not {k}"));
            }
        }
    }
    if bad.is_empty() { Ok(()) } else { Err(format!("this card doesn't match the board's format (docs/board-contract.md):\n  - {}", bad.join("\n  - "))) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board() -> (tempfile::TempDir, Board) {
        let t = tempfile::tempdir().unwrap();
        let b = Board { dir: t.path().join("board"), app_slug: None };
        b.step("a1", Some("now"), &[("title", "First")], None, true).unwrap();
        (t, b)
    }

    #[test]
    fn cards_in_the_wrong_shape_are_refused_with_where_things_go() {
        let (_t, b) = board();
        let flat = json!({ "id": "f", "stage": "a1", "title": "F", "q": "Q?", "options": [{ "label": "Yes", "rec": true }],
                           "why": "because", "basis": "taste", "image": "~/Downloads/film.mp4" });
        let e = b.add(flat, false, false).unwrap_err();
        for want in ["`q` goes inside `decision`", "`options` goes inside `decision`", "`image` goes inside `body`", "use `body.video`"] {
            assert!(e.contains(want), "{want} missing from: {e}");
        }
        let e = b.add(json!({ "id": "g", "stage": "a1", "title": "G", "decision": { "q": "Q?", "options": [{ "label": "Yes" }] } }), false, false).unwrap_err();
        assert!(e.contains("option 1 needs an `o`"), "{e}");
        b.add(json!({ "id": "h", "stage": "a1", "title": "H", "body": { "video": "~/Downloads/film.mp4" },
                      "decision": { "q": "Q?", "why": ["because"], "options": [{ "o": "yes", "label": "Yes", "rec": true }] } }), false, false).unwrap();
        assert!(b.update("h", json!({ "q": "moved?" }), None, false).unwrap_err().contains("`q` goes inside `decision`"));
    }

    #[test]
    fn seen_is_for_cards_that_ask_nothing() {
        let (_t, b) = board();
        b.add(json!({ "id": "s", "stage": "a1", "title": "S" }), false, false).unwrap();
        b.add(json!({ "id": "d", "stage": "a1", "title": "D", "decision": { "q": "Q", "options": [{ "o": "a", "label": "A", "rec": 1 }] } }), false, false).unwrap();
        assert!(b.seen("d").unwrap_err().contains("decide"));
        b.seen("s").unwrap();
        let rec = read(&b.p("record.json"), json!([]));
        assert_eq!((rec[0]["way"].clone(), rec[0]["pick"].clone()), (json!("seen"), Value::Null));
        assert!(!b.answered().contains("s"));
    }

    #[test]
    fn decide_records_the_recommendation() {
        let (_t, b) = board();
        b.add(json!({ "id": "d", "stage": "a1", "title": "D", "blocking": true,
                      "decision": { "q": "Q", "options": [{ "o": "a", "label": "A", "rec": 1 }, { "o": "b", "label": "B" }] } }), false, true).unwrap();
        b.add(json!({ "id": "s", "stage": "a1", "title": "S" }), false, false).unwrap();
        assert!(b.decide("s", &[], None).unwrap_err().contains("no decision"));
        assert!(b.decide("d", &["z".into()], None).unwrap_err().contains("no option"));
        let e = b.decide("d", &[], Some("said so")).unwrap();
        assert_eq!((e["pick"].clone(), e["way"].clone(), e["from"].clone()), (json!("a"), json!("agreed"), json!("chat")));
        assert!(b.answered().contains("d"));
        assert_eq!(b.card("d").unwrap()["blocking"], false);
        b.kept("d", None, false).unwrap();
        assert_eq!(b.decide("d", &["b".into()], None).unwrap()["way"], "chose");
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
        b.step("b1", Some("next"), &[], None, true).unwrap();
        let c = b.add(json!({ "id": "x", "stage": "a1", "title": "X" }), false, false).unwrap();
        let moved = b.update("x", json!({ "stage": "b1" }), None, false).unwrap();
        assert_eq!(moved["updated"], c["updated"]);
        let v2 = b.update("x", json!({ "summary": "S" }), Some("added a summary"), false).unwrap();
        assert_eq!(v2["version"], 2);
        assert_eq!(v2["changed"], "added a summary");
        assert!(b.update("x", json!({ "stage": "zz" }), None, false).is_err());
    }

    #[test]
    fn a_steps_doc_is_its_plan_section() {
        let t = tempfile::tempdir().unwrap();
        let b = Board { dir: t.path().join(".switchboard/board"), app_slug: None };
        std::fs::create_dir_all(t.path().join("plans")).unwrap();
        std::fs::write(t.path().join("plans/p.md"), "# Plan\n\n### A1 · One\n\n- [x] done\n\n### A2 · Two\n\ntwo\n").unwrap();
        assert_eq!(b.doc_section("plans/p.md#A1").unwrap().trim(), "- [x] done");
        assert_eq!(b.doc_section("plans/p.md#A2").unwrap().trim(), "two");
        assert!(b.doc_section("plans/p.md#A9").is_none());
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
    fn a_steps_doc_must_name_a_real_section() {
        let t = tempfile::tempdir().unwrap();
        let b = Board { dir: t.path().join(".switchboard/board"), app_slug: None };
        std::fs::create_dir_all(t.path().join("plans")).unwrap();
        std::fs::write(t.path().join("plans/p.md"), "# Plan\n\n### M3 · Build\n\nText.\n").unwrap();
        b.step("m3", Some("now"), &[("doc", "plans/p.md#M3")], None, true).unwrap();
        assert!(b.step("m3", None, &[("doc", "plans/p.md#M4")], None, false).unwrap_err().contains("no heading"));
        assert!(b.step("m3", None, &[("doc", "plans/q.md#M3")], None, false).unwrap_err().contains("no file"));
    }

    #[test]
    fn steps_settle_working_and_reads() {
        let (_t, b) = board();
        b.step("b0", Some("done"), &[("label", "B0")], Some("a1"), true).unwrap();
        assert_eq!(b.step_ids(), vec!["a1", "b0"]);
        assert!(b.step("a1", Some("soon"), &[], None, false).is_err());
        // A typo is an error, not a new step; --new on an existing one is refused.
        assert!(b.step("a11", Some("now"), &[], None, false).unwrap_err().contains("--new adds one"));
        assert!(b.step("a1", None, &[], None, true).unwrap_err().contains("exists"));
        // Only what's passed changes; "" resets; after moves an existing step.
        let (s, ch) = b.step("a1", None, &[("blurb", "One line"), ("label", "A1")], None, false).unwrap();
        assert_eq!((s["blurb"].clone(), s["state"].clone(), ch.clone()), (json!("One line"), json!("now"), vec!["blurb".to_string(), "label".into()]));
        assert!(b.step("a1", None, &[("blurb", "One line")], None, false).unwrap().1.is_empty());
        let (s, _) = b.step("a1", None, &[("label", "")], None, false).unwrap();
        assert!(s.get("label").is_none());
        let order = |b: &Board| b.board()["stages"].as_array().unwrap().iter().map(|s| s["id"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        assert_eq!(order(&b), ["a1", "b0"]);
        assert_eq!(b.step("a1", None, &[], Some("b0"), false).unwrap().1, ["moved after b0"]);
        assert_eq!(order(&b), ["b0", "a1"]);
        assert!(b.step("a1", None, &[], Some("zz"), false).is_err());
        // --doc names a real section (here the board has no home, so only its shape is checked).
        assert!(b.step("a1", None, &[("doc", "plans/x.md#")], None, false).unwrap_err().contains("heading"));
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
