//! Suggesting a project's home folder: the repo folder that holds its `.switchboard/`
//! (`project.yaml`, and the board in `.switchboard/board/`). A project has no home until the
//! author keeps its settings in a repo; the app's Keep panel and `switchboard keep` both start
//! from this suggestion, so an agent can propose one and adopt it on the author's yes.
//!
//! The rule follows how the author's projects are already set up. When one repo holds more of
//! the project's folders than any other, its root (kinpay, set-hero). When the folders are
//! separate repos side by side, the `e2e` one: it exercises the whole product, so it belongs to
//! the project rather than one part (career-path, citybuild, otesha). Else a folder that holds
//! the others, or a single folder's repo.

use crate::project::Project;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HomeSuggestion {
    /// The folder to propose, if the rule finds one.
    pub suggested: Option<PathBuf>,
    /// Why, in a few words, for the card or the CLI.
    pub reason: Option<String>,
    /// Every sensible choice: the project's command folders and the repos they're in.
    pub candidates: Vec<PathBuf>,
}

/// The suggestion for a project, from the folders its commands run in (ad-hoc terminals
/// left out). A project that already has a home still gets a suggestion; callers decide.
pub fn suggest(project: &Project) -> HomeSuggestion {
    let folders: Vec<PathBuf> = match &project.config {
        Ok(cfg) => cfg
            .compose(&project.dir)
            .config
            .resolve_commands()
            .into_iter()
            .filter(|c| !c.terminal)
            .filter_map(|c| c.cwd)
            .filter(|p| p.is_dir() && !p.starts_with(&project.dir))
            .collect(),
        Err(_) => Vec::new(),
    };
    suggest_from(&folders)
}

/// The rule itself, on a list of folders (duplicates allowed).
pub fn suggest_from(folders: &[PathBuf]) -> HomeSuggestion {
    let mut dirs: Vec<PathBuf> = folders.iter().map(|p| clean(p)).collect();
    dirs.sort();
    dirs.dedup();

    let mut candidates = dirs.clone();
    for d in &dirs {
        if let Some(root) = repo_root(d) {
            candidates.push(root);
        }
    }
    candidates.sort();
    candidates.dedup();

    let pick = |p: PathBuf, why: &str| HomeSuggestion { suggested: Some(p), reason: Some(why.to_string()), candidates: candidates.clone() };

    if dirs.is_empty() {
        return HomeSuggestion { suggested: None, reason: None, candidates };
    }
    // 1. A repo holding more of the project's folders than any other.
    let mut counts: Vec<(PathBuf, usize)> = Vec::new();
    for root in dirs.iter().filter_map(|d| repo_root(d)) {
        match counts.iter_mut().find(|(r, _)| *r == root) {
            Some((_, n)) => *n += 1,
            None => counts.push((root, 1)),
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1));
    if let Some((root, n)) = counts.first() {
        let clear = counts.get(1).map_or(true, |(_, m)| m < n);
        if *n >= 2 && clear {
            return pick(root.clone(), "most of the project's folders are in this repo");
        }
    }
    // 2. Separate repos: an e2e folder.
    if dirs.len() > 1 {
        let mut e2e: Vec<&PathBuf> = dirs.iter().filter(|d| is_e2e(d)).collect();
        e2e.sort_by_key(|d| d.components().count());
        if let Some(d) = e2e.first() {
            return pick((*d).clone(), "the e2e folder exercises the whole product");
        }
    }
    // 3. A folder that holds the others; with one folder, its repo.
    if let Some(d) = dirs.iter().find(|d| dirs.iter().all(|o| o.starts_with(d))) {
        if dirs.len() == 1 {
            return pick(repo_root(d).unwrap_or_else(|| d.clone()), "the project's only repo");
        }
        return pick(d.clone(), "it holds the project's other folders");
    }
    HomeSuggestion { suggested: None, reason: None, candidates }
}

fn is_e2e(dir: &Path) -> bool {
    let name = dir.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    name == "e2e" || name.ends_with("-e2e") || name.ends_with("_e2e")
}

/// The nearest folder at or above `dir` with a `.git`.
fn repo_root(dir: &Path) -> Option<PathBuf> {
    dir.ancestors().find(|a| a.join(".git").exists()).map(Path::to_path_buf)
}

fn clean(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(paths: &[&str]) -> tempfile::TempDir {
        let t = tempfile::tempdir().unwrap();
        for p in paths {
            std::fs::create_dir_all(t.path().join(p)).unwrap();
        }
        t
    }
    fn at(t: &tempfile::TempDir, p: &str) -> PathBuf {
        clean(&t.path().join(p))
    }

    #[test]
    fn e2e_wins_among_side_by_side_repos() {
        // career-path, citybuild, otesha, thrive: each folder its own repo.
        let t = tree(&["cp/frontend/.git", "cp/backend/.git", "cp/e2e/.git"]);
        let s = suggest_from(&[at(&t, "cp/frontend"), at(&t, "cp/backend"), at(&t, "cp/e2e")]);
        assert_eq!(s.suggested, Some(at(&t, "cp/e2e")));
    }

    #[test]
    fn a_suffixed_e2e_folder_counts() {
        let t = tree(&["web/.git", "journeys-e2e/.git"]);
        let s = suggest_from(&[at(&t, "web"), at(&t, "journeys-e2e")]);
        assert_eq!(s.suggested, Some(at(&t, "journeys-e2e")));
    }

    #[test]
    fn a_repo_holding_most_folders_beats_another_repos_e2e() {
        // kinpay: three folders in the kinpay repo; Circles' e2e is its own repo.
        let t = tree(&["kp/.git", "kp/app", "kp/studio", "kpb/.git", "kpc/backend/.git", "kpc/journeys-e2e/.git"]);
        let s = suggest_from(&[at(&t, "kp"), at(&t, "kp/app"), at(&t, "kp/studio"), at(&t, "kpb"),
                               at(&t, "kpc/backend"), at(&t, "kpc/journeys-e2e")]);
        assert_eq!(s.suggested, Some(at(&t, "kp")));
    }

    #[test]
    fn a_monorepo_with_one_folder_elsewhere() {
        // set-hero: app and hapi in the set-hero repo, prod-app in another.
        let t = tree(&["sh/.git", "sh/app", "sh/hapi", "hz/.git", "hz/prod-app"]);
        let s = suggest_from(&[at(&t, "sh/app"), at(&t, "sh/hapi"), at(&t, "hz/prod-app")]);
        assert_eq!(s.suggested, Some(at(&t, "sh")));
    }

    #[test]
    fn one_repo_suggests_its_root() {
        // switchboard itself: board/ and docs-site/ in one repo.
        let t = tree(&["sb/.git", "sb/board", "sb/docs-site"]);
        let s = suggest_from(&[at(&t, "sb/board"), at(&t, "sb/docs-site")]);
        assert_eq!(s.suggested, Some(at(&t, "sb")));
    }

    #[test]
    fn separate_repos_without_e2e_get_no_suggestion() {
        let t = tree(&["a/.git", "b/.git"]);
        let s = suggest_from(&[at(&t, "a"), at(&t, "b")]);
        assert_eq!(s.suggested, None);
        assert_eq!(s.candidates, vec![at(&t, "a"), at(&t, "b")]);
    }

    #[test]
    fn no_folders_no_suggestion() {
        assert_eq!(suggest_from(&[]).suggested, None);
    }
}
