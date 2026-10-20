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
    /// What the agent says it's doing (`switchboard board working`), and since when (seconds).
    var working = "", workingAt: Double = 0
    var needsYou: Int { cards.filter(\.needsYou).count }
    var blocking: Int { cards.filter { $0.needsYou && $0.blocking }.count }

    init() {}
    init(json: String) {
        self.json = json
        guard let root = (try? JSONSerialization.jsonObject(with: Data(json.utf8))) as? [String: Any] else { return }
        let plain = { (h: Any?) -> String in
            (h as? String ?? "").replacingOccurrences(of: "<[^>]+>", with: "", options: .regularExpression)
        }
        let proj = root["project"] as? [String: Any] ?? [:]
        projectDir = proj["dir"] as? String ?? ""; projectName = plain(proj["name"])
        if let w = proj["working"] as? [String: Any] {
            working = plain(w["text"]); workingAt = (w["since"] as? Double ?? 0) / 1000
        }
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
    /// Bring the Boards window to a project (B5).
    @Published var showBoards: (slug: String?, at: Date)?

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

/// The board pane: the board page itself (`board/index.html`, bundled), the same page the
/// browser shows, so the Mac and the page never drift (lc-home, revised 2026-10-20). The app hands
/// it the board and its updates, and records its Send through the engine.
struct BoardPane: View {
    @EnvironmentObject var store: EngineStore
    @ObservedObject private var cache = BoardCache.shared
    let project: String

    var body: some View {
        BoardPageView(project: project, json: cache.data(project)?.json ?? "", engine: store.engine)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(WindowGrower(minWidth: 1360, minHeight: 820))
            .onAppear { cache.watch(project, engine: store.engine) }
            .onDisappear { cache.unwatch(project) }
    }
}

struct BoardPageView: NSViewRepresentable {
    let project: String
    let json: String
    let engine: Engine
    /// The Boards window (B5) opens the page in review mode and offers the next project there.
    var review = false
    var next: String? = nil
    var onNext: (() -> Void)? = nil

    func makeCoordinator() -> Coordinator { Coordinator(project: project, engine: engine) }

    func makeNSView(context: Context) -> WKWebView {
        let config = WKWebViewConfiguration()
        // Served as sbboard://board/, not file://, so a card can show an image where it already is.
        let coordinator = context.coordinator
        config.setURLSchemeHandler(BoardScheme(named: { [weak coordinator] in coordinator?.pushed ?? "" }), forURLScheme: BoardScheme.scheme)
        config.userContentController.add(context.coordinator, name: "board")
        let mode = review ? "window.BOARD_MODE = 'review'; window.BOARD_NEXT = \(Self.js(next ?? ""));" : ""
        config.userContentController.addUserScript(WKUserScript(source: "window.BOARD = \(json.isEmpty ? "null" : json);" + mode,
                                                                injectionTime: .atDocumentStart, forMainFrameOnly: true))
        let web = WKWebView(frame: .zero, configuration: config)
        web.setValue(false, forKey: "drawsBackground")
        web.navigationDelegate = context.coordinator
        context.coordinator.web = web
        context.coordinator.pushed = json
        context.coordinator.next = next ?? ""
        context.coordinator.onNext = onNext
        web.load(URLRequest(url: URL(string: "\(BoardScheme.scheme)://board/index.html")!))
        return web
    }

    func updateNSView(_ web: WKWebView, context: Context) {
        context.coordinator.push(json)
        context.coordinator.onNext = onNext
        if review, (next ?? "") != context.coordinator.next {
            context.coordinator.next = next ?? ""
            web.evaluateJavaScript("window.__boardNext && window.__boardNext(\(Self.js(next ?? "")));")
        }
    }

    /// A string as a JavaScript literal.
    static func js(_ s: String) -> String {
        (try? String(data: JSONSerialization.data(withJSONObject: [s]), encoding: .utf8)).flatMap { $0.map { String($0.dropFirst().dropLast()) } } ?? "''"
    }

    static func dismantleNSView(_ web: WKWebView, coordinator: Coordinator) {
        web.configuration.userContentController.removeScriptMessageHandler(forName: "board")
    }

    final class Coordinator: NSObject, WKScriptMessageHandler, WKNavigationDelegate {
        let project: String
        let engine: Engine
        weak var web: WKWebView?
        var pushed = ""
        var loaded = false
        var next = ""
        var onNext: (() -> Void)?
        init(project: String, engine: Engine) { self.project = project; self.engine = engine }

        /// A changed board reaches the open page (the page's own `ingest`, as when it polls).
        func push(_ json: String) {
            guard !json.isEmpty, json != pushed else { return }
            pushed = json
            if loaded { web?.evaluateJavaScript("window.__boardIngest && window.__boardIngest(\(json));") }
        }

        func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
            loaded = true
            webView.evaluateJavaScript("window.__boardIngest && window.__boardIngest(\(pushed.isEmpty ? "null" : pushed));")
        }

        /// The page's Send: record it through the engine and tell the page how it went.
        func userContentController(_ c: WKUserContentController, didReceive message: WKScriptMessage) {
            guard let body = message.body as? [String: Any] else { return }
            if body["type"] as? String == "next" { onNext?(); return }
            guard body["type"] as? String == "send", let send = body["send"],
                  let data = try? JSONSerialization.data(withJSONObject: send), let json = String(data: data, encoding: .utf8) else { return }
            let ok: Bool
            do { try engine.boardRecordSend(slug: project, sendJson: json); ok = true }
            catch { NSLog("board send failed: \(error)"); ok = false }
            web?.evaluateJavaScript("window.__boardSent && window.__boardSent(\(ok));")
            Task { @MainActor in BoardCache.shared.refresh(self.project) }
        }
    }
}

/// The board page's own origin: the page and its script from the app bundle, and the images and
/// videos the cards point at where they already are, e.g. `~/Downloads/shot.png` (no copies to
/// clean up). Only a file a card on this board names, a media type, inside the home folder: a
/// card's HTML can't read anything else. Byte ranges are honoured, so videos play and seek.
final class BoardScheme: NSObject, WKURLSchemeHandler {
    static let scheme = "sbboard"
    private static let bundled: Set<String> = ["index.html", "card.html", "marked.min.js"]
    private static let page = ["html": "text/html", "js": "text/javascript"]
    static let media = ["png": "image/png", "jpg": "image/jpeg", "jpeg": "image/jpeg", "gif": "image/gif", "webp": "image/webp",
                        "heic": "image/heic", "svg": "image/svg+xml", "mp4": "video/mp4", "mov": "video/quicktime", "m4v": "video/mp4",
                        "webm": "video/webm", "mp3": "audio/mpeg", "m4a": "audio/mp4", "wav": "audio/wav", "pdf": "application/pdf"]
    /// The board's JSON as the page has it, to check a file is one its cards name.
    let named: () -> String
    init(named: @escaping () -> String) { self.named = named }

    func webView(_ webView: WKWebView, start task: WKURLSchemeTask) {
        let url = task.request.url ?? URL(string: "\(Self.scheme)://board/")!
        guard let (file, type) = file(for: url), let data = try? Data(contentsOf: file, options: .mappedIfSafe) else {
            task.didReceive(HTTPURLResponse(url: url, statusCode: 404, httpVersion: "HTTP/1.1", headerFields: [:])!)
            task.didFinish()
            return
        }
        var status = 200, body = data
        var headers = ["Content-Type": type, "Accept-Ranges": "bytes", "Cache-Control": "no-cache"]
        if let range = task.request.value(forHTTPHeaderField: "Range"), let (lo, hi) = Self.range(range, count: data.count) {
            status = 206; body = data.subdata(in: lo..<(hi + 1))
            headers["Content-Range"] = "bytes \(lo)-\(hi)/\(data.count)"
        }
        headers["Content-Length"] = String(body.count)
        task.didReceive(HTTPURLResponse(url: url, statusCode: status, httpVersion: "HTTP/1.1", headerFields: headers)!)
        task.didReceive(body)
        task.didFinish()
    }
    func webView(_ webView: WKWebView, stop task: WKURLSchemeTask) {}

    private func file(for url: URL) -> (URL, String)? {
        let path = url.path.removingPercentEncoding ?? url.path
        let trimmed = path.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        if Self.bundled.contains(trimmed), let f = Bundle.main.resourceURL?.appendingPathComponent(trimmed) {
            return (f, Self.page[f.pathExtension] ?? "text/plain")
        }
        // `~/x` arrives as /~/x; an absolute `/Users/me/x` as itself.
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        let written: [String], full: String
        if trimmed.hasPrefix("~/") {
            full = home + "/" + trimmed.dropFirst(2); written = [trimmed, full]
        } else {
            full = path; written = [path, "~" + path.dropFirst(home.count)]
        }
        guard let type = Self.media[(full as NSString).pathExtension.lowercased()] else { return nil }
        let f = URL(fileURLWithPath: full).standardizedFileURL.resolvingSymlinksInPath()
        guard f.path.hasPrefix(home + "/"), full.hasPrefix(home + "/") else { return nil }
        let json = named()
        guard written.contains(where: { json.contains("\"" + $0 + "\"") || json.contains("\\\"" + $0 + "\\\"") || json.contains("'" + $0 + "'") }) else { return nil }
        var dir: ObjCBool = false
        guard FileManager.default.fileExists(atPath: f.path, isDirectory: &dir), !dir.boolValue else { return nil }
        return (f, type)
    }

    /// `bytes=a-b`, `bytes=a-` or `bytes=-n`, clamped to the file.
    private static func range(_ header: String, count: Int) -> (Int, Int)? {
        guard count > 0, header.hasPrefix("bytes="), !header.contains(",") else { return nil }
        let parts = header.dropFirst(6).split(separator: "-", omittingEmptySubsequences: false).map(String.init)
        guard parts.count == 2 else { return nil }
        let lo: Int, hi: Int
        if parts[0].isEmpty { guard let n = Int(parts[1]), n > 0 else { return nil }; lo = max(0, count - n); hi = count - 1 }
        else { guard let a = Int(parts[0]), a < count else { return nil }; lo = a; hi = min(Int(parts[1]) ?? count - 1, count - 1) }
        return lo <= hi ? (lo, hi) : nil
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

/// The board page needs room: when it opens in a narrower window, the window grows to fit, up to
/// the screen's visible size. A window already that big is left alone.
/// Every window opens maximized: filling the screen's visible frame (not full-screen mode), once,
/// when it first appears; the author can resize it after.
struct WindowMaximizer: NSViewRepresentable {
    final class Probe: NSView {
        private var done = false
        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            guard !done, let w = window else { return }
            done = true
            DispatchQueue.main.async {
                guard let screen = (w.screen ?? NSScreen.main)?.visibleFrame, w.frame != screen else { return }
                w.setFrame(screen, display: true)
            }
        }
    }
    func makeNSView(context: Context) -> NSView { Probe() }
    func updateNSView(_ v: NSView, context: Context) {}
}

struct WindowGrower: NSViewRepresentable {
    let minWidth: CGFloat, minHeight: CGFloat
    func makeNSView(context: Context) -> NSView {
        let v = NSView()
        DispatchQueue.main.async { grow(v.window) }
        return v
    }
    func updateNSView(_ v: NSView, context: Context) {}
    private func grow(_ window: NSWindow?) {
        guard let window, let screen = window.screen?.visibleFrame else { return }
        var f = window.frame
        let w = min(max(f.width, minWidth), screen.width), h = min(max(f.height, minHeight), screen.height)
        guard w > f.width || h > f.height else { return }
        f.origin.x = min(max(screen.minX, f.midX - w / 2), screen.maxX - w)
        f.origin.y = min(max(screen.minY, f.maxY - h), screen.maxY - h)
        f.size = CGSize(width: w, height: h)
        window.setFrame(f, display: true, animate: true)
    }
}
