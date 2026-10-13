import SwiftUI
import SwiftTerm

/// Hosts the per-command `TerminalView` from the registry, swapping on selection change.
struct TerminalContainer: NSViewRepresentable {
    let ref: CommandRef
    /// Called when the terminal inside takes keyboard focus (click or programmatic).
    var onFocus: () -> Void = {}

    func makeNSView(context: Context) -> ContainerView {
        let v = ContainerView()
        v.wantsLayer = true
        v.onFocus = onFocus
        v.show(ref)
        return v
    }

    func updateNSView(_ nsView: ContainerView, context: Context) {
        nsView.onFocus = onFocus
        nsView.show(ref)
    }

    final class ContainerView: NSView {
        private var current: CommandRef?
        var onFocus: () -> Void = {}
        private var responderObservation: NSKeyValueObservation?

        override var acceptsFirstResponder: Bool { true }

        override var wantsUpdateLayer: Bool { true }
        override func updateLayer() {
            layer?.backgroundColor = (subviews.first as? TerminalView)?.nativeBackgroundColor.cgColor
        }

        func show(_ ref: CommandRef) {
            guard ref != current else { return }
            current = ref
            subviews.forEach { $0.removeFromSuperview() }
            let host = TerminalRegistry.shared.host(for: ref)
            let t = host.terminal
            addSubview(t)
            needsDisplay = true
            NSLayoutConstraint.activate([
                t.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
                t.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -4),
                t.topAnchor.constraint(equalTo: topAnchor, constant: 6),
                t.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -2),
            ])
            DispatchQueue.main.async { [weak self] in
                self?.window?.makeFirstResponder(t)
            }
        }

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            if let t = subviews.first { window?.makeFirstResponder(t) }
            // Follow keyboard focus: when our terminal becomes first responder, the pane is focused.
            responderObservation = window?.observe(\.firstResponder, options: [.new]) { [weak self] win, _ in
                guard let self, let t = self.subviews.first, win.firstResponder === t else { return }
                DispatchQueue.main.async { self.onFocus() }
            }
        }

        override func viewDidChangeEffectiveAppearance() {
            super.viewDidChangeEffectiveAppearance()
            TerminalRegistry.shared.applyAppearance()
        }

        override func mouseDown(with event: NSEvent) {
            if let t = subviews.first { window?.makeFirstResponder(t) }
            super.mouseDown(with: event)
        }
    }
}

struct PaneFooter: View {
    @EnvironmentObject var store: EngineStore
    let ref: CommandRef
    let command: CommandInfo?
    let state: CommandState

    var body: some View {
        HStack(spacing: 14) {
            StatusDot(status: state.status)
            Text(command?.displayName ?? ref.address).fontWeight(.medium)
            if let command {
                Label(command.folderLabel, systemImage: "folder")
                    .foregroundStyle(command.cwdExists ? Color.secondary : Color.orange)
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .help(command.cwdPath ?? "")
                    .onTapGesture { if let p = command.cwdPath, command.cwdExists { store.revealFolder(p) } }
            }
            if let pid = state.pid, state.isAlive {
                Text("pid \(pid)").monospacedDigit()
            }
            if state.isAlive, let started = state.startedAt {
                TimelineView(.periodic(from: .now, by: 1)) { ctx in
                    Text(uptime(since: started, now: ctx.date)).monospacedDigit()
                }
            }
            if !state.isAlive {
                Text(state.summary)
                    .foregroundStyle(state.status == .crashed || state.status == .error ? Color.red : Color.secondary)
                    .lineLimit(1)
            }
            Spacer()
            if state.needsRestart {
                Button {
                    store.restart(ref)
                } label: {
                    Label("Config changed · restart to apply", systemImage: "arrow.clockwise")
                }
                .buttonStyle(.link)
                .font(.caption)
            }
            if let title = store.terminalTitles[ref], !title.isEmpty, state.isAlive {
                Text(title).foregroundStyle(.secondary).lineLimit(1).truncationMode(.middle)
            }
            HStack(spacing: 4) {
                if state.isAlive {
                    Button { store.stop(ref) } label: { Image(systemName: "stop.fill") }
                        .help("Stop (⌘.)")
                    Button { store.restart(ref) } label: { Image(systemName: "arrow.clockwise") }
                        .help("Restart (⌘R)")
                } else {
                    Button { store.start(ref) } label: { Image(systemName: "play.fill") }
                        .help("Start")
                        .disabled(command?.cwdExists == false)
                }
            }
            .buttonStyle(.borderless)
        }
        .font(.callout)
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background(.bar)
    }

    private func uptime(since startedAt: Double, now: Date) -> String {
        let secs = max(0, Int(now.timeIntervalSince1970 - startedAt))
        let h = secs / 3600, m = (secs % 3600) / 60, s = secs % 60
        return h > 0 ? String(format: "%d:%02d:%02d", h, m, s) : String(format: "%d:%02d", m, s)
    }
}

/// Gray stopped, yellow starting, green running, red crashed, hollow check when done.
struct StatusDot: View {
    let status: CommandStatus
    var size: CGFloat = 9

    var body: some View {
        Group {
            switch status {
            case .stopped:
                Circle().fill(Color.gray.opacity(0.5))
            case .starting:
                Circle().fill(Color.yellow)
            case .running:
                Circle().fill(Color.green)
            case .crashed:
                Circle().fill(Color.red)
            case .exited:
                Image(systemName: "checkmark.circle")
                    .resizable()
                    .foregroundStyle(.secondary)
            case .error:
                Image(systemName: "exclamationmark.triangle.fill")
                    .resizable()
                    .foregroundStyle(.orange)
            }
        }
        .frame(width: size, height: size)
        .help(status.label)
    }
}
