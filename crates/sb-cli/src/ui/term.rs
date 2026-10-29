//! One command's terminal inside the UI: an attach connection to the host feeding a vt100
//! screen we draw ourselves, and keystrokes encoded back into the bytes a terminal sends.

use super::theme::Palette;
use anyhow::{bail, Result};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use sb_core::control::{self, Request};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

const SCROLLBACK: usize = 5000;

pub struct TermView {
    pub project: String,
    pub address: String,
    parser: Arc<Mutex<vt100::Parser>>,
    writer: Mutex<UnixStream>,
    size: (u16, u16),
    /// Lines scrolled back from the bottom.
    pub scrolled: usize,
}

impl TermView {
    /// Attach to `project/address` sized `cols`×`rows`. Output marks `dirty`.
    pub fn open(project: &str, address: &str, cols: u16, rows: u16, dirty: Arc<AtomicBool>) -> Result<TermView> {
        let (cols, rows) = (cols.max(2), rows.max(2));
        let mut stream = UnixStream::connect(control::socket_path())?;
        let req = Request::Attach { project: project.into(), address: address.into(), cols, rows };
        writeln!(stream, "{}", serde_json::to_string(&req)?)?;
        // The reply line, a byte at a time so the output after it stays in the socket.
        let mut line = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            if stream.read(&mut byte)? == 0 {
                bail!("Switchboard closed the connection");
            }
            if byte[0] == b'\n' {
                break;
            }
            line.push(byte[0]);
        }
        let reply: serde_json::Value = serde_json::from_slice(&line)?;
        if reply["ok"] != true {
            bail!("{}", reply["error"].as_str().unwrap_or("attach failed"));
        }
        let parser = Arc::new(Mutex::new(vt100::Parser::new(rows, cols, SCROLLBACK)));
        {
            let (parser, mut reader) = (Arc::clone(&parser), stream.try_clone()?);
            std::thread::spawn(move || {
                let mut buf = [0u8; 16384];
                while let Ok(n) = reader.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    parser.lock().unwrap().process(&buf[..n]);
                    dirty.store(true, Ordering::SeqCst);
                }
            });
        }
        Ok(TermView { project: project.into(), address: address.into(), parser, writer: Mutex::new(stream), size: (cols, rows), scrolled: 0 })
    }

    pub fn send(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        self.scroll_to_bottom();
        let _ = self.writer.lock().unwrap().write_all(&control::frame(control::FRAME_INPUT, bytes));
    }

    /// Fit the command's terminal (and our screen) to `cols`×`rows`.
    pub fn fit(&mut self, cols: u16, rows: u16) {
        let (cols, rows) = (cols.max(2), rows.max(2));
        if (cols, rows) == self.size {
            return;
        }
        self.size = (cols, rows);
        self.parser.lock().unwrap().screen_mut().set_size(rows, cols);
        let _ = self.writer.lock().unwrap().write_all(&control::resize_frame(cols, rows));
    }

    pub fn scroll(&mut self, lines: isize) {
        let mut p = self.parser.lock().unwrap();
        let want = (self.scrolled as isize + lines).max(0) as usize;
        p.screen_mut().set_scrollback(want);
        // vt100 clamps to what it has; read back where it landed.
        self.scrolled = p.screen().scrollback();
    }

    /// Wipe this view's screen and scrollback (the Mac's Clear). The command is untouched.
    pub fn clear(&mut self) {
        let (cols, rows) = self.size;
        *self.parser.lock().unwrap() = vt100::Parser::new(rows, cols, SCROLLBACK);
        self.scrolled = 0;
    }

    /// The URL under `col` on visible `row`, if any: plain `http(s)://` links, which covers
    /// Markdown `[text](url)` links too.
    pub fn link_at(&self, row: u16, col: u16) -> Option<String> {
        let p = self.parser.lock().unwrap();
        let screen = p.screen();
        let (_, cols) = screen.size();
        // One string per column, so a column maps straight to a position in the line.
        let cells: Vec<String> = (0..cols).map(|c| screen.cell(row, c).map(|x| x.contents().to_string()).filter(|t| !t.is_empty()).unwrap_or_else(|| " ".into())).collect();
        let line: String = cells.concat();
        let mut start_col = Vec::with_capacity(cells.len());
        let mut at = 0;
        for c in &cells {
            start_col.push(at);
            at += c.len();
        }
        let byte = *start_col.get(col as usize)?;
        let mut search = 0;
        while let Some(found) = line[search..].find("http") {
            let begin = search + found;
            let rest = &line[begin..];
            if !(rest.starts_with("http://") || rest.starts_with("https://")) {
                search = begin + 4;
                continue;
            }
            let len = rest.find(|ch: char| ch.is_whitespace() || matches!(ch, '"' | '\'' | '<' | '>' | '`')).unwrap_or(rest.len());
            let mut url = &rest[..len];
            // Trailing punctuation and a Markdown link's closing paren belong to the prose.
            while url.ends_with(['.', ',', ';', ':', ')', ']', '}']) {
                url = &url[..url.len() - 1];
            }
            if byte >= begin && byte < begin + url.len() {
                return Some(url.to_string());
            }
            search = begin + len.max(1);
        }
        None
    }

    fn scroll_to_bottom(&mut self) {
        if self.scrolled > 0 {
            self.parser.lock().unwrap().screen_mut().set_scrollback(0);
            self.scrolled = 0;
        }
    }

    /// Draw the screen into `area`; returns the cursor cell when it should show.
    pub fn render(&self, buf: &mut Buffer, area: Rect, pal: &Palette) -> Option<(u16, u16)> {
        let p = self.parser.lock().unwrap();
        let screen = p.screen();
        let base = Style::default().fg(pal.terminal_text).bg(pal.terminal);
        for y in 0..area.height {
            for x in 0..area.width {
                let cell = &mut buf[(area.x + x, area.y + y)];
                cell.reset();
                cell.set_style(base);
                let Some(c) = screen.cell(y, x) else { continue };
                if c.is_wide_continuation() {
                    cell.set_diff_option(ratatui::buffer::CellDiffOption::Skip);
                    continue;
                }
                let text = c.contents();
                cell.set_symbol(if text.is_empty() { " " } else { text });
                let (mut fg, mut bg) = (color(c.fgcolor(), pal, pal.terminal_text), color(c.bgcolor(), pal, pal.terminal));
                if c.inverse() {
                    std::mem::swap(&mut fg, &mut bg);
                }
                let mut style = Style::default().fg(fg).bg(bg);
                if c.bold() {
                    style = style.add_modifier(Modifier::BOLD);
                }
                if c.dim() {
                    style = style.add_modifier(Modifier::DIM);
                }
                if c.italic() {
                    style = style.add_modifier(Modifier::ITALIC);
                }
                if c.underline() {
                    style = style.add_modifier(Modifier::UNDERLINED);
                }
                cell.set_style(style);
            }
        }
        if screen.hide_cursor() || self.scrolled > 0 {
            return None;
        }
        let (row, col) = screen.cursor_position();
        (row < area.height && col < area.width).then(|| (area.x + col, area.y + row))
    }
}

impl Drop for TermView {
    fn drop(&mut self) {
        // Ends the reader thread; the command keeps running under the host.
        let _ = self.writer.lock().unwrap().shutdown(std::net::Shutdown::Both);
    }
}

fn color(c: vt100::Color, pal: &Palette, default: Color) -> Color {
    match c {
        vt100::Color::Default => default,
        vt100::Color::Idx(i) if (i as usize) < 16 => pal.ansi[i as usize],
        vt100::Color::Idx(i) => Color::Indexed(i),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

/// The bytes a terminal sends for `key` (xterm conventions).
pub fn encode(key: KeyEvent) -> Vec<u8> {
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let mut out = match key.code {
        KeyCode::Char(c) if ctrl => {
            let b = c.to_ascii_lowercase() as u8;
            match b {
                b'a'..=b'z' => vec![b - b'a' + 1],
                b' ' | b'2' | b'@' => vec![0],
                b'[' | b'3' => vec![0x1b],
                b'\\' | b'4' => vec![0x1c],
                b'^' | b'6' => vec![0x1e],
                b'_' | b'-' | b'7' => vec![0x1f],
                _ => c.to_string().into_bytes(),
            }
        }
        KeyCode::Char(c) => c.to_string().into_bytes(),
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::F(n) => match n {
            1 => b"\x1bOP".to_vec(),
            2 => b"\x1bOQ".to_vec(),
            3 => b"\x1bOR".to_vec(),
            4 => b"\x1bOS".to_vec(),
            5 => b"\x1b[15~".to_vec(),
            6 => b"\x1b[17~".to_vec(),
            7 => b"\x1b[18~".to_vec(),
            8 => b"\x1b[19~".to_vec(),
            9 => b"\x1b[20~".to_vec(),
            10 => b"\x1b[21~".to_vec(),
            11 => b"\x1b[23~".to_vec(),
            12 => b"\x1b[24~".to_vec(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    };
    if alt && !out.is_empty() {
        out.insert(0, 0x1b);
    }
    out
}
