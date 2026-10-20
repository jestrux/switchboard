//! `switchboard board …`: the agent writes a project's board (plans/06-board.md, B1). The board is
//! files (docs/board-contract.md), so this needs no app; the page and the phone read the files.

use anyhow::{anyhow, Result};
use clap::Subcommand;
use sb_core::board::{self, Board};
use serde_json::{json, Value};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Subcommand)]
pub enum BoardCmd {
    /// Post a card (JSON from a file, or - for stdin)
    Add {
        file: String,
        /// Ask for it now: a Needs you notification on the page and the phone
        #[arg(long)]
        ask: bool,
        /// The agent can't go on without it
        #[arg(long)]
        blocking: bool,
        /// Don't push to the phone
        #[arg(long)]
        no_push: bool,
        /// The activity log's line (default "Posted a card: …")
        #[arg(long)]
        log: Option<String>,
    },
    /// Merge fields into a card (JSON from a file, or - for stdin)
    Update {
        id: String,
        file: String,
        /// What changed: makes it a new version, which comes back to the author
        #[arg(long)]
        changed: Option<String>,
        /// A wording fix that isn't news
        #[arg(long)]
        quiet: bool,
    },
    /// "I need this one now": the page and the phone say so
    Ask {
        id: String,
        #[arg(long)]
        blocking: bool,
        #[arg(long)]
        no_push: bool,
    },
    /// Take a card off the board
    Remove { id: String },
    /// A call going ahead unless the author objects
    Settle {
        id: String,
        text: Vec<String>,
        #[arg(long)]
        stage: Option<String>,
        #[arg(long)]
        remove: bool,
    },
    /// What the agent is doing now
    Working {
        text: Vec<String>,
        #[arg(long)]
        stage: Option<String>,
        #[arg(long)]
        item: Option<String>,
        #[arg(long)]
        progress: Option<f64>,
        #[arg(long)]
        clear: bool,
    },
    /// Add to the activity log
    Log {
        text: Vec<String>,
        #[arg(long)]
        item: Option<String>,
    },
    /// A step of the plan, shown in the sidebar: done, now, next or later
    Step {
        id: String,
        state: Option<String>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        label: Option<String>,
        #[arg(long)]
        blurb: Option<String>,
        /// The plan section its panel shows, e.g. plans/06-board.md#B1
        #[arg(long)]
        doc: Option<String>,
        #[arg(long)]
        after: Option<String>,
        /// Remove it (only when it has no cards)
        #[arg(long)]
        remove: bool,
    },
    /// The author decided in conversation: record their pick (the recommended one unless --pick)
    Decide {
        ids: Vec<String>,
        /// The option(s) they chose, if not the recommendation
        #[arg(long, value_delimiter = ',')]
        pick: Vec<String>,
        /// What they said, kept with the decision
        #[arg(long)]
        note: Option<String>,
    },
    /// The author has seen these (cards that ask nothing), on every device
    Seen { ids: Vec<String> },
    /// A decision's work is done; it leaves What's owed
    Kept {
        id: String,
        #[arg(long)]
        commit: Option<String>,
        #[arg(long)]
        undo: bool,
    },
    /// The cards, and whether each is still open
    List,
    /// The author's sends not read yet (then marks them read)
    Answers {
        /// Block until one lands
        #[arg(long)]
        wait: bool,
        /// Seconds before giving up (exit 124)
        #[arg(long)]
        timeout: Option<u64>,
        #[arg(long)]
        json: bool,
        /// Every send, read or not
        #[arg(long)]
        all: bool,
    },
    /// Where this folder's board is
    Where,
}

fn input(src: &str) -> Result<Value> {
    let text = if src == "-" {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s)?;
        s
    } else {
        std::fs::read_to_string(src)?
    };
    serde_json::from_str(&text).map_err(|e| anyhow!("not JSON: {e}"))
}

fn e<T>(r: std::result::Result<T, String>) -> Result<T> {
    r.map_err(|m| anyhow!(m))
}

fn joined(words: &[String]) -> Option<String> {
    let t = words.join(" ");
    (!t.trim().is_empty()).then_some(t)
}

pub fn run(projects_dir: &Path, dir: Option<PathBuf>, cmd: BoardCmd) -> Result<i32> {
    let b = match dir.or_else(|| std::env::var_os("BOARD_DIR").map(PathBuf::from)) {
        Some(d) => Board { dir: d, app_slug: None },
        None => board::locate(&std::env::current_dir()?, projects_dir).map_err(|e| anyhow!(e))?,
    };
    match cmd {
        BoardCmd::Where => println!("{}", b.dir.display()),
        BoardCmd::Add { file, ask, blocking, no_push, log } => {
            let project = b.app_slug.as_ref().map(|s| sb_core::project::Project::load(&projects_dir.join(s)));
            e(b.start_app_board(project.as_ref()))?;
            let c = e(b.add(input(&file)?, ask, blocking))?;
            let id = c["id"].as_str().unwrap_or_default().to_string();
            e(b.log(&log.unwrap_or_else(|| format!("Posted a card: {}", board::plain(c["title"].as_str().unwrap_or("")))), Some(&id)))?;
            println!("added {id}{}", if ask { " (asked)" } else { "" });
            if ask && !no_push {
                push(&b, &c);
            }
        }
        BoardCmd::Update { id, file, changed, quiet } => {
            let c = e(b.update(&id, input(&file)?, changed.as_deref(), quiet))?;
            if let Some(what) = &changed {
                e(b.log(&format!("Changed: {} (v{})", board::plain(c["title"].as_str().unwrap_or("")), c["version"]), Some(&id)))?;
                let _ = what;
            }
            println!("updated {id}{}", if changed.is_some() { format!(" to v{}", c["version"]) } else { String::new() });
        }
        BoardCmd::Ask { id, blocking, no_push } => {
            let c = e(b.ask(&id, blocking))?;
            let blocking = c["blocking"] == true;
            e(b.log(&format!("{}{}", if blocking { "Blocked on you: " } else { "Asked: " }, board::plain(c["title"].as_str().unwrap_or(""))), Some(&id)))?;
            println!("asked {id}{}", if blocking { " (blocking)" } else { "" });
            if !no_push && std::env::var("BOARD_PUSH").as_deref() != Ok("off") {
                push(&b, &c);
            }
        }
        BoardCmd::Remove { id } => {
            e(b.remove(&id))?;
            println!("removed {id}");
        }
        BoardCmd::Settle { id, text, stage, remove } => {
            let key = if remove { e(b.settle(&id, None, None))? } else {
                let t = joined(&text).ok_or_else(|| anyhow!("give the call's text"))?;
                e(b.settle(&id, Some(&t), stage.as_deref()))?
            };
            println!("{} {key}", if remove { "removed" } else { "going ahead:" });
        }
        BoardCmd::Working { text, stage, item, progress, clear } => {
            if clear {
                e(b.working(None, None, None, None))?;
                println!("cleared");
            } else {
                let t = joined(&text).ok_or_else(|| anyhow!("say what you're working on, or --clear"))?;
                e(b.working(Some(&t), stage.as_deref(), item.as_deref(), progress))?;
                println!("working: {t}");
            }
        }
        BoardCmd::Log { text, item } => {
            e(b.log(&joined(&text).ok_or_else(|| anyhow!("give the text"))?, item.as_deref()))?;
            println!("logged");
        }
        BoardCmd::Step { id, state, title, label, blurb, doc, after, remove } => {
            if remove {
                e(b.remove_step(&id))?;
                println!("removed step {id}");
                return Ok(0);
            }
            let mut fields = vec![];
            for (k, v) in [("title", &title), ("label", &label), ("blurb", &blurb), ("doc", &doc)] {
                if let Some(v) = v {
                    fields.push((k, v.as_str()));
                }
            }
            let s = e(b.step(&id, state.as_deref(), &fields, after.as_deref()))?;
            if let Some(st @ ("now" | "done")) = state.as_deref() {
                let label = s["label"].as_str().map(|l| format!("{l} · ")).unwrap_or_default();
                e(b.log(&format!("{}: {label}{}", if st == "now" { "Now" } else { "Done" }, board::plain(s["title"].as_str().unwrap_or(&id))), None))?;
            }
            println!("{id}{}", s["state"].as_str().map(|x| format!(" · {x}")).unwrap_or_default());
        }
        BoardCmd::Decide { ids, pick, note } => {
            for id in ids {
                let entry = e(b.decide(&id, &pick, note.as_deref()))?;
                let labels: Vec<&str> = entry["labels"].as_array().into_iter().flatten().filter_map(|l| l.as_str()).collect();
                e(b.log(&format!("Decided in chat: {} · {}", board::plain(entry["title"].as_str().unwrap_or(&id)), labels.join(", ")), Some(&id)))?;
                println!("decided {id}: {}", labels.join(", "));
            }
        }
        BoardCmd::Seen { ids } => {
            for id in ids {
                e(b.seen(&id))?;
                println!("seen {id}");
            }
        }
        BoardCmd::Kept { id, commit, undo } => {
            e(b.kept(&id, commit.as_deref(), undo))?;
            if !undo {
                let title = b.card(&id).and_then(|c| c["title"].as_str().map(board::plain)).unwrap_or_else(|| id.clone());
                e(b.log(&format!("Kept: {title}"), Some(&id)))?;
            }
            println!("{} {id}", if undo { "owed again:" } else { "kept" });
        }
        BoardCmd::List => {
            let answered = b.answered();
            for c in b.cards() {
                let id = c["id"].as_str().unwrap_or("");
                let mark = if answered.contains(id) { "✓" } else if c["decision"].is_object() { "○" } else { "·" };
                // A closed pipe (`| head`) ends the listing quietly.
                use std::io::Write;
                if writeln!(std::io::stdout(), "{mark} {:<16} {:<8} {}", id, c["stage"].as_str().unwrap_or(""), board::plain(c["title"].as_str().unwrap_or(""))).is_err() {
                    break;
                }
            }
        }
        BoardCmd::Answers { wait, timeout, json, all } => {
            let deadline = timeout.map(|t| Instant::now() + Duration::from_secs(t));
            let (mut fresh, mut total) = b.sends(all);
            while wait && fresh.is_empty() {
                if deadline.is_some_and(|d| Instant::now() > d) {
                    eprintln!("board: no answers yet");
                    return Ok(124);
                }
                std::thread::sleep(Duration::from_secs(1));
                (fresh, total) = b.sends(all);
            }
            if fresh.is_empty() {
                println!("No new answers.");
                return Ok(0);
            }
            if json {
                println!("{}", serde_json::to_string_pretty(&fresh)?);
            } else {
                let texts: Vec<&str> = fresh.iter().filter_map(|s| s["text"].as_str()).collect();
                println!("{}", texts.join("\n\n"));
            }
            if !all {
                e(b.mark_read(total))?;
            }
        }
    }
    Ok(0)
}

/// The phone and iPad hear about an asked card through the crotchet worker's push (Fuchi's
/// NOTIFY_PLAN): category `fuchi.view.accept` when the agent is sure (Go with my pick and Reply),
/// `fuchi.view.reply` otherwise; the view opens this card's page, served on the local network.
fn push(b: &Board, c: &Value) {
    let backend = std::env::var("FUCHI_BACKEND").unwrap_or_else(|_| "https://backend.wakyj07.workers.dev".into());
    let base = std::env::var("BOARD_URL").unwrap_or_else(|_| format!("http://{}:5198", lan_address()));
    let project = b.board()["project"].clone();
    let d = &c["decision"];
    let opts: Vec<&Value> = d["options"].as_array().into_iter().flatten().collect();
    let recs: Vec<&&Value> = opts.iter().filter(|o| o["rec"] == true).collect();
    let sure = d.is_object() && d["unsure"].is_null() && d["multi"] != true && recs.len() == 1 && d["basis"] == "evidence";
    let label = |o: &Value| board::plain(o["label"].as_str().unwrap_or(""));
    let lower = |t: &str| { let t = board::plain(t); let mut cs = t.chars(); cs.next().map(|f| f.to_lowercase().collect::<String>() + cs.as_str()).unwrap_or_default() };
    let body = if !d.is_object() { "Something to look at. Hold to see it.".to_string() }
        else if !d["unsure"].is_null() { "I'm not sure: hold to look.".to_string() }
        else if sure {
            let then = recs[0]["then"].as_str().map(|t| format!(" Then I'll {}", lower(t))).unwrap_or_default();
            format!("I'd go with “{}”.{then}", label(recs[0]))
        } else if !recs.is_empty() { format!("I'd lean “{}”: hold to see why.", recs.iter().map(|o| label(o)).collect::<Vec<_>>().join("”, “")) }
        else { format!("{} Hold to answer.", board::plain(d["q"].as_str().unwrap_or(""))) };
    let title = board::plain(c["title"].as_str().unwrap_or(""));
    let id = c["id"].as_str().unwrap_or("");
    let send = |pick: Value, labels: Value, way: &str, note: Option<&str>| {
        let text = [Some(format!("{}: from the notification", project["name"].as_str().unwrap_or("Board"))), Some(format!("{title} [{id}]")),
                    labels.as_array().filter(|l| !l.is_empty()).map(|l| format!("    → {}  (recommended)", l.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(", "))),
                    note.map(|n| format!("    note: {n}"))].into_iter().flatten().collect::<Vec<_>>().join("\n");
        json!({
        "project": project["dir"], "from": "notification", "objections": [],
        "text": text,
        "answers": [{ "id": id, "stage": c["stage"], "title": title, "version": c["version"], "pick": pick, "labels": labels,
                      "rec": recs.first().map(|o| o["o"].clone()), "way": way, "notes": note.map(|n| vec![json!({ "text": n })]).unwrap_or_default() }],
        })
    };
    let mut actions = serde_json::Map::new();
    if sure {
        actions.insert("accept".into(), json!({ "post": format!("{base}/answers"), "body": send(recs[0]["o"].clone(), json!([label(recs[0])]), "agreed", None) }));
    }
    if d.is_object() {
        actions.insert("reply".into(), json!({ "post": format!("{base}/answers"), "body": send(Value::Null, json!([]), "note", Some("{{text}}")) }));
    }
    let spec = json!({ "kind": "web", "ext": "board", "id": id, "url": format!("{base}/card.html?id={id}"), "actions": actions });
    let category = if sure { "fuchi.view.accept" } else if d.is_object() { "fuchi.view.reply" } else { "fuchi.view" };
    let data = json!({ "fuchi": spec.to_string() }).to_string();
    let title = format!("{}{title}", if c["blocking"] == true { "Blocking · " } else { "" });
    let topic = std::env::var("BOARD_PUSH_TOPIC").unwrap_or_else(|_| "fuchi".into());
    let out = std::process::Command::new("curl")
        .args(["-sS", "--max-time", "10", "-G", &format!("{backend}/firebase/notify")])
        .args(["--data-urlencode", &format!("topic={topic}"), "--data-urlencode", &format!("title={title}"),
               "--data-urlencode", &format!("body={body}"), "--data-urlencode", &format!("category={category}"),
               "--data-urlencode", &format!("data={data}")])
        .output();
    match out {
        Ok(o) if String::from_utf8_lossy(&o.stdout).contains("\"success\":true") =>
            println!("pushed to the phone{}", if sure { " (with Go with my pick)" } else { "" }),
        Ok(o) => println!("push didn't go out: {}", String::from_utf8_lossy(&o.stdout).chars().take(120).collect::<String>()),
        Err(e) => println!("push didn't go out: {e}"),
    }
}

fn lan_address() -> String {
    std::process::Command::new("ipconfig").args(["getifaddr", "en0"]).output().ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "localhost".into())
}

