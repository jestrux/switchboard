//! Switchboard engine: config loading, process supervision and PTY sessions.
//!
//! The `sb` CLI and the Mac app (through `sb-ffi`) are two front ends on this library.

pub mod config;
pub mod control;
pub mod engine;
pub mod board;
pub mod fragment;
pub mod home;
pub mod icons;
pub mod pidfile;
pub mod ports;
pub mod project;
pub mod session;
pub mod solo;
pub mod terminals;
pub mod tools;
pub mod watcher;

pub use config::{CommandConfig, ConfigError, GroupConfig, ProjectConfig, ResolvedCommand};
pub use engine::{CommandState, Engine, EngineError, Event, Status};
pub use project::{Conflict, InstallError, Project};
pub use session::{CommandKey, Exit, Session};
