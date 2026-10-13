import Foundation
import SwiftUI

/// Maximum terminals shown at once in one window.
let maxPanes = 4

/// A binary tree of panes: leaves show a command (or nothing yet), splits divide space.
indirect enum PaneLayout: Codable, Equatable {
    case leaf(PaneLeaf)
    case split(PaneSplit)

    static func single(_ ref: CommandRef? = nil) -> PaneLayout {
        .leaf(PaneLeaf(id: UUID(), ref: ref))
    }

    var leaves: [PaneLeaf] {
        switch self {
        case .leaf(let l): return [l]
        case .split(let s): return s.first.leaves + s.second.leaves
        }
    }

    var leafCount: Int { leaves.count }

    func leaf(_ id: UUID) -> PaneLeaf? { leaves.first { $0.id == id } }

    func leaf(showing ref: CommandRef) -> PaneLeaf? { leaves.first { $0.ref == ref } }

    /// Replace the leaf `id` with a split holding it and a new empty pane.
    func splitting(_ id: UUID, axis: SplitAxis, newLeaf: PaneLeaf) -> PaneLayout {
        switch self {
        case .leaf(let l) where l.id == id:
            return .split(PaneSplit(id: UUID(), axis: axis, fraction: 0.5, first: .leaf(l), second: .leaf(newLeaf)))
        case .leaf:
            return self
        case .split(var s):
            s.first = s.first.splitting(id, axis: axis, newLeaf: newLeaf)
            s.second = s.second.splitting(id, axis: axis, newLeaf: newLeaf)
            return .split(s)
        }
    }

    /// Remove the leaf `id`; its sibling takes the space. Returns nil if this was the only leaf.
    func closing(_ id: UUID) -> PaneLayout? {
        switch self {
        case .leaf(let l):
            return l.id == id ? nil : self
        case .split(var s):
            if case .leaf(let l) = s.first, l.id == id { return s.second }
            if case .leaf(let l) = s.second, l.id == id { return s.first }
            s.first = s.first.closing(id) ?? s.first
            s.second = s.second.closing(id) ?? s.second
            return .split(s)
        }
    }

    func updatingLeaf(_ id: UUID, _ change: (inout PaneLeaf) -> Void) -> PaneLayout {
        switch self {
        case .leaf(var l) where l.id == id:
            change(&l)
            return .leaf(l)
        case .leaf:
            return self
        case .split(var s):
            s.first = s.first.updatingLeaf(id, change)
            s.second = s.second.updatingLeaf(id, change)
            return .split(s)
        }
    }

    func updatingSplit(_ id: UUID, fraction: Double) -> PaneLayout {
        switch self {
        case .leaf:
            return self
        case .split(var s):
            if s.id == id {
                s.fraction = min(0.85, max(0.15, fraction))
            } else {
                s.first = s.first.updatingSplit(id, fraction: fraction)
                s.second = s.second.updatingSplit(id, fraction: fraction)
            }
            return .split(s)
        }
    }

    /// Leaves in visual order (left-to-right, top-to-bottom) for focus cycling.
    func nextLeaf(after id: UUID) -> PaneLeaf? {
        let all = leaves
        guard let i = all.firstIndex(where: { $0.id == id }) else { return all.first }
        return all[(i + 1) % all.count]
    }
}

struct PaneLeaf: Codable, Equatable, Identifiable {
    let id: UUID
    var ref: CommandRef?
}

enum SplitAxis: String, Codable {
    case horizontal, vertical
}

struct PaneSplit: Codable, Equatable {
    let id: UUID
    var axis: SplitAxis
    /// Share of the space given to `first` (0.15 … 0.85).
    var fraction: Double
    var first: PaneLayout
    var second: PaneLayout
}

