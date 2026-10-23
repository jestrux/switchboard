import ServiceManagement
import SwiftUI

/// Settings › Appearance: theme, appearance, project rail.
struct SettingsView: View {
    @AppStorage(ThemeSettings.themeKey) private var themeID: SBThemeID = .graphite
    @AppStorage(ThemeSettings.appearanceKey) private var appearance: SBAppearance = .system
    @AppStorage(ThemeSettings.railKey) private var rail = true
    @AppStorage(Desk.switcherKey) private var productSwitcher = "tabs"
    @State private var openAtLogin = LoginItem.isOn
    @State private var loginProblem: String?
    @Environment(\.colorScheme) private var scheme

    private var look: Look { Look(theme: themeID.theme, scheme: scheme) }

    var body: some View {
        let p = look.p
        VStack(alignment: .leading, spacing: 0) {
            Text("Appearance").font(.system(size: 20, weight: .semibold)).foregroundStyle(p.text)
            Text("Changes apply instantly. Running commands are not affected.")
                .font(SBType.secondary).foregroundStyle(p.text3).padding(.top, 4)

            section("Theme", p)
            HStack(spacing: 12) {
                ForEach(SBThemeID.allCases) { id in
                    ThemeCard(id: id, selected: themeID == id, scheme: scheme) { themeID = id }
                }
            }

            section("Appearance", p)
            HStack(spacing: 2) {
                ForEach(SBAppearance.allCases) { mode in
                    let on = appearance == mode
                    Button { appearance = mode } label: {
                        Text(mode.rawValue.capitalized)
                            .font(.system(size: 12, weight: on ? .semibold : .regular))
                            .foregroundStyle(on ? p.text : p.text2)
                            .padding(.horizontal, 14).padding(.vertical, 6)
                            .background(on ? p.pane : .clear, in: RoundedRectangle(cornerRadius: 6, style: .continuous))
                            .overlay { if on { RoundedRectangle(cornerRadius: 6).strokeBorder(p.line) } }
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(3)
            .background(p.sidebar, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
            .hairline(p.line, radius: 8)
            Text("System follows macOS. Every theme has a light and a dark palette. A project can pin its own theme and appearance for its window with `theme:` and `appearance:` in config.yaml.")
                .font(SBType.secondary).foregroundStyle(p.text3).padding(.top, 8)
                .fixedSize(horizontal: false, vertical: true)

            section("Sidebar", p)
            HStack(alignment: .top, spacing: 16) {
                VStack(alignment: .leading, spacing: 3) {
                    Text("Project rail").font(SBType.body).foregroundStyle(p.text)
                    Text("Projects sit in a rail of icons; the sidebar lists one project at a time. Off shows every project in one tree.")
                        .font(SBType.secondary).foregroundStyle(p.text3)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer()
                Toggle("", isOn: $rail).labelsHidden().toggleStyle(.switch).tint(p.accent)
            }

            HStack(alignment: .top, spacing: 16) {
                VStack(alignment: .leading, spacing: 3) {
                    Text("Products in a company").font(SBType.body).foregroundStyle(p.text)
                    Text("A company is one icon on the rail; its products are picked in the sidebar, as tabs or from the title. Tabs become the menu past three products.")
                        .font(SBType.secondary).foregroundStyle(p.text3)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer()
                Picker("", selection: $productSwitcher) {
                    Text("Tabs").tag("tabs")
                    Text("Menu").tag("menu")
                }
                .labelsHidden().pickerStyle(.segmented).fixedSize()
            }
            .padding(.top, 14)

            section("Startup", p)
            HStack(alignment: .top, spacing: 16) {
                VStack(alignment: .leading, spacing: 3) {
                    Text("Open at login").font(SBType.body).foregroundStyle(p.text)
                    Text(loginProblem ?? "Switchboard opens when you log in to this Mac. No commands start until you start them.")
                        .font(SBType.secondary).foregroundStyle(loginProblem == nil ? p.text3 : p.error)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer()
                Toggle("", isOn: Binding(get: { openAtLogin }, set: { on in
                    loginProblem = LoginItem.set(on)
                    openAtLogin = LoginItem.isOn
                })).labelsHidden().toggleStyle(.switch).tint(p.accent)
            }
            .onAppear { openAtLogin = LoginItem.isOn }

            section("Preview", p)
            let previewScheme: ColorScheme = appearance.colorScheme ?? scheme
            let k = 504.0 / 1120.0
            ShellPreview(theme: themeID.theme, scheme: previewScheme, rail: rail)
                .scaleEffect(k, anchor: .topLeading)
                .frame(width: 504, height: 640 * k, alignment: .topLeading)
                .clipped()
        }
        .padding(28)
        .frame(width: 560)
        .background(p.window)
        .environment(\.sbTheme, themeID.theme)
        .preferredColorScheme(appearance.colorScheme)
    }

    private func section(_ title: String, _ p: SBPalette) -> some View {
        Text(title.uppercased())
            .font(.system(size: 11, weight: .semibold)).tracking(0.66)
            .foregroundStyle(p.text3)
            .padding(.top, 24).padding(.bottom, 10)
    }
}

/// A small preview tile of a theme (sidebar strip, pane, accent dot).
private struct ThemeCard: View {
    let id: SBThemeID
    let selected: Bool
    let scheme: ColorScheme
    let pick: () -> Void

    var body: some View {
        let t = id.theme
        let p = t.palette(scheme)
        let note: String = { switch id { case .graphite: return "Default · calm"; case .patchBay: return "Warm · lit LEDs"; case .paper: return "Soft · floating" } }()
        Button(action: pick) {
            VStack(alignment: .leading, spacing: 8) {
                ZStack(alignment: .topLeading) {
                    RoundedRectangle(cornerRadius: 8, style: .continuous).fill(p.window)
                    HStack(spacing: 0) {
                        VStack(alignment: .leading, spacing: 5) {
                            Capsule().fill(p.text3).frame(width: 34, height: 4)
                            Capsule().fill(p.selection).frame(width: 46, height: 8)
                            Capsule().fill(p.text4).frame(width: 30, height: 4)
                        }
                        .padding(8)
                        .frame(width: 70, alignment: .leading)
                        .background(p.sidebar)
                        RoundedRectangle(cornerRadius: t.style.paneRadius / 2, style: .continuous)
                            .fill(p.terminal)
                            .padding(t.style.paneInset / 2)
                            .overlay(alignment: .topLeading) {
                                VStack(alignment: .leading, spacing: 4) {
                                    Capsule().fill(p.ansi.indices.contains(2) ? Color(nsColor: p.ansi[2]) : p.running).frame(width: 28, height: 3)
                                    Capsule().fill(p.terminalDim).frame(width: 50, height: 3)
                                    Capsule().fill(p.terminalDim).frame(width: 40, height: 3)
                                }
                                .padding(10 + t.style.paneInset / 2)
                            }
                    }
                    .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
                    Circle().fill(p.accent).frame(width: 6, height: 6).padding(6)
                }
                .frame(height: 78)
                Text(t.name).font(.system(size: 12, weight: .semibold)).foregroundStyle(p.text)
                Text(note).font(.system(size: 11)).foregroundStyle(p.text3)
            }
            .padding(8)
            .background(p.pane, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            .overlay {
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .strokeBorder(selected ? p.accent : p.line, lineWidth: selected ? 1.5 : 1)
            }
        }
        .buttonStyle(.plain)
    }
}

/// Open at login, through the system's login item service. The system owns the truth
/// (the user can also switch it off in System Settings › General › Login Items), so this
/// reads the service rather than keeping a flag of its own.
enum LoginItem {
    static var isOn: Bool { SMAppService.mainApp.status == .enabled }

    /// Returns a message when the change did not take.
    @discardableResult
    static func set(_ on: Bool) -> String? {
        do {
            if on { try SMAppService.mainApp.register() } else { try SMAppService.mainApp.unregister() }
            return nil
        } catch {
            return "Could not change this: \(error.localizedDescription)"
        }
    }

    /// On by default: the first launch of the installed app registers it once. After that
    /// the user's choice stands, including switching it off in System Settings.
    static func enableOnFirstLaunch() {
        let key = "loginItem.defaulted"
        guard !UserDefaults.standard.bool(forKey: key),
              Bundle.main.bundlePath.hasPrefix("/Applications/") else { return }
        UserDefaults.standard.set(true, forKey: key)
        if !isOn { set(true) }
    }
}
