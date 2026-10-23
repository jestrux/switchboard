import AppKit
import SwiftUI

/// How the author arranges their desk: which company a project is under. The rail shows one icon
/// per company; the sidebar picks the product (tabs, or a title menu). It's personal, so it lives
/// in the app's settings, never in a repo. A company is only a label: nothing runs, starts or stops
/// at company level.
@MainActor
final class Desk: ObservableObject {
    static let shared = Desk()
    /// Settings › Sidebar: how a company's products are picked. Tabs fall back to the menu past three.
    static let switcherKey = "deskSwitcher"
    @Published private(set) var companies: [String: String]
    /// The product last used in each company, which its rail tile opens.
    @Published private(set) var last: [String: String]
    private let defaults = UserDefaults.standard

    init() {
        companies = defaults.dictionary(forKey: "deskCompanies") as? [String: String] ?? [:]
        last = defaults.dictionary(forKey: "deskLastProduct") as? [String: String] ?? [:]
    }

    func company(_ slug: String) -> String? { companies[slug] }
    var allCompanies: [String] { Set(companies.values).sorted { $0.localizedCaseInsensitiveCompare($1) == .orderedAscending } }

    func setCompany(_ name: String?, for slug: String) {
        let name = name?.trimmingCharacters(in: .whitespaces)
        companies[slug] = (name?.isEmpty ?? true) ? nil : name
        defaults.set(companies, forKey: "deskCompanies")
    }

    /// The other products of a project's company, itself included, in rail order; empty when
    /// it's alone (then everything looks as it did before companies).
    func products(of project: ProjectInfo, in projects: [ProjectInfo]) -> [ProjectInfo] {
        guard let c = companies[project.slug] else { return [] }
        let all = projects.filter { companies[$0.slug] == c }
        return all.count > 1 ? all : []
    }

    func remember(_ slug: String) {
        guard let c = companies[slug], last[c] != slug else { return }
        last[c] = slug
        defaults.set(last, forKey: "deskLastProduct")
    }

    /// The product a company's tile opens: the last one used, else its first.
    func current(_ company: String, members: [ProjectInfo]) -> ProjectInfo? {
        members.first { $0.slug == last[company] } ?? members.first
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

    /// The rail in the author's order: one entry per company (where its first product is), and
    /// every project that isn't under one. A company with one product is just that project.
    func entries(_ projects: [ProjectInfo]) -> [RailEntry] {
        var out: [RailEntry] = [], seen = Set<String>()
        for p in projects {
            guard let c = companies[p.slug] else { out.append(.project(p)); continue }
            let members = projects.filter { companies[$0.slug] == c }
            if members.count == 1 { out.append(.project(p)); continue }
            if seen.insert(c).inserted { out.append(.company(c, members)) }
        }
        return out
    }

    /// A one-field prompt (New Company…).
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

/// A project's company, in its context menu.
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
                if let name = Desk.ask("Which company is \(project.name) under?", message: "Its products share one icon on the rail, and you pick between them in the sidebar. Nothing runs at company level.", placeholder: "SetHero") {
                    desk.setCompany(name, for: project.slug)
                }
            }
            if current != nil { Button("None") { desk.setCompany(nil, for: project.slug) } }
        }
    }
}

/// Picking a company's product in the sidebar header: tabs (the default, up to three products) or
/// the title as a menu (Settings, or past three). In All Projects it switches the sidebar; in a
/// project's own window it opens or focuses that product's window.
struct ProductPicker: View {
    @EnvironmentObject var store: EngineStore
    @ObservedObject private var boards = BoardCache.shared
    @ObservedObject private var desk = Desk.shared
    @AppStorage(Desk.switcherKey) private var style = "tabs"
    @Environment(\.sbTheme) private var theme
    @Environment(\.colorScheme) private var scheme
    let company: String
    let products: [ProjectInfo]
    let current: ProjectInfo
    let pick: (ProjectInfo) -> Void

    static func usesTabs(_ count: Int, style: String) -> Bool { style != "menu" && count <= 3 }

    /// The tabs row, under the company's name.
    var tabs: some View {
        let p = theme.palette(scheme)
        return HStack(spacing: 2) {
            ForEach(products, id: \.slug) { prod in
                let on = prod.slug == current.slug
                Button { pick(prod) } label: {
                    HStack(spacing: 5) {
                        Text(prod.name).font(.system(size: 12, weight: .semibold)).lineLimit(1)
                        needs(prod, p)
                    }
                    .foregroundStyle(on ? p.text : p.text2)
                    .frame(maxWidth: .infinity).frame(height: 24)
                    .background(on ? p.pane : .clear, in: RoundedRectangle(cornerRadius: 6, style: .continuous))
                    .shadow(color: on ? .black.opacity(0.12) : .clear, radius: 1, y: 1)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .help(desk.displayName(prod))
            }
        }
        .padding(2)
        .background(p.hover, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
    }

    /// The product's name as a menu, with the company small above it.
    func menuTitle<T: View>(_ title: T) -> some View {
        let p = theme.palette(scheme)
        return VStack(alignment: .leading, spacing: 1) {
            Text(company).font(.system(size: 11, weight: .semibold)).foregroundStyle(p.text3)
            Menu {
                ForEach(products, id: \.slug) { prod in
                    let n = boards.data(prod.slug)?.needsYou ?? 0
                    Button { pick(prod) } label: {
                        let name = prod.name + (n > 0 ? "  ·  \(n) need you" : "")
                        if prod.slug == current.slug { Label(name, systemImage: "checkmark") } else { Text(name) }
                    }
                }
            } label: {
                HStack(spacing: 4) {
                    title.truncationMode(.tail)
                    Image(systemName: "chevron.down").font(.system(size: 9, weight: .bold)).foregroundStyle(p.text3)
                }
                .contentShape(Rectangle())
            }
            // .button + .plain keeps the label as drawn (title, then chevron); it may shrink.
            .menuStyle(.button).buttonStyle(.plain).menuIndicator(.hidden)
            .layoutPriority(-1)
            .help("Switch product in \(company)")
        }
    }

    @ViewBuilder
    private func needs(_ prod: ProjectInfo, _ p: SBPalette) -> some View {
        let n = boards.data(prod.slug)?.needsYou ?? 0
        if n > 0 {
            Text("\(n)").font(.system(size: 9.5, weight: .bold)).monospacedDigit()
                .foregroundStyle(p.onAccent).padding(.horizontal, 4).frame(minWidth: 14, minHeight: 14)
                .background(p.accent, in: Capsule())
        }
    }

    var body: some View { tabs }
}
