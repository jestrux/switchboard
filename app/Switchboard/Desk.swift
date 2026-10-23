import AppKit
import SwiftUI

/// How the author arranges their desk: which company a project is under, a short product mark
/// for products that share a logo, and which companies are folded in the rail. It's personal, so
/// it lives in the app's settings, never in a repo. A company is only a label: nothing runs, starts
/// or stops at company level.
@MainActor
final class Desk: ObservableObject {
    static let shared = Desk()
    @Published private(set) var companies: [String: String]
    @Published private(set) var marks: [String: String]
    @Published private(set) var collapsed: Set<String>
    private let defaults = UserDefaults.standard

    init() {
        companies = defaults.dictionary(forKey: "deskCompanies") as? [String: String] ?? [:]
        marks = defaults.dictionary(forKey: "deskMarks") as? [String: String] ?? [:]
        collapsed = Set(defaults.stringArray(forKey: "deskCollapsed") ?? [])
    }

    func company(_ slug: String) -> String? { companies[slug] }
    var allCompanies: [String] { Set(companies.values).sorted { $0.localizedCaseInsensitiveCompare($1) == .orderedAscending } }

    func setCompany(_ name: String?, for slug: String) {
        let name = name?.trimmingCharacters(in: .whitespaces)
        companies[slug] = (name?.isEmpty ?? true) ? nil : name
        defaults.set(companies, forKey: "deskCompanies")
    }

    /// The tile's mark: the one set, else the name's initials, and only for a project that shares
    /// its company with another (where the logos are likely the same). "" turns it off.
    func mark(_ project: ProjectInfo, among projects: [ProjectInfo]) -> String? {
        if let m = marks[project.slug] { return m.isEmpty ? nil : m }
        guard let c = companies[project.slug], projects.filter({ companies[$0.slug] == c }).count > 1 else { return nil }
        return Self.initials(project.name)
    }

    func setMark(_ mark: String?, for slug: String) {
        marks[slug] = mark.map { String($0.trimmingCharacters(in: .whitespaces).prefix(3)).uppercased() }
        defaults.set(marks, forKey: "deskMarks")
    }

    func toggle(_ company: String) {
        if collapsed.contains(company) { collapsed.remove(company) } else { collapsed.insert(company) }
        defaults.set(Array(collapsed).sorted(), forKey: "deskCollapsed")
    }

    /// "SetHero · Horizon" where a project is under a company, else its name.
    func displayName(_ project: ProjectInfo) -> String {
        companies[project.slug].map { "\($0) · \(project.name)" } ?? project.name
    }
    func displayName(slug: String, store: EngineStore) -> String {
        store.project(slug).map(displayName) ?? slug
    }

    enum RailEntry: Identifiable {
        case project(ProjectInfo)
        case company(String, [ProjectInfo])
        var id: String {
            switch self {
            case .project(let p): return "p:" + p.slug
            case .company(let c, _): return "c:" + c
            }
        }
    }

    /// The rail in the author's order, each company's projects gathered where its first one is.
    func entries(_ projects: [ProjectInfo]) -> [RailEntry] {
        var out: [RailEntry] = [], seen = Set<String>()
        for p in projects {
            guard let c = companies[p.slug] else { out.append(.project(p)); continue }
            if seen.insert(c).inserted { out.append(.company(c, projects.filter { companies[$0.slug] == c })) }
        }
        return out
    }

    static func initials(_ name: String) -> String {
        let words = name.split(whereSeparator: { !$0.isLetter && !$0.isNumber })
        if words.count > 1 { return words.prefix(2).compactMap(\.first).map(String.init).joined().uppercased() }
        return String(name.prefix(2)).uppercased()
    }

    /// A one-field prompt (New Company…, Product Mark…).
    static func ask(_ title: String, message: String, value: String = "", placeholder: String = "") -> String? {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = message
        let field = NSTextField(frame: NSRect(x: 0, y: 0, width: 240, height: 24))
        field.stringValue = value
        field.placeholderString = placeholder
        alert.accessoryView = field
        alert.addButton(withTitle: "OK")
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = field
        return alert.runModal() == .alertFirstButtonReturn ? field.stringValue : nil
    }
}

/// A project's place on the desk, in its context menu: its company, and its mark.
struct DeskMenu: View {
    @ObservedObject private var desk = Desk.shared
    let project: ProjectInfo

    var body: some View {
        let current = desk.company(project.slug)
        Menu("Company") {
            ForEach(desk.allCompanies, id: \.self) { c in
                Button { desk.setCompany(c, for: project.slug) } label: {
                    if c == current { Label(c, systemImage: "checkmark") } else { Text(c) }
                }
            }
            if !desk.allCompanies.isEmpty { Divider() }
            Button("New Company…") {
                if let name = Desk.ask("Which company is \(project.name) under?", message: "A label that gathers its products in the rail. Nothing runs at company level.", placeholder: "SetHero") {
                    desk.setCompany(name, for: project.slug)
                }
            }
            if current != nil { Button("None") { desk.setCompany(nil, for: project.slug) } }
        }
        if current != nil {
            Button("Product Mark…") {
                let now = desk.marks[project.slug] ?? Desk.initials(project.name)
                if let m = Desk.ask("A short mark for \(project.name)", message: "Shown on its tile, so products with the same logo read apart. Up to three characters; empty for none.", value: now) {
                    desk.setMark(m, for: project.slug)
                }
            }
        }
    }
}
