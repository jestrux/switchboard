import SwiftUI

/// App-wide appearance settings (Settings › Appearance). Per-project overrides from
/// `config.yaml` win inside that project's own window; see `WindowContext.resolveLook`.
enum ThemeSettings {
    static let themeKey = "appearance.theme"
    static let appearanceKey = "appearance.mode"
    static let railKey = "appearance.rail"
}

/// The look a window renders with: theme plus a resolved colour scheme.
struct WindowLook: Equatable {
    var themeID: SBThemeID
    var appearance: SBAppearance
    /// True when the project's config pinned the theme, false when the global setting applies.
    var pinnedTheme: Bool
    var pinnedAppearance: Bool

    var theme: SBTheme { themeID.theme }
}

extension SBThemeID {
    /// `config.yaml` spelling (`patch-bay`) ↔ enum case.
    static func fromConfig(_ raw: String?) -> SBThemeID? {
        switch raw {
        case "graphite": return .graphite
        case "patch-bay", "patchbay", "patchBay": return .patchBay
        case "paper": return .paper
        default: return nil
        }
    }
}

extension SBAppearance {
    static func fromConfig(_ raw: String?) -> SBAppearance? {
        raw.flatMap(SBAppearance.init(rawValue:))
    }
}

extension EnvironmentValues {
    /// The current theme's palette for the view's colour scheme.
    var sbPalette: SBPalette { sbTheme.palette(colorScheme) }
    var sbGlows: Bool { sbTheme.glows(colorScheme) }
}

/// Convenience for views: `@Environment(\.sbTheme)`, palette and style in one place.
struct Look {
    let theme: SBTheme
    let scheme: ColorScheme
    var p: SBPalette { theme.palette(scheme) }
    var s: SBThemeStyle { theme.style }
    var glows: Bool { theme.glows(scheme) }
    var isDark: Bool { scheme == .dark }
}

extension View {
    /// Hairline border in the palette's `line` colour.
    func hairline(_ color: Color, radius: CGFloat = 0, dashed: Bool = false) -> some View {
        overlay {
            RoundedRectangle(cornerRadius: radius, style: .continuous)
                .strokeBorder(color, style: StrokeStyle(lineWidth: 1, dash: dashed ? [4, 3] : []))
        }
    }
}

/// Formats durations and times the way the design writes them.
enum Fmt {
    static func uptime(since startedAt: Double, now: Date = Date()) -> String {
        duration(max(0, now.timeIntervalSince1970 - startedAt))
    }

    static func duration(_ secs: TimeInterval) -> String {
        let s = Int(secs)
        let h = s / 3600, m = (s % 3600) / 60, sec = s % 60
        return h > 0 ? String(format: "%d:%02d:%02d", h, m, sec) : String(format: "%d:%02d", m, sec)
    }

    static func clock(_ epoch: Double) -> String {
        let f = DateFormatter()
        f.dateFormat = "HH:mm:ss"
        return f.string(from: Date(timeIntervalSince1970: epoch))
    }

    /// `~/…/citybuild/frontend`: keep the last two path components.
    static func shortFolder(_ path: String?) -> String {
        guard let path else { return "" }
        let parts = path.split(separator: "/")
        if parts.count <= 3 { return path }
        return "~/…/" + parts.suffix(2).joined(separator: "/")
    }
}
