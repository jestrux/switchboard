// Tiny UI driver for screenshots: window sizing and synthetic mouse events, run as one
// sequence so the app stays frontmost between steps.
//
//   uidrive activate                 bring Switchboard to the front
//   uidrive window                   print "id x y w h" of the main window
//   uidrive resize W H               resize (and position) the main window
//   uidrive move X Y                 move the cursor (hover)
//   uidrive click X Y | rclick X Y   left / right click at screen point (origin top-left)
//   uidrive drag X1 Y1 X2 Y2          drag with the left button
//   uidrive key CODE [cmd|shift|ctrl|alt]*   press a key by virtual key code
//   uidrive type TEXT                type a string
//   uidrive sleep MS
//   uidrive shot PATH                window-only screenshot (no shadow)
//   uidrive shotapp PATH             every Switchboard window composited (menus, alerts, sheets)
//   uidrive region PATH              screenshot of the window's rect from the whole screen (menus included)
//
// Several commands can be chained: uidrive activate click 200 300 sleep 500 shot out.png
import AppKit
import ApplicationServices

var args = Array(CommandLine.arguments.dropFirst())

func mainWindow() -> (id: Int, frame: CGRect)? {
    let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
    for w in list {
        guard (w[kCGWindowOwnerName as String] as? String) == "Switchboard",
              (w[kCGWindowLayer as String] as? Int) == 0,
              let dict = w[kCGWindowBounds as String] as? NSDictionary,
              let b = CGRect(dictionaryRepresentation: dict),
              let id = w[kCGWindowNumber as String] as? Int,
              b.width > 300 else { continue }
        return (id, b)
    }
    return nil
}

func axWindow() -> AXUIElement? {
    guard let app = NSRunningApplication.runningApplications(withBundleIdentifier: "dev.waky.switchboard").first else { return nil }
    let ax = AXUIElementCreateApplication(app.processIdentifier)
    var value: AnyObject?
    AXUIElementCopyAttributeValue(ax, kAXWindowsAttribute as CFString, &value)
    return (value as? [AXUIElement])?.first
}

func post(_ type: CGEventType, _ p: CGPoint, button: CGMouseButton = .left) {
    let e = CGEvent(mouseEventSource: nil, mouseType: type, mouseCursorPosition: p, mouseButton: button)
    e?.post(tap: .cghidEventTap)
    usleep(60_000)
}

func next() -> String { args.isEmpty ? "" : args.removeFirst() }
func num() -> Double { Double(next()) ?? 0 }

func capture(_ path: String, region: Bool) {
    guard let w = mainWindow() else { print("no window"); return }
    let task = Process()
    task.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
    if region {
        let r = w.frame
        task.arguments = ["-x", "-R", "\(Int(r.minX)),\(Int(r.minY)),\(Int(r.width)),\(Int(r.height))", path]
    } else {
        task.arguments = ["-x", "-o", "-l", String(w.id), path]
    }
    try? task.run(); task.waitUntilExit()
}

while !args.isEmpty {
    let cmd = next()
    switch cmd {
    case "activate":
        guard let app = NSRunningApplication.runningApplications(withBundleIdentifier: "dev.waky.switchboard").first else { exit(1) }
        app.activate(options: [.activateIgnoringOtherApps, .activateAllWindows])
        if let win = axWindow() { AXUIElementPerformAction(win, kAXRaiseAction as CFString) }
        usleep(300_000)
    case "window":
        if let w = mainWindow() { print("\(w.id) \(Int(w.frame.minX)) \(Int(w.frame.minY)) \(Int(w.frame.width)) \(Int(w.frame.height))") }
        else { print("none"); exit(1) }
    case "resize":
        let w = CGFloat(num()), h = CGFloat(num())
        guard let win = axWindow() else { print("no AX window"); exit(1) }
        let screen = NSScreen.main!.frame
        var pos = CGPoint(x: (screen.width - w) / 2, y: 60)
        var size = CGSize(width: w, height: h)
        AXUIElementSetAttributeValue(win, kAXPositionAttribute as CFString, AXValueCreate(.cgPoint, &pos)!)
        AXUIElementSetAttributeValue(win, kAXSizeAttribute as CFString, AXValueCreate(.cgSize, &size)!)
        usleep(300_000)
    case "move", "click", "rclick", "rdown", "rup":
        let p = CGPoint(x: num(), y: num())
        post(.mouseMoved, p)
        if cmd == "click" { post(.leftMouseDown, p); post(.leftMouseUp, p) }
        if cmd == "rclick" { post(.rightMouseDown, p, button: .right); post(.rightMouseUp, p, button: .right) }
        if cmd == "rdown" { post(.rightMouseDown, p, button: .right) }
        if cmd == "rup" { post(.rightMouseUp, p, button: .right) }
    case "drag":
        let a = CGPoint(x: num(), y: num()), b = CGPoint(x: num(), y: num())
        post(.mouseMoved, a); post(.leftMouseDown, a)
        for i in 1...10 {
            let t = CGFloat(i) / 10
            post(.leftMouseDragged, CGPoint(x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t))
        }
        post(.leftMouseUp, b)
    case "key":
        let code = CGKeyCode(num())
        var flags: CGEventFlags = []
        while let m = args.first, ["cmd", "shift", "ctrl", "alt"].contains(m) {
            _ = next()
            switch m { case "cmd": flags.insert(.maskCommand); case "shift": flags.insert(.maskShift)
                       case "ctrl": flags.insert(.maskControl); default: flags.insert(.maskAlternate) }
        }
        for down in [true, false] {
            let e = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: down)
            e?.flags = flags
            e?.post(tap: .cghidEventTap)
            usleep(40_000)
        }
    case "type":
        let text = next()
        for ch in text.utf16 {
            for down in [true, false] {
                let e = CGEvent(keyboardEventSource: nil, virtualKey: 0, keyDown: down)
                var c = ch
                e?.keyboardSetUnicodeString(stringLength: 1, unicodeString: &c)
                e?.post(tap: .cghidEventTap)
                usleep(8_000)
            }
        }
    case "sleep":
        usleep(UInt32(num() * 1000))
    case "shot":
        capture(next(), region: false)
    case "region":
        capture(next(), region: true)
    case "shotapp":
        // Every on-screen window owned by Switchboard (menus, sheets, alerts), via screencapture -l
        // per window, composited in z-order over the main window.
        let path = next()
        guard let app = NSRunningApplication.runningApplications(withBundleIdentifier: "dev.waky.switchboard").first,
              let w = mainWindow() else { print("no window"); break }
        let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
        var layers: [(CGRect, String)] = []  // front-most first
        var bounds = w.frame
        let tmp = NSTemporaryDirectory()
        for (i, info) in list.enumerated() where (info[kCGWindowOwnerPID as String] as? Int32) == app.processIdentifier {
            guard let id = info[kCGWindowNumber as String] as? Int,
                  let d = info[kCGWindowBounds as String] as? NSDictionary,
                  let r = CGRect(dictionaryRepresentation: d), r.width > 1, r.height > 1 else { continue }
            let file = tmp + "sbwin-\(i).png"
            let task = Process()
            task.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
            task.arguments = ["-x", "-o", "-l", String(id), file]
            try? task.run(); task.waitUntilExit()
            if FileManager.default.fileExists(atPath: file) { layers.append((r, file)); bounds = bounds.union(r) }
        }
        let scale = NSScreen.main!.backingScaleFactor
        let out = NSImage(size: NSSize(width: bounds.width * scale, height: bounds.height * scale))
        out.lockFocus()
        NSColor.clear.set(); NSRect(origin: .zero, size: out.size).fill()
        for (r, file) in layers.reversed() {
            guard let img = NSImage(contentsOfFile: file) else { continue }
            // Flip from top-left screen coords to the image's bottom-left coords.
            let x = (r.minX - bounds.minX) * scale
            let y = (bounds.maxY - r.maxY) * scale
            img.draw(in: NSRect(x: x, y: y, width: r.width * scale, height: r.height * scale))
        }
        out.unlockFocus()
        if let tiff = out.tiffRepresentation, let rep = NSBitmapImageRep(data: tiff),
           let data = rep.representation(using: .png, properties: [:]) { try? data.write(to: URL(fileURLWithPath: path)) }
    default:
        print("unknown command \(cmd)"); exit(2)
    }
}
