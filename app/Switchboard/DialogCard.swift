import SwiftUI

/// A request for one of the app's dialogs, answered with the index of the chosen button.
struct DialogRequest: Identifiable {
    enum Kind { case invalidFolder, nameConflict, deleteProject, leftoverSessions }
    let id = UUID()
    let kind: Kind
    let title: String
    let subtitle: String
    /// Monospace or plain facts shown in the sidebar-coloured box; `(icon, text, tint)`.
    var facts: [DialogFact] = []
    var note: String? = nil
    /// Left-aligned secondary action (e.g. Reveal in Finder).
    var alt: DialogButton? = nil
    var buttons: [DialogButton]
    /// Window that should present it; nil = the main window.
    var window: UUID? = nil
}

struct DialogFact: Identifiable {
    let id = UUID()
    var icon: String? = nil
    var text: String
    var detail: String? = nil
    var tint: DialogTint = .normal
    var mono = false
}

enum DialogTint { case normal, error, crashed, running }

struct DialogButton: Identifiable {
    enum Style { case primary, secondary, defaultSecondary, danger, dangerOutline }
    let id = UUID()
    let label: String
    var kbd: String = ""
    var style: Style = .secondary
    /// Index reported back when chosen.
    let value: Int
}

/// Presents dialog requests and resumes the awaiting caller with the chosen button value.
@MainActor
final class DialogCenter: ObservableObject {
    static let shared = DialogCenter()
    @Published var current: DialogRequest?
    private var continuation: CheckedContinuation<Int, Never>?

    func ask(_ request: DialogRequest) async -> Int {
        if let c = continuation { c.resume(returning: -1); continuation = nil }
        current = request
        return await withCheckedContinuation { continuation = $0 }
    }

    func answer(_ value: Int) {
        current = nil
        continuation?.resume(returning: value)
        continuation = nil
    }
}

/// The designed dialog card: 460 pt, app icon, title, consequence line, facts box, buttons.
struct DialogCardView: View {
    let request: DialogRequest
    let answer: (Int) -> Void
    @Environment(\.sbTheme) private var theme
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let p = theme.palette(scheme)
        VStack(alignment: .leading, spacing: 0) {
            Image(scheme == .dark ? "DialogIconDark" : "DialogIconLight")
                .resizable().interpolation(.high)
                .frame(width: 48, height: 48)
                .clipShape(RoundedRectangle(cornerRadius: 11, style: .continuous))
            Text(request.title).font(SBType.dialogTitle).foregroundStyle(p.text).padding(.top, 14)
            Text(request.subtitle).font(SBType.secondary).foregroundStyle(p.text2).padding(.top, 4)
                .fixedSize(horizontal: false, vertical: true)
            if !request.facts.isEmpty {
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(request.facts) { f in
                        HStack(alignment: .top, spacing: 8) {
                            if let icon = f.icon {
                                Image(systemName: icon).font(.system(size: 11)).foregroundStyle(tint(f.tint, p)).frame(width: 14)
                            }
                            VStack(alignment: .leading, spacing: 2) {
                                Text(f.text)
                                    .font(f.mono ? SBType.meta : SBType.secondary)
                                    .foregroundStyle(f.tint == .normal ? p.text : tint(f.tint, p))
                                if let d = f.detail {
                                    Text(d).font(SBType.secondary).foregroundStyle(p.text3)
                                }
                            }
                        }
                    }
                }
                .padding(12)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(p.sidebar, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                .hairline(p.line, radius: 8)
                .padding(.top, 14)
            }
            if let note = request.note {
                Text(note).font(SBType.secondary).foregroundStyle(p.text3).padding(.top, 10)
                    .fixedSize(horizontal: false, vertical: true)
            }
            HStack(spacing: 8) {
                if let alt = request.alt { dialogButton(alt, p) }
                Spacer()
                ForEach(request.buttons) { b in dialogButton(b, p) }
            }
            .padding(.top, 20)
        }
        .padding(22)
        .frame(width: SBMetrics.dialogWidth)
        .background(p.pane, in: RoundedRectangle(cornerRadius: SBMetrics.dialogRadius, style: .continuous))
        .hairline(p.line, radius: SBMetrics.dialogRadius)
        .background {
            // Esc = cancel (first non-destructive secondary), Return = the primary / default button.
            Button("") { answer(request.buttons.first { $0.style == .secondary || $0.style == .defaultSecondary }?.value ?? -1) }
                .keyboardShortcut(.cancelAction).hidden()
            Button("") { answer(request.buttons.first { $0.style == .primary || $0.style == .defaultSecondary }?.value ?? -1) }
                .keyboardShortcut(.defaultAction).hidden()
        }
    }

    private func tint(_ t: DialogTint, _ p: SBPalette) -> Color {
        switch t { case .normal: return p.text2; case .error: return p.error; case .crashed: return p.crashed; case .running: return p.running }
    }

    private func dialogButton(_ b: DialogButton, _ p: SBPalette) -> some View {
        let (bg, fg, border): (Color, Color, Color) = {
            switch b.style {
            case .primary: return (p.accent, p.onAccent, p.accent)
            case .danger: return (p.crashed, .white, p.crashed)
            case .dangerOutline: return (p.button, p.crashed, p.crashed)
            case .defaultSecondary: return (p.button, p.text, p.accent)
            case .secondary: return (p.button, p.text, p.buttonLine)
            }
        }()
        return Button { answer(b.value) } label: {
            HStack(spacing: 6) {
                Text(b.label).font(.system(size: 12, weight: b.style == .primary || b.style == .danger ? .semibold : .regular))
                if !b.kbd.isEmpty { Text(b.kbd).font(.system(size: 10)).opacity(0.6) }
            }
            .foregroundStyle(fg)
            .padding(.horizontal, 12)
            .frame(height: SBMetrics.buttonHeight)
            .background(bg, in: RoundedRectangle(cornerRadius: SBMetrics.buttonRadius, style: .continuous))
            .overlay { RoundedRectangle(cornerRadius: SBMetrics.buttonRadius, style: .continuous).strokeBorder(border) }
        }
        .buttonStyle(.plain)
    }
}
