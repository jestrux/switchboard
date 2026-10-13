import SwiftUI

/// Status by shape and colour: ring, spinning arc, breathing dot, check, draining arc, triangle.
/// 12 pt glyph in an 18 pt box (scaled by `size`).
struct StatusGlyph: View {
    let status: CommandStatus
    var restartAt: Double? = nil
    var restartTotal: Double? = nil
    var size: CGFloat = 12
    @Environment(\.sbTheme) private var theme
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var p: SBPalette { theme.palette(scheme) }
    private var box: CGFloat { size * 1.5 }

    var body: some View {
        let colour = p.color(sbStatus)
        Group {
            switch status {
            case .stopped:
                Circle().strokeBorder(p.stopped, lineWidth: size / 6).padding(size / 4)
            case .starting:
                StartingGlyph(track: p.startingSoft, arc: p.starting, animate: !reduceMotion)
            case .running:
                RunningGlyph(colour: p.running, animate: !reduceMotion)
            case .exited:
                DoneGlyph(ring: p.stopped, check: p.text2)
            case .crashed:
                CrashedGlyph(core: p.crashed, track: p.crashedSoft, restartAt: restartAt, total: restartTotal)
            case .error:
                TriangleGlyph().fill(p.error).padding(size / 9)
            }
        }
        .frame(width: box, height: box)
        .shadow(color: glowColour(colour), radius: theme.glows(scheme) && isLive ? 4 : 0)
        .accessibilityLabel(status.label)
    }

    private var isLive: Bool { status == .running || status == .starting || status == .crashed || status == .error }
    private func glowColour(_ c: Color) -> Color { theme.glows(scheme) && isLive ? c : .clear }

    private var sbStatus: SBStatus {
        switch status {
        case .stopped: return .stopped
        case .starting: return .starting
        case .running: return .running
        case .exited: return .done
        case .crashed: return .crashed
        case .error: return .error
        }
    }
}

private struct RunningGlyph: View {
    let colour: Color
    let animate: Bool
    @State private var breathe = false
    var body: some View {
        ZStack {
            Circle().fill(colour.opacity(breathe ? 0.10 : 0.22))
            Circle().fill(colour).padding(4.0 / 18 * 18 * 0.25)
                .scaleEffect(0.53)
        }
        .onAppear {
            guard animate else { return }
            withAnimation(.easeInOut(duration: SBMotion.runningBreathe).repeatForever(autoreverses: true)) { breathe = true }
        }
    }
}

private struct StartingGlyph: View {
    let track: Color
    let arc: Color
    let animate: Bool
    @State private var spin = false
    var body: some View {
        ZStack {
            Circle().strokeBorder(track, lineWidth: 2).padding(3)
            Circle().trim(from: 0, to: 0.25)
                .stroke(arc, style: StrokeStyle(lineWidth: 2, lineCap: .round))
                .padding(4)
                .rotationEffect(.degrees(spin ? 360 : 0))
        }
        .onAppear {
            guard animate else { return }
            withAnimation(.linear(duration: SBMotion.startingSpin).repeatForever(autoreverses: false)) { spin = true }
        }
    }
}

private struct DoneGlyph: View {
    let ring: Color
    let check: Color
    var body: some View {
        ZStack {
            Circle().strokeBorder(ring, lineWidth: 1.6).padding(2)
            CheckShape().stroke(check, style: StrokeStyle(lineWidth: 1.8, lineCap: .round, lineJoin: .round))
                .padding(5)
        }
    }
}

/// Red core, soft track, and an arc that drains over the backoff delay.
private struct CrashedGlyph: View {
    let core: Color
    let track: Color
    let restartAt: Double?
    let total: Double?
    var body: some View {
        TimelineView(.animation(minimumInterval: 0.1, paused: restartAt == nil)) { ctx in
            let fraction: CGFloat = {
                guard let restartAt, let total, total > 0 else { return 0 }
                let remaining = restartAt - ctx.date.timeIntervalSince1970
                return CGFloat(min(1, max(0, remaining / total)))
            }()
            ZStack {
                Circle().strokeBorder(track, lineWidth: 2).padding(1)
                Circle().trim(from: 0, to: fraction)
                    .stroke(core, style: StrokeStyle(lineWidth: 2, lineCap: .round))
                    .rotationEffect(.degrees(-90))
                    .padding(2)
                Circle().fill(core).scaleEffect(0.39)
            }
        }
    }
}

struct TriangleGlyph: Shape {
    func path(in r: CGRect) -> Path {
        var p = Path()
        p.move(to: CGPoint(x: r.midX, y: r.minY + r.height * 0.08))
        p.addLine(to: CGPoint(x: r.maxX - r.width * 0.06, y: r.maxY - r.height * 0.1))
        p.addLine(to: CGPoint(x: r.minX + r.width * 0.06, y: r.maxY - r.height * 0.1))
        p.closeSubpath()
        return p
    }
}

private struct CheckShape: Shape {
    func path(in r: CGRect) -> Path {
        var p = Path()
        p.move(to: CGPoint(x: r.minX + r.width * 0.1, y: r.midY + r.height * 0.05))
        p.addLine(to: CGPoint(x: r.minX + r.width * 0.4, y: r.maxY - r.height * 0.1))
        p.addLine(to: CGPoint(x: r.maxX - r.width * 0.05, y: r.minY + r.height * 0.1))
        return p
    }
}

/// Dashed square used for an empty pane.
struct EmptyGlyph: View {
    @Environment(\.sbPalette) private var p
    var body: some View {
        RoundedRectangle(cornerRadius: 3)
            .strokeBorder(p.text3, style: StrokeStyle(lineWidth: 1.6, dash: [3, 2]))
            .frame(width: 12, height: 12)
            .frame(width: 18, height: 18)
    }
}
