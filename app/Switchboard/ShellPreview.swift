import SwiftUI

/// A static miniature of the shell (rail, sidebar, one pane) drawn with a theme and
/// scheme, for Settings › Appearance. Content is illustrative, not live.
struct ShellPreview: View {
    let theme: SBTheme
    let scheme: ColorScheme
    let rail: Bool

    private var p: SBPalette { theme.palette(scheme) }
    private var s: SBThemeStyle { theme.style }
    private let w: CGFloat = 1120
    private let h: CGFloat = 640

    var body: some View {
        HStack(spacing: 0) {
            if rail {
                railColumn
                Rectangle().fill(p.line).frame(width: 1)
            }
            sidebar
            Rectangle().fill(s.paneInset > 0 ? .clear : p.line).frame(width: 1)
            paneArea
        }
        .frame(width: w, height: h)
        .background(p.window)
        .environment(\.sbTheme, theme)
        .environment(\.colorScheme, scheme)
        .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay { RoundedRectangle(cornerRadius: 10, style: .continuous).strokeBorder(p.line) }
        .allowsHitTesting(false)
    }

    private var railColumn: some View {
        VStack(spacing: 14) {
            HStack(spacing: 6) {
                ForEach([0xFF5F57, 0xFEBC2E, 0x28C840], id: \.self) { c in Circle().fill(Color(hex: UInt32(c))).frame(width: 11, height: 11) }
            }
            .padding(.bottom, 3)
            tile("CB", bg: Color(hex: 0xF2EEE6), fg: Color(hex: 0xD9541F), badge: "2", selected: true)
            tile("K", bg: Color(hex: 0xD7F25A), fg: Color(hex: 0x7A2BD6), badge: nil, selected: false)
            Rectangle().fill(p.line).frame(width: 24, height: 1)
            RoundedRectangle(cornerRadius: s.tileRadius).strokeBorder(p.buttonLine, style: StrokeStyle(lineWidth: 1, dash: [3, 3]))
                .frame(width: 40, height: 40)
                .overlay { Image(systemName: "plus").font(.system(size: 13, weight: .semibold)).foregroundStyle(p.text2) }
            Spacer()
        }
        .padding(.top, 18)
        .frame(width: SBMetrics.railWidth)
        .background(p.rail)
    }

    private func tile(_ initials: String, bg: Color, fg: Color, badge: String?, selected: Bool) -> some View {
        ZStack(alignment: .bottomTrailing) {
            RoundedRectangle(cornerRadius: s.tileRadius, style: .continuous).fill(bg)
                .overlay { Text(initials).font(.system(size: 13, weight: .bold)).foregroundStyle(fg) }
                .frame(width: 40, height: 40)
            if let badge {
                Text(badge).font(.system(size: 10, weight: .bold)).foregroundStyle(scheme == .dark ? Color(hex: 0x0B0C0E) : .white)
                    .frame(minWidth: 17, minHeight: 17).background(p.running, in: Capsule())
                    .overlay(Capsule().strokeBorder(p.rail, lineWidth: 2)).offset(x: 5, y: 5)
            }
        }
        .padding(4)
        .background { if selected { RoundedRectangle(cornerRadius: s.tileRadius + 4, style: .continuous).strokeBorder(p.accent, lineWidth: 2) } }
    }

    private var sidebar: some View {
        VStack(alignment: .leading, spacing: 0) {
            if rail {
                VStack(alignment: .leading, spacing: 4) {
                    HStack {
                        Text("CityBuild").font(SBType.sidebarTitle).foregroundStyle(p.text)
                        Spacer()
                        miniButton("Start all", icon: "play.fill")
                    }
                    HStack(spacing: 4) {
                        Text("2 of 9 running").foregroundStyle(p.text3)
                        Text("·").foregroundStyle(p.text4)
                        Text("1 crashed").foregroundStyle(p.crashed)
                    }
                    .font(SBType.secondary)
                }
                .padding(EdgeInsets(top: 44, leading: 14, bottom: 12, trailing: 14))
            } else {
                HStack(spacing: 6) {
                    ForEach([0xFF5F57, 0xFEBC2E, 0x28C840], id: \.self) { c in Circle().fill(Color(hex: UInt32(c))).frame(width: 12, height: 12) }
                    Spacer()
                }
                .padding(.horizontal, 14).frame(height: 52)
                HStack(spacing: 9) {
                    RoundedRectangle(cornerRadius: 6).fill(Color(hex: 0xF2EEE6)).frame(width: 22, height: 22)
                        .overlay { Text("CB").font(.system(size: 9, weight: .bold)).foregroundStyle(Color(hex: 0xD9541F)) }
                    Text("CityBuild").font(.system(size: 13, weight: .semibold)).foregroundStyle(p.text)
                    Spacer()
                    HStack(spacing: 5) { Circle().fill(p.running).frame(width: 6, height: 6); Text("2").font(SBType.meta).foregroundStyle(p.text2) }
                }
                .padding(.horizontal, 18).frame(height: 34)
            }
            VStack(alignment: .leading, spacing: 1) {
                row("Claude Code", status: .stopped, meta: "", indent: 0)
                group("Frontend")
                row("dev", status: .running, meta: ":3006", indent: 22, selected: true)
                row("Storybook", status: .running, meta: ":6006", indent: 22)
                row("typecheck", status: .exited, meta: "exit 0", indent: 22)
                row("test", status: .stopped, meta: "", indent: 22)
                group("Backend")
                row("make:dev", status: .crashed, meta: "↻ 16s", indent: 22)
                row("make:migrate", status: .stopped, meta: "", indent: 22)
                row("make:seed", status: .error, meta: "no folder", indent: 22)
                group("E2E", collapsed: true)
            }
            .padding(.horizontal, 8)
            Spacer()
            Rectangle().fill(p.line).frame(height: 1)
            HStack(spacing: 6) {
                Circle().fill(p.running).frame(width: 6, height: 6)
                Text("3 running").foregroundStyle(p.text2)
                Text("·").foregroundStyle(p.text4)
                Text("1 crashed").foregroundStyle(p.crashed)
                Spacer()
                Text("Stop all").foregroundStyle(p.text3)
            }
            .font(SBType.secondary).padding(.horizontal, 14).frame(height: 30)
        }
        .frame(width: rail ? SBMetrics.sidebarWidthRail : SBMetrics.sidebarWidthTree)
        .background(p.sidebar)
    }

    private func group(_ name: String, collapsed: Bool = false) -> some View {
        HStack(spacing: 6) {
            Image(systemName: collapsed ? "chevron.right" : "chevron.down").font(.system(size: 8, weight: .bold)).foregroundStyle(p.text4).frame(width: 10)
            Text(s.groupLabelUppercase ? name.uppercased() : name).font(SBType.groupLabel(s)).tracking(s.groupLabelTracking).foregroundStyle(p.text3)
            Spacer()
            if collapsed { Text("3 idle").font(SBType.meta).foregroundStyle(p.text3) }
        }
        .padding(.horizontal, 10).frame(height: SBMetrics.groupRow)
    }

    private func row(_ name: String, status: CommandStatus, meta: String, indent: CGFloat, selected: Bool = false) -> some View {
        let metaColour: Color = { switch status { case .crashed: return p.crashed; case .error: return p.error; case .starting: return p.starting; default: return p.text3 } }()
        return HStack(spacing: 8) {
            StatusGlyph(status: status)
            Text(name).font(.system(size: 13, weight: selected ? .semibold : .regular))
                .foregroundStyle(selected ? p.selectionText : (status == .stopped ? p.text2 : p.text))
            Spacer()
            Text(meta).font(SBType.meta).foregroundStyle(metaColour)
        }
        .padding(.leading, indent + 6).padding(.trailing, 8)
        .frame(height: SBMetrics.commandRow)
        .background {
            if selected {
                RoundedRectangle(cornerRadius: s.rowRadius, style: .continuous).fill(p.selection)
                    .overlay(alignment: .leading) {
                        if s.selection == .accentBar { Rectangle().fill(p.accent).frame(width: 2).padding(.vertical, 6).padding(.leading, 2) }
                    }
                    .overlay { if s.selection == .card { RoundedRectangle(cornerRadius: s.rowRadius, style: .continuous).strokeBorder(p.line) } }
            }
        }
    }

    private func miniButton(_ label: String, icon: String) -> some View {
        HStack(spacing: 5) {
            Image(systemName: icon).font(.system(size: 8, weight: .bold))
            Text(label).font(.system(size: 12))
        }
        .foregroundStyle(p.text).padding(.horizontal, 9).frame(height: 26)
        .background(p.button, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
        .overlay { RoundedRectangle(cornerRadius: 7, style: .continuous).strokeBorder(p.buttonLine) }
    }

    private var paneArea: some View {
        VStack(spacing: 0) {
            HStack(spacing: 8) {
                StatusGlyph(status: .running)
                (Text("Frontend / ").foregroundStyle(p.text3) + Text("dev").foregroundStyle(p.text))
                    .font(.system(size: 13, weight: .semibold))
                Image(systemName: "chevron.down").font(.system(size: 8, weight: .bold)).foregroundStyle(p.text3)
                Text("Running").font(SBType.secondary).foregroundStyle(p.text3)
                Text(":3006").font(SBType.meta).foregroundStyle(p.text3)
                Spacer()
                ForEach(["stop.fill", "arrow.clockwise", "rectangle.split.2x1", "arrow.up.left.and.arrow.down.right", "xmark"], id: \.self) { i in
                    Image(systemName: i).font(.system(size: 10, weight: .semibold)).foregroundStyle(p.text2).frame(width: 24, height: 24)
                }
            }
            .padding(.horizontal, 10).frame(height: SBMetrics.paneBar)
            .background(p.button)
            .overlay(alignment: .top) { Rectangle().fill(p.accent).frame(height: 2) }
            .overlay(alignment: .bottom) { Rectangle().fill(p.line).frame(height: 1) }
            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 8) {
                    Text("── started 10:14:04 ──").foregroundStyle(p.text4)
                }
                termLine([("> ", p.terminalDim), ("dev", p.terminalText)])
                termLine([("> ", p.terminalDim), ("react-router dev --port 3006", p.terminalText)])
                Text(" ")
                termLine([("  ➜  ", Color(nsColor: p.ansi[2])), ("Local:   ", p.terminalText), ("http://localhost:3006/", Color(nsColor: p.ansi[6]))])
                termLine([("  ➜  ", Color(nsColor: p.ansi[2])), ("Network: use ", p.terminalDim), ("--host", p.terminalText), (" to expose", p.terminalDim)])
                termLine([("  ➜  ", Color(nsColor: p.ansi[2])), ("press ", p.terminalDim), ("h + enter", p.terminalText), (" to show help", p.terminalDim)])
                termLine([("10:16:12 ", p.terminalDim), ("[vite] hmr update ", Color(nsColor: p.ansi[2])), ("/app/routes/home.tsx", p.terminalDim)])
                Rectangle().fill(p.accent).frame(width: 7, height: 14)
                Spacer()
            }
            .font(.system(size: 12.5, design: .monospaced))
            .padding(SBMetrics.terminalInset)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .background(p.terminal)
            HStack(spacing: 14) {
                Text("~/…/citybuild/frontend").foregroundStyle(p.text3)
                HStack(spacing: 4) { Text("pid").foregroundStyle(p.text3); Text("6964").foregroundStyle(p.text2) }
                HStack(spacing: 4) { Text("up").foregroundStyle(p.text3); Text("1:59").foregroundStyle(p.text2) }
                Spacer()
                Text("132×38").foregroundStyle(p.text3)
            }
            .font(SBType.footer).padding(.horizontal, 14).frame(height: SBMetrics.paneFooter)
            .background(p.pane)
            .overlay(alignment: .top) { Rectangle().fill(p.line).frame(height: 1) }
        }
        .background(p.pane)
        .clipShape(RoundedRectangle(cornerRadius: s.paneRadius, style: .continuous))
        .overlay { if s.paneInset > 0 { RoundedRectangle(cornerRadius: s.paneRadius, style: .continuous).strokeBorder(p.line) } }
        .shadow(color: s.paneInset > 0 ? .black.opacity(scheme == .dark ? 0.3 : 0.05) : .clear, radius: 12, y: 6)
        .padding(s.paneInset)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(p.window)
    }

    private func termLine(_ parts: [(String, Color)]) -> Text {
        parts.reduce(Text("")) { $0 + Text($1.0).foregroundStyle($1.1) }
    }
}
