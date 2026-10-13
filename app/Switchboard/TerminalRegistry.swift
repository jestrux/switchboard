import AppKit
import Foundation
import SwiftTerm
import SwiftUI

/// One SwiftTerm view per command, kept alive for the life of the app so switching
/// panes reattaches with scrollback and alternate screen intact (like tmux windows).
///
/// Output arrives on engine threads. Bytes for a command whose view does not exist yet
/// are buffered and replayed when the view is created, so nothing is lost or reordered.
final class TerminalRegistry {
    static let shared = TerminalRegistry()

    private let lock = NSLock()
    private var hosts: [CommandRef: TerminalHost] = [:]
    /// Views by ref, for feeding bytes from engine threads (feed is thread-safe).
    private var feeders: [CommandRef: TerminalView] = [:]
    private var pending: [CommandRef: Data] = [:]
    private var started: Set<CommandRef> = []
    private let pendingCap = 4 * 1024 * 1024

    /// Thread-safe. Feeds the view if it exists, otherwise buffers.
    func feed(_ ref: CommandRef, _ bytes: Data) {
        lock.lock()
        if let view = feeders[ref] {
            lock.unlock()
            view.feed(byteArray: ArraySlice([UInt8](bytes)))
            return
        }
        var buf = pending[ref] ?? Data()
        buf.append(bytes)
        if buf.count > pendingCap {
            // Keep the newest bytes; cut at a newline so replay starts on a fresh line.
            let drop = buf.count - pendingCap * 3 / 4
            let cut = buf[drop...].firstIndex(of: 0x0a).map { $0 + 1 } ?? drop
            buf.removeSubrange(..<cut)
        }
        pending[ref] = buf
        lock.unlock()
    }

    /// Called when a session (re)starts: print a separator so consecutive runs are easy
    /// to tell apart.
    func noteStart(_ ref: CommandRef) {
        let time = DateFormatter.localizedString(from: Date(), dateStyle: .none, timeStyle: .medium)
        // Mode resets (alt screen, mouse, paste) are emitted by the engine on exit, only
        // when the program left them on; here we just normalise attributes.
        let reset = "\u{1b}[0m"
        let hasHistory: Bool = {
            lock.lock(); defer { lock.unlock() }
            return !started.insert(ref).inserted
        }()
        let separator = hasHistory ? "\r\n\u{1b}[2m── started \(time) ──\u{1b}[0m\r\n" : ""
        feed(ref, Data((reset + separator).utf8))
    }

    /// Main thread only. Creates the view on first use and replays buffered output.
    @MainActor
    func host(for ref: CommandRef) -> TerminalHost {
        lock.lock()
        if let h = hosts[ref] {
            lock.unlock()
            return h
        }
        let host = TerminalHost(ref: ref)
        hosts[ref] = host
        feeders[ref] = host.terminal
        let buffered = pending.removeValue(forKey: ref)
        lock.unlock()
        if let buffered, !buffered.isEmpty {
            host.terminal.feed(byteArray: ArraySlice([UInt8](buffered)))
        }
        return host
    }

    /// Has this command been started at least once this session?
    func hasStarted(_ ref: CommandRef) -> Bool {
        lock.lock(); defer { lock.unlock() }
        return started.contains(ref)
    }

    @MainActor
    func existingHost(for ref: CommandRef) -> TerminalHost? {
        lock.lock(); defer { lock.unlock() }
        return hosts[ref]
    }

}

/// Owns one `TerminalView` and wires its delegate to the engine.
@MainActor
final class TerminalHost: NSObject, TerminalViewDelegate {
    let ref: CommandRef
    let terminal: TerminalView
    private var lastSize: (Int, Int) = (0, 0)

    init(ref: CommandRef) {
        self.ref = ref
        var options = TerminalOptions.default
        options.scrollback = 10_000
        options.termName = "xterm-256color"
        terminal = TerminalView(frame: NSRect(x: 0, y: 0, width: 800, height: 500), font: nil, options: options)
        super.init()
        terminal.font = SBType.terminal
        terminal.terminalDelegate = self
        terminal.allowMouseReporting = true
        terminal.optionAsMetaKey = true
        terminal.translatesAutoresizingMaskIntoConstraints = false
    }

    private var appliedLook: (SBThemeID, ColorScheme)?

    /// Apply the window's theme palette to the terminal (background, text, cursor, ANSI).
    func applyTheme(_ look: Look) {
        let key = (look.theme.id, look.scheme)
        if let a = appliedLook, a == key { return }
        appliedLook = key
        let p = look.p
        terminal.nativeBackgroundColor = NSColor(p.terminal)
        terminal.nativeForegroundColor = NSColor(p.terminalText)
        terminal.caretColor = NSColor(p.accent)
        terminal.installColors(p.ansi.map { c in
            let rgb = c.usingColorSpace(.sRGB) ?? c
            return SwiftTerm.Color(red8: UInt16(rgb.redComponent * 255), green8: UInt16(rgb.greenComponent * 255), blue8: UInt16(rgb.blueComponent * 255))
        })
    }

    /// Clear screen and scrollback (Cmd+K).
    func clear() {
        terminal.feed(text: "\u{1b}[H\u{1b}[2J")
        terminal.clearScrollback()
    }

    // MARK: TerminalViewDelegate

    func send(source: TerminalView, data: ArraySlice<UInt8>) {
        let bytes = Data(data)
        try? EngineStore.shared.engine.writeInput(project: ref.project, address: ref.address, bytes: bytes)
    }

    func sizeChanged(source: TerminalView, newCols: Int, newRows: Int) {
        guard newCols > 0, newRows > 0, (newCols, newRows) != lastSize else { return }
        lastSize = (newCols, newRows)
        let engine = EngineStore.shared.engine
        engine.resize(project: ref.project, address: ref.address, cols: UInt16(newCols), rows: UInt16(newRows))
        // New sessions should match the pane they will most likely be shown in.
        engine.setDefaultSize(cols: UInt16(newCols), rows: UInt16(newRows))
    }

    func setTerminalTitle(source: TerminalView, title: String) {
        EngineStore.shared.terminalTitles[ref] = title
    }

    func hostCurrentDirectoryUpdate(source: TerminalView, directory: String?) {}
    func scrolled(source: TerminalView, position: Double) {}
    func bell(source: TerminalView) { NSSound.beep() }

    func requestOpenLink(source: TerminalView, link: String, params: [String: String]) {
        if let url = URL(string: link) { NSWorkspace.shared.open(url) }
    }

    func clipboardCopy(source: TerminalView, content: Data) {
        if let s = String(data: content, encoding: .utf8) {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(s, forType: .string)
        }
    }

    func clipboardRead(source: TerminalView) -> Data? {
        NSPasteboard.general.string(forType: .string).map { Data($0.utf8) }
    }

    // Kitty clipboard protocol (OSC 5522): not offered to programs in v1.
    func kittyClipboardCapabilities(source: TerminalView) -> KittyClipboardCapabilities { [] }
    func kittyClipboardAvailableMimeTypes(source: TerminalView, location: KittyClipboardLocation) -> [String]? { nil }
    func kittyClipboardRead(source: TerminalView, location: KittyClipboardLocation, mimeType: String) -> KittyClipboardReadResult? { nil }
    func kittyClipboardWrite(source: TerminalView, location: KittyClipboardLocation, content: KittyClipboardWriteContent) -> KittyClipboardWriteResult { .unsupported }
    func kittyClipboardRequestPermission(source: TerminalView, request: KittyClipboardPermissionRequest) -> KittyClipboardPermissionResult { .deny }
    func iTermContent(source: TerminalView, content: ArraySlice<UInt8>) {}
    func rangeChanged(source: TerminalView, startY: Int, endY: Int) {}

}
