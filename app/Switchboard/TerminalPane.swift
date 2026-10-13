import SwiftUI
import SwiftTerm

/// Hosts the per-command `TerminalView` from the registry, swapping on selection change.
struct TerminalContainer: NSViewRepresentable {
    let ref: CommandRef
    let look: Look
    /// Called when the terminal inside takes keyboard focus (click or programmatic).
    var onFocus: () -> Void = {}

    func makeNSView(context: Context) -> ContainerView {
        let v = ContainerView()
        v.wantsLayer = true
        v.onFocus = onFocus
        v.show(ref)
        TerminalRegistry.shared.host(for: ref).applyTheme(look)
        v.needsDisplay = true
        return v
    }

    func updateNSView(_ nsView: ContainerView, context: Context) {
        nsView.onFocus = onFocus
        nsView.show(ref)
        TerminalRegistry.shared.host(for: ref).applyTheme(look)
        nsView.needsDisplay = true
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
                t.leadingAnchor.constraint(equalTo: leadingAnchor, constant: SBMetrics.terminalInset.leading),
                t.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -SBMetrics.terminalInset.trailing),
                t.topAnchor.constraint(equalTo: topAnchor, constant: SBMetrics.terminalInset.top),
                t.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -SBMetrics.terminalInset.bottom),
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


        override func mouseDown(with event: NSEvent) {
            if let t = subviews.first { window?.makeFirstResponder(t) }
            super.mouseDown(with: event)
        }
    }
}

