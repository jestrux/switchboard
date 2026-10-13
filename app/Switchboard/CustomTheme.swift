import SwiftUI
import AppKit

/// A project's custom theme from `config.yaml`: a base theme plus token overrides per scheme.
struct CustomThemeSpec: Decodable {
    var name: String?
    var base: String?
    var dark: [String: TokenValue]?
    var light: [String: TokenValue]?

    enum TokenValue: Decodable {
        case color(String)
        case list([String])
        init(from decoder: Decoder) throws {
            let c = try decoder.singleValueContainer()
            if let s = try? c.decode(String.self) { self = .color(s) }
            else { self = .list(try c.decode([String].self)) }
        }
    }

    static func parse(_ json: String?) -> CustomThemeSpec? {
        guard let json, let data = json.data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(CustomThemeSpec.self, from: data)
    }

    /// The resolved theme: the base's style, each palette with these overrides applied.
    func resolve(baseFallback: SBThemeID) -> SBTheme {
        let baseID = SBThemeID.fromConfig(base) ?? baseFallback
        let b = baseID.theme
        return SBTheme(id: baseID,
                       name: name ?? b.name,
                       style: b.style,
                       dark: b.dark.applying(dark ?? [:]),
                       light: b.light.applying(light ?? [:]))
    }
}

extension Color {
    init?(hexString: String) {
        var s = hexString.trimmingCharacters(in: .whitespaces)
        guard s.hasPrefix("#") else { return nil }
        s.removeFirst()
        guard s.count == 6, let v = UInt32(s, radix: 16) else { return nil }
        self.init(hex: v)
    }
}

extension NSColor {
    convenience init?(hexString: String) {
        var s = hexString.trimmingCharacters(in: .whitespaces)
        guard s.hasPrefix("#") else { return nil }
        s.removeFirst()
        guard s.count == 6, let v = UInt32(s, radix: 16) else { return nil }
        self.init(hex: v)
    }
}

extension SBPalette {
    /// A copy with the named tokens replaced. Unknown keys and bad colours are ignored
    /// (the engine already validated them).
    func applying(_ overrides: [String: CustomThemeSpec.TokenValue]) -> SBPalette {
        func c(_ key: String, _ current: Color) -> Color {
            if case .color(let hex)? = overrides[key], let col = Color(hexString: hex) { return col }
            return current
        }
        var ansiOut = ansi
        if case .list(let list)? = overrides["ansi"], list.count == 16 {
            ansiOut = list.enumerated().map { NSColor(hexString: $1) ?? ansi[$0] }
        }
        return SBPalette(
            window: c("window", window), rail: c("rail", rail), sidebar: c("sidebar", sidebar), line: c("line", line),
            pane: c("pane", pane), header: c("header", header), terminal: c("terminal", terminal),
            terminalText: c("terminalText", terminalText), terminalDim: c("terminalDim", terminalDim),
            text: c("text", text), text2: c("text2", text2), text3: c("text3", text3), text4: c("text4", text4),
            hover: c("hover", hover), selection: c("selection", selection), selectionText: c("selectionText", selectionText),
            accent: c("accent", accent), onAccent: c("onAccent", onAccent), accentSoft: c("accentSoft", accentSoft),
            accentSoftText: c("accentSoftText", accentSoftText), button: c("button", button), buttonLine: c("buttonLine", buttonLine),
            running: c("running", running), runningSoft: c("runningSoft", runningSoft), starting: c("starting", starting),
            startingSoft: c("startingSoft", startingSoft), crashed: c("crashed", crashed), crashedSoft: c("crashedSoft", crashedSoft),
            error: c("error", error), errorSoft: c("errorSoft", errorSoft), stopped: c("stopped", stopped), stoppedSoft: c("stoppedSoft", stoppedSoft),
            ansi: ansiOut)
    }
}
