import Foundation
import SwiftUI

/// What a window shows: every project, or a single one.
enum WindowScope: Hashable {
    case all
    case project(String)

    var persistenceKey: String {
        switch self {
        case .all: return "selection.main"
        case .project(let slug): return "selection.project.\(slug)"
        }
    }
}

/// Per-window state: scope and selection. One instance per open window.
@MainActor
final class WindowContext: ObservableObject, Identifiable {
    let id = UUID()
    let scope: WindowScope
    @Published var selection: CommandRef? {
        didSet { persist() }
    }

    init(scope: WindowScope) {
        self.scope = scope
        if let data = UserDefaults.standard.data(forKey: scope.persistenceKey),
           let ref = try? JSONDecoder().decode(CommandRef.self, from: data) {
            selection = ref
        }
    }

    /// Human name used in "Showing in the … window" placeholders.
    func title(store: EngineStore) -> String {
        switch scope {
        case .all: return "main"
        case .project(let slug): return store.project(slug)?.name ?? slug
        }
    }

    /// Projects visible in this window. The main board hides projects that are open in
    /// their own window.
    func projects(in store: EngineStore) -> [ProjectInfo] {
        switch scope {
        case .all: return store.projects.filter { !store.openProjectWindows.contains($0.slug) }
        case .project(let slug): return store.projects.filter { $0.slug == slug }
        }
    }

    /// Drop a selection that no longer exists; pick a default when nothing is selected.
    func reconcile(with store: EngineStore) {
        let visible = projects(in: store)
        if let sel = selection, !visible.contains(where: { $0.slug == sel.project }) || store.command(sel) == nil {
            selection = nil
        }
        if selection == nil, let p = projects(in: store).first(where: \.isValid), let c = p.allCommands.first {
            selection = CommandRef(project: p.slug, address: c.address)
        }
    }

    private func persist() {
        if let selection, let data = try? JSONEncoder().encode(selection) {
            UserDefaults.standard.set(data, forKey: scope.persistenceKey)
        } else {
            UserDefaults.standard.removeObject(forKey: scope.persistenceKey)
        }
    }
}

/// Snapshot of the focused window, read by the menu bar commands.
struct WindowFocus {
    let context: WindowContext
    let selection: CommandRef?
}

struct WindowFocusKey: FocusedValueKey {
    typealias Value = WindowFocus
}

extension FocusedValues {
    var windowFocus: WindowFocus? {
        get { self[WindowFocusKey.self] }
        set { self[WindowFocusKey.self] = newValue }
    }
}
