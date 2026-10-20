import SwiftUI
import WebKit

// The board in a project window (plans/06-board.md, B2): where a coding agent hands the project
// to its author. Native list and steps (the Mail model, decided on the board as lc-home); each
// card's body in a web view of the same card page the phone shows. The board is files written by
// `switchboard board …` (docs/board-contract.md); this reads them through the engine.

extension CommandRef {
    /// A pane can show the project's board instead of a command (decided on the board, w-pane).
    static func board(_ project: String) -> CommandRef { CommandRef(project: project, address: "@board") }
    var isBoard: Bool { address == "@board" }
}

// MARK: - Data

struct BoardStep: Identifiable, Hashable {
    let id: String, label: String, title: String, state: String
}

struct BoardCard: Identifiable, Hashable {
    let id: String, stage: String, title: String, summary: String
    let question: String?, unsure: Bool, blocking: Bool, version: Int
    let basis: String, multi: Bool, notePrompt: String
    let askedAt: Double, updated: Double, why: String
    let options: [BoardOption]
    /// Answered (a pick for this version is in the Record), from any device.
    let decided: [String]?
    var needsYou: Bool { question != nil && decided == nil }
    /// One recommendation, resting on evidence, not a pick-any and not unsure: ↵ takes it.
    var sure: Bool { question != nil && !unsure && !multi && basis == "evidence" && options.filter(\.rec).count == 1 }
}

struct BoardOption: Hashable {
    let o: String, label: String, rec: Bool, then: String
}

struct BoardDecision: Identifiable, Hashable {
    let id: String, title: String, labels: [String], notes: [String], at: Double
}

struct BoardData {
    var json = ""
    var steps: [BoardStep] = []
    var cards: [BoardCard] = []
    var record: [BoardDecision] = []
    var projectDir = "", projectName = ""
    var needsYou: Int { cards.filter(\.needsYou).count }

    init() {}
    init(json: String) {
        self.json = json
        guard let root = (try? JSONSerialization.jsonObject(with: Data(json.utf8))) as? [String: Any] else { return }
        let plain = { (h: Any?) -> String in
            (h as? String ?? "").replacingOccurrences(of: "<[^>]+>", with: "", options: .regularExpression)
        }
        let proj = root["project"] as? [String: Any] ?? [:]
        projectDir = proj["dir"] as? String ?? ""; projectName = plain(proj["name"])
        steps = (root["stages"] as? [[String: Any]] ?? []).map {
            BoardStep(id: $0["id"] as? String ?? "", label: $0["label"] as? String ?? "", title: plain($0["title"]), state: $0["state"] as? String ?? "")
        }
        let rec = root["record"] as? [[String: Any]] ?? []
        record = rec.reversed().map {
            BoardDecision(id: $0["id"] as? String ?? "", title: plain($0["title"]), labels: $0["labels"] as? [String] ?? [],
                          notes: $0["notes"] as? [String] ?? [], at: ($0["at"] as? Double ?? 0) / 1000)
        }
        cards = (root["items"] as? [[String: Any]] ?? []).map { c in
            let d = c["decision"] as? [String: Any]
            let version = c["version"] as? Int ?? 1
            let picked = rec.last { ($0["id"] as? String) == (c["id"] as? String) && !($0["pick"] is NSNull) && $0["pick"] != nil && ($0["version"] as? Int ?? 1) >= version }
            return BoardCard(
                id: c["id"] as? String ?? "", stage: c["stage"] as? String ?? "", title: plain(c["title"]), summary: plain(c["summary"]),
                question: d.map { plain($0["q"]) }, unsure: d?["unsure"] != nil, blocking: c["blocking"] as? Bool ?? false, version: version,
                basis: d?["basis"] as? String ?? "", multi: d?["multi"] as? Bool ?? false, notePrompt: plain(c["notePrompt"]),
                askedAt: c["askedAt"] as? Double ?? 0, updated: c["updated"] as? Double ?? 0,
                why: plain((d?["why"] as? [String])?.first ?? (d?["unsure"] as? [String: Any])?["why"]),
                options: (d?["options"] as? [[String: Any]] ?? []).map {
                    BoardOption(o: "\($0["o"] ?? "")", label: plain($0["label"]), rec: $0["rec"] as? Bool ?? false, then: plain($0["then"]))
                },
                decided: picked.map { $0["labels"] as? [String] ?? [] })
        }
    }
}

/// Every open board, refreshed every 2 s while something shows it, re-parsed only when it changed.
@MainActor
final class BoardCache: ObservableObject {
    static let shared = BoardCache()
    @Published private(set) var boards: [String: BoardData] = [:]
    private var watching: [String: Int] = [:]
    /// Every project, for the menu bar's count across them (B3).
    private var everyProject: (() -> [String])?
    /// Open a project's board in its window (the menu bar's Open in Switchboard).
    @Published var showBoard: (slug: String, at: Date)?

    func watchAll(_ projects: @escaping () -> [String], engine: Engine) {
        self.engine = engine
        everyProject = projects
        for slug in projects() { refresh(slug) }
        if timer == nil {
            timer = Timer.scheduledTimer(withTimeInterval: 2, repeats: true) { [weak self] _ in
                Task { @MainActor in self?.refreshAll() }
            }
        }
    }

    /// What needs the author across projects, blocking first, then the latest asked.
    func needsYou() -> [(slug: String, card: BoardCard, data: BoardData)] {
        boards.flatMap { slug, data in data.cards.filter(\.needsYou).map { (slug: slug, card: $0, data: data) } }
            .sorted { a, b in
                if a.card.blocking != b.card.blocking { return a.card.blocking }
                return max(a.card.askedAt, a.card.updated) > max(b.card.askedAt, b.card.updated)
            }
    }
    var totalNeedsYou: Int { boards.values.reduce(0) { $0 + $1.needsYou } }
    private var timer: Timer?
    weak var engine: Engine?

    func data(_ slug: String) -> BoardData? { boards[slug] }

    func watch(_ slug: String, engine: Engine) {
        self.engine = engine
        watching[slug, default: 0] += 1
        refresh(slug)
        if timer == nil {
            timer = Timer.scheduledTimer(withTimeInterval: 2, repeats: true) { [weak self] _ in
                Task { @MainActor in self?.refreshAll() }
            }
        }
    }
    func unwatch(_ slug: String) {
        watching[slug, default: 1] -= 1
        if watching[slug] ?? 0 <= 0 { watching[slug] = nil }
        if watching.isEmpty && everyProject == nil { timer?.invalidate(); timer = nil }
    }
    func refresh(_ slug: String) {
        guard let engine else { return }
        let json = engine.boardSnapshot(slug: slug)
        if json == boards[slug]?.json { return }
        boards[slug] = json.map(BoardData.init(json:))
    }
    private func refreshAll() { for slug in Set(watching.keys).union(everyProject?() ?? []) { refresh(slug) } }
}

// MARK: - Sidebar row

/// "Board · 3" at the top of a project's commands; it opens the board in the focused pane.
struct BoardRow: View {
    @EnvironmentObject var store: EngineStore
    @EnvironmentObject var context: WindowContext
    @ObservedObject private var cache = BoardCache.shared
    @Environment(\.sbPalette) private var p
    let project: String
    let indent: CGFloat

    var body: some View {
        // Zero-height anchor: an empty view never appears, so it would never start watching.
        VStack(spacing: 0) {
            Color.clear.frame(height: 0)
            if let data = cache.data(project) {
                let selected = context.selection == .board(project)
                HStack(spacing: 8) {
                    Image(systemName: "checklist").font(.system(size: 11, weight: .medium)).foregroundStyle(p.text3).frame(width: 14)
                    Text("Board").font(.system(size: 13, weight: selected ? .semibold : .regular))
                        .foregroundStyle(selected ? p.selectionText : p.text)
                    Spacer(minLength: 4)
                    if data.needsYou > 0 {
                        Text("\(data.needsYou)").font(.system(size: 10, weight: .semibold)).monospacedDigit()
                            .foregroundStyle(p.onAccent).padding(.horizontal, 5).frame(minWidth: 16, minHeight: 16)
                            .background(p.accent, in: Capsule())
                            .help("\(data.needsYou) need you")
                    }
                }
                .padding(.leading, indent + 8).padding(.trailing, 8).frame(height: 28)
                .background(selected ? p.selection : .clear, in: RoundedRectangle(cornerRadius: 6, style: .continuous))
                .contentShape(Rectangle())
                .onTapGesture { context.selection = .board(project) }
            }
        }
        .onAppear { cache.watch(project, engine: store.engine) }
        .onDisappear { cache.unwatch(project) }
    }
}

// MARK: - The pane

struct BoardPane: View {
    @EnvironmentObject var store: EngineStore
    @ObservedObject private var cache = BoardCache.shared
    @Environment(\.sbPalette) private var p
    let project: String
    @State private var tab: Tab = .needs
    @State private var selected: String?
    @State private var picks: [String] = []
    @State private var note = ""
    @FocusState private var focused: Bool
    /// Review mode: one card at a time, blocking first, the queue fixed when it starts.
    @State private var queue: [String]? = nil

    enum Tab: String, CaseIterable { case needs = "Needs you", all = "Everything", record = "Record" }

    var body: some View {
        let data = cache.data(project) ?? BoardData()
        Group {
            if let queue { review(data, queue) } else { browse(data) }
        }
        .background(p.pane)
        .focusable().focused($focused).focusEffectDisabled()
        .onKeyPress { key in handleKey(key, data) }
        .onChange(of: selected) { _, _ in picks = []; note = "" }
        .onAppear { cache.watch(project, engine: store.engine); focused = true }
        .onDisappear { cache.unwatch(project) }
    }

    private func startReview(_ data: BoardData) {
        let needs = visible(data).filter(\.needsYou)
        let q = (needs.filter(\.blocking) + needs.filter { !$0.blocking }).map(\.id)
        guard !q.isEmpty else { return }
        queue = q; selected = q.first; focused = true
    }

    @ViewBuilder private func review(_ data: BoardData, _ q: [String]) -> some View {
        let i = q.firstIndex(of: selected ?? "") ?? 0
        let card = data.cards.first { $0.id == q[min(i, q.count - 1)] }
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                Button { queue = nil } label: { Label("Exit review", systemImage: "chevron.left") }
                    .buttonStyle(.plain).foregroundStyle(p.text2).keyboardShortcut(.escape, modifiers: [])
                Spacer()
                // One segment per card: done, here, to come.
                HStack(spacing: 3) {
                    ForEach(Array(q.enumerated()), id: \.offset) { j, id in
                        let done = data.cards.first { $0.id == id }?.decided != nil
                        Capsule().fill(j == i ? p.accent : (done ? p.running : p.line)).frame(width: j == i ? 14 : 6, height: 4)
                    }
                }
                .frame(maxWidth: 360)
                Text("\(i + 1) of \(q.count)").font(.system(size: 12, design: .monospaced)).foregroundStyle(p.text3)
                Spacer()
            }
            .padding(.horizontal, 14).frame(height: 40)
            Divider()
            if let card {
                CardBodyView(json: data.json, cardId: card.id).frame(maxWidth: .infinity, maxHeight: .infinity)
                if card.question != nil {
                    Divider()
                    AnswerBar(project: project, card: card, projectDir: data.projectDir, projectName: data.projectName,
                              picks: $picks, note: $note, onSent: { stepReview(1, q) })
                }
                Divider()
                HStack {
                    Button("Previous  K") { stepReview(-1, q) }.disabled(i == 0)
                    Spacer()
                    Text(card.sure ? "1–\(card.options.count) pick · ↵ my pick · ⌘↵ send" : "1–\(card.options.count) pick · ⌘↵ send")
                        .font(.system(size: 11)).foregroundStyle(p.text3)
                    Spacer()
                    Button(i + 1 < q.count ? "Skip  J" : "Done") { if i + 1 < q.count { stepReview(1, q) } else { queue = nil } }
                }
                .buttonStyle(.borderless).padding(.horizontal, 14).frame(height: 36)
            }
        }
    }

    private func stepReview(_ n: Int, _ q: [String]) {
        let i = q.firstIndex(of: selected ?? "") ?? 0
        let j = i + n
        if j >= q.count { queue = nil; cache.refresh(project); return }
        selected = q[max(0, j)]; picks = []; note = ""
        cache.refresh(project)
    }

    @ViewBuilder private func browse(_ data: BoardData) -> some View {
        HStack(spacing: 0) {
            VStack(spacing: 0) {
                Picker("", selection: $tab) {
                    ForEach(Tab.allCases, id: \.self) { t in
                        Text(t == .needs ? "Needs you \(data.needsYou)" : t.rawValue).tag(t)
                    }
                }
                .pickerStyle(.segmented).labelsHidden().padding(.horizontal, 10).padding(.top, 10)
                Button { startReview(data) } label: {
                    Label("Review one by one", systemImage: "play.fill").frame(maxWidth: .infinity)
                }
                .disabled(data.needsYou == 0).padding(10)
                Divider()
                if tab == .record { recordList(data) } else { cardList(data) }
            }
            .frame(width: 300)
            Divider()
            detail(data)
        }
    }

    /// J/K move, 1–9 pick, ↵ takes my pick when I'm sure (never on an unsure card), ⌘↵ sends.
    private func handleKey(_ key: KeyPress, _ data: BoardData) -> KeyPress.Result {
        let cards = visible(data)
        let i = cards.firstIndex { $0.id == selected }
        let card = i.map { cards[$0] }
        if let q = queue {
            switch key.characters {
            case "j": stepReview(1, q); return .handled
            case "k": stepReview(-1, q); return .handled
            default: break
            }
        } else if key.characters == "r" { startReview(data); return .handled }
        switch key.characters {
        case "j": if let i, i + 1 < cards.count { selected = cards[i + 1].id } else if i == nil { selected = cards.first?.id }; return .handled
        case "k": if let i, i > 0 { selected = cards[i - 1].id }; return .handled
        default: break
        }
        if let card, let n = Int(key.characters), n >= 1, n <= card.options.count {
            let o = card.options[n - 1].o
            if card.multi { if let j = picks.firstIndex(of: o) { picks.remove(at: j) } else { picks.append(o) } }
            else { picks = picks == [o] ? [] : [o] }
            return .handled
        }
        if key.key == .return, let card {
            if key.modifiers.contains(.command) || !picks.isEmpty {
                send(card, data); return .handled
            }
            if card.sure, let rec = card.options.first(where: \.rec) { picks = [rec.o]; return .handled }
        }
        return .ignored
    }

    private func visible(_ data: BoardData) -> [BoardCard] {
        let cards = tab == .needs ? data.cards.filter(\.needsYou) : data.cards
        return data.steps.flatMap { s in cards.filter { $0.stage == s.id } }
    }

    private func send(_ card: BoardCard, _ data: BoardData) {
        guard BoardSend.send(store: store, project: project, card: card, picks: picks, note: note,
                             projectDir: data.projectDir, projectName: data.projectName) else { return }
        if let q = queue { stepReview(1, q) } else { advance(after: card.id, in: data) }
    }

    /// After a send: the next card that needs you, or stay.
    private func advance(after id: String, in data: BoardData) {
        picks = []; note = ""
        let cards = visible(data).filter { $0.id != id }
        let order = visible(data).map(\.id)
        let from = order.firstIndex(of: id) ?? 0
        selected = cards.first { (order.firstIndex(of: $0.id) ?? 0) > from && $0.needsYou }?.id ?? cards.first(where: \.needsYou)?.id ?? id
        cache.refresh(project)
    }

    private func cardList(_ data: BoardData) -> some View {
        let cards = tab == .needs ? data.cards.filter(\.needsYou) : data.cards
        let stepsWith = data.steps.filter { s in cards.contains { $0.stage == s.id } }
        return List(selection: $selected) {
            ForEach(stepsWith) { s in
                Section {
                    ForEach(cards.filter { $0.stage == s.id }) { c in cardRow(c).tag(c.id) }
                } header: {
                    HStack(spacing: 6) {
                        if !s.label.isEmpty { Text(s.label).font(.system(size: 11, design: .monospaced)).foregroundStyle(p.text3) }
                        Text(s.title).font(.system(size: 11, weight: .semibold)).foregroundStyle(p.text2)
                        if s.state == "now" { Text("now").font(.system(size: 10)).foregroundStyle(p.accentSoftText) }
                    }
                }
            }
            if cards.isEmpty {
                Text(tab == .needs ? "Nothing needs you." : "No cards yet.").foregroundStyle(p.text3).frame(maxWidth: .infinity).padding(.top, 24)
            }
        }
        .listStyle(.sidebar)
        .scrollContentBackground(.hidden)
        .onAppear { if selected == nil { selected = cards.first?.id } }
    }

    private func cardRow(_ c: BoardCard) -> some View {
        HStack(alignment: .top, spacing: 8) {
            Circle().strokeBorder(c.needsYou ? p.accent : p.text3, lineWidth: 1.5).frame(width: 10, height: 10).padding(.top, 3)
                .overlay { if c.decided != nil { Image(systemName: "checkmark").font(.system(size: 7, weight: .bold)).foregroundStyle(p.text3) } }
            VStack(alignment: .leading, spacing: 2) {
                Text(c.title).font(.system(size: 13, weight: .medium)).foregroundStyle(p.text).lineLimit(2)
                Text(c.decided.map { "→ " + $0.joined(separator: ", ") } ?? c.question ?? "For you to see")
                    .font(.system(size: 11)).foregroundStyle(p.text3).lineLimit(1)
            }
            Spacer(minLength: 0)
            if c.blocking && c.needsYou { pill("Blocking", fg: p.crashed) }
            else if c.unsure && c.needsYou { pill("Unsure", fg: p.starting) }
        }
        .padding(.vertical, 2)
    }

    private func pill(_ t: String, fg: Color) -> some View {
        Text(t).font(.system(size: 10, weight: .semibold)).foregroundStyle(fg)
            .padding(.horizontal, 6).frame(height: 16).background(fg.opacity(0.12), in: Capsule())
    }

    private func recordList(_ data: BoardData) -> some View {
        List(data.record) { r in
            VStack(alignment: .leading, spacing: 2) {
                Text(r.title).font(.system(size: 13, weight: .medium)).foregroundStyle(p.text).lineLimit(2)
                Text(r.labels.isEmpty ? (r.notes.first ?? "A note") : "→ " + r.labels.joined(separator: ", "))
                    .font(.system(size: 11)).foregroundStyle(p.text3).lineLimit(2)
                Text(Date(timeIntervalSince1970: r.at).formatted(date: .abbreviated, time: .shortened)).font(.system(size: 10)).foregroundStyle(p.text3)
            }
            .padding(.vertical, 2)
        }
        .listStyle(.sidebar).scrollContentBackground(.hidden)
    }

    @ViewBuilder private func detail(_ data: BoardData) -> some View {
        if tab != .record, let id = selected, data.cards.contains(where: { $0.id == id }) {
            VStack(spacing: 0) {
                CardBodyView(json: data.json, cardId: id).frame(maxWidth: .infinity, maxHeight: .infinity)
                if let card = data.cards.first(where: { $0.id == id }), card.question != nil {
                    Divider()
                    AnswerBar(project: project, card: card, projectDir: data.projectDir, projectName: data.projectName,
                              picks: $picks, note: $note, onSent: { advance(after: id, in: data) })
                }
            }
        } else {
            VStack(spacing: 8) {
                Image(systemName: "checklist").font(.system(size: 26)).foregroundStyle(p.text3)
                Text(tab == .record ? "What you decided, newest first." : "Pick a card.").foregroundStyle(p.text3)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }
}

// MARK: - A card's body

/// The card page (`board/card.html`, bundled) in its `in=app` mode: just the card. The board's
/// data is handed to it before it runs, as `window.BOARD`, so it needs no server.
struct CardBodyView: NSViewRepresentable {
    let json: String
    let cardId: String

    func makeNSView(context: Context) -> WKWebView {
        let web = WKWebView(frame: .zero, configuration: WKWebViewConfiguration())
        web.setValue(false, forKey: "drawsBackground")
        load(web, context: context)
        return web
    }

    func updateNSView(_ web: WKWebView, context: Context) {
        if context.coordinator.loaded != json + cardId { load(web, context: context) }
    }

    func makeCoordinator() -> Coordinator { Coordinator() }
    final class Coordinator { var loaded = "" }

    private func load(_ web: WKWebView, context: Context) {
        context.coordinator.loaded = json + cardId
        guard let page = Bundle.main.url(forResource: "card", withExtension: "html") else { return }
        let ucc = web.configuration.userContentController
        ucc.removeAllUserScripts()
        ucc.addUserScript(WKUserScript(source: "window.BOARD = \(json);", injectionTime: .atDocumentStart, forMainFrameOnly: true))
        var c = URLComponents(url: page, resolvingAgainstBaseURL: false)!
        c.queryItems = [URLQueryItem(name: "id", value: cardId), URLQueryItem(name: "in", value: "app")]
        if let url = c.url { web.loadFileURL(url, allowingReadAccessTo: page.deletingLastPathComponent()) }
    }
}

// MARK: - Answering

/// What a send says (docs/board-contract.md, "A send"), recorded through the engine: the agent
/// hears it with `switchboard board answers --wait`, and the Record keeps the decision.
enum BoardSend {
    @MainActor
    static func send(store: EngineStore, project: String, card: BoardCard, picks: [String], note: String,
                     projectDir: String, projectName: String) -> Bool {
        let text = note.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !picks.isEmpty || !text.isEmpty else { return false }
        let chosen = card.options.filter { picks.contains($0.o) }
        let recs = card.options.filter(\.rec).map(\.o)
        let agreed = !chosen.isEmpty && Set(chosen.map(\.o)) == Set(recs)
        let labels = chosen.map(\.label)
        var lines = ["\(projectName.isEmpty ? "Board" : projectName): from Switchboard", "  \(card.title) [\(card.id)]"]
        if !labels.isEmpty { lines.append("    → \(labels.joined(separator: ", "))" + (recs.isEmpty ? "" : agreed ? "  (recommended)" : "  (NOT the recommendation)")) }
        if !text.isEmpty { lines.append("    note: " + text.replacingOccurrences(of: "\n", with: "\n          ")) }
        let pick: Any = chosen.isEmpty ? NSNull() : (card.multi ? chosen.map(\.o) as Any : chosen[0].o as Any)
        let send: [String: Any] = [
            "project": projectDir, "sentAt": Int(Date().timeIntervalSince1970 * 1000), "from": "app", "objections": [],
            "text": lines.joined(separator: "\n"),
            "answers": [["id": card.id, "stage": card.stage, "title": card.title, "version": card.version, "pick": pick,
                         "labels": labels, "rec": card.multi ? recs as Any : (recs.first as Any? ?? NSNull()),
                         "way": chosen.isEmpty ? "note" : (agreed ? "agreed" : "way"),
                         "notes": text.isEmpty ? [] : [["text": text]]]],
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: send), let json = String(data: data, encoding: .utf8) else { return false }
        do { try store.engine.boardRecordSend(slug: project, sendJson: json); return true }
        catch { NSLog("board send failed: \(error)"); return false }
    }
}

/// Under the card: its options (numbered, mine marked), what picking one commits me to, a note,
/// and Send. The same answer the page and the phone send.
struct AnswerBar: View {
    @EnvironmentObject var store: EngineStore
    @Environment(\.sbPalette) private var p
    let project: String
    let card: BoardCard
    let projectDir: String, projectName: String
    @Binding var picks: [String]
    @Binding var note: String
    let onSent: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if let decided = card.decided {
                Label("Decided: " + decided.joined(separator: ", "), systemImage: "checkmark.circle")
                    .font(.system(size: 12, weight: .medium)).foregroundStyle(p.text2)
            }
            HStack(spacing: 6) {
                if let q = card.question { Text(q).font(.system(size: 12, weight: .semibold)).foregroundStyle(p.text) }
                if card.sure { Text("↵ takes my pick").font(.system(size: 11)).foregroundStyle(p.text3) }
                if card.unsure { Text("I'm not sure: look before you pick").font(.system(size: 11)).foregroundStyle(p.starting) }
            }
            FlowOptions(card: card, picks: $picks)
            if let then = card.options.first(where: { picks.contains($0.o) && !$0.then.isEmpty })?.then {
                Text("Then I'll " + then.prefix(1).lowercased() + then.dropFirst()).font(.system(size: 12)).foregroundStyle(p.text2)
            }
            HStack(spacing: 8) {
                TextField(card.notePrompt.isEmpty ? "A note for the agent (optional)" : card.notePrompt, text: $note, axis: .vertical)
                    .textFieldStyle(.roundedBorder).lineLimit(1...4)
                Button("Send") {
                    if BoardSend.send(store: store, project: project, card: card, picks: picks, note: note,
                                      projectDir: projectDir, projectName: projectName) { onSent() }
                }
                .keyboardShortcut(.return, modifiers: .command)
                .disabled(picks.isEmpty && note.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
        .padding(12)
        .background(p.sidebar)
    }
}

struct FlowOptions: View {
    @Environment(\.sbPalette) private var p
    let card: BoardCard
    @Binding var picks: [String]

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            ForEach(Array(card.options.enumerated()), id: \.offset) { i, o in
                let on = picks.contains(o.o)
                Button {
                    if card.multi { if let j = picks.firstIndex(of: o.o) { picks.remove(at: j) } else { picks.append(o.o) } }
                    else { picks = on ? [] : [o.o] }
                } label: {
                    HStack(spacing: 8) {
                        Text("\(i + 1)").font(.system(size: 11, design: .monospaced)).foregroundStyle(on ? p.onAccent : p.text3).frame(width: 14)
                        Text(o.label).font(.system(size: 13, weight: on ? .semibold : .regular)).foregroundStyle(on ? p.onAccent : p.text)
                            .multilineTextAlignment(.leading)
                        Spacer(minLength: 4)
                        if o.rec { Text(card.unsure ? "leaning" : "my pick").font(.system(size: 10, weight: .medium)).foregroundStyle(on ? p.onAccent : p.running) }
                    }
                    .padding(.horizontal, 10).padding(.vertical, 7)
                    .background(on ? p.accent : p.pane, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
                    .overlay { RoundedRectangle(cornerRadius: 7, style: .continuous).strokeBorder(on ? .clear : p.line) }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
    }
}
