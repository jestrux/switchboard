import SwiftUI

// Every project's board in one place (plans/06-board.md, B5). The Boards window is for working
// through what needs you (decided on the board, w-modes): the projects that need you on the left,
// blocking first, and the chosen project's board on the right in review mode; at the end of its
// cards it offers the next project. A board can also be torn off into a window of its own.

/// Window › Boards (⌘⇧B), the menu bar's Review All, and a project's tile.
struct BoardsView: View {
    @EnvironmentObject var store: EngineStore
    @ObservedObject private var boards = BoardCache.shared
    @AppStorage(ThemeSettings.themeKey) private var themeID: SBThemeID = .graphite
    @AppStorage(ThemeSettings.appearanceKey) private var appearance: SBAppearance = .system
    @State private var selected: String?

    var body: some View {
        BoardsContent(selected: $selected)
            .background(WindowMaximizer())
            .environment(\.sbTheme, themeID.theme)
            .preferredColorScheme(appearance.colorScheme)
            .onAppear {
                boards.watchAll({ store.projects.map(\.slug) }, engine: store.engine)
                if let wanted = boards.showBoards?.slug { selected = wanted }
            }
            .onChange(of: boards.showBoards?.at) { _, _ in if let wanted = boards.showBoards?.slug { selected = wanted } }
    }
}

private struct BoardsContent: View {
    @EnvironmentObject var store: EngineStore
    @ObservedObject private var boards = BoardCache.shared
    @Environment(\.sbPalette) private var p
    @Binding var selected: String?

    /// Projects with a board, those that need you first: blocking, then by how many.
    private var rows: [(slug: String, data: BoardData)] {
        store.projects.compactMap { proj in boards.data(proj.slug).map { (slug: proj.slug, data: $0) } }
            .sorted { a, b in
                if (a.data.blocking > 0) != (b.data.blocking > 0) { return a.data.blocking > 0 }
                if (a.data.needsYou > 0) != (b.data.needsYou > 0) { return a.data.needsYou > 0 }
                return a.data.needsYou > b.data.needsYou
            }
    }

    var body: some View {
        let rows = self.rows
        let waiting = rows.filter { $0.data.needsYou > 0 }
        let current = selected.flatMap { s in rows.first { $0.slug == s } } ?? waiting.first ?? rows.first
        HStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 2) {
                Text(waiting.isEmpty ? "Nothing needs you" : "Needs you · \(waiting.reduce(0) { $0 + $1.data.needsYou })")
                    .font(.system(size: 13, weight: .semibold)).foregroundStyle(p.text)
                    .padding(.horizontal, 10).padding(.top, 38).padding(.bottom, 8)
                ScrollView {
                    VStack(spacing: 2) {
                        ForEach(rows, id: \.slug) { row in
                            BoardsRow(slug: row.slug, data: row.data, selected: row.slug == current?.slug)
                                .onTapGesture { selected = row.slug }
                        }
                    }
                }
            }
            .padding(.horizontal, 8)
            .frame(width: 250)
            .background(p.sidebar)
            Rectangle().fill(p.line).frame(width: 1)
            ZStack {
                p.pane
                if let current {
                    let next = waiting.first { $0.slug != current.slug }
                    BoardPageView(project: current.slug, json: current.data.json, engine: store.engine, review: true,
                                  next: next.map { store.project($0.slug)?.name ?? $0.slug },
                                  onNext: { if let next { selected = next.slug } })
                        .id(current.slug)  // a fresh page per project, so it opens in review mode
                } else {
                    Text("No project has a board yet.").foregroundStyle(p.text3)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .ignoresSafeArea(edges: .top)
        .navigationTitle(current.map { "Boards · " + (store.project($0.slug)?.name ?? $0.slug) } ?? "Boards")
    }
}

private struct BoardsRow: View {
    @EnvironmentObject var store: EngineStore
    @Environment(\.sbPalette) private var p
    let slug: String
    let data: BoardData
    let selected: Bool

    var body: some View {
        let proj = store.project(slug)
        HStack(alignment: .top, spacing: 8) {
            if let proj { ProjectIcon(project: proj, size: 22, radius: 5) }
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text(proj?.name ?? slug).font(.system(size: 13, weight: selected ? .semibold : .medium))
                        .foregroundStyle(selected ? p.selectionText : p.text).lineLimit(1)
                    Spacer(minLength: 4)
                    if data.blocking > 0 {
                        Text("Blocking").font(.system(size: 10, weight: .semibold)).foregroundStyle(p.crashed)
                            .padding(.horizontal, 5).frame(height: 15).background(p.crashed.opacity(0.12), in: Capsule())
                    }
                    if data.needsYou > 0 {
                        Text("\(data.needsYou)").font(.system(size: 10, weight: .semibold)).monospacedDigit()
                            .foregroundStyle(p.onAccent).padding(.horizontal, 5).frame(minWidth: 16, minHeight: 16)
                            .background(p.accent, in: Capsule())
                    }
                }
                Text(data.working.isEmpty ? (data.needsYou > 0 ? "Waiting on you" : "Nothing needs you") : data.working)
                    .font(.system(size: 11)).foregroundStyle(selected ? p.selectionText.opacity(0.8) : p.text3).lineLimit(2)
            }
        }
        .padding(.horizontal, 8).padding(.vertical, 7)
        .background(selected ? p.selection : .clear, in: RoundedRectangle(cornerRadius: 6, style: .continuous))
        .contentShape(Rectangle())
        .opacity(data.needsYou > 0 || selected ? 1 : 0.7)
    }
}

/// A project's board torn off into a window of its own: the whole board, as in its pane.
struct BoardWindow: View {
    @EnvironmentObject var store: EngineStore
    @ObservedObject private var boards = BoardCache.shared
    @AppStorage(ThemeSettings.themeKey) private var themeID: SBThemeID = .graphite
    @AppStorage(ThemeSettings.appearanceKey) private var appearance: SBAppearance = .system
    let slug: String

    var body: some View {
        BoardPageView(project: slug, json: boards.data(slug)?.json ?? "", engine: store.engine)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(WindowMaximizer())
            .environment(\.sbTheme, themeID.theme)
            .preferredColorScheme(appearance.colorScheme)
            .navigationTitle("Board · " + (store.project(slug)?.name ?? slug))
            .onAppear { boards.watch(slug, engine: store.engine) }
            .onDisappear { boards.unwatch(slug) }
    }
}
