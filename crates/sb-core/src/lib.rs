//! Switchboard engine: config loading, process supervision and PTY sessions.
//!
//! The `sb` CLI and the Mac app (through `sb-ffi`) are two front ends on this library.

pub mod config;
pub mod engine;
pub mod pidfile;
pub mod project;
pub mod session;
pub mod watcher;

pub use config::{CommandConfig, ConfigError, GroupConfig, ProjectConfig, ResolvedCommand};
pub use engine::{CommandState, Engine, EngineError, Event, Status};
pub use project::{Conflict, InstallError, Project};
pub use session::{CommandKey, Exit, Session};
