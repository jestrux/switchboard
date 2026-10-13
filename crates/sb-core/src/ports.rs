//! Finding the address a dev server says it is listening on, from its output.
//!
//! Servers announce themselves in a handful of shapes: `Local: http://localhost:3006/`,
//! `Uvicorn running on http://127.0.0.1:8010`, `listening on :8080`. We read what the
//! program printed rather than asking the OS, so the port is the one it actually took
//! (Vite falling back from 3000 to 3002, for instance).

/// A detected local address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listening {
    pub port: u16,
    /// A URL to open: `http://localhost:<port>` (https when the output said so).
    pub url: String,
}

const HOSTS: [&str; 5] = ["localhost:", "127.0.0.1:", "0.0.0.0:", "[::1]:", "[::]:"];
/// Lines that mention an address they failed to reach or could not bind.
const NOT_LISTENING: [&str; 9] = [
    "refused", "econnrefused", "eaddrinuse", "in use", "failed", "error", "timeout", "timed out", "unreachable",
];

/// Remove ANSI escape sequences (CSI and OSC) so colours do not split the text we match.
pub fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.peek() {
            Some('[') => {
                chars.next();
                for n in chars.by_ref() {
                    if ('@'..='~').contains(&n) { break; }
                }
            }
            Some(']') => {
                chars.next();
                while let Some(n) = chars.next() {
                    if n == '\u{7}' { break; }
                    if n == '\u{1b}' { chars.next(); break; }
                }
            }
            _ => { chars.next(); }
        }
    }
    out
}

fn port_at(text: &str) -> Option<u16> {
    let digits: String = text.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.len() < 2 || digits.len() > 5 { return None; }
    digits.parse::<u16>().ok().filter(|p| *p >= 80)
}

/// The address one line of output announces, if any. Pass a line without ANSI codes.
pub fn detect(line: &str) -> Option<Listening> {
    let lower = line.to_lowercase();
    if NOT_LISTENING.iter().any(|w| lower.contains(w)) {
        return None;
    }
    // host:port, with or without a scheme.
    for host in HOSTS {
        if let Some(i) = lower.find(host) {
            if let Some(port) = port_at(&lower[i + host.len()..]) {
                let https = lower[..i].ends_with("https://");
                return Some(Listening { port, url: format!("{}://localhost:{port}", if https { "https" } else { "http" }) });
            }
        }
    }
    // "listening on :8080", "listening on port 8080", "started on port 8080".
    for cue in ["listening on", "listening at", "started on", "running on", "running at", "ready on"] {
        if let Some(i) = lower.find(cue) {
            let rest = lower[i + cue.len()..].trim_start();
            let rest = rest.strip_prefix("port").map(str::trim_start).unwrap_or(rest);
            let rest = rest.strip_prefix(':').unwrap_or(rest);
            if let Some(port) = port_at(rest) {
                return Some(Listening { port, url: format!("http://localhost:{port}") });
            }
        }
    }
    None
}

/// Feeds PTY output in arbitrary chunks and reports the first announced address.
#[derive(Default)]
pub struct Scanner {
    pending: Vec<u8>,
    found: bool,
}

impl Scanner {
    /// Returns the address the first time a complete line announces one.
    pub fn feed(&mut self, chunk: &[u8]) -> Option<Listening> {
        if self.found {
            return None;
        }
        self.pending.extend_from_slice(chunk);
        let mut result = None;
        while let Some(end) = self.pending.iter().position(|b| *b == b'\n' || *b == b'\r') {
            let line: Vec<u8> = self.pending.drain(..=end).collect();
            if result.is_none() {
                let text = strip_ansi(&String::from_utf8_lossy(&line));
                result = detect(text.trim());
            }
        }
        // A program that never prints a newline must not grow this without bound.
        if self.pending.len() > 8 * 1024 {
            self.pending.clear();
        }
        if result.is_some() {
            self.found = true;
            self.pending = Vec::new();
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn port(line: &str) -> Option<u16> {
        detect(&strip_ansi(line)).map(|l| l.port)
    }

    #[test]
    fn finds_the_usual_announcements() {
        assert_eq!(port("  \u{1b}[32m➜\u{1b}[39m  \u{1b}[1mLocal\u{1b}[22m:   \u{1b}[36mhttp://localhost:\u{1b}[1m3006\u{1b}[22m/\u{1b}[39m"), Some(3006));
        assert_eq!(port("INFO:     Uvicorn running on http://127.0.0.1:8010 (Press CTRL+C to quit)"), Some(8010));
        assert_eq!(port("Started connect web server on http://localhost:9000"), Some(9000));
        assert_eq!(port("│   - Local:    http://localhost:6006/   │"), Some(6006));
        assert_eq!(port("Server listening on :8080"), Some(8080));
        assert_eq!(port("api listening on port 4000"), Some(4000));
        assert_eq!(port("Forwarding  https://abc.ngrok.app -> http://localhost:3000"), Some(3000));
        assert_eq!(detect("ready - started server on 0.0.0.0:3000, url: https://localhost:3000").unwrap().url, "https://localhost:3000");
        assert_eq!(detect("Local: https://localhost:5173/").unwrap().url, "https://localhost:5173");
    }

    #[test]
    fn ignores_lines_that_are_not_a_server_starting() {
        assert_eq!(port("> react-router dev --port 3006"), None, "the echoed command is not an announcement");
        assert_eq!(port("Port 3000 is in use, trying another one..."), None);
        assert_eq!(port("panic: dial tcp 127.0.0.1:5432: connect: connection refused"), None);
        assert_eq!(port("Error: listen EADDRINUSE: address already in use :::3000"), None);
        assert_eq!(port("10:16:12 [vite] hmr update /app/routes/home.tsx"), None);
        assert_eq!(port("Found 0 errors."), None);
    }

    #[test]
    fn scanner_handles_split_chunks_and_reports_once() {
        let mut s = Scanner::default();
        assert_eq!(s.feed(b"> dev\r\n> vite\r\n  Local:   http://local"), None);
        assert_eq!(s.feed(b"host:51").map(|l| l.port), None, "the line is not complete yet");
        assert_eq!(s.feed(b"73/\r\n  Network: use --host\r\n").map(|l| l.port), Some(5173));
        assert_eq!(s.feed(b"  Local:   http://localhost:9999/\r\n"), None, "first one wins");
    }
}
