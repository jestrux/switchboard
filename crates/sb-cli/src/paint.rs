//! Colour only for people: none when the output is piped (agents, scripts) or `NO_COLOR`
//! is set. Same method names as `owo_colors`, so call sites read the same.

use owo_colors::{OwoColorize, Style};
use std::fmt::Display;
use std::io::IsTerminal;
use std::sync::OnceLock;

fn on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal())
}

pub trait Paint: Display {
    fn styled(&self, style: Style) -> String {
        if on() { format!("{}", self.style(style)) } else { self.to_string() }
    }
    fn green(&self) -> String { self.styled(Style::new().green()) }
    fn red(&self) -> String { self.styled(Style::new().red()) }
    fn yellow(&self) -> String { self.styled(Style::new().yellow()) }
    fn dimmed(&self) -> String { self.styled(Style::new().dimmed()) }
    fn bold(&self) -> String { self.styled(Style::new().bold()) }
    fn underline(&self) -> String { self.styled(Style::new().underline()) }
}

impl<T: Display + ?Sized> Paint for T {}
