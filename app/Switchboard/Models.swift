import Foundation
import SwiftUI

/// Identifies a command across the app: project slug plus address.
struct CommandRef: Hashable, Codable {
    let project: String
    let address: String
}

extension CommandStatus {
    var label: String {
        switch self {
        case .stopped: return "Stopped"
        case .starting: return "Starting"
        case .running: return "Running"
        case .exited: return "Done"
        case .crashed: return "Crashed"
        case .error: return "Error"
        }
    }

    var isAlive: Bool { self == .starting || self == .running }
}

extension CommandState {
    var isAlive: Bool { status.isAlive }
    static let idle = CommandState(status: .stopped, pid: nil, exitCode: nil, signal: nil, startedAt: nil, endedAt: nil, message: nil, needsRestart: false, restartAt: nil)

    /// Short text for footers and tooltips.
    var summary: String {
        switch status {
        case .stopped:
            if let exitCode, pid != nil { return "Stopped · exit \(exitCode)" }
            return "Stopped"
        case .starting: return "Starting…"
        case .running: return pid.map { "Running · pid \($0)" } ?? "Running"
        case .exited: return "Done · exit 0"
        case .crashed:
            var s = signal.map { "Killed by \($0)" } ?? exitCode.map { "Exited with code \($0)" } ?? "Crashed"
            if let message { s += " · \(message)" }
            return s
        case .error: return message ?? "Could not start"
        }
    }
}

extension ProjectInfo {
    var allCommands: [CommandInfo] { commands + groups.flatMap(\.commands) }
    var isValid: Bool { error == nil }
}

/// A folder a project's commands run in, as a candidate home for its repo settings.
struct WorkingFolder: Identifiable, Hashable {
    /// Absolute path.
    let path: String
    /// Names of the groups (or commands) that use it, e.g. "Frontend, Tunnels".
    let label: String
    var id: String { path }
}

extension ProjectInfo {
    /// Distinct folders this project's groups and commands run in that exist on this Mac,
    /// in sidebar order. Two groups sharing a folder appear once with both names.
    var workingFolders: [WorkingFolder] {
        var order: [String] = []
        var names: [String: [String]] = [:]
        func add(_ raw: String?, _ name: String) {
            guard let raw else { return }
            let path = ((raw as NSString).expandingTildeInPath as NSString).standardizingPath
            var isDir: ObjCBool = false
            guard FileManager.default.fileExists(atPath: path, isDirectory: &isDir), isDir.boolValue else { return }
            if names[path] == nil { order.append(path); names[path] = [] }
            if !names[path]!.contains(name) { names[path]!.append(name) }
        }
        for c in commands where !c.isTerminal { add(c.cwd, c.name) }
        for g in groups {
            add(g.cwd, g.name)
            // Commands that override the group's folder.
            for c in g.commands where !c.isTerminal && c.cwd != g.cwd { add(c.cwd, "\(g.name) / \(c.name)") }
        }
        return order.map { WorkingFolder(path: $0, label: names[$0]!.joined(separator: ", ")) }
    }
}

extension CommandInfo {
    /// `~`-relative folder for display, or a hint when none is set.
    var folderLabel: String { cwd ?? "~ (no folder set)" }
}

extension String {
    /// Collapse the home folder to `~`.
    var abbreviatingHome: String {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        if hasPrefix(home) { return "~" + dropFirst(home.count) }
        return self
    }
}

extension Color {
    /// Parse `#RRGGBB` / `#RRGGBBAA`; nil when malformed.
    init?(hex: String) {
        var s = hex.trimmingCharacters(in: .whitespacesAndNewlines)
        guard s.hasPrefix("#") else { return nil }
        s.removeFirst()
        guard s.count == 6 || s.count == 8, let v = UInt64(s, radix: 16) else { return nil }
        let r, g, b, a: Double
        if s.count == 8 {
            r = Double((v >> 24) & 0xff) / 255; g = Double((v >> 16) & 0xff) / 255
            b = Double((v >> 8) & 0xff) / 255; a = Double(v & 0xff) / 255
        } else {
            r = Double((v >> 16) & 0xff) / 255; g = Double((v >> 8) & 0xff) / 255
            b = Double(v & 0xff) / 255; a = 1
        }
        self = Color(.sRGB, red: r, green: g, blue: b, opacity: a)
    }

    /// Stable fallback color derived from a name, for projects with no `color`.
    static func fallback(for name: String) -> Color {
        let palette: [Color] = [.purple, .blue, .teal, .green, .orange, .pink, .indigo, .mint]
        let h = name.unicodeScalars.reduce(0) { ($0 &* 31) &+ Int($1.value) }
        return palette[abs(h) % palette.count]
    }
}

extension Error {
    /// The engine's errors carry a plain message; show that instead of the enum dump.
    var readableMessage: String {
        if let e = self as? SbError {
            switch e { case .Message(let m): return m }
        }
        return localizedDescription
    }
}
