//! Named icons for groups: an explicit `icon:` in the config, or one derived from the
//! group's name, id and commands.

/// Icon keys a group may name in `icon:`. The app maps each to a glyph.
pub const GROUP_ICONS: [&str; 16] = [
    "frontend", "backend", "e2e", "terminal", "mobile", "docs", "database", "worker",
    "tunnel", "infra", "ai", "design", "scripts", "storybook", "api", "folder",
];

/// Keyword → icon, checked in order against the group's id and name, then its commands.
const RULES: [(&[&str], &str); 13] = [
    (&["e2e", "test", "qa", "journey", "playwright", "cypress"], "e2e"),
    (&["storybook"], "storybook"),
    (&["mobile", "ios", "android", "expo", "react-native", "flutter"], "mobile"),
    (&["tunnel", "ngrok", "proxy", "cloudflared"], "tunnel"),
    (&["worker", "queue", "jobs", "celery", "sidekiq", "cron"], "worker"),
    (&["database", "postgres", "mysql", "redis", "mongo", "db"], "database"),
    (&["infra", "docker", "deploy", "terraform", "k8s", "compose"], "infra"),
    (&["docs", "documentation", "wiki"], "docs"),
    (&["design", "figma", "tokens"], "design"),
    (&["ai", "agent", "claude", "llm"], "ai"),
    (&["api", "hapi", "graphql", "gateway"], "api"),
    (&["backend", "server", "uvicorn", "django", "rails", "laravel", "fastapi"], "backend"),
    (&["frontend", "web", "app", "ui", "client", "dashboard", "site", "vite", "next", "react-router"], "frontend"),
];

fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

fn matches(text: &str) -> Option<&'static str> {
    let lower = text.to_lowercase();
    let ws = words(text);
    for (keys, icon) in RULES {
        for key in keys {
            // Multi-part keys (react-native) match as substrings; plain ones as whole words,
            // so "app" does not fire inside "wrapper".
            let hit = if key.contains('-') { lower.contains(key) } else { ws.iter().any(|w| w == key) };
            if hit {
                return Some(icon);
            }
        }
    }
    None
}

/// The icon for a group: its explicit one if valid, else derived from its id and name,
/// then from what its commands run, else `folder`.
pub fn group_icon(explicit: Option<&str>, id: &str, name: &str, runs: &[&str]) -> &'static str {
    if let Some(e) = explicit {
        if let Some(known) = GROUP_ICONS.iter().find(|k| **k == e) {
            return known;
        }
    }
    if let Some(i) = matches(&format!("{id} {name}")) {
        return i;
    }
    for run in runs {
        if let Some(i) = matches(run) {
            return i;
        }
    }
    "folder"
}
