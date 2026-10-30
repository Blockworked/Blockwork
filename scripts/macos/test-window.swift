// Native fixture for Blockwork's macOS screen/window/input integration tests.
// swiftc -module-cache-path /tmp/blockwork-swift-cache scripts/macos/test-window.swift -o /tmp/blockwork-test-window
import AppKit

let logPath = ProcessInfo.processInfo.environment["BLOCKWORK_TEST_EVENT_LOG"]
    ?? "/tmp/blockwork-macos-input-events.jsonl"
FileManager.default.createFile(atPath: logPath, contents: Data())
let eventLog = FileHandle(forWritingAtPath: logPath)!

class TestApplication: NSApplication {
    override func sendEvent(_ event: NSEvent) {
        let types: Set<NSEvent.EventType> = [.keyDown, .keyUp, .flagsChanged,
            .leftMouseDown, .leftMouseUp, .leftMouseDragged, .rightMouseDown,
            .rightMouseUp, .rightMouseDragged, .otherMouseDown, .otherMouseUp,
            .otherMouseDragged, .scrollWheel]
        if types.contains(event.type) {
            var row: [String: Any] = ["type": event.type.rawValue,
                "flags": event.modifierFlags.rawValue,
                "time": event.timestamp]
            if [.keyDown, .keyUp].contains(event.type) {
                row["characters"] = event.characters ?? ""
                row["keyCode"] = event.keyCode
            }
            if [.leftMouseDown, .leftMouseUp, .leftMouseDragged, .rightMouseDown,
                .rightMouseUp, .rightMouseDragged, .otherMouseDown, .otherMouseUp,
                .otherMouseDragged, .scrollWheel].contains(event.type) {
                row["button"] = event.buttonNumber
                row["dx"] = event.deltaX
                row["dy"] = event.deltaY
                if let point = event.cgEvent?.location {
                    row["x"] = point.x
                    row["y"] = point.y
                }
            }
            if let bytes = try? JSONSerialization.data(withJSONObject: row, options: [.sortedKeys]) {
                eventLog.write(bytes)
                eventLog.write(Data([10]))
            }
        }
        super.sendEvent(event)
    }
}

class TestView: NSView {
    override func draw(_ dirtyRect: NSRect) {
        NSColor.black.setFill()
        bounds.fill()
        NSColor(srgbRed: 1, green: 0, blue: 0, alpha: 1).setFill()
        NSRect(x: 280, y: 180, width: 80, height: 80).fill()
        let text = "BLOCKWORK MACOS 12345" as NSString
        text.draw(at: NSPoint(x: 35, y: 310), withAttributes: [
            .font: NSFont.systemFont(ofSize: 28), .foregroundColor: NSColor.white])
    }
}

let app = TestApplication.shared
app.setActivationPolicy(.regular)
// Supply the standard responder-chain shortcuts so clipboard/chord tests use
// the same Edit-menu behavior as an ordinary native macOS text editor.
let menu = NSMenu()
let applicationItem = NSMenuItem()
let applicationMenu = NSMenu()
applicationMenu.addItem(withTitle: "Quit Blockwork Test Window", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
applicationItem.submenu = applicationMenu
menu.addItem(applicationItem)
let editItem = NSMenuItem(title: "Edit", action: nil, keyEquivalent: "")
let editMenu = NSMenu(title: "Edit")
for (title, action, key) in [("Cut", "cut:", "x"), ("Copy", "copy:", "c"),
                            ("Paste", "paste:", "v"), ("Select All", "selectAll:", "a")] {
    editMenu.addItem(withTitle: title, action: NSSelectorFromString(action), keyEquivalent: key)
}
editItem.submenu = editMenu
menu.addItem(editItem)
app.mainMenu = menu
let window = NSWindow(contentRect: NSRect(x: 200, y: 200, width: 640, height: 400),
    styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
// Keep exact-color fixtures independent of the display backing color space.
window.colorSpace = .sRGB
window.title = "Blockwork macOS Test Window"
let view = TestView(frame: NSRect(x: 0, y: 0, width: 640, height: 400))
let editor = NSTextView(frame: NSRect(x: 30, y: 25, width: 580, height: 100))
editor.isRichText = false
editor.font = NSFont.systemFont(ofSize: 20)
editor.backgroundColor = .white
editor.textColor = .black
view.addSubview(editor)
window.contentView = view
window.makeKeyAndOrderFront(nil)
window.makeFirstResponder(editor)
app.activate(ignoringOtherApps: true)
DispatchQueue.main.async {
    if let path = ProcessInfo.processInfo.environment["BLOCKWORK_TEST_READY_FILE"] {
        try? Data("ready".utf8).write(to: URL(fileURLWithPath: path))
    }
    print("Blockwork macOS test fixture ready; input log: \(logPath)")
    fflush(stdout)
}
app.run()
