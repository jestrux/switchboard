// Theme.swift — Switchboard design tokens.
// Generated from the same table as DESIGN.md; edit the table, not this file by hand.
// Three themes (Graphite default, Patch Bay, Paper), each with a light and a dark palette.

import SwiftUI
import AppKit

// MARK: - Settings

enum SBThemeID: String, CaseIterable, Identifiable, Codable {
    case graphite, patchBay, paper
    var id: String { rawValue }
    var theme: SBTheme {
        switch self {
        case .graphite: return .graphite
        case .patchBay: return .patchBay
        case .paper: return .paper
        }
    }
}

enum SBAppearance: String, CaseIterable, Identifiable, Codable {
    case system, light, dark
    var id: String { rawValue }
    /// nil follows the system.
    var colorScheme: ColorScheme? {
        switch self {
        case .system: return nil
        case .light: return .light
        case .dark: return .dark
        }
    }
}

// MARK: - Palette

struct SBPalette {
    let window: Color
    let rail: Color
    let sidebar: Color
    let line: Color
    let pane: Color
    let header: Color
    let terminal: Color
    let terminalText: Color
    let terminalDim: Color
    let text: Color
    let text2: Color
    let text3: Color
    let text4: Color
    let hover: Color
    let selection: Color
    let selectionText: Color
    let accent: Color
    let onAccent: Color
    let accentSoft: Color
    let accentSoftText: Color
    let button: Color
    let buttonLine: Color
    let running: Color
    let runningSoft: Color
    let starting: Color
    let startingSoft: Color
    let crashed: Color
    let crashedSoft: Color
    let error: Color
    let errorSoft: Color
    let stopped: Color
    let stoppedSoft: Color
    /// 16-colour ANSI palette for SwiftTerm: 0–7 normal, 8–15 bright.
    let ansi: [NSColor]
}

// MARK: - Per-theme style flags

struct SBThemeStyle {
    enum Selection { case accentBar, card }
    let groupLabelMonospaced: Bool
    let groupLabelUppercase: Bool
    let groupLabelTracking: CGFloat
    let groupLabelSize: CGFloat
    let groupLabelWeight: Font.Weight
    let rowRadius: CGFloat
    let cardRadius: CGFloat
    let tileRadius: CGFloat
    /// Paper floats the pane area as a card; others are flush.
    let paneInset: CGFloat
    let paneRadius: CGFloat
    let selection: Selection
    /// Patch Bay lights its status glyphs (dark mode only).
    let glowInDark: Bool
    let dashedWarningBorder: Bool
}

struct SBTheme {
    let id: SBThemeID
    let name: String
    let style: SBThemeStyle
    let dark: SBPalette
    let light: SBPalette

    func palette(_ scheme: ColorScheme) -> SBPalette { scheme == .dark ? dark : light }
    func glows(_ scheme: ColorScheme) -> Bool { style.glowInDark && scheme == .dark }
}

extension SBTheme {
    static let graphite = SBTheme(
        id: .graphite,
        name: "Graphite",
        style: SBThemeStyle(
            groupLabelMonospaced: false, groupLabelUppercase: true,
            groupLabelTracking: 0.66, groupLabelSize: 11, groupLabelWeight: .semibold,
            rowRadius: 7, cardRadius: 10, tileRadius: 10,
            paneInset: 0, paneRadius: 0,
            selection: .accentBar, glowInDark: false, dashedWarningBorder: false
        ),
        dark: SBPalette(
            window: Color(hex: 0x0B0C0E),
            rail: Color(hex: 0x0B0C0E),
            sidebar: Color(hex: 0x0E0F11),
            line: Color(hex: 0x1E2025),
            pane: Color(hex: 0x0E0F11),
            header: Color(hex: 0x0E0F11),
            terminal: Color(hex: 0x111214),
            terminalText: Color(hex: 0xD5D7DC),
            terminalDim: Color(hex: 0x6B7079),
            text: Color(hex: 0xE6E7EA),
            text2: Color(hex: 0xA9ACB4),
            text3: Color(hex: 0x7D828B),
            text4: Color(hex: 0x4D525B),
            hover: Color(hex: 0x16171A),
            selection: Color(hex: 0x1D2233),
            selectionText: Color(hex: 0xFFFFFF),
            accent: Color(hex: 0x7C9CFF),
            onAccent: Color(hex: 0x0B0C0E),
            accentSoft: Color(hex: 0x1D2233),
            accentSoftText: Color(hex: 0xA3B8FF),
            button: Color(hex: 0x16171A),
            buttonLine: Color(hex: 0x2A2D33),
            running: Color(hex: 0x3DD68C),
            runningSoft: Color(hex: 0x12301F),
            starting: Color(hex: 0xF5B83D),
            startingSoft: Color(hex: 0x33280F),
            crashed: Color(hex: 0xFF5C5C),
            crashedSoft: Color(hex: 0x3A1A1A),
            error: Color(hex: 0xFF8A3D),
            errorSoft: Color(hex: 0x2A1A0F),
            stopped: Color(hex: 0x5A5F68),
            stoppedSoft: Color(hex: 0x2A2D33),
            ansi: ([0x1E2025, 0xFF5C5C, 0x3DD68C, 0xF5B83D, 0x7C9CFF, 0xC792EA, 0x5FD7E8, 0xC9CBD1, 0x4D525B, 0xFF7B7B, 0x6BE5A8, 0xFFD06B, 0xA3B8FF, 0xDDB3F5, 0x8AE6F2, 0xFFFFFF] as [UInt32]).map { NSColor(hex: $0) }
        ),
        light: SBPalette(
            window: Color(hex: 0xECEEF1),
            rail: Color(hex: 0xECEEF1),
            sidebar: Color(hex: 0xF4F5F7),
            line: Color(hex: 0xE1E3E8),
            pane: Color(hex: 0xFFFFFF),
            header: Color(hex: 0xFFFFFF),
            terminal: Color(hex: 0xFCFCFD),
            terminalText: Color(hex: 0x24262B),
            terminalDim: Color(hex: 0x8A9099),
            text: Color(hex: 0x17181B),
            text2: Color(hex: 0x4F545C),
            text3: Color(hex: 0x7D838C),
            text4: Color(hex: 0xB3B8C0),
            hover: Color(hex: 0xE9EBEF),
            selection: Color(hex: 0xE3E9FF),
            selectionText: Color(hex: 0x17181B),
            accent: Color(hex: 0x4A6CF7),
            onAccent: Color(hex: 0xFFFFFF),
            accentSoft: Color(hex: 0xE3E9FF),
            accentSoftText: Color(hex: 0x2F4FD6),
            button: Color(hex: 0xFFFFFF),
            buttonLine: Color(hex: 0xDADDE3),
            running: Color(hex: 0x14965A),
            runningSoft: Color(hex: 0xDDF4E8),
            starting: Color(hex: 0xB07800),
            startingSoft: Color(hex: 0xFBF0D6),
            crashed: Color(hex: 0xD93A3F),
            crashedSoft: Color(hex: 0xFCE3E3),
            error: Color(hex: 0xC9580B),
            errorSoft: Color(hex: 0xFDEEE2),
            stopped: Color(hex: 0xA8ADB6),
            stoppedSoft: Color(hex: 0xDADDE3),
            ansi: ([0x1A1B1E, 0xD93A3F, 0x128A4A, 0xA86F00, 0x3B5BDB, 0x8E44AD, 0x0E8399, 0xD9DCE1, 0x5F646D, 0xE5484D, 0x18A058, 0xC98A0B, 0x4A6CF7, 0xA660C8, 0x12A3BD, 0xF4F5F7] as [UInt32]).map { NSColor(hex: $0) }
        )
    )

    static let patchBay = SBTheme(
        id: .patchBay,
        name: "Patch Bay",
        style: SBThemeStyle(
            groupLabelMonospaced: true, groupLabelUppercase: true,
            groupLabelTracking: 1.05, groupLabelSize: 10.5, groupLabelWeight: .medium,
            rowRadius: 6, cardRadius: 12, tileRadius: 9,
            paneInset: 0, paneRadius: 0,
            selection: .accentBar, glowInDark: true, dashedWarningBorder: true
        ),
        dark: SBPalette(
            window: Color(hex: 0x12100D),
            rail: Color(hex: 0x12100D),
            sidebar: Color(hex: 0x181511),
            line: Color(hex: 0x2A251E),
            pane: Color(hex: 0x1C1813),
            header: Color(hex: 0x1C1813),
            terminal: Color(hex: 0x0F0D0B),
            terminalText: Color(hex: 0xD8D0C2),
            terminalDim: Color(hex: 0x7A705F),
            text: Color(hex: 0xEDE6DA),
            text2: Color(hex: 0xA99F8E),
            text3: Color(hex: 0x8A7F6C),
            text4: Color(hex: 0x5A5246),
            hover: Color(hex: 0x221E18),
            selection: Color(hex: 0x2E1F17),
            selectionText: Color(hex: 0xFFF8EC),
            accent: Color(hex: 0xFFB224),
            onAccent: Color(hex: 0x1A1208),
            accentSoft: Color(hex: 0x33250E),
            accentSoftText: Color(hex: 0xFFCB66),
            button: Color(hex: 0x1F1B16),
            buttonLine: Color(hex: 0x3A332A),
            running: Color(hex: 0x9ACD6B),
            runningSoft: Color(hex: 0x22301A),
            starting: Color(hex: 0xFFB224),
            startingSoft: Color(hex: 0x33250E),
            crashed: Color(hex: 0xFF6B5A),
            crashedSoft: Color(hex: 0x3A1E18),
            error: Color(hex: 0xFF8A3D),
            errorSoft: Color(hex: 0x2C1C0E),
            stopped: Color(hex: 0x5A5246),
            stoppedSoft: Color(hex: 0x3A332A),
            ansi: ([0x2A251E, 0xFF6B5A, 0x9ACD6B, 0xFFB224, 0x7FA7D9, 0xD08BB8, 0x6CC4B0, 0xD8D0C2, 0x5A5246, 0xFF8C7D, 0xB5E08C, 0xFFCB66, 0xA3C2E8, 0xE3A9CE, 0x92DCC9, 0xFFF8EC] as [UInt32]).map { NSColor(hex: $0) }
        ),
        light: SBPalette(
            window: Color(hex: 0xE6DECF),
            rail: Color(hex: 0xE6DECF),
            sidebar: Color(hex: 0xEFE8DB),
            line: Color(hex: 0xDDD2BF),
            pane: Color(hex: 0xF7F2E9),
            header: Color(hex: 0xF7F2E9),
            terminal: Color(hex: 0xFBF8F2),
            terminalText: Color(hex: 0x2B241A),
            terminalDim: Color(hex: 0x8F836E),
            text: Color(hex: 0x241E16),
            text2: Color(hex: 0x5F5545),
            text3: Color(hex: 0x857965),
            text4: Color(hex: 0xB8AC96),
            hover: Color(hex: 0xE6DCCB),
            selection: Color(hex: 0xF5DDB0),
            selectionText: Color(hex: 0x241E16),
            accent: Color(hex: 0xC47A00),
            onAccent: Color(hex: 0x1A1208),
            accentSoft: Color(hex: 0xF5DDB0),
            accentSoftText: Color(hex: 0x7A4C00),
            button: Color(hex: 0xFBF8F2),
            buttonLine: Color(hex: 0xD6CAB4),
            running: Color(hex: 0x4E8A1A),
            runningSoft: Color(hex: 0xE1EDCF),
            starting: Color(hex: 0xA86800),
            startingSoft: Color(hex: 0xF7E4BE),
            crashed: Color(hex: 0xC2412E),
            crashedSoft: Color(hex: 0xF6DDD6),
            error: Color(hex: 0xB8560B),
            errorSoft: Color(hex: 0xF8E6D3),
            stopped: Color(hex: 0xA89B84),
            stoppedSoft: Color(hex: 0xD6CAB4),
            ansi: ([0x2B241A, 0xC2412E, 0x4E8A1A, 0xA86800, 0x2F64A8, 0xA0457D, 0x1F8573, 0xE0D6C4, 0x6F6453, 0xD2402E, 0x5FA02A, 0xC27F00, 0x3F78C2, 0xB85A93, 0x2A9C88, 0xFBF8F2] as [UInt32]).map { NSColor(hex: $0) }
        )
    )

    static let paper = SBTheme(
        id: .paper,
        name: "Paper",
        style: SBThemeStyle(
            groupLabelMonospaced: false, groupLabelUppercase: false,
            groupLabelTracking: 0, groupLabelSize: 11.5, groupLabelWeight: .semibold,
            rowRadius: 8, cardRadius: 14, tileRadius: 11,
            paneInset: 10, paneRadius: 14,
            selection: .card, glowInDark: false, dashedWarningBorder: false
        ),
        dark: SBPalette(
            window: Color(hex: 0x161618),
            rail: Color(hex: 0x161618),
            sidebar: Color(hex: 0x161618),
            line: Color(hex: 0x2C2C30),
            pane: Color(hex: 0x1F1F22),
            header: Color(hex: 0x1F1F22),
            terminal: Color(hex: 0x1B1B1E),
            terminalText: Color(hex: 0xE2E1DD),
            terminalDim: Color(hex: 0x7C7B77),
            text: Color(hex: 0xEDEDEB),
            text2: Color(hex: 0xA3A29E),
            text3: Color(hex: 0x85847F),
            text4: Color(hex: 0x55544F),
            hover: Color(hex: 0x232326),
            selection: Color(hex: 0x2A2A2F),
            selectionText: Color(hex: 0xFFFFFF),
            accent: Color(hex: 0x8B8BFF),
            onAccent: Color(hex: 0x121214),
            accentSoft: Color(hex: 0x26264A),
            accentSoftText: Color(hex: 0xB4B4FF),
            button: Color(hex: 0x262629),
            buttonLine: Color(hex: 0x35353A),
            running: Color(hex: 0x3FCB7E),
            runningSoft: Color(hex: 0x16301F),
            starting: Color(hex: 0xF2B33D),
            startingSoft: Color(hex: 0x33280F),
            crashed: Color(hex: 0xFF6B6B),
            crashedSoft: Color(hex: 0x3A1C1C),
            error: Color(hex: 0xFF9447),
            errorSoft: Color(hex: 0x2E1D10),
            stopped: Color(hex: 0x5F5E5A),
            stoppedSoft: Color(hex: 0x35353A),
            ansi: ([0x2A2A2D, 0xFF6B6B, 0x3FCB7E, 0xF2B33D, 0x8B8BFF, 0xC99BFF, 0x5CC9D6, 0xD6D5D1, 0x6E6D69, 0xFF8A8A, 0x6BDDA0, 0xFFCC6B, 0xA9A9FF, 0xDAB8FF, 0x86DCE6, 0xFFFFFF] as [UInt32]).map { NSColor(hex: $0) }
        ),
        light: SBPalette(
            window: Color(hex: 0xEFEEEA),
            rail: Color(hex: 0xEFEEEA),
            sidebar: Color(hex: 0xEFEEEA),
            line: Color(hex: 0xE2E0DA),
            pane: Color(hex: 0xFFFFFF),
            header: Color(hex: 0xFFFFFF),
            terminal: Color(hex: 0xFBFBF9),
            terminalText: Color(hex: 0x1F1F21),
            terminalDim: Color(hex: 0x8C8A84),
            text: Color(hex: 0x1A1A1C),
            text2: Color(hex: 0x4A4946),
            text3: Color(hex: 0x7A7873),
            text4: Color(hex: 0xB8B6B0),
            hover: Color(hex: 0xE6E4DF),
            selection: Color(hex: 0xFFFFFF),
            selectionText: Color(hex: 0x1A1A1C),
            accent: Color(hex: 0x3B3BD9),
            onAccent: Color(hex: 0xFFFFFF),
            accentSoft: Color(hex: 0xE7E7FB),
            accentSoftText: Color(hex: 0x3030B8),
            button: Color(hex: 0xFFFFFF),
            buttonLine: Color(hex: 0xE2E0DA),
            running: Color(hex: 0x1F8A4C),
            runningSoft: Color(hex: 0xDFF2E6),
            starting: Color(hex: 0xA6720A),
            startingSoft: Color(hex: 0xF8EDD4),
            crashed: Color(hex: 0xD13434),
            crashedSoft: Color(hex: 0xFBE2E2),
            error: Color(hex: 0xB9520E),
            errorSoft: Color(hex: 0xFCEDE1),
            stopped: Color(hex: 0xA5A39D),
            stoppedSoft: Color(hex: 0xD6D4CE),
            ansi: ([0x1A1A1C, 0xCC2F2F, 0x1F8A4C, 0x9A6A00, 0x3B3BD9, 0x8B3FC1, 0x0F7F8F, 0xD6D4CE, 0x6E6D69, 0xD93A3A, 0x1F9D55, 0xB07800, 0x5555E8, 0x9F55D6, 0x14A0B3, 0xFBFBF9] as [UInt32]).map { NSColor(hex: $0) }
        )
    )
}

// MARK: - Metrics (points)

enum SBMetrics {
    static let spacing: [CGFloat] = [2, 4, 6, 8, 10, 12, 14, 16, 20, 24]

    static let railWidth: CGFloat = 72
    static let railTile: CGFloat = 40
    static let sidebarWidthRail: CGFloat = 236
    static let sidebarWidthTree: CGFloat = 272
    static let sidebarRange: ClosedRange<CGFloat> = 220...420

    static let projectRow: CGFloat = 34
    static let groupRow: CGFloat = 28
    static let commandRow: CGFloat = 30
    /// Puts a command's status dot directly under its group's icon.
    static let commandIndent: CGFloat = 12
    static let statusGlyph: CGFloat = 12       // drawn in an 18 pt box

    static let paneBar: CGFloat = 36
    static let paneFooter: CGFloat = 22
    static let terminalInset = EdgeInsets(top: 10, leading: 14, bottom: 10, trailing: 14)
    static let paneFullWidth: CGFloat = 560    // ≥ full bar
    static let paneMidWidth: CGFloat = 360     // ≥ mid bar, below = min
    static let paneMinimum = CGSize(width: 320, height: 180)
    static let maxPanes = 4
    static let dividerHit: CGFloat = 7

    static let projectHeaderIcon: CGFloat = 36
    static let dialogWidth: CGFloat = 460
    static let dialogRadius: CGFloat = 14
    static let buttonHeight: CGFloat = 30
    static let buttonRadius: CGFloat = 8
    static let chipHeight: CGFloat = 20
    static let windowMinimum = CGSize(width: 820, height: 480)
}

// MARK: - Type (SF Pro / SF Mono)

enum SBType {
    static let sidebarTitle = Font.system(size: 17, weight: .semibold)
    static let projectWindowTitle = Font.system(size: 20, weight: .semibold)
    static let paneTitle = Font.system(size: 13, weight: .semibold)
    static let body = Font.system(size: 13)
    static let secondary = Font.system(size: 12)
    static let dialogTitle = Font.system(size: 15, weight: .semibold)
    static let meta = Font.system(size: 11, design: .monospaced)          // uptime, pid; use .monospacedDigit()
    static let footer = Font.system(size: 10.5, design: .monospaced)
    static let terminal = NSFont.monospacedSystemFont(ofSize: 12.5, weight: .regular)

    static func groupLabel(_ s: SBThemeStyle) -> Font {
        .system(size: s.groupLabelSize, weight: s.groupLabelWeight, design: s.groupLabelMonospaced ? .monospaced : .default)
    }
}

// MARK: - Motion

enum SBMotion {
    static let runningBreathe: Double = 2.4      // halo opacity 0.22 ↔ 0.10
    static let startingSpin: Double = 0.9        // one turn
    static let extractLift: Double = 0.18        // tile scale 1 → 1.06, fade out
    static let railSettle = Animation.spring(response: 0.22, dampingFraction: 0.9)
    static let returnHalo: Double = 0.6
    static let reduceMotionFade: Double = 0.12
    // Backoff ring/bar drains linearly over the engine-reported delay.
}

// MARK: - Status → colour

enum SBStatus { case stopped, starting, running, done, crashed, error }

extension SBPalette {
    func color(_ s: SBStatus) -> Color {
        switch s {
        case .stopped: return stopped
        case .starting: return starting
        case .running: return running
        case .done: return text2
        case .crashed: return crashed
        case .error: return error
        }
    }
    func softColor(_ s: SBStatus) -> Color {
        switch s {
        case .stopped, .done: return hover
        case .starting: return startingSoft
        case .running: return runningSoft
        case .crashed: return crashedSoft
        case .error: return errorSoft
        }
    }
}

// MARK: - Environment

private struct SBThemeKey: EnvironmentKey { static let defaultValue: SBTheme = .graphite }
extension EnvironmentValues {
    var sbTheme: SBTheme {
        get { self[SBThemeKey.self] }
        set { self[SBThemeKey.self] = newValue }
    }
}

// MARK: - Hex helpers

extension Color {
    init(hex: UInt32) {
        self.init(.sRGB,
                  red: Double((hex >> 16) & 0xFF) / 255,
                  green: Double((hex >> 8) & 0xFF) / 255,
                  blue: Double(hex & 0xFF) / 255,
                  opacity: 1)
    }
}

extension NSColor {
    convenience init(hex: UInt32) {
        self.init(srgbRed: CGFloat((hex >> 16) & 0xFF) / 255,
                  green: CGFloat((hex >> 8) & 0xFF) / 255,
                  blue: CGFloat(hex & 0xFF) / 255,
                  alpha: 1)
    }
}
