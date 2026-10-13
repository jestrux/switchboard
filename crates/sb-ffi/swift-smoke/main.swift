// M2 smoke test: start a command through the generated bindings and receive events.
import Foundation
import SwitchboardCore

final class Listener: EngineListener {
    let lock = NSLock()
    var output = Data()
    var statuses: [CommandStatus] = []
    let done = DispatchSemaphore(value: 0)
    func onProjectsChanged() {}
    func onStatus(project: String, address: String, state: CommandState) {
        lock.lock(); statuses.append(state.status); lock.unlock()
        if state.status == .exited || state.status == .crashed { done.signal() }
    }
    func onOutput(project: String, address: String, bytes: Data) {
        lock.lock(); output.append(bytes); lock.unlock()
    }
}

let tmp = FileManager.default.temporaryDirectory.appendingPathComponent("sb-smoke-\(getpid())/Projects")
try! FileManager.default.createDirectory(at: tmp.appendingPathComponent("demo"), withIntermediateDirectories: true)
try! "version: 1\nname: Demo\ncommands:\n  - id: hi\n    run: echo -e '\\e[35mhello from swift\\e[0m'\n".write(to: tmp.appendingPathComponent("demo/config.yaml"), atomically: true, encoding: .utf8)

let engine = Engine.open(projectsDir: tmp.path)
let listener = Listener()
_ = engine.subscribe(listener: listener)
let projects = engine.projects()
precondition(projects.count == 1 && projects[0].name == "Demo", "project loaded")
precondition(projects[0].commands[0].state.status == .stopped)
try! engine.start(project: "demo", address: "hi")
precondition(listener.done.wait(timeout: .now() + 10) == .success, "command finished")
let text = String(decoding: listener.output, as: UTF8.self)
precondition(text.contains("\u{1b}[35mhello from swift"), "got colored output: \(text.debugDescription)")
precondition(listener.statuses.first == .starting && listener.statuses.last == .exited, "statuses \(listener.statuses)")
let final = engine.state(project: "demo", address: "hi")
precondition(final.exitCode == 0 && final.pid != nil)
precondition(engine.rawTail(project: "demo", address: "hi").count == listener.output.count)
precondition(validateConfig(path: "/nonexistent").count == 1)
print("ok: \(listener.statuses) \(text.trimmingCharacters(in: .whitespacesAndNewlines).debugDescription) shell=\(loginShell())")
