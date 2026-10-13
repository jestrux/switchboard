//! Debounced FSEvents watch over the Projects directory.

use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

const QUIET_PERIOD: Duration = Duration::from_millis(400);

pub struct Watcher {
    _inner: RecommendedWatcher,
}

impl Watcher {
    /// Call `on_change` once per burst of filesystem events under `dir`.
    pub fn new(dir: &Path, on_change: impl Fn() + Send + 'static) -> Result<Watcher, String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let (tx, rx) = mpsc::channel::<()>();
        let mut inner = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                // Ignore pure access events; everything else may change a config or icon.
                if !matches!(event.kind, notify::EventKind::Access(_)) {
                    let _ = tx.send(());
                }
            }
        })
        .map_err(|e| e.to_string())?;
        inner
            .watch(dir, RecursiveMode::Recursive)
            .map_err(|e| e.to_string())?;

        std::thread::Builder::new()
            .name("sb-watch".into())
            .spawn(move || {
                while rx.recv().is_ok() {
                    // Coalesce the burst: wait for a quiet period.
                    while rx.recv_timeout(QUIET_PERIOD).is_ok() {}
                    on_change();
                }
            })
            .map_err(|e| e.to_string())?;

        Ok(Watcher { _inner: inner })
    }
}
