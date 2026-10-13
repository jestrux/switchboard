import AppKit
import Darwin
import ImageIO
import SwiftTerm

/// Image references in terminal output: hover shows a preview, click opens the file in
/// the Mac's default image app.
///
/// Two kinds are recognised. A path to an image file, absolute, `~`-relative or relative
/// to the command's folder, counts only when the file exists. A `[Image #N]` marker, which
/// Claude Code prints for an image pasted into its prompt, is looked up in that session's
/// transcript and written to a cache file.
///
/// Text comes from SwiftTerm's own buffer, so it follows the scroll position and joins
/// lines the terminal wrapped. SwiftTerm does not make that buffer public, so it is reached
/// by reflection; if a SwiftTerm update renames it, the engine's parsed screen is used
/// instead, which covers everything except scrolled-back output.
@MainActor
enum ImageLinks {
    struct Found {
        let file: URL
        /// Columns of the reference on the hovered row.
        let columns: Range<Int>
    }

    private static let extensions = try! NSRegularExpression(
        pattern: #"\.(?:png|jpe?g|gif|webp|heic|heif|tiff?|bmp|svg|avif)(?![A-Za-z0-9_])"#, options: [.caseInsensitive])
    private static let pasted = try! NSRegularExpression(pattern: #"\[Image #(\d+)\]"#)
    private static let delimiters = CharacterSet.whitespaces.union(CharacterSet(charactersIn: "\"'`([{<>=:,│|⎿●"))

    /// The image reference at a visible cell of a terminal, if any.
    static func find(ref: CommandRef, view: TerminalView, row: Int, col: Int) -> Found? {
        let screen = Screen(view: view, ref: ref)
        guard row >= 0, row < screen.rows, col >= 0, col < screen.cols else { return nil }
        for span in spans(screen, row: row) {
            guard let offset = span.offset(row: row, col: col) else { continue }
            if let found = find(in: span, at: offset, row: row, ref: ref) { return found }
        }
        return nil
    }

    private static func find(in span: Span, at offset: Int, row: Int, ref: CommandRef) -> Found? {
        let ns = span.text as NSString
        let all = NSRange(location: 0, length: ns.length)

        for m in pasted.matches(in: span.text, range: all) where NSLocationInRange(offset, m.range) {
            guard let n = Int(ns.substring(with: m.range(at: 1))),
                  let pid = EngineStore.shared.state(ref).pid,
                  let file = PastedImages.file(commandPid: pid_t(pid), number: n) else { return nil }
            return span.columns(of: m.range, row: row).map { Found(file: file, columns: $0) }
        }

        let bases = folders(for: ref)
        for m in extensions.matches(in: span.text, range: all) {
            let end = m.range.location + m.range.length
            guard end > offset else { continue }
            let key = "\(span.text)\u{0}\(end)\u{0}\(bases.joined(separator: ":"))"
            let resolved: Resolved?
            if let cached = memo[key] {
                resolved = cached
            } else {
                resolved = resolve(ns, end: end, limit: m.range.location, bases: bases)
                if memo.count > 300 { memo.removeAll() }
                memo[key] = .some(resolved)
            }
            guard let resolved, resolved.start <= offset else { continue }
            let range = NSRange(location: resolved.start, length: end - resolved.start)
            return span.columns(of: range, row: row).map { Found(file: resolved.file, columns: $0) }
        }
        return nil
    }

    private struct Resolved { let start: Int; let file: URL }
    private static var memo: [String: Resolved?] = [:]

    /// The longest stretch of text ending at `end` that names an existing image file.
    /// Trying the longest first is what lets paths with spaces work.
    private static func resolve(_ ns: NSString, end: Int, limit: Int, bases: [String]) -> Resolved? {
        var tried = 0
        var start = max(0, end - 500)
        while start <= limit, tried < 200 {
            defer { start += 1 }
            let first = ns.character(at: start)
            guard let scalar = Unicode.Scalar(first), !CharacterSet.whitespaces.contains(scalar) else { continue }
            if start > 0 {
                guard let before = Unicode.Scalar(ns.character(at: start - 1)), delimiters.contains(before) else { continue }
            }
            tried += 1
            var text = ns.substring(with: NSRange(location: start, length: end - start))
            if text.hasPrefix("file://") {
                text = String(text.dropFirst(7)).removingPercentEncoding ?? String(text.dropFirst(7))
            }
            text = text.replacingOccurrences(of: "\\ ", with: " ")
            let candidates: [String]
            if text.hasPrefix("/") {
                candidates = [text]
            } else if text.hasPrefix("~/") {
                candidates = [NSHomeDirectory() + String(text.dropFirst(1))]
            } else {
                candidates = bases.map { ($0 as NSString).appendingPathComponent(text) }
            }
            for path in candidates {
                var isDir: ObjCBool = false
                if FileManager.default.fileExists(atPath: path, isDirectory: &isDir), !isDir.boolValue {
                    return Resolved(start: start, file: URL(fileURLWithPath: path).standardizedFileURL)
                }
            }
        }
        return nil
    }

    /// Folders a relative path may be relative to: the command's folder from its config,
    /// and the folder its process is in right now (a shell that has `cd`'d elsewhere).
    private static func folders(for ref: CommandRef) -> [String] {
        var out: [String] = []
        if let cwd = EngineStore.shared.command(ref)?.cwdPath { out.append(cwd) }
        if let pid = EngineStore.shared.state(ref).pid, let live = Process.currentDirectory(of: pid_t(pid)), !out.contains(live) {
            out.append(live)
        }
        return out
    }

    // MARK: Text

    /// A stretch of text made of one or more terminal rows, with each UTF-16 unit mapped
    /// back to the cell it came from.
    private struct Span {
        var text = ""
        var cells: [(row: Int, col: Int)] = []

        func offset(row: Int, col: Int) -> Int? {
            cells.firstIndex { $0.row == row && $0.col == col }
        }

        /// The columns a range covers on one row.
        func columns(of range: NSRange, row: Int) -> Range<Int>? {
            let cols = cells[range.location..<min(cells.count, range.location + range.length)].filter { $0.row == row }.map(\.col)
            guard let lo = cols.min(), let hi = cols.max() else { return nil }
            return lo..<(hi + 1)
        }

        mutating func append(_ screen: Screen, row: Int, trimLeft: Bool, trimRight: Bool) {
            var parts: [(String, Int)] = []
            for col in 0..<screen.cols {
                let t = screen.text(row, col)
                parts.append((t.isEmpty || t == "\u{0}" ? " " : t, col))
            }
            if trimRight { while let last = parts.last, last.0 == " " { parts.removeLast() } }
            if trimLeft { while let first = parts.first, first.0 == " " { parts.removeFirst() } }
            for (t, col) in parts {
                text += t
                cells.append(contentsOf: Array(repeating: (row, col), count: t.utf16.count))
            }
        }
    }

    /// The visible rows of a terminal as cells.
    private struct Screen {
        let rows: Int
        let cols: Int
        let text: (_ row: Int, _ col: Int) -> String
        /// Is this row the continuation of the one above?
        let wrapped: (_ row: Int) -> Bool

        @MainActor
        init(view: TerminalView, ref: CommandRef) {
            if let terminal = ImageLinks.terminal(of: view) {
                rows = terminal.rows
                cols = terminal.cols
                text = { terminal.getText(col: $1, row: $0) ?? " " }
                wrapped = { terminal.getLine(row: $0)?.isWrapped ?? false }
            } else {
                let lines = EngineStore.shared.engine.screenText(project: ref.project, address: ref.address)
                    .components(separatedBy: "\n").map { $0.map(String.init) }
                rows = lines.count
                cols = lines.map(\.count).max() ?? 0
                text = { $1 < lines[$0].count ? lines[$0][$1] : " " }
                wrapped = { _ in false }
            }
        }
    }

    private static let terminals = NSMapTable<TerminalView, Terminal>(keyOptions: .weakMemory, valueOptions: .weakMemory)

    private static func terminal(of view: TerminalView) -> Terminal? {
        if let known = terminals.object(forKey: view) { return known }
        var mirror: Mirror? = Mirror(reflecting: view)
        while let m = mirror {
            if let found = m.children.first(where: { $0.label == "terminal" })?.value as? Terminal {
                terminals.setObject(found, forKey: view)
                return found
            }
            mirror = m.superclassMirror
        }
        return nil
    }

    /// Readings of the text around a row, most trusted first: the line as the terminal
    /// wrapped it, then the row glued to its neighbour, for programs (Claude Code among
    /// them) that break long lines themselves.
    private static func spans(_ terminal: Screen, row: Int) -> [Span] {
        func wrapped(_ r: Int) -> Bool { terminal.wrapped(r) }
        var first = row
        while first > 0, wrapped(first) { first -= 1 }
        var last = row
        while last + 1 < terminal.rows, wrapped(last + 1) { last += 1 }
        var logical = Span()
        for r in first...last { logical.append(terminal, row: r, trimLeft: false, trimRight: r == last) }
        var out = [logical]
        if last + 1 < terminal.rows {
            var s = Span()
            for r in first...last { s.append(terminal, row: r, trimLeft: false, trimRight: r == last) }
            s.append(terminal, row: last + 1, trimLeft: true, trimRight: true)
            out.append(s)
        }
        if first > 0 {
            var s = Span()
            s.append(terminal, row: first - 1, trimLeft: false, trimRight: true)
            var rest = Span()
            for r in first...last { rest.append(terminal, row: r, trimLeft: r == first, trimRight: r == last) }
            s.text += rest.text
            s.cells += rest.cells
            out.append(s)
        }
        return out
    }
}

// MARK: - Pasted images

/// Recovers images pasted into a Claude Code prompt. Claude Code keeps no image file: the
/// image lives in the session transcript, in the message that carried it. The session is
/// found through the process: Claude Code records each running session by pid.
@MainActor
enum PastedImages {
    private static var ready: [String: URL] = [:]
    private static var missing: [String: Date] = [:]
    private static var loading: Set<String> = []

    /// The cached file for `[Image #number]` in the Claude session running under a command,
    /// or nil while it is being looked up (the hover refreshes when it arrives).
    static func file(commandPid: pid_t, number: Int) -> URL? {
        let key = "\(commandPid)-\(number)"
        if let url = ready[key] { return url }
        // A marker still in the prompt is not in the transcript yet; look again shortly.
        if let at = missing[key], Date().timeIntervalSince(at) < 3 { return nil }
        guard loading.insert(key).inserted else { return nil }
        DispatchQueue.global(qos: .userInitiated).async {
            let url = extract(commandPid: commandPid, number: number)
            DispatchQueue.main.async {
                loading.remove(key)
                if let url { ready[key] = url; LinkHandling.refreshHover() } else { missing[key] = Date() }
            }
        }
        return nil
    }

    private nonisolated static func extract(commandPid: pid_t, number: Int) -> URL? {
        let fm = FileManager.default
        let claude = fm.homeDirectoryForCurrentUser.appendingPathComponent(".claude")
        guard let session = sessionID(under: commandPid, in: claude.appendingPathComponent("sessions")) else { return nil }
        // One folder per session, so the file can carry the name the user saw.
        let cache = fm.urls(for: .cachesDirectory, in: .userDomainMask)[0].appendingPathComponent("dev.waky.switchboard/pasted-images/\(session)")
        if let hit = (try? fm.contentsOfDirectory(at: cache, includingPropertiesForKeys: nil))?
            .first(where: { $0.deletingPathExtension().lastPathComponent == "Image \(number)" }) { return hit }
        guard let projects = try? fm.contentsOfDirectory(at: claude.appendingPathComponent("projects"), includingPropertiesForKeys: nil),
              let transcript = projects.map({ $0.appendingPathComponent("\(session).jsonl") }).first(where: { fm.fileExists(atPath: $0.path) }),
              let data = try? Data(contentsOf: transcript, options: .mappedIfSafe) else { return nil }
        let needle = Data("\"imagePasteIds\"".utf8)
        // Newest first: the message that carried the image is usually recent.
        var end = data.endIndex
        while end > data.startIndex {
            let start = data[..<(end - 1)].lastIndex(of: 0x0a).map { $0 + 1 } ?? data.startIndex
            let line = data[start..<end]
            end = start
            guard line.range(of: needle) != nil,
                  let obj = try? JSONSerialization.jsonObject(with: line) as? [String: Any],
                  let ids = obj["imagePasteIds"] as? [Int], let index = ids.firstIndex(of: number),
                  let content = (obj["message"] as? [String: Any])?["content"] as? [[String: Any]] else { continue }
            let images = content.filter { $0["type"] as? String == "image" }
            guard index < images.count, let source = images[index]["source"] as? [String: Any],
                  let base64 = source["data"] as? String, let bytes = Data(base64Encoded: base64) else { continue }
            let type = (source["media_type"] as? String ?? "image/png").replacingOccurrences(of: "image/", with: "")
            let file = cache.appendingPathComponent("Image \(number).\(type == "jpeg" ? "jpg" : type)")
            try? fm.createDirectory(at: cache, withIntermediateDirectories: true)
            guard (try? bytes.write(to: file, options: .atomic)) != nil else { return nil }
            return file
        }
        return nil
    }

    /// The Claude session whose process is the command or one of its descendants.
    private nonisolated static func sessionID(under root: pid_t, in dir: URL) -> String? {
        guard let files = try? FileManager.default.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil) else { return nil }
        var best: (String, Double)?
        for file in files where file.pathExtension == "json" {
            guard let pid = pid_t(file.deletingPathExtension().lastPathComponent), Process.isDescendant(pid, of: root),
                  let data = try? Data(contentsOf: file),
                  let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let id = obj["sessionId"] as? String else { continue }
            let at = (obj["updatedAt"] as? Double) ?? (obj["startedAt"] as? Double) ?? 0
            if best == nil || at > best!.1 { best = (id, at) }
        }
        return best?.0
    }
}

extension Process {
    static func parent(of pid: pid_t) -> pid_t? {
        var info = proc_bsdinfo()
        let size = Int32(MemoryLayout<proc_bsdinfo>.size)
        return proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, &info, size) == size ? pid_t(info.pbi_ppid) : nil
    }

    static func isDescendant(_ pid: pid_t, of root: pid_t) -> Bool {
        var current = pid
        for _ in 0..<32 {
            if current == root { return true }
            guard let up = parent(of: current), up > 1, up != current else { return false }
            current = up
        }
        return false
    }

    static func currentDirectory(of pid: pid_t) -> String? {
        var info = proc_vnodepathinfo()
        let size = Int32(MemoryLayout<proc_vnodepathinfo>.size)
        guard proc_pidinfo(pid, PROC_PIDVNODEPATHINFO, 0, &info, size) == size else { return nil }
        let path = withUnsafePointer(to: &info.pvi_cdir.vip_path) {
            $0.withMemoryRebound(to: CChar.self, capacity: Int(MAXPATHLEN)) { String(cString: $0) }
        }
        return path.isEmpty ? nil : path
    }
}

// MARK: - Preview

/// The floating preview shown beside a hovered image reference.
@MainActor
enum ImagePreview {
    private static var panel: ImagePreviewPanel?
    private static var shown: URL?
    private static var wanted: URL?
    private static var thumbs: [String: (NSImage, CGSize)] = [:]

    /// Shows the preview for a file next to a rect (window coordinates) after a short
    /// dwell, so sweeping the pointer across a path does not flash it.
    static func show(_ file: URL, near rect: NSRect, in window: NSWindow) {
        if shown == file, panel?.isVisible == true { return }
        guard wanted != file else { return }
        wanted = file
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.18) {
            guard wanted == file else { return }
            load(file) { image, pixels in
                guard wanted == file, let image else { return }
                present(image, pixels: pixels, file: file, near: rect, in: window)
            }
        }
    }

    static func hide() {
        wanted = nil
        shown = nil
        guard let panel, panel.isVisible else { return }
        panel.parent?.removeChildWindow(panel)
        panel.orderOut(nil)
    }

    private static func present(_ image: NSImage, pixels: CGSize, file: URL, near rect: NSRect, in window: NSWindow) {
        let panel = panel ?? ImagePreviewPanel()
        self.panel = panel
        let maxSide = CGSize(width: 380, height: 300)
        let natural = CGSize(width: max(1, pixels.width / 2), height: max(1, pixels.height / 2))
        let k = min(1, maxSide.width / natural.width, maxSide.height / natural.height)
        let size = CGSize(width: max(120, (natural.width * k).rounded()), height: max(40, (natural.height * k).rounded()))
        let caption = "\(file.lastPathComponent)  ·  \(Int(pixels.width)) × \(Int(pixels.height))"
        let frameSize = panel.layout(image: image, imageSize: size, caption: caption)

        let anchor = window.convertToScreen(rect)
        let visible = (window.screen ?? NSScreen.main)?.visibleFrame ?? anchor
        var origin = NSPoint(x: anchor.minX, y: anchor.maxY + 8)
        if origin.y + frameSize.height > visible.maxY { origin.y = anchor.minY - 8 - frameSize.height }
        origin.x = min(max(visible.minX + 8, origin.x), visible.maxX - frameSize.width - 8)
        origin.y = max(visible.minY + 8, origin.y)
        panel.setFrame(NSRect(origin: origin, size: frameSize), display: true)
        if panel.parent !== window {
            panel.parent?.removeChildWindow(panel)
            window.addChildWindow(panel, ordered: .above)
        }
        panel.orderFront(nil)
        shown = file
    }

    private static func load(_ file: URL, done: @escaping (NSImage?, CGSize) -> Void) {
        let stamp = (try? file.resourceValues(forKeys: [.contentModificationDateKey]))?.contentModificationDate?.timeIntervalSince1970 ?? 0
        let key = "\(file.path)@\(stamp)"
        if let hit = thumbs[key] { return done(hit.0, hit.1) }
        DispatchQueue.global(qos: .userInitiated).async {
            var image: NSImage?
            var pixels = CGSize.zero
            if let source = CGImageSourceCreateWithURL(file as CFURL, nil),
               let cg = CGImageSourceCreateThumbnailAtIndex(source, 0, [
                   kCGImageSourceCreateThumbnailFromImageAlways: true,
                   kCGImageSourceCreateThumbnailWithTransform: true,
                   kCGImageSourceThumbnailMaxPixelSize: 760,
               ] as CFDictionary) {
                let props = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any]
                let w = props?[kCGImagePropertyPixelWidth] as? CGFloat ?? CGFloat(cg.width)
                let h = props?[kCGImagePropertyPixelHeight] as? CGFloat ?? CGFloat(cg.height)
                let turned = [5, 6, 7, 8].contains(props?[kCGImagePropertyOrientation] as? Int ?? 1)
                pixels = turned ? CGSize(width: h, height: w) : CGSize(width: w, height: h)
                image = NSImage(cgImage: cg, size: NSSize(width: cg.width, height: cg.height))
            } else if let other = NSImage(contentsOf: file), other.isValid {
                // SVG and anything else ImageIO does not read.
                pixels = CGSize(width: other.size.width * 2, height: other.size.height * 2)
                image = other
            }
            DispatchQueue.main.async {
                if let image {
                    if thumbs.count > 40 { thumbs.removeAll() }
                    thumbs[key] = (image, pixels)
                }
                done(image, pixels)
            }
        }
    }
}

/// A borderless, click-through panel: the image on a rounded card with a caption.
final class ImagePreviewPanel: NSPanel {
    private let card = NSView()
    private let imageView = NSImageView()
    private let label = NSTextField(labelWithString: "")
    private let pad: CGFloat = 6

    init() {
        super.init(contentRect: NSRect(x: 0, y: 0, width: 200, height: 160), styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        isOpaque = false
        backgroundColor = .clear
        hasShadow = true
        ignoresMouseEvents = true
        isReleasedWhenClosed = false
        level = .floating
        collectionBehavior = [.transient, .ignoresCycle]
        card.wantsLayer = true
        card.layer?.cornerRadius = 10
        card.layer?.cornerCurve = .continuous
        card.layer?.borderWidth = 1
        card.layer?.masksToBounds = true
        imageView.imageScaling = .scaleProportionallyUpOrDown
        imageView.wantsLayer = true
        imageView.layer?.cornerRadius = 5
        imageView.layer?.masksToBounds = true
        label.font = .systemFont(ofSize: 11)
        label.lineBreakMode = .byTruncatingMiddle
        card.addSubview(imageView)
        card.addSubview(label)
        contentView = card
    }

    /// Lays the card out for an image size and returns the panel size.
    func layout(image: NSImage, imageSize: CGSize, caption: String) -> CGSize {
        card.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor
        card.layer?.borderColor = NSColor.separatorColor.cgColor
        label.textColor = .secondaryLabelColor
        label.stringValue = caption
        imageView.image = image
        let captionHeight: CGFloat = 18
        let size = CGSize(width: imageSize.width + pad * 2, height: imageSize.height + pad * 2 + captionHeight)
        label.frame = NSRect(x: pad + 2, y: pad - 1, width: size.width - pad * 2 - 4, height: captionHeight - 2)
        imageView.frame = NSRect(x: pad, y: pad + captionHeight, width: imageSize.width, height: imageSize.height)
        return size
    }
}
