import SwiftUI

// The menu bar (plans/06-board.md, B3): what needs you across every project, and a popover to
// answer the next card without opening a window. Template icon from the design handoff.

struct MenuBarLabel: View {
    @ObservedObject private var boards = BoardCache.shared
    var body: some View {
        let n = boards.totalNeedsYou
        HStack(spacing: 3) {
            Image("MenuBarIcon")
            if n > 0 { Text("\(n)").monospacedDigit() }
        }
        .help(n > 0 ? "\(n) need you" : "Switchboard")
    }
}

struct MenuBarBoard: View {
    @EnvironmentObject var store: EngineStore
    @ObservedObject private var boards = BoardCache.shared
    @Environment(\.openWindow) private var openWindow
    @Environment(\.sbPalette) private var p
    @State private var skipped: Set<String> = []
    @State private var picks: [String] = []
    @State private var note = ""

    var body: some View {
        let all = boards.needsYou()
        let queue = all.filter { !skipped.contains($0.slug + "/" + $0.card.id) }
        let next = queue.first ?? all.first
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text(all.isEmpty ? "Nothing needs you" : "Needs you · \(all.count)").font(.system(size: 13, weight: .semibold))
                Spacer()
                if all.count > 0 {
                    let projects = Set(all.map(\.slug)).count
                    Text(projects == 1 ? "1 project" : "\(projects) projects").font(.system(size: 11)).foregroundStyle(p.text3)
                }
            }
            if let next {
                let proj = store.project(next.slug)
                HStack(spacing: 6) {
                    if let proj { ProjectIcon(project: proj, size: 16, radius: 4) }
                    Text(proj?.name ?? next.slug).font(.system(size: 11, weight: .medium)).foregroundStyle(p.text2)
                    if next.card.blocking {
                        Text("Blocking").font(.system(size: 10, weight: .semibold)).foregroundStyle(p.crashed)
                            .padding(.horizontal, 5).frame(height: 15).background(p.crashed.opacity(0.12), in: Capsule())
                    }
                }
                Text(next.card.title).font(.system(size: 14, weight: .semibold)).foregroundStyle(p.text).fixedSize(horizontal: false, vertical: true)
                if !next.card.summary.isEmpty {
                    Text(next.card.summary).font(.system(size: 12)).foregroundStyle(p.text2).lineLimit(4)
                }
                if let rec = next.card.options.first(where: \.rec) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text((next.card.unsure ? "I'm not sure; leaning “" : "I'd go with “") + rec.label + "”" + (next.card.basis.isEmpty ? "" : " · " + (next.card.basis == "evidence" ? "Evidence" : "Taste")))
                            .font(.system(size: 12, weight: .semibold)).foregroundStyle(p.text)
                        if !next.card.why.isEmpty { Text(next.card.why).font(.system(size: 11)).foregroundStyle(p.text2).lineLimit(3) }
                    }
                    .padding(8).frame(maxWidth: .infinity, alignment: .leading)
                    .background(p.accentSoft.opacity(0.6), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                }
                if let q = next.card.question { Text(q).font(.system(size: 12, weight: .semibold)).foregroundStyle(p.text) }
                FlowOptions(card: next.card, picks: $picks)
                if let then = next.card.options.first(where: { picks.contains($0.o) && !$0.then.isEmpty })?.then {
                    Text("Then I'll " + then.prefix(1).lowercased() + then.dropFirst()).font(.system(size: 11)).foregroundStyle(p.text2)
                }
                TextField("A note for the agent (optional)", text: $note, axis: .vertical).textFieldStyle(.roundedBorder).lineLimit(1...3)
                HStack {
                    Button("Open in Switchboard") { open(next.slug) }.buttonStyle(.link)
                    Button("Review All") { review(next.slug) }.buttonStyle(.link)
                    Spacer()
                    if queue.count > 1 {
                        Button("Skip") { skipped.insert(next.slug + "/" + next.card.id); picks = []; note = "" }
                    }
                    Button("Send") {
                        if BoardSend.send(store: store, project: next.slug, card: next.card, picks: picks, note: note,
                                          projectDir: next.data.projectDir, projectName: next.data.projectName) {
                            picks = []; note = ""; boards.refresh(next.slug)
                        }
                    }
                    .keyboardShortcut(.return, modifiers: .command)
                    .disabled(picks.isEmpty && note.trimmingCharacters(in: .whitespaces).isEmpty)
                }
            } else {
                Text("When an agent asks you something, it shows here.").font(.system(size: 12)).foregroundStyle(p.text3)
                Button("Open Switchboard") { openWindow(id: "main"); NSApp.activate(ignoringOtherApps: true) }.buttonStyle(.link)
            }
        }
        .padding(14)
        .frame(width: 380)
        .background(p.pane)
        .onAppear { boards.watchAll({ store.projects.map(\.slug) }, engine: store.engine) }
    }

    private func review(_ slug: String) {
        NSApp.activate(ignoringOtherApps: true)
        boards.showBoards = (slug, Date())
        openWindow(id: "boards")
    }

    private func open(_ slug: String) {
        NSApp.activate(ignoringOtherApps: true)
        if store.openProjectWindows.contains(slug) { openWindow(id: "project", value: slug) } else { openWindow(id: "main") }
        boards.showBoard = (slug, Date())
    }
}
