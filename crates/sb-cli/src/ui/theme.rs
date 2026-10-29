//! The Mac app's palette tokens: Graphite (dark) as the base, a project's custom theme on
//! top, so a project looks the way it does in its own Mac window.

use ratatui::style::Color;
use sb_core::config::ThemeSpec;

#[derive(Clone)]
pub struct Palette {
    pub window: Color,
    pub rail: Color,
    pub sidebar: Color,
    pub line: Color,
    pub header: Color,
    pub terminal: Color,
    pub terminal_text: Color,
    pub text: Color,
    pub text2: Color,
    pub text3: Color,
    pub text4: Color,
    pub hover: Color,
    pub selection: Color,
    pub selection_text: Color,
    pub accent: Color,
    pub on_accent: Color,
    pub button: Color,
    pub running: Color,
    pub starting: Color,
    pub crashed: Color,
    pub error: Color,
    pub stopped: Color,
    pub ansi: [Color; 16],
}

const fn hex(v: u32) -> Color {
    Color::Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// Graphite, dark: `app/Switchboard/Theme.swift`.
pub fn graphite() -> Palette {
    Palette {
        window: hex(0x0B0C0E),
        rail: hex(0x0B0C0E),
        sidebar: hex(0x0E0F11),
        line: hex(0x1E2025),
        header: hex(0x0E0F11),
        terminal: hex(0x111214),
        terminal_text: hex(0xD5D7DC),
        text: hex(0xE6E7EA),
        text2: hex(0xA9ACB4),
        text3: hex(0x7D828B),
        text4: hex(0x4D525B),
        hover: hex(0x16171A),
        selection: hex(0x1D2233),
        selection_text: hex(0xFFFFFF),
        accent: hex(0x7C9CFF),
        on_accent: hex(0x0B0C0E),
        button: hex(0x16171A),
        running: hex(0x3DD68C),
        starting: hex(0xF5B83D),
        crashed: hex(0xFF5C5C),
        error: hex(0xFF8A3D),
        stopped: hex(0x5A5F68),
        ansi: [
            hex(0x1E2025), hex(0xFF5C5C), hex(0x3DD68C), hex(0xF5B83D), hex(0x7C9CFF), hex(0xC792EA), hex(0x5FD7E8), hex(0xC9CBD1),
            hex(0x4D525B), hex(0xFF7B7B), hex(0x6BE5A8), hex(0xFFD06B), hex(0xA3B8FF), hex(0xDDB3F5), hex(0x8AE6F2), hex(0xFFFFFF),
        ],
    }
}

pub fn parse_hex(s: &str) -> Option<Color> {
    let h = s.strip_prefix('#')?;
    let h = h.get(..6)?;
    u32::from_str_radix(h, 16).ok().map(hex)
}

/// Graphite with the project's dark-scheme overrides applied.
pub fn for_project(theme: Option<&ThemeSpec>) -> Palette {
    let mut p = graphite();
    let Some(ThemeSpec::Custom(custom)) = theme else { return p };
    let Some(tokens) = &custom.dark else { return p };
    for (key, value) in tokens {
        if key == "ansi" {
            for (i, c) in value.as_sequence().into_iter().flatten().enumerate().take(16) {
                if let Some(c) = c.as_str().and_then(parse_hex) {
                    p.ansi[i] = c;
                }
            }
            continue;
        }
        let Some(c) = value.as_str().and_then(parse_hex) else { continue };
        let slot = match key.as_str() {
            "window" => &mut p.window,
            "rail" => &mut p.rail,
            "sidebar" => &mut p.sidebar,
            "line" => &mut p.line,
            "header" => &mut p.header,
            "terminal" => &mut p.terminal,
            "terminalText" => &mut p.terminal_text,
            "text" => &mut p.text,
            "text2" => &mut p.text2,
            "text3" => &mut p.text3,
            "text4" => &mut p.text4,
            "hover" => &mut p.hover,
            "selection" => &mut p.selection,
            "selectionText" => &mut p.selection_text,
            "accent" => &mut p.accent,
            "onAccent" => &mut p.on_accent,
            "button" => &mut p.button,
            "running" => &mut p.running,
            "starting" => &mut p.starting,
            "crashed" => &mut p.crashed,
            "error" => &mut p.error,
            "stopped" => &mut p.stopped,
            _ => continue,
        };
        *slot = c;
    }
    p
}
