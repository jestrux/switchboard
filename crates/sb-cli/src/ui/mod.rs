//! `switchboard ui`: the Mac app's window in a terminal. A rail of projects, the project's
//! groups and commands with their status, and up to four live terminals from any project,
//! each with its pane bar and footer. A client of the host, so quitting stops nothing.

mod term;
mod theme;

use crate::app;
use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::crossterm::execute;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::Frame;
use sb_core::control::Request;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};
use term::TermView;
use theme::Palette;

const RAIL_W: u16 = 6;
const SIDEBAR_DEFAULT: u16 = 36;
const SIDEBAR_RANGE: (u16, u16) = (28, 72);
/// Ctrl+]: from a terminal back to the list (as `switchboard attach` detaches).
const BACK: char = ']';
/// As the Mac app: four panes at most.
const MAX_PANES: usize = 4;

struct Command {
    address: String,
    name: String,
    terminal: bool,
}

struct Group {
    id: String,
    name: String,
    commands: Vec<Command>,
}

struct ProjectView {
    slug: String,
    name: String,
    color: Option<Color>,
    palette: Palette,
    /// Why the config doesn't load, line by line; the project then lists nothing else.
    errors: Vec<String>,
    /// Ungrouped commands first, as the sidebar shows them.
    loose: Vec<Command>,
    groups: Vec<Group>,
    /// Groups start collapsed, as in the app; these are the open ones.
    expanded: HashSet<String>,
    selected: usize,
    /// Shown before: keep where the selection was left.
    visited: bool,
}

#[derive(Clone, PartialEq)]
enum Row {
    Group(String),
    Command(String),
}

impl ProjectView {
    fn rows(&self) -> Vec<Row> {
        let mut rows: Vec<Row> = self.loose.iter().map(|c| Row::Command(c.address.clone())).collect();
        for g in &self.groups {
            rows.push(Row::Group(g.id.clone()));
            if self.expanded.contains(&g.id) {
                rows.extend(g.commands.iter().map(|c| Row::Command(c.address.clone())));
            }
        }
        rows
    }

    fn command(&self, address: &str) -> Option<&Command> {
        self.all().find(|c| c.address == address)
    }

    fn all(&self) -> impl Iterator<Item = &Command> {
        self.loose.iter().chain(self.groups.iter().flat_map(|g| g.commands.iter()))
    }

    fn group_of(&self, address: &str) -> Option<&Group> {
        self.groups.iter().find(|g| g.commands.iter().any(|c| c.address == address))
    }

    /// Open the group holding `address`, so its row can be shown, and return the row.
    fn reveal(&mut self, address: &str) -> Option<usize> {
        if let Some(g) = self.group_of(address).map(|g| g.id.clone()) {
            self.expanded.insert(g);
        }
        self.rows().iter().position(|r| matches!(r, Row::Command(a) if a == address))
    }

    /// The group a new terminal goes in: the selected group, or the selected command's.
    fn selected_group(&self) -> Option<String> {
        match self.rows().get(self.selected) {
            Some(Row::Group(g)) => Some(g.clone()),
            Some(Row::Command(a)) => self.group_of(a).map(|g| g.id.clone()),
            None => None,
        }
    }
}

fn load_projects(dir: &Path) -> Vec<ProjectView> {
    let mut dirs: Vec<_> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.join(sb_core::project::CONFIG_FILE).is_file()).collect();
    dirs.sort();
    let mut out = Vec::new();
    for d in dirs {
        let p = sb_core::project::Project::load(&d);
        let mut view = ProjectView {
            slug: p.slug.clone(),
            name: p.name().to_string(),
            color: None,
            palette: theme::graphite(),
            errors: Vec::new(),
            loose: Vec::new(),
            groups: Vec::new(),
            expanded: HashSet::new(),
            selected: 0,
            visited: false,
        };
        match &p.effective {
            Err(e) => view.errors = e.messages(),
            Ok(cfg) => {
                let cmd = |c: &sb_core::CommandConfig, group: Option<&str>| Command {
                    address: match group {
                        Some(g) if !c.terminal => format!("{g}/{}", c.id),
                        _ => c.id.clone(),
                    },
                    name: c.display_name().to_string(),
                    terminal: c.terminal,
                };
                view.color = cfg.color.as_deref().and_then(theme::parse_hex);
                view.palette = theme::for_project(cfg.theme.as_ref());
                view.loose = cfg.commands.iter().map(|c| cmd(c, None)).collect();
                view.groups = cfg
                    .groups
                    .iter()
                    .map(|g| Group { id: g.id.clone(), name: g.display_name().to_uppercase(), commands: g.commands.iter().map(|c| cmd(c, Some(&g.id))).collect() })
                    .collect();
                // Terminals in a group: merge_into keeps their group in the address-less
                // ungrouped list, so move each into its group by its folder's group entry.
                for t in sb_core::terminals::load(&p.dir) {
                    let address = t.address();
                    if let Some(g) = t.group.as_deref().and_then(|g| view.groups.iter_mut().find(|x| x.id == g)) {
                        view.loose.retain(|c| c.address != address);
                        if !g.commands.iter().any(|c| c.address == address) {
                            g.commands.push(Command { address, name: t.name.clone(), terminal: true });
                        }
                    }
                }
            }
        }
        out.push(view);
    }
    out
}

/// What decides whether the projects need reading again: each config's and terminal
/// list's modification time.
fn projects_signature(dir: &Path) -> Vec<(PathBuf, Option<SystemTime>)> {
    let mut sig = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        for f in [sb_core::project::CONFIG_FILE, "terminals.yaml"] {
            let p = e.path().join(f);
            sig.push((p.clone(), std::fs::metadata(&p).and_then(|m| m.modified()).ok()));
        }
    }
    sig.sort();
    sig
}

/// Where the window's layout is kept between launches.
fn layout_path() -> PathBuf {
    sb_core::project::app_support_dir().join("ui-layout.json")
}

/// The terminal rectangles for `n` panes in `r`: one fills it; two sit side by side (or
/// stacked); three put one on the left and two stacked on the right; four make a grid.
fn pane_rects(r: Rect, n: usize, stacked: bool) -> Vec<Rect> {
    let half_w = r.width / 2;
    let half_h = r.height / 2;
    let left = Rect::new(r.x, r.y, half_w, r.height);
    let right = Rect::new(r.x + half_w, r.y, r.width - half_w, r.height);
    match n {
        0 | 1 => vec![r],
        2 if stacked => vec![Rect::new(r.x, r.y, r.width, half_h), Rect::new(r.x, r.y + half_h, r.width, r.height - half_h)],
        2 => vec![left, right],
        3 => vec![left, Rect::new(right.x, r.y, right.width, half_h), Rect::new(right.x, r.y + half_h, right.width, r.height - half_h)],
        _ => vec![
            Rect::new(r.x, r.y, half_w, half_h),
            Rect::new(right.x, r.y, right.width, half_h),
            Rect::new(r.x, r.y + half_h, half_w, r.height - half_h),
            Rect::new(right.x, r.y + half_h, right.width, r.height - half_h),
        ],
    }
}

/// A pane's terminal area inside its rectangle: below the bar, above the footer, padded.
fn term_area(pane: Rect) -> Rect {
    let body_h = pane.height.saturating_sub(2);
    Rect::new(pane.x + 2, pane.y + 2, pane.width.saturating_sub(4), body_h.saturating_sub(1))
}

fn open_external(target: &str) {
    let _ = std::process::Command::new("xdg-open").arg(target).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn();
}

#[derive(Clone, Copy, PartialEq)]
enum Focus {
    Sidebar,
    Terminal,
}

#[derive(Clone)]
enum Hit {
    Project(usize),
    Row(usize),
    StartAll,
    StopAll,
    Stop(usize),
    Restart(usize),
    Close(usize),
    Pane(usize),
    Port(u64),
    RowToggle(String),
    RowRestart(String),
    GroupStart(String),
    GroupStop(String),
    Divider,
}

/// A one-line question in the sidebar's foot: a terminal's new name.
struct Prompt {
    label: String,
    text: String,
    project: String,
    address: String,
}

struct App {
    projects_dir: PathBuf,
    projects: Vec<ProjectView>,
    signature: Vec<(PathBuf, Option<SystemTime>)>,
    current: usize,
    /// slug → address → the command's state, from the host.
    live: Arc<Mutex<HashMap<String, HashMap<String, Value>>>>,
    dirty: Arc<AtomicBool>,
    panes: Vec<TermView>,
    focused: usize,
    /// Two panes: one above the other rather than side by side.
    stacked: bool,
    sidebar_w: u16,
    dragging_divider: bool,
    hovered_row: Option<usize>,
    main_area: Rect,
    focus: Focus,
    prompt: Option<Prompt>,
    hits: Vec<(Rect, Hit)>,
    flash: Option<(String, Instant)>,
    quit: bool,
}

pub fn run(projects_dir: &Path) -> Result<i32> {
    // Something must host the engine; start it if nothing does.
    app::client(true)?;
    let projects = load_projects(projects_dir);
    if projects.is_empty() {
        anyhow::bail!("no projects in {}; `switchboard import <repo folder>` adds one", projects_dir.display());
    }
    let dirty = Arc::new(AtomicBool::new(true));
    let live = Arc::new(Mutex::new(HashMap::new()));
    {
        let (live, dirty) = (Arc::clone(&live), Arc::clone(&dirty));
        std::thread::spawn(move || loop {
            if let Ok(mut c) = app::client(false) {
                while let Ok(v) = c.call(&Request::Status { project: None }) {
                    let mut map: HashMap<String, HashMap<String, Value>> = HashMap::new();
                    for p in v["projects"].as_array().into_iter().flatten() {
                        let slug = p["slug"].as_str().unwrap_or_default().to_string();
                        let cmds = p["commands"].as_array().into_iter().flatten().map(|c| (c["address"].as_str().unwrap_or_default().to_string(), c.clone())).collect();
                        map.insert(slug, cmds);
                    }
                    *live.lock().unwrap() = map;
                    dirty.store(true, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(400));
                }
            }
            std::thread::sleep(Duration::from_secs(1));
        });
    }

    let mut app = App {
        projects_dir: projects_dir.to_path_buf(),
        signature: projects_signature(projects_dir),
        projects,
        current: 0,
        live,
        dirty,
        panes: Vec::new(),
        focused: 0,
        stacked: false,
        sidebar_w: SIDEBAR_DEFAULT,
        dragging_divider: false,
        hovered_row: None,
        main_area: Rect::default(),
        focus: Focus::Sidebar,
        prompt: None,
        hits: Vec::new(),
        flash: None,
        quit: false,
    };
    let saved = std::fs::read_to_string(layout_path()).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).unwrap_or(Value::Null);
    app.sidebar_w = saved["sidebar_width"].as_u64().map(|w| (w as u16).clamp(SIDEBAR_RANGE.0, SIDEBAR_RANGE.1)).unwrap_or(SIDEBAR_DEFAULT);
    let mut terminal = ratatui::init();
    execute!(std::io::stdout(), EnableMouseCapture, EnableBracketedPaste)?;
    // Draw once first: attaching sizes each command's terminal to its pane, so the panes'
    // places must be known. Then the saved layout, else the first running command.
    std::thread::sleep(Duration::from_millis(450));
    app.restore_expanded(&saved);
    terminal.draw(|f| app.draw(f))?;
    if !app.restore_panes(&saved) {
        app.open_initial();
    }
    let result = app.event_loop(&mut terminal);
    app.save_layout();
    let _ = execute!(std::io::stdout(), DisableMouseCapture, DisableBracketedPaste);
    ratatui::restore();
    result.map(|_| 0)
}

impl App {
    fn project(&self) -> &ProjectView {
        &self.projects[self.current]
    }

    fn project_index(&self, slug: &str) -> Option<usize> {
        self.projects.iter().position(|p| p.slug == slug)
    }

    fn state(&self, slug: &str, address: &str) -> Option<Value> {
        self.live.lock().unwrap().get(slug).and_then(|m| m.get(address)).cloned()
    }

    fn alive(s: Option<&Value>) -> bool {
        matches!(s.and_then(|s| s["status"].as_str()), Some("running" | "starting"))
    }

    fn count_alive<'a>(&self, slug: &str, cmds: impl Iterator<Item = &'a Command>) -> (usize, usize) {
        let live = self.live.lock().unwrap();
        let m = live.get(slug);
        let (mut running, mut total) = (0, 0);
        for c in cmds {
            if !c.terminal {
                total += 1;
            }
            if Self::alive(m.and_then(|m| m.get(&c.address))) {
                running += 1;
            }
        }
        (running, total)
    }

    fn running_count(&self, p: &ProjectView) -> (usize, usize) {
        self.count_alive(&p.slug, p.all())
    }

    fn first_running(&self, i: usize) -> Option<String> {
        let p = &self.projects[i];
        p.all().find(|c| Self::alive(self.state(&p.slug, &c.address).as_ref())).map(|c| c.address.clone())
    }

    fn open_initial(&mut self) {
        let pick = (0..self.projects.len()).find_map(|i| self.first_running(i).map(|a| (i, a)));
        let (i, address) = match pick {
            Some(p) => p,
            None => (0, self.projects[0].all().next().map(|c| c.address.clone()).unwrap_or_default()),
        };
        self.current = i;
        if let Some(r) = self.projects[i].reveal(&address) {
            self.projects[i].selected = r;
        }
        self.projects[i].visited = true;
        self.show_selected();
    }

    // ----- layout and memory ----------------------------------------------------------

    /// The layout as saved, for writing only when it changed.
    fn layout_json(&self) -> String {
        let panes: Vec<Value> = self.panes.iter().map(|t| json!([t.project, t.address])).collect();
        let expanded: serde_json::Map<String, Value> = self.projects.iter().map(|p| (p.slug.clone(), json!(p.expanded.iter().collect::<Vec<_>>()))).collect();
        json!({
            "panes": panes,
            "focused": self.focused,
            "stacked": self.stacked,
            "project": self.project().slug,
            "sidebar_width": self.sidebar_w,
            "expanded": expanded,
        })
        .to_string()
    }

    fn save_layout(&self) {
        let _ = std::fs::write(layout_path(), self.layout_json());
    }

    /// Open groups as they were left; a project never seen opens the groups that have
    /// something running.
    fn restore_expanded(&mut self, saved: &Value) {
        for i in 0..self.projects.len() {
            let slug = self.projects[i].slug.clone();
            match saved["expanded"][&slug].as_array() {
                Some(ids) => self.projects[i].expanded = ids.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
                None => {
                    let open: HashSet<String> = self.projects[i]
                        .groups
                        .iter()
                        .filter(|g| g.commands.iter().any(|c| Self::alive(self.state(&slug, &c.address).as_ref())))
                        .map(|g| g.id.clone())
                        .collect();
                    self.projects[i].expanded = open;
                }
            }
        }
    }

    /// Reopen the panes of the last session. False when there is nothing to restore.
    fn restore_panes(&mut self, v: &Value) -> bool {
        let wanted: Vec<(String, String)> = v["panes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| Some((p[0].as_str()?.to_string(), p[1].as_str()?.to_string())))
            .filter(|(slug, address)| self.project_index(slug).is_some_and(|i| self.projects[i].command(address).is_some()))
            .take(MAX_PANES)
            .collect();
        if wanted.is_empty() {
            return false;
        }
        self.stacked = v["stacked"].as_bool().unwrap_or(false);
        let rects = pane_rects(self.main_area, wanted.len(), self.stacked);
        for ((slug, address), rect) in wanted.iter().zip(rects) {
            if let Some(t) = self.open_view(slug, address, rect) {
                self.panes.push(t);
            }
        }
        self.focused = (v["focused"].as_u64().unwrap_or(0) as usize).min(self.panes.len().saturating_sub(1));
        for p in &mut self.projects {
            p.visited = true;
        }
        self.sync_sidebar_to_focused();
        if let Some(i) = v["project"].as_str().and_then(|s| self.project_index(s)) {
            self.current = i;
        }
        !self.panes.is_empty()
    }

    /// Read the projects again after a config or terminal list changed, keeping what was
    /// selected and open.
    fn reload_projects(&mut self) {
        let mut fresh = load_projects(&self.projects_dir);
        for p in &mut fresh {
            if let Some(old) = self.projects.iter().find(|o| o.slug == p.slug) {
                p.expanded = old.expanded.clone();
                p.visited = old.visited;
                let was = match old.rows().get(old.selected) {
                    Some(Row::Command(a)) => Some(Row::Command(a.clone())),
                    Some(Row::Group(g)) => Some(Row::Group(g.clone())),
                    None => None,
                };
                p.selected = was.and_then(|w| p.rows().iter().position(|r| *r == w)).unwrap_or(old.selected.min(p.rows().len().saturating_sub(1)));
            }
        }
        let current = self.project().slug.clone();
        self.projects = fresh;
        self.current = self.project_index(&current).unwrap_or(0);
        // A pane whose command went away (a closed terminal) closes.
        let gone: Vec<usize> = (0..self.panes.len())
            .filter(|&j| {
                let t = &self.panes[j];
                !self.project_index(&t.project).is_some_and(|i| self.projects[i].command(&t.address).is_some())
            })
            .collect();
        for j in gone.into_iter().rev() {
            self.panes.remove(j);
        }
        self.focused = self.focused.min(self.panes.len().saturating_sub(1));
        // Never leave the window empty: show something running in this project.
        if self.panes.is_empty() {
            if let Some(a) = self.first_running(self.current) {
                self.select_address(&a);
            }
        }
        self.dirty.store(true, Ordering::SeqCst);
    }

    /// Point the sidebar at the focused pane's project and command.
    fn sync_sidebar_to_focused(&mut self) {
        let Some(t) = self.panes.get(self.focused) else { return };
        let (slug, address) = (t.project.clone(), t.address.clone());
        if let Some(i) = self.project_index(&slug) {
            self.current = i;
            if let Some(r) = self.projects[i].reveal(&address) {
                self.projects[i].selected = r;
            }
            self.projects[i].visited = true;
        }
    }

    fn pane_of(&self, slug: &str, address: &str) -> Option<usize> {
        self.panes.iter().position(|t| t.project == slug && t.address == address)
    }

    fn selected_row(&self) -> Option<Row> {
        self.project().rows().get(self.project().selected).cloned()
    }

    fn selected_address(&self) -> Option<String> {
        match self.selected_row() {
            Some(Row::Command(a)) => Some(a),
            _ => None,
        }
    }

    fn open_view(&mut self, slug: &str, address: &str, rect: Rect) -> Option<TermView> {
        let a = term_area(rect);
        match TermView::open(slug, address, a.width.max(20), a.height.max(5), Arc::clone(&self.dirty)) {
            Ok(t) => Some(t),
            Err(e) => {
                self.say(format!("{e}"));
                None
            }
        }
    }

    /// Show the selected command: focus its pane when it has one, else put it in the
    /// focused pane (the Mac app's selection).
    fn show_selected(&mut self) {
        let Some(address) = self.selected_address() else { return };
        let slug = self.project().slug.clone();
        if let Some(j) = self.pane_of(&slug, &address) {
            self.focused = j;
            return;
        }
        let n = self.panes.len().max(1);
        let rect = pane_rects(self.main_area, n, self.stacked)[self.focused.min(n - 1)];
        let Some(view) = self.open_view(&slug, &address, rect) else { return };
        if self.panes.is_empty() {
            self.panes.push(view);
            self.focused = 0;
        } else {
            self.panes[self.focused] = view;
        }
    }

    /// Select `address` in the current project, opening its group, and show it.
    fn select_address(&mut self, address: &str) {
        let p = &mut self.projects[self.current];
        if let Some(r) = p.reveal(address) {
            p.selected = r;
        }
        self.show_selected();
    }

    /// A new pane with the selected command; `down` stacks two panes rather than placing
    /// them side by side.
    fn split(&mut self, down: bool) {
        if self.panes.len() >= MAX_PANES {
            self.say(format!("{MAX_PANES} panes at most"));
            return;
        }
        let Some(address) = self.selected_address() else { return };
        let slug = self.project().slug.clone();
        if self.pane_of(&slug, &address).is_some() {
            self.say(format!("{address} is already in a pane; select another command to split with"));
            return;
        }
        if self.panes.len() == 1 {
            self.stacked = down;
        }
        let n = self.panes.len() + 1;
        let rect = pane_rects(self.main_area, n, self.stacked)[n - 1];
        if let Some(view) = self.open_view(&slug, &address, rect) {
            self.panes.push(view);
            self.focused = n - 1;
        }
    }

    fn close_pane(&mut self, j: usize) {
        if self.panes.len() <= 1 || j >= self.panes.len() {
            return;
        }
        // Dropping the view detaches; the command keeps running.
        self.panes.remove(j);
        self.focused = self.focused.min(self.panes.len() - 1);
        self.sync_sidebar_to_focused();
    }

    fn focus_pane(&mut self, j: usize) {
        if j < self.panes.len() {
            self.focused = j;
            self.sync_sidebar_to_focused();
        }
    }

    fn say(&mut self, msg: String) {
        self.flash = Some((msg, Instant::now()));
        self.dirty.store(true, Ordering::SeqCst);
    }

    // ----- actions ----------------------------------------------------------------

    /// Run a host request off the UI thread; stop waits for exits.
    fn act(&mut self, req: Request, label: String) {
        let dirty = Arc::clone(&self.dirty);
        self.say(label);
        std::thread::spawn(move || {
            if let Ok(mut c) = app::client(false) {
                let _ = c.call(&req);
            }
            dirty.store(true, Ordering::SeqCst);
        });
    }

    fn toggle(&mut self, slug: String, address: String) {
        if Self::alive(self.state(&slug, &address).as_ref()) {
            self.act(Request::Stop { project: slug, target: Some(address.clone()) }, format!("stopping {address}"));
        } else {
            self.act(Request::Start { project: slug, target: Some(address.clone()) }, format!("starting {address}"));
        }
    }

    fn restart(&mut self, slug: String, address: String) {
        self.act(Request::Restart { project: slug, target: Some(address.clone()) }, format!("restarting {address}"));
    }

    fn toggle_selected(&mut self) {
        match self.selected_row() {
            Some(Row::Command(a)) => self.toggle(self.project().slug.clone(), a),
            // On a group: start what starts with it, or stop it when anything runs.
            Some(Row::Group(g)) => {
                let slug = self.project().slug.clone();
                let running = self.project().groups.iter().find(|x| x.id == g).is_some_and(|x| x.commands.iter().any(|c| Self::alive(self.state(&slug, &c.address).as_ref())));
                if running {
                    self.act(Request::Stop { project: slug, target: Some(g.clone()) }, format!("stopping {g}"));
                } else {
                    self.act(Request::Start { project: slug, target: Some(g.clone()) }, format!("starting {g}"));
                }
            }
            None => {}
        }
    }

    fn restart_selected(&mut self) {
        match self.selected_row() {
            Some(Row::Command(a)) => self.restart(self.project().slug.clone(), a),
            Some(Row::Group(g)) => {
                let slug = self.project().slug.clone();
                self.act(Request::Restart { project: slug, target: Some(g.clone()) }, format!("restarting {g}"));
            }
            None => {}
        }
    }

    fn start_all(&mut self) {
        let slug = self.project().slug.clone();
        self.act(Request::Start { project: slug.clone(), target: None }, format!("starting {slug}"));
    }

    fn stop_all(&mut self) {
        let slug = self.project().slug.clone();
        self.act(Request::Stop { project: slug.clone(), target: None }, format!("stopping everything in {slug}"));
    }

    /// A login shell in the selected group's folder, shown and ready to type into.
    fn new_terminal(&mut self) {
        let slug = self.project().slug.clone();
        let group = self.project().selected_group();
        let reply = app::client(false).and_then(|mut c| c.call(&Request::NewTerminal { project: slug.clone(), group: group.clone() }).map_err(|e| anyhow::anyhow!(e)));
        match reply {
            Ok(v) => {
                let address = v["address"].as_str().unwrap_or_default().to_string();
                self.reload_projects();
                self.signature = projects_signature(&self.projects_dir);
                self.select_address(&address);
                self.focus = Focus::Terminal;
            }
            Err(e) => self.say(format!("{e}")),
        }
    }

    fn selected_terminal(&self) -> Option<String> {
        let a = self.selected_address()?;
        self.project().command(&a).filter(|c| c.terminal).map(|_| a)
    }

    fn close_terminal(&mut self) {
        let Some(address) = self.selected_terminal() else {
            self.say("only a terminal can be closed; s stops a command".into());
            return;
        };
        let slug = self.project().slug.clone();
        self.act(Request::CloseTerminal { project: slug, address }, "closing the terminal".into());
    }

    fn rename_terminal(&mut self) {
        let Some(address) = self.selected_terminal() else {
            self.say("only a terminal can be renamed".into());
            return;
        };
        let name = self.project().command(&address).map(|c| c.name.rsplit(" / ").next().unwrap_or(&c.name).to_string()).unwrap_or_default();
        self.prompt = Some(Prompt { label: "Rename terminal".into(), text: name, project: self.project().slug.clone(), address });
    }

    fn clear_focused(&mut self) {
        if let Some(t) = self.panes.get_mut(self.focused) {
            t.clear();
        }
    }

    fn open_port(&mut self) {
        let target = self.selected_address().or_else(|| self.panes.get(self.focused).map(|t| t.address.clone()));
        let port = target.and_then(|a| self.state(&self.project().slug, &a)).and_then(|s| s["port"].as_u64());
        match port {
            Some(p) => open_external(&format!("http://localhost:{p}")),
            None => self.say("no listening port detected for this command".into()),
        }
    }

    fn move_selection(&mut self, by: isize) {
        let n = self.project().rows().len() as isize;
        if n == 0 {
            return;
        }
        let p = &mut self.projects[self.current];
        p.selected = (p.selected as isize + by).clamp(0, n - 1) as usize;
        self.show_selected();
    }

    fn switch_project(&mut self, i: usize) {
        if i < self.projects.len() && i != self.current {
            self.current = i;
            // First visit: land on its first running command, as the app opens a project.
            if !self.projects[i].visited {
                if let Some(a) = self.first_running(i) {
                    if let Some(r) = self.projects[i].reveal(&a) {
                        self.projects[i].selected = r;
                    }
                }
            }
            self.projects[i].visited = true;
            self.show_selected();
        }
    }

    fn activate_row(&mut self, i: usize) {
        self.projects[self.current].selected = i;
        match self.selected_row() {
            Some(Row::Group(id)) => {
                let p = &mut self.projects[self.current];
                if !p.expanded.remove(&id) {
                    p.expanded.insert(id);
                }
            }
            Some(Row::Command(_)) => self.show_selected(),
            None => {}
        }
    }

    // ----- input ------------------------------------------------------------------

    fn event_loop(&mut self, terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
        let mut last_tick = Instant::now();
        let mut last_scan = Instant::now();
        let mut saved = self.layout_json();
        while !self.quit {
            // Keep the saved layout current: Super+W closes the terminal under us, with no
            // clean quit to save it then.
            let now = self.layout_json();
            if now != saved {
                let _ = std::fs::write(layout_path(), &now);
                saved = now;
            }
            // Live reload: a saved config or a terminal list shows within about a second.
            if last_scan.elapsed() >= Duration::from_millis(800) {
                last_scan = Instant::now();
                let sig = projects_signature(&self.projects_dir);
                if sig != self.signature {
                    self.signature = sig;
                    self.reload_projects();
                }
            }
            // Uptimes tick once a second even when nothing prints.
            if self.dirty.swap(false, Ordering::SeqCst) || last_tick.elapsed() >= Duration::from_secs(1) {
                last_tick = Instant::now();
                terminal.draw(|f| self.draw(f))?;
            }
            if !event::poll(Duration::from_millis(30))? {
                continue;
            }
            match event::read()? {
                Event::Key(k) if k.kind != KeyEventKind::Release => self.on_key(k),
                Event::Paste(text) if self.focus == Focus::Terminal => {
                    if let Some(t) = self.panes.get_mut(self.focused) {
                        t.send(text.as_bytes());
                    }
                }
                Event::Paste(text) => {
                    if let Some(p) = self.prompt.as_mut() {
                        p.text.push_str(&text);
                    }
                }
                Event::Mouse(m) => self.on_mouse(m),
                _ => {}
            }
            self.dirty.store(true, Ordering::SeqCst);
        }
        Ok(())
    }

    fn on_prompt_key(&mut self, k: event::KeyEvent) {
        let Some(p) = self.prompt.as_mut() else { return };
        match k.code {
            KeyCode::Esc => self.prompt = None,
            KeyCode::Enter => {
                let p = self.prompt.take().unwrap();
                if !p.text.trim().is_empty() {
                    self.act(Request::RenameTerminal { project: p.project, address: p.address, name: p.text.trim().to_string() }, "renamed".into());
                }
            }
            KeyCode::Backspace => {
                p.text.pop();
            }
            KeyCode::Char(c) => p.text.push(c),
            _ => {}
        }
    }

    fn on_key(&mut self, k: event::KeyEvent) {
        if self.prompt.is_some() {
            self.on_prompt_key(k);
            return;
        }
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        if self.focus == Focus::Terminal {
            if ctrl && matches!(k.code, KeyCode::Char(BACK) | KeyCode::Char('5')) {
                self.focus = Focus::Sidebar;
                return;
            }
            if let Some(t) = self.panes.get_mut(self.focused) {
                t.send(&term::encode(k));
            }
            return;
        }
        match k.code {
            KeyCode::Char('q') => self.quit = true,
            // Ctrl+W (and Ctrl+Q, which `q` already covers) close the window as on a Mac; in a pane they're
            // the program's (a shell deletes a word with Ctrl+W).
            KeyCode::Char('c' | 'w') if ctrl => self.quit = true,
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Left | KeyCode::Char('h') => {
                // On a command: up to its group; on an open group: close it.
                match self.selected_row() {
                    Some(Row::Group(g)) => {
                        self.projects[self.current].expanded.remove(&g);
                    }
                    Some(Row::Command(a)) => {
                        let p = &self.projects[self.current];
                        if let Some(g) = p.group_of(&a).map(|g| g.id.clone()) {
                            if let Some(r) = p.rows().iter().position(|r| *r == Row::Group(g.clone())) {
                                self.projects[self.current].selected = r;
                            }
                        }
                    }
                    None => {}
                }
            }
            KeyCode::Char('[') => self.switch_project(self.current.saturating_sub(1)),
            KeyCode::Char(']') => self.switch_project(self.current + 1),
            KeyCode::Char(d @ '1'..='9') => self.switch_project(d as usize - '1' as usize),
            KeyCode::Tab => self.focus_pane((self.focused + 1) % self.panes.len().max(1)),
            KeyCode::BackTab => self.focus_pane((self.focused + self.panes.len().max(1) - 1) % self.panes.len().max(1)),
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => match self.selected_row() {
                Some(Row::Group(_)) => self.activate_row(self.project().selected),
                Some(Row::Command(_)) => {
                    self.show_selected();
                    self.focus = Focus::Terminal;
                }
                None => {}
            },
            KeyCode::Char(' ') => match self.selected_row() {
                Some(Row::Group(_)) => self.activate_row(self.project().selected),
                _ => self.toggle_selected(),
            },
            KeyCode::Char('s') => self.toggle_selected(),
            KeyCode::Char('r') => self.restart_selected(),
            KeyCode::Char('a') => self.start_all(),
            KeyCode::Char('X') => self.stop_all(),
            KeyCode::Char('|') | KeyCode::Char('\\') => self.split(false),
            KeyCode::Char('-') => self.split(true),
            KeyCode::Char('x') => self.close_pane(self.focused),
            KeyCode::Char('t') => self.new_terminal(),
            KeyCode::Char('D') => self.close_terminal(),
            KeyCode::Char('R') => self.rename_terminal(),
            KeyCode::Char('c') => self.clear_focused(),
            KeyCode::Char('o') => self.open_port(),
            KeyCode::Char('?') => self.say("t terminal · R rename · D close terminal · c clear · o open port · | - split · x close pane · a/X start/stop all".into()),
            KeyCode::PageUp => self.scroll_focused(-(self.page() as isize)),
            KeyCode::PageDown => self.scroll_focused(self.page() as isize),
            _ => {}
        }
    }

    fn page(&self) -> u16 {
        let n = self.panes.len().max(1);
        let r = pane_rects(self.main_area, n, self.stacked)[self.focused.min(n - 1)];
        (term_area(r).height / 2).max(1)
    }

    fn scroll_focused(&mut self, lines: isize) {
        self.scroll_pane(self.focused, lines);
    }

    /// Negative scrolls back toward older output.
    fn scroll_pane(&mut self, j: usize, lines: isize) {
        if let Some(t) = self.panes.get_mut(j) {
            t.scroll(-lines);
        }
    }

    fn on_mouse(&mut self, m: event::MouseEvent) {
        let at = Position::new(m.column, m.row);
        if self.dragging_divider {
            match m.kind {
                MouseEventKind::Drag(MouseButton::Left) => {
                    self.sidebar_w = m.column.saturating_sub(RAIL_W).saturating_add(1).clamp(SIDEBAR_RANGE.0, SIDEBAR_RANGE.1);
                }
                _ => self.dragging_divider = false,
            }
            return;
        }
        let hit = self.hits.iter().rev().find(|(r, _)| r.contains(at)).map(|(_, h)| h.clone());
        match m.kind {
            MouseEventKind::Moved => {
                self.hovered_row = self.hits.iter().find_map(|(r, h)| match h {
                    Hit::Row(i) if r.contains(at) => Some(*i),
                    _ => None,
                });
            }
            MouseEventKind::ScrollUp => match hit {
                Some(Hit::Pane(j)) => self.scroll_pane(j, -3),
                _ => self.move_selection(-1),
            },
            MouseEventKind::ScrollDown => match hit {
                Some(Hit::Pane(j)) => self.scroll_pane(j, 3),
                _ => self.move_selection(1),
            },
            MouseEventKind::Down(MouseButton::Left) => match hit {
                Some(Hit::Divider) => self.dragging_divider = true,
                Some(Hit::Project(i)) => {
                    self.switch_project(i);
                    self.focus = Focus::Sidebar;
                }
                Some(Hit::Row(i)) => {
                    self.activate_row(i);
                    self.focus = Focus::Sidebar;
                }
                Some(Hit::StartAll) => self.start_all(),
                Some(Hit::StopAll) => self.stop_all(),
                Some(Hit::Stop(j)) => {
                    if let Some(t) = self.panes.get(j) {
                        self.toggle(t.project.clone(), t.address.clone());
                    }
                }
                Some(Hit::Restart(j)) => {
                    if let Some(t) = self.panes.get(j) {
                        self.restart(t.project.clone(), t.address.clone());
                    }
                }
                Some(Hit::Close(j)) => self.close_pane(j),
                Some(Hit::Port(p)) => open_external(&format!("http://localhost:{p}")),
                Some(Hit::RowToggle(a)) => self.toggle(self.project().slug.clone(), a),
                Some(Hit::RowRestart(a)) => self.restart(self.project().slug.clone(), a),
                Some(Hit::GroupStart(g)) => {
                    let slug = self.project().slug.clone();
                    self.act(Request::Start { project: slug, target: Some(g.clone()) }, format!("starting {g}"));
                }
                Some(Hit::GroupStop(g)) => {
                    let slug = self.project().slug.clone();
                    self.act(Request::Stop { project: slug, target: Some(g.clone()) }, format!("stopping {g}"));
                }
                Some(Hit::Pane(j)) => {
                    // A link under the pointer opens; otherwise the click focuses the pane.
                    let rects = pane_rects(self.main_area, self.panes.len(), self.stacked);
                    let area = rects.get(j).map(|r| term_area(*r));
                    let link = area.filter(|a| a.contains(at)).and_then(|a| self.panes[j].link_at(at.y - a.y, at.x - a.x));
                    match link {
                        Some(url) => {
                            open_external(&url);
                            self.say(format!("opened {url}"));
                        }
                        None => {
                            self.focus_pane(j);
                            self.focus = Focus::Terminal;
                        }
                    }
                }
                None => {}
            },
            _ => {}
        }
    }

    // ----- drawing -------------------------------------------------------------

    fn draw(&mut self, f: &mut Frame) {
        let area = f.area();
        self.hits.clear();
        let pal = self.project().palette.clone();
        let buf = f.buffer_mut();
        fill(buf, area, Style::default().bg(pal.window));

        let side_w = self.sidebar_w.min(area.width.saturating_sub(RAIL_W + 20));
        let rail = Rect::new(area.x, area.y, RAIL_W.min(area.width), area.height);
        let side = Rect::new(rail.right(), area.y, side_w, area.height);
        let main = Rect::new(side.right(), area.y, area.width.saturating_sub(RAIL_W + side_w), area.height);
        self.main_area = main;
        self.draw_rail(buf, rail, &pal);
        self.draw_sidebar(buf, side, &pal);
        let cursor = self.draw_panes(buf, main, &pal);
        if let (Some(c), Focus::Terminal, None) = (cursor, self.focus, &self.prompt) {
            f.set_cursor_position(c);
        }
    }

    fn draw_rail(&mut self, buf: &mut Buffer, r: Rect, pal: &Palette) {
        fill(buf, r, Style::default().bg(pal.rail));
        let mut y = r.y + 1;
        for i in 0..self.projects.len() {
            if y + 2 > r.bottom() {
                break;
            }
            let p = &self.projects[i];
            let (running, _) = self.running_count(p);
            let tile_bg = if p.errors.is_empty() { p.color.unwrap_or(p.palette.accent) } else { pal.crashed };
            let tile = Rect::new(r.x + 1, y, 4, 1);
            fill(buf, tile, Style::default().bg(tile_bg));
            buf.set_string(tile.x + 1, y, initials(&p.name), Style::default().fg(Color::White).bg(tile_bg).add_modifier(Modifier::BOLD));
            if i == self.current {
                buf.set_string(r.x, y, "▌", Style::default().fg(pal.accent).bg(pal.rail));
            }
            if !p.errors.is_empty() {
                buf.set_string(r.x + 1, y + 1, "▲", Style::default().fg(pal.error).bg(pal.rail));
            } else if running > 0 {
                buf.set_string(r.x + 1, y + 1, format!("●{running}"), Style::default().fg(p.palette.running).bg(pal.rail));
            }
            self.hits.push((Rect::new(r.x, y, r.width, 2), Hit::Project(i)));
            y += 3;
        }
    }

    fn draw_sidebar(&mut self, buf: &mut Buffer, r: Rect, pal: &Palette) {
        fill(buf, r, Style::default().bg(pal.sidebar));
        let divider_x = r.right().saturating_sub(1);
        for y in r.y..r.bottom() {
            buf.set_string(divider_x, y, "│", Style::default().fg(if self.dragging_divider { pal.accent } else { pal.line }).bg(pal.sidebar));
        }
        let inner_w = r.width.saturating_sub(2);
        let (running, total) = self.running_count(self.project());
        let p = self.project();
        let bg = Style::default().bg(pal.sidebar);
        let split = self.panes.len() > 1;

        let start = " ▶ Start all ";
        let start_w = start.chars().count() as u16;
        buf.set_string(r.x + 2, r.y + 1, truncate(&p.name, inner_w.saturating_sub(start_w + 3) as usize), bg.fg(pal.text).add_modifier(Modifier::BOLD));
        let sx = r.x + inner_w.saturating_sub(start_w);
        buf.set_string(sx, r.y + 1, start, Style::default().fg(pal.text).bg(pal.button));
        let start_hit = Rect::new(sx, r.y + 1, start_w, 1);
        let panes = self.panes.len().max(1);
        buf.set_string(r.x + 2, r.y + 2, format!("{running} of {total} running · {panes} pane{}", if panes == 1 { "" } else { "s" }), bg.fg(pal.text3));

        let footer_y = r.bottom().saturating_sub(1);
        let list = Rect::new(r.x, r.y + 4, inner_w, footer_y.saturating_sub(r.y + 5));
        let mut row_hits = Vec::new();
        let mut action_hits = Vec::new();

        if !p.errors.is_empty() {
            // The broken-config card, in place of the commands.
            buf.set_string(r.x + 2, list.y, "▲ The config has errors", bg.fg(pal.error).add_modifier(Modifier::BOLD));
            let mut y = list.y + 2;
            for e in &p.errors {
                for line in wrap(e, inner_w.saturating_sub(4) as usize) {
                    if y >= list.bottom() {
                        break;
                    }
                    buf.set_string(r.x + 2, y, line, bg.fg(pal.text2));
                    y += 1;
                }
            }
            if y + 1 < list.bottom() {
                buf.set_string(r.x + 2, y + 1, "Saving a fixed file clears this.", bg.fg(pal.text3));
            }
        } else {
            let rows = p.rows();
            let selected = p.selected;
            let skip = selected.saturating_sub(list.height.saturating_sub(1) as usize);
            let now = SystemTime::now();
            for (i, row) in rows.iter().enumerate().skip(skip).take(list.height as usize) {
                let y = list.y + (i - skip) as u16;
                let is_sel = i == selected;
                let hovered = self.hovered_row == Some(i);
                let row_bg = if is_sel { pal.selection } else if hovered { pal.hover } else { pal.sidebar };
                fill(buf, Rect::new(r.x, y, inner_w + 1, 1), Style::default().bg(row_bg));
                if is_sel {
                    let bar = if self.focus == Focus::Sidebar { pal.accent } else { pal.text4 };
                    buf.set_string(r.x, y, "▌", Style::default().fg(bar).bg(row_bg));
                }
                let rb = Style::default().bg(row_bg);
                match row {
                    Row::Group(id) => {
                        let g = p.groups.iter().find(|g| &g.id == id).unwrap();
                        let open = p.expanded.contains(id);
                        let chevron = if open { "⌄" } else { "›" };
                        buf.set_string(r.x + 2, y, format!("{chevron} {}", truncate(&g.name, inner_w.saturating_sub(16) as usize)), rb.fg(pal.text3).add_modifier(Modifier::BOLD));
                        let (g_running, g_total) = self.count_alive(&p.slug, g.commands.iter());
                        if hovered {
                            // Start all and Stop all for the group.
                            let bx = r.x + inner_w.saturating_sub(5);
                            buf.set_string(bx, y, "▶  ■", rb.fg(pal.text));
                            action_hits.push((Rect::new(bx, y, 1, 1), Hit::GroupStart(id.clone())));
                            action_hits.push((Rect::new(bx + 3, y, 1, 1), Hit::GroupStop(id.clone())));
                        } else {
                            // The count the app shows on a group row.
                            let (text, color) = if g_running > 0 { (format!("{g_running} running"), pal.running) } else { (format!("{g_total} idle"), pal.text4) };
                            buf.set_string(r.x + inner_w.saturating_sub(text.len() as u16), y, &text, rb.fg(color));
                        }
                    }
                    Row::Command(a) => {
                        let c = p.command(a).unwrap();
                        let indent = if p.loose.iter().any(|l| &l.address == a) { 2 } else { 4 };
                        let st = self.state(&p.slug, a);
                        let (glyph, color) = glyph(st.as_ref(), pal);
                        buf.set_string(r.x + indent, y, glyph, rb.fg(color));
                        let alive = Self::alive(st.as_ref());

                        // The right side, built from the edge inward: actions on hover, else
                        // uptime or countdown, the port, the apply badge, the pane number.
                        let mut right = inner_w;
                        let mut put = |text: &str, style: Style, buf: &mut Buffer| -> u16 {
                            right = right.saturating_sub(text.chars().count() as u16);
                            buf.set_string(r.x + right, y, text, style);
                            right = right.saturating_sub(1);
                            r.x + right + 1
                        };
                        if hovered {
                            if alive {
                                let x = put("↻", rb.fg(pal.text), buf);
                                action_hits.push((Rect::new(x, y, 1, 1), Hit::RowRestart(a.clone())));
                                let x = put("■", rb.fg(pal.text), buf);
                                action_hits.push((Rect::new(x, y, 1, 1), Hit::RowToggle(a.clone())));
                            } else {
                                let x = put("▶", rb.fg(pal.text), buf);
                                action_hits.push((Rect::new(x, y, 1, 1), Hit::RowToggle(a.clone())));
                            }
                        } else if st.as_ref().is_some_and(|s| s["restarting"] == true) {
                            let msg = st.as_ref().and_then(|s| s["message"].as_str()).unwrap_or("restarting").replace("crashed, ", "");
                            put(&msg, rb.fg(pal.crashed), buf);
                        } else if let Some(up) = st.as_ref().filter(|_| alive).and_then(|s| uptime(s, now)) {
                            put(&up, rb.fg(pal.text3), buf);
                        }
                        if let Some(port) = st.as_ref().filter(|_| alive).and_then(|s| s["port"].as_u64()) {
                            let x = put(&format!(":{port}"), rb.fg(pal.accent), buf);
                            action_hits.push((Rect::new(x, y, format!(":{port}").len() as u16, 1), Hit::Port(port)));
                        }
                        if st.as_ref().is_some_and(|s| s["needs_restart"] == true) && alive {
                            put("apply", Style::default().fg(pal.on_accent).bg(pal.starting), buf);
                        }
                        if let Some(j) = self.pane_of(&p.slug, a).filter(|_| split) {
                            let tag_style = if j == self.focused { Style::default().fg(pal.on_accent).bg(pal.accent) } else { Style::default().fg(pal.text2).bg(pal.button) };
                            put(&format!("{}", j + 1), tag_style, buf);
                        }
                        let name = if c.terminal { format!("▭ {}", c.name.rsplit(" / ").next().unwrap_or(&c.name)) } else { c.name.clone() };
                        let text_style = if is_sel { Style::default().fg(pal.selection_text).add_modifier(Modifier::BOLD) } else { Style::default().fg(pal.text2) };
                        let max = right.saturating_sub(indent + 2) as usize;
                        buf.set_string(r.x + indent + 2, y, truncate(&name, max), text_style.bg(row_bg));
                    }
                }
                row_hits.push((Rect::new(r.x, y, inner_w, 1), Hit::Row(i)));
            }
        }

        for x in r.x..divider_x {
            buf.set_string(x, footer_y.saturating_sub(1), "─", Style::default().fg(pal.line).bg(pal.sidebar));
        }
        if let Some(p) = &self.prompt {
            // The prompt takes the foot while it's open.
            let text = format!("{}: {}", p.label, p.text);
            let shown = truncate(&text, inner_w.saturating_sub(3) as usize);
            buf.set_string(r.x + 2, footer_y, &shown, bg.fg(pal.text));
            buf.set_string(r.x + 2 + shown.chars().count() as u16, footer_y, "▏", bg.fg(pal.accent));
        } else {
            buf.set_string(r.x + 2, footer_y, "●", bg.fg(if running > 0 { pal.running } else { pal.stopped }));
            buf.set_string(r.x + 4, footer_y, format!("{running} running"), bg.fg(pal.text3));
            if running > 0 {
                let stop = "Stop all";
                let stx = r.x + inner_w.saturating_sub(stop.len() as u16);
                buf.set_string(stx, footer_y, stop, bg.fg(pal.text3));
                self.hits.push((Rect::new(stx, footer_y, stop.len() as u16, 1), Hit::StopAll));
            }
        }
        self.hits.push((start_hit, Hit::StartAll));
        self.hits.push((Rect::new(divider_x, r.y, 1, r.height), Hit::Divider));
        self.hits.extend(row_hits);
        // Buttons sit on rows, so they're found before the row under them.
        self.hits.extend(action_hits);
    }

    fn draw_panes(&mut self, buf: &mut Buffer, main: Rect, pal: &Palette) -> Option<(u16, u16)> {
        if main.width < 10 || main.height < 4 {
            return None;
        }
        if self.panes.is_empty() {
            fill(buf, main, Style::default().bg(pal.terminal));
            buf.set_string(main.x + 2, main.y + 2, "Select a command", Style::default().fg(pal.text3).bg(pal.terminal));
            return None;
        }
        let rects = pane_rects(main, self.panes.len(), self.stacked);
        let mut cursor = None;
        for (j, rect) in rects.iter().enumerate() {
            if let Some(c) = self.draw_pane(buf, j, *rect) {
                if j == self.focused {
                    cursor = Some(c);
                }
            }
        }
        for rect in &rects {
            if rect.x > main.x {
                for y in rect.y..rect.bottom() {
                    buf.set_string(rect.x, y, "│", Style::default().fg(pal.line).bg(pal.header));
                }
            }
        }
        cursor
    }

    fn draw_pane(&mut self, buf: &mut Buffer, j: usize, rect: Rect) -> Option<(u16, u16)> {
        let (slug, address) = {
            let t = &self.panes[j];
            (t.project.clone(), t.address.clone())
        };
        let pi = self.project_index(&slug);
        // Each pane wears its own project's colours.
        let pal = pi.map(|i| self.projects[i].palette.clone()).unwrap_or_else(theme::graphite);
        let split = self.panes.len() > 1;
        let focused = j == self.focused;
        let bar = Rect::new(rect.x, rect.y, rect.width, 1);
        let foot = Rect::new(rect.x, rect.bottom().saturating_sub(1), rect.width, 1);
        let body = Rect::new(rect.x, rect.y + 1, rect.width, rect.height.saturating_sub(2));
        let bar_bg = if focused && split { pal.selection } else { pal.header };
        fill(buf, bar, Style::default().bg(bar_bg));
        fill(buf, foot, Style::default().bg(pal.header));
        fill(buf, body, Style::default().bg(pal.terminal));
        self.hits.push((body, Hit::Pane(j)));

        let st = self.state(&slug, &address);
        let (glyph, gcolor) = glyph(st.as_ref(), &pal);
        let (project_name, name, group, cmd_terminal) = match pi.map(|i| &self.projects[i]) {
            Some(p) => {
                let c = p.command(&address);
                (
                    p.name.clone(),
                    c.map(|c| c.name.rsplit(" / ").next().unwrap_or(&c.name).to_string()).unwrap_or_else(|| address.clone()),
                    p.group_of(&address).map(|g| title_case(&g.name)),
                    c.is_some_and(|c| c.terminal),
                )
            }
            None => (slug.clone(), address.clone(), None, false),
        };
        let hb = Style::default().bg(bar_bg);
        let mut x = bar.x + 2;
        if split {
            let tag = format!("{}", j + 1);
            let tag_style = if focused { Style::default().fg(pal.on_accent).bg(pal.accent) } else { Style::default().fg(pal.text2).bg(pal.button) };
            buf.set_string(x, bar.y, &tag, tag_style.add_modifier(Modifier::BOLD));
            x += 2;
        }
        buf.set_string(x, bar.y, glyph, hb.fg(gcolor));
        x += 2;
        if pi != Some(self.current) || split {
            buf.set_string(x, bar.y, format!("{project_name} · "), hb.fg(pal.text3));
            x += project_name.chars().count() as u16 + 3;
        }
        if let Some(g) = &group {
            buf.set_string(x, bar.y, format!("{g} / "), hb.fg(pal.text2).add_modifier(Modifier::BOLD));
            x += g.chars().count() as u16 + 3;
        }
        let shown_name = if cmd_terminal { format!("▭ {name}") } else { name.clone() };
        buf.set_string(x, bar.y, &shown_name, hb.fg(pal.text).add_modifier(Modifier::BOLD));
        x += shown_name.chars().count() as u16 + 1;
        let now = SystemTime::now();
        let alive = Self::alive(st.as_ref());
        if let Some(port) = st.as_ref().filter(|_| alive).and_then(|s| s["port"].as_u64()) {
            let label = format!(":{port}");
            buf.set_string(x, bar.y, &label, hb.fg(pal.accent));
            self.hits.push((Rect::new(x, bar.y, label.len() as u16, 1), Hit::Port(port)));
            x += label.len() as u16 + 1;
        }
        x += 1;
        let controls = if split { if alive { "■  ↻  ✕ " } else { "▶  ↻  ✕ " } } else if alive { "■  ↻ " } else { "▶  ↻ " };
        let cx = bar.right().saturating_sub(controls.chars().count() as u16 + 1);
        let status = st.as_ref().map(|s| describe(s, now)).unwrap_or_else(|| "Stopped".into());
        let room = cx.saturating_sub(x + 1) as usize;
        buf.set_string(x, bar.y, truncate(&status, room), hb.fg(pal.text3));
        buf.set_string(cx, bar.y, controls, hb.fg(pal.text2));
        self.hits.push((Rect::new(cx, bar.y, 2, 1), Hit::Stop(j)));
        self.hits.push((Rect::new(cx + 3, bar.y, 2, 1), Hit::Restart(j)));
        if split {
            self.hits.push((Rect::new(cx + 6, bar.y, 2, 1), Hit::Close(j)));
        }

        let mut parts = Vec::new();
        if let Some(cwd) = st.as_ref().and_then(|s| s["cwd"].as_str()) {
            parts.push(short_path(cwd));
        }
        if let Some(pid) = st.as_ref().and_then(|s| s["pid"].as_u64()).filter(|_| alive) {
            parts.push(format!("pid {pid}"));
        }
        if let Some(up) = st.as_ref().filter(|_| alive).and_then(|s| uptime(s, now)) {
            parts.push(format!("up {up}"));
        } else if let Some(summary) = st.as_ref().filter(|_| !alive).and_then(exit_summary) {
            parts.push(summary);
        }
        let info = parts.join("   ");
        let fb = Style::default().bg(pal.header);
        buf.set_string(foot.x + 2, foot.y, truncate(&info, foot.width.saturating_sub(4) as usize), fb.fg(pal.text3));
        if focused {
            let hint = match (&self.flash, self.focus) {
                (Some((m, at)), _) if at.elapsed() < Duration::from_secs(3) => m.clone(),
                (_, Focus::Terminal) => "typing into the terminal · Ctrl+] back to the list".into(),
                (_, Focus::Sidebar) => "↑↓ select · Enter type · s start/stop · t terminal · | - split · ? more · Ctrl+W close".into(),
            };
            let room = foot.width.saturating_sub(info.chars().count() as u16 + 8) as usize;
            if room > 12 {
                let hint = truncate(&hint, room);
                let hx = foot.right().saturating_sub(hint.chars().count() as u16 + 2);
                buf.set_string(hx, foot.y, &hint, fb.fg(pal.text4));
            }
        }

        let area = term_area(rect);
        // A command that has never run shows what it would run, not an empty terminal.
        let never_ran = st.as_ref().is_some_and(|s| s["status"] == "stopped" && s["started_at"].is_null() && s["ended_at"].is_null());
        if never_ran && !cmd_terminal {
            let s = st.as_ref().unwrap();
            let tb = Style::default().bg(pal.terminal);
            let mut y = area.y + 1;
            buf.set_string(area.x, y, format!("{name} has not run yet"), tb.fg(pal.text).add_modifier(Modifier::BOLD));
            y += 2;
            for (label, value) in [
                ("run", s["run"].as_str().unwrap_or("").to_string()),
                ("folder", s["cwd"].as_str().map(short_path).unwrap_or_else(|| "—".into())),
                ("autostart", if s["autostart"] == true { "on".into() } else { "off".into() }),
            ] {
                buf.set_string(area.x, y, format!("{label:<10}"), tb.fg(pal.text3));
                buf.set_string(area.x + 10, y, truncate(&value, area.width.saturating_sub(10) as usize), tb.fg(pal.terminal_text));
                y += 1;
            }
            y += 1;
            let start = " ▶ Start ";
            buf.set_string(area.x, y, start, Style::default().fg(pal.on_accent).bg(pal.accent).add_modifier(Modifier::BOLD));
            self.hits.push((Rect::new(area.x, y, start.chars().count() as u16, 1), Hit::Stop(j)));
            buf.set_string(area.x + start.chars().count() as u16 + 2, y, "or press s", tb.fg(pal.text3));
            // Keep the attach sized for when it starts.
            self.panes[j].fit(area.width, area.height);
            return None;
        }
        let t = &mut self.panes[j];
        t.fit(area.width, area.height);
        if t.scrolled > 0 {
            let note = format!(" ↑ {} lines back · PgDn ", t.scrolled);
            buf.set_string(area.right().saturating_sub(note.len() as u16), body.y, &note, Style::default().fg(pal.on_accent).bg(pal.accent));
        }
        t.render(buf, area, &pal)
    }
}

// ----- helpers ---------------------------------------------------------------------

fn fill(buf: &mut Buffer, r: Rect, style: Style) {
    for y in r.y..r.bottom() {
        for x in r.x..r.right() {
            let c = &mut buf[(x, y)];
            c.reset();
            c.set_style(style);
        }
    }
}

fn glyph(s: Option<&Value>, pal: &Palette) -> (&'static str, Color) {
    match s.and_then(|s| s["status"].as_str()) {
        Some("running") => ("●", pal.running),
        Some("starting") => ("◐", pal.starting),
        Some("crashed") if s.is_some_and(|s| s["restarting"] == true) => ("◐", pal.crashed),
        Some("crashed") => ("●", pal.crashed),
        Some("error") => ("▲", pal.error),
        Some("exited") => ("✓", pal.text3),
        _ => ("○", pal.stopped),
    }
}

fn uptime(s: &Value, now: SystemTime) -> Option<String> {
    let started = s["started_at"].as_f64()?;
    let now = now.duration_since(SystemTime::UNIX_EPOCH).ok()?.as_secs_f64();
    Some(clock((now - started).max(0.0) as u64))
}

fn clock(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
    } else {
        format!("{}:{:02}", secs / 60, secs % 60)
    }
}

/// The footer's account of a run that ended: how and after how long.
fn exit_summary(s: &Value) -> Option<String> {
    let (start, end) = (s["started_at"].as_f64()?, s["ended_at"].as_f64()?);
    let ran = clock((end - start).max(0.0) as u64);
    let how = match (s["status"].as_str(), s["exit_code"].as_i64(), s["signal"].as_str()) {
        (Some("stopped"), ..) => "stopped".to_string(),
        (_, Some(0), _) => "exited 0".to_string(),
        (_, Some(c), _) => format!("exited {c}"),
        (_, None, Some(sig)) => format!("killed by {sig}"),
        _ => "ended".to_string(),
    };
    Some(format!("{how} after {ran}"))
}

fn describe(s: &Value, now: SystemTime) -> String {
    match s["status"].as_str() {
        Some("running") => format!("Running · {}", uptime(s, now).unwrap_or_default()),
        Some("starting") => "Starting…".into(),
        Some("crashed") => s["message"].as_str().map(|m| format!("Crashed · {m}")).unwrap_or_else(|| "Crashed".into()),
        Some("error") => s["message"].as_str().unwrap_or("Error").to_string(),
        Some("exited") => "Finished".into(),
        _ => "Stopped".into(),
    }
}

fn initials(name: &str) -> String {
    let caps: String = name.chars().filter(|c| c.is_uppercase()).take(2).collect();
    if caps.chars().count() == 2 {
        caps
    } else {
        name.chars().take(2).collect::<String>().to_uppercase()
    }
}

fn title_case(s: &str) -> String {
    let lower = s.to_lowercase();
    let mut c = lower.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max.saturating_sub(1)).collect::<String>() + "…"
    }
}

fn wrap(s: &str, width: usize) -> Vec<String> {
    let width = width.max(10);
    let mut lines = vec![String::new()];
    for word in s.split_whitespace() {
        let last = lines.last_mut().unwrap();
        if !last.is_empty() && last.chars().count() + 1 + word.chars().count() > width {
            lines.push(word.to_string());
        } else {
            if !last.is_empty() {
                last.push(' ');
            }
            last.push_str(word);
        }
    }
    lines
}

fn short_path(p: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let p = if !home.is_empty() && p.starts_with(&home) { format!("~{}", &p[home.len()..]) } else { p.to_string() };
    let parts: Vec<&str> = p.split('/').collect();
    if parts.len() > 4 {
        format!("{}/…/{}", parts[0], parts[parts.len() - 2..].join("/"))
    } else {
        p
    }
}
