import AppKit
import SwiftTerm

/// ⌘-click and ⌘-hover for links in terminals.
///
/// SwiftTerm already detects plain URLs and OSC 8 hyperlinks (underlining them while ⌘
/// is held) and asks the delegate to open them. This adds Markdown links, `[text](url)`,
/// the way Claude Code prints them in plain output: ⌘-hovering the link text underlines
/// it and ⌘-clicking anywhere on the text or the URL opens it. It reads the line from the
/// engine's parsed screen, since SwiftTerm exposes no text access.
@MainActor
enum LinkHandling {
    private static var monitors: [Any] = []
    private static let markdown = try! NSRegularExpression(pattern: #"\[([^\]\n]+)\]\(([^)\s]+)\)"#)
    private static let plainURL = try! NSRegularExpression(pattern: #"(?:https?|ftp)://[^\s<>"'\[\]()]+|www\.[^\s<>"'\[\]()]+"#)
    /// Set when a ⌘-mouse-down was handled here, so the matching mouse-up is swallowed too
    /// (otherwise SwiftTerm would open the same URL a second time).
    private static var swallowNextMouseUp = false

    static func install() {
        guard monitors.isEmpty else { return }
        hoverTrace = ProcessInfo.processInfo.environment["SB_TRACE_LINKS"] != nil || UserDefaults.standard.bool(forKey: "traceLinks")
        monitors.append(NSEvent.addLocalMonitorForEvents(matching: .leftMouseDown) { event in
            guard event.modifierFlags.contains(.command) else { return event }
            guard let window = event.window, let hit = link(in: window, at: event.locationInWindow) else { return event }
            open(hit.url)
            swallowNextMouseUp = true
            return nil
        }!)
        monitors.append(NSEvent.addLocalMonitorForEvents(matching: .leftMouseUp) { event in
            guard swallowNextMouseUp else { return event }
            swallowNextMouseUp = false
            return nil
        }!)
        monitors.append(NSEvent.addLocalMonitorForEvents(matching: [.mouseMoved, .flagsChanged, .mouseExited]) { event in
            forwardModifierToTerminal(event)
            updateHover(event)
            return event
        }!)
    }

    /// SwiftTerm learns that ⌘ is down from `flagsChanged`, which AppKit sends only to the
    /// first responder. Pass it to the terminal under the pointer so plain-URL and OSC 8
    /// underlines and clicks work in panes that don't have keyboard focus.
    private static func forwardModifierToTerminal(_ event: NSEvent) {
        guard event.type == .flagsChanged, let window = event.window else { return }
        let location = window.mouseLocationOutsideOfEventStream
        guard let (_, view) = TerminalRegistry.shared.terminal(in: window, at: location),
              window.firstResponder !== view else { return }
        view.flagsChanged(with: event)
    }

    private struct Hit {
        let view: NSView
        let url: String
        let rect: NSRect
    }

    /// The Markdown or plain URL link under the pointer, if any. OSC 8 hyperlinks are
    /// SwiftTerm's: it underlines, previews and opens them itself.
    private static func link(in window: NSWindow, at locationInWindow: NSPoint) -> Hit? {
        guard let (ref, view) = TerminalRegistry.shared.terminal(in: window, at: locationInWindow) else { return nil }
        let point = view.convert(locationInWindow, from: nil)
        let cell = cellSize(for: view)
        let col = Int(point.x / cell.width)
        // The view is not flipped: y grows upward, row 0 is the top line.
        let fromTop = view.bounds.height - point.y
        let row = Int(fromTop / cell.height)
        let screen = EngineStore.shared.engine.screenText(project: ref.project, address: ref.address)
        let lines = screen.components(separatedBy: "\n")
        guard row >= 0, row < lines.count else { return nil }
        let line = lines[row]
        let ns = line as NSString
        let all = NSRange(location: 0, length: ns.length)
        func hit(range: NSRange, url: String) -> Hit {
            // Column → UTF-16 offset is 1:1 for the ASCII/Latin text these links use.
            let x = CGFloat(range.location) * cell.width
            let width = CGFloat(range.length) * cell.width
            let y = view.bounds.height - CGFloat(row + 1) * cell.height
            return Hit(view: view, url: url, rect: NSRect(x: x, y: y, width: width, height: cell.height))
        }
        // Markdown first: its URL would otherwise also match as a plain URL.
        for m in markdown.matches(in: line, range: all) where NSLocationInRange(col, m.range) {
            return hit(range: m.range, url: ns.substring(with: m.range(at: 2)))
        }
        for m in plainURL.matches(in: line, range: all) {
            var range = m.range
            // Trailing punctuation belongs to the sentence, not the URL.
            while range.length > 0, ".,;:!?".contains(Character(ns.substring(with: NSRange(location: range.location + range.length - 1, length: 1)))) {
                range.length -= 1
            }
            if NSLocationInRange(col, range) { return hit(range: range, url: ns.substring(with: range)) }
        }
        return nil
    }

    private static func updateHover(_ event: NSEvent) {
        // Synthetic or fallback mouse-moved events can carry stale locations; the window's
        // own pointer position is authoritative.
        let window = event.window ?? NSApp.keyWindow
        let location = window?.mouseLocationOutsideOfEventStream ?? event.locationInWindow
        let hit = event.modifierFlags.contains(.command) && event.type != .mouseExited && window != nil
            ? link(in: window!, at: location) : nil
        if let hit, let window = hit.view.window {
            // The underline lives in a transparent child window: nothing in the view
            // hierarchy (SwiftTerm's Metal layer included) can draw over it.
            let colour = (hit.view as? TerminalView)?.nativeForegroundColor ?? NSColor.linkColor
            let inWindow = hit.view.convert(hit.rect, to: nil)
            let line = NSRect(x: inWindow.minX, y: inWindow.minY + 1, width: inWindow.width, height: 1)
            let onScreen = window.convertToScreen(line)
            let overlay = underlineWindow ?? LinkUnderlineWindow()
            underlineWindow = overlay
            overlay.backgroundColor = colour
            overlay.setFrame(onScreen, display: true)
            if overlay.parent !== window {
                overlay.parent?.removeChildWindow(overlay)
                window.addChildWindow(overlay, ordered: .above)
            }
            overlay.orderFront(nil)
            if hoverTrace { log("hover hit \(hit.url) screen=\(onScreen)") }
            NSCursor.pointingHand.set()
        } else if let overlay = underlineWindow, overlay.isVisible {
            overlay.parent?.removeChildWindow(overlay)
            overlay.orderOut(nil)
            NSCursor.arrow.set()
        }
    }

    private static var underlineWindow: LinkUnderlineWindow?
    static var hoverTrace = false

    /// Opens a URL string, defaulting to https for bare `www.` hosts. Logged for tests.
    static func open(_ link: String) {
        var text = link
        if text.hasPrefix("www.") { text = "https://" + text }
        guard let url = URL(string: text) else { return }
        log("open \(url.absoluteString)")
        NSWorkspace.shared.open(url)
    }

    /// Same cell geometry SwiftTerm uses (advance of "W", ascent + descent + leading, pixel-snapped).
    private static func cellSize(for view: NSView) -> CGSize {
        let font = SBType.terminal
        let ct = font as CTFont
        var chars: [UniChar] = [0x57]
        var glyphs: [CGGlyph] = [0]
        let glyph: NSGlyph = CTFontGetGlyphsForCharacters(ct, &chars, &glyphs, 1) ? NSGlyph(glyphs[0]) : font.glyph(withName: "W")
        let width = font.advancement(forGlyph: glyph).width
        let height = ceil(CTFontGetAscent(ct) + CTFontGetDescent(ct) + CTFontGetLeading(ct))
        let scale = view.window?.backingScaleFactor ?? 2
        return CGSize(width: max(1, (width * scale).rounded() / scale), height: max(1, ceil(height * scale) / scale))
    }

    static func log(_ line: String) {
        let dir = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Logs/Switchboard")
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let file = dir.appendingPathComponent("links.log")
        let entry = "\(ISO8601DateFormatter().string(from: Date())) \(line)\n"
        if let h = try? FileHandle(forWritingTo: file) {
            h.seekToEndOfFile(); h.write(Data(entry.utf8)); try? h.close()
        } else {
            try? entry.write(to: file, atomically: true, encoding: .utf8)
        }
    }
}

/// A borderless, click-through 1 pt window used as the hover underline.
final class LinkUnderlineWindow: NSWindow {
    init() {
        super.init(contentRect: NSRect(x: 0, y: 0, width: 10, height: 1), styleMask: .borderless, backing: .buffered, defer: false)
        isOpaque = true
        hasShadow = false
        ignoresMouseEvents = true
        isReleasedWhenClosed = false
        level = .floating
        collectionBehavior = [.transient, .ignoresCycle]
    }
}
