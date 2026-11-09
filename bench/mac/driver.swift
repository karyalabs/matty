// The benchmark's macOS input driver and sync marker.
//
// bench/run.py starts this and sends it one command per line on stdin; it
// answers each with "ok …" or "err …" on stdout. It does two jobs:
//
// 1. Posts input with CGEvent (keys, typing, mouse, scroll wheel) and places
//    windows with the Accessibility API, the same way for both clients.
// 2. Shows a small always-on-top barcode (the "marker") just below the app
//    window. Any command prefixed with "@N " first redraws the marker to show
//    the number N, then acts. The screen recording therefore carries, in the
//    same frames as the app, the exact moment each input was sent, and
//    bench/analyze.py measures latency from video alone, identically for
//    both apps.
//
// Build: swiftc -O -o bench/out/driver bench/mac/driver.swift (bench/run.py does it).
// Needs Accessibility permission for the terminal that runs it.

import AppKit
import ApplicationServices
import CoreGraphics
import Foundation

setvbuf(stdout, nil, _IOLBF, 0)

// `driver disclaim EXE ARGS…` becomes EXE (same pid, via POSIX_SPAWN_SETEXEC)
// as its own "responsible process", like an app opened from the Finder. Without
// it macOS charges the app's privacy requests to whatever started run.py (a
// terminal, an agent app); when that host's Info.plist lacks a usage string
// the app asks for, TCC kills the app. Mattermost asks for Focus status at
// launch and dies with SIGABRT that way. run.py launches both apps like this.
if CommandLine.arguments.count > 2, CommandLine.arguments[1] == "disclaim" {
    let argv = Array(CommandLine.arguments.dropFirst(2))
    var attr: posix_spawnattr_t?
    posix_spawnattr_init(&attr)
    posix_spawnattr_setflags(&attr, Int16(POSIX_SPAWN_SETEXEC))
    typealias SetDisclaim = @convention(c) (UnsafeMutablePointer<posix_spawnattr_t?>, Int32) -> Int32
    if let sym = dlsym(UnsafeMutableRawPointer(bitPattern: -2), "responsibility_spawnattrs_setdisclaim") {
        _ = unsafeBitCast(sym, to: SetDisclaim.self)(&attr, 1)
    } else {
        fputs("disclaim: responsibility_spawnattrs_setdisclaim not found; launching as a plain child\n", stderr)
    }
    var cargs: [UnsafeMutablePointer<CChar>?] = argv.map { strdup($0) } + [nil]
    let rc = posix_spawn(nil, argv[0], nil, &attr, &cargs, environ)
    fputs("disclaim: could not start \(argv[0]): \(String(cString: strerror(rc)))\n", stderr)
    exit(127)
}

let markerCells = 18 // [white] 16 bits of N, most significant first [black]

final class MarkerView: NSView {
    var value: UInt16 = 0
    override var isFlipped: Bool { true }
    override func draw(_ dirtyRect: NSRect) {
        let w = bounds.width / CGFloat(markerCells)
        for i in 0..<markerCells {
            let on: Bool
            if i == 0 { on = true } else if i == markerCells - 1 { on = false } else {
                on = (value >> UInt16(16 - i)) & 1 == 1
            }
            (on ? NSColor.white : NSColor.black).setFill()
            NSRect(x: CGFloat(i) * w, y: 0, width: w + 1, height: bounds.height).fill()
        }
    }
}

let app = NSApplication.shared
app.setActivationPolicy(.accessory)

let panel = NSPanel(contentRect: NSRect(x: 0, y: 0, width: 360, height: 20),
                    styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
panel.level = .screenSaver
panel.ignoresMouseEvents = true
panel.hasShadow = false
panel.isOpaque = true
panel.collectionBehavior = [.canJoinAllSpaces, .stationary, .ignoresCycle]
let markerView = MarkerView(frame: panel.contentView!.bounds)
markerView.autoresizingMask = [.width, .height]
panel.contentView = markerView

func primaryHeight() -> CGFloat { NSScreen.screens.first?.frame.height ?? 900 }

/// Global top-left coordinates (CGEvent, AX) to Cocoa's bottom-left.
func cocoaRect(x: CGFloat, y: CGFloat, w: CGFloat, h: CGFloat) -> NSRect {
    NSRect(x: x, y: primaryHeight() - y - h, width: w, height: h)
}

func setMarker(_ n: UInt16) {
    markerView.value = n
    markerView.needsDisplay = true
    markerView.displayIfNeeded()
    panel.display()
    CATransaction.flush()
}

let source = CGEventSource(stateID: .hidSystemState)

let keycodes: [String: CGKeyCode] = [
    "a": 0, "s": 1, "d": 2, "f": 3, "h": 4, "g": 5, "z": 6, "x": 7, "c": 8, "v": 9,
    "b": 11, "q": 12, "w": 13, "e": 14, "r": 15, "y": 16, "t": 17, "1": 18, "2": 19,
    "3": 20, "4": 21, "6": 22, "5": 23, "=": 24, "9": 25, "7": 26, "-": 27, "8": 28,
    "0": 29, "]": 30, "o": 31, "u": 32, "[": 33, "i": 34, "p": 35, "l": 37, "j": 38,
    "'": 39, "k": 40, ";": 41, "\\": 42, ",": 43, "/": 44, "n": 45, "m": 46, ".": 47,
    "`": 50, " ": 49,
    "return": 36, "enter": 36, "tab": 48, "space": 49, "delete": 51, "backspace": 51,
    "escape": 53, "esc": 53, "left": 123, "right": 124, "down": 125, "up": 126,
    "pageup": 116, "pagedown": 121, "home": 115, "end": 119, "f5": 96,
]
let shifted: [Character: Character] = [
    "!": "1", "@": "2", "#": "3", "$": "4", "%": "5", "^": "6", "&": "7", "*": "8",
    "(": "9", ")": "0", "_": "-", "+": "=", "{": "[", "}": "]", "|": "\\", ":": ";",
    "\"": "'", "<": ",", ">": ".", "?": "/", "~": "`",
]

func flags(_ spec: String) -> CGEventFlags {
    var f: CGEventFlags = []
    for part in spec.split(separator: ",") {
        switch part.lowercased() {
        case "cmd", "command": f.insert(.maskCommand)
        case "shift": f.insert(.maskShift)
        case "alt", "option", "opt": f.insert(.maskAlternate)
        case "ctrl", "control": f.insert(.maskControl)
        default: break
        }
    }
    return f
}

func press(_ code: CGKeyCode, _ f: CGEventFlags) {
    let down = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: true)!
    down.flags = f
    down.post(tap: .cghidEventTap)
    usleep(8_000)
    let up = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: false)!
    up.flags = f
    up.post(tap: .cghidEventTap)
}

func typeChar(_ c: Character) {
    if let code = keycodes[String(c)] {
        press(code, [])
    } else if c.isUppercase, let code = keycodes[c.lowercased()] {
        press(code, .maskShift)
    } else if let base = shifted[c], let code = keycodes[String(base)] {
        press(code, .maskShift)
    } else {
        // Anything else: a Unicode key event.
        let utf16 = Array(String(c).utf16)
        for down in [true, false] {
            let e = CGEvent(keyboardEventSource: source, virtualKey: 0, keyDown: down)!
            e.keyboardSetUnicodeString(stringLength: utf16.count, unicodeString: utf16)
            e.post(tap: .cghidEventTap)
        }
    }
}

func mouse(_ type: CGEventType, _ p: CGPoint) {
    CGEvent(mouseEventSource: source, mouseType: type, mouseCursorPosition: p, mouseButton: .left)!
        .post(tap: .cghidEventTap)
}

func scroll(dy: Int32, dx: Int32) {
    let e = CGEvent(scrollWheelEvent2Source: source, units: .pixel, wheelCount: 2,
                    wheel1: dy, wheel2: dx, wheel3: 0)!
    e.setIntegerValueField(.scrollWheelEventIsContinuous, value: 1)
    e.post(tap: .cghidEventTap)
}

/// The app's biggest AX window. Electron apps (Mattermost) also own small
/// helper windows, and "main" or "focused" can be one of those.
func axWindow(_ pid: pid_t) -> AXUIElement? {
    let appEl = AXUIElementCreateApplication(pid)
    var value: CFTypeRef?
    var found: [AXUIElement] = []
    if AXUIElementCopyAttributeValue(appEl, kAXWindowsAttribute as CFString, &value) == .success,
       let list = value as? [AXUIElement] { found = list }
    for attr in [kAXMainWindowAttribute, kAXFocusedWindowAttribute] where found.isEmpty {
        if AXUIElementCopyAttributeValue(appEl, attr as CFString, &value) == .success, let v = value {
            found = [v as! AXUIElement]
        }
    }
    func area(_ w: AXUIElement) -> CGFloat {
        var v: CFTypeRef?
        var size = CGSize.zero
        if AXUIElementCopyAttributeValue(w, kAXSizeAttribute as CFString, &v) == .success, let v {
            AXValueGetValue(v as! AXValue, .cgSize, &size)
        }
        return size.width * size.height
    }
    return found.max { area($0) < area($1) }
}

func windows(_ pid: pid_t) -> [CGRect] {
    guard let info = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID)
        as? [[String: Any]] else { return [] }
    return info.compactMap { w in
        guard (w[kCGWindowOwnerPID as String] as? pid_t) == pid,
              (w[kCGWindowLayer as String] as? Int) == 0,
              let b = w[kCGWindowBounds as String] as? [String: CGFloat] else { return nil }
        let r = CGRect(x: b["X"] ?? 0, y: b["Y"] ?? 0, width: b["Width"] ?? 0, height: b["Height"] ?? 0)
        return r.width > 100 && r.height > 100 ? r : nil
    }.sorted { $0.width * $0.height > $1.width * $1.height } // biggest (the app window) first
}

func now() -> UInt64 { DispatchTime.now().uptimeNanoseconds }

func handle(_ raw: String) -> String {
    var words = raw.split(separator: " ", omittingEmptySubsequences: true).map(String.init)
    guard !words.isEmpty else { return "err empty" }
    if words[0].hasPrefix("@"), let n = UInt16(words[0].dropFirst()) {
        setMarker(n)
        words.removeFirst()
        if words.isEmpty { return "ok \(now())" }
    }
    let t = now()
    let args = Array(words.dropFirst())
    func num(_ i: Int) -> Double { i < args.count ? Double(args[i]) ?? 0 : 0 }
    switch words[0] {
    case "check":
        return "ok ax=\(AXIsProcessTrusted() ? 1 : 0) screen=\(CGPreflightScreenCaptureAccess() ? 1 : 0)"
    case "request":
        let opts = [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true] as CFDictionary
        let ax = AXIsProcessTrustedWithOptions(opts)
        let sc = CGRequestScreenCaptureAccess()
        return "ok ax=\(ax ? 1 : 0) screen=\(sc ? 1 : 0)"
    case "screen":
        let s = NSScreen.screens.first!
        return "ok \(Int(s.frame.width)) \(Int(s.frame.height)) \(s.backingScaleFactor)"
    case "markrect":
        panel.setFrame(cocoaRect(x: num(0), y: num(1), w: num(2), h: num(3)), display: true)
        panel.orderFrontRegardless()
        setMarker(markerView.value)
        return "ok"
    case "markhide":
        panel.orderOut(nil)
        return "ok"
    case "key":
        guard let name = args.first, let code = keycodes[name.lowercased()] else { return "err unknown key" }
        press(code, flags(args.count > 1 ? args[1] : ""))
    case "char":
        // "char x"; a space is "char space".
        let s = args.first == "space" ? " " : (args.first ?? "")
        for c in s { typeChar(c) }
    case "type":
        // type <delay ms> <text…>
        let delay = UInt32(num(0) * 1000)
        let text = args.dropFirst().joined(separator: " ")
        for c in text { typeChar(c); usleep(delay) }
    case "move":
        mouse(.mouseMoved, CGPoint(x: num(0), y: num(1)))
    case "click":
        let p = CGPoint(x: num(0), y: num(1))
        mouse(.mouseMoved, p); usleep(30_000)
        mouse(.leftMouseDown, p); usleep(30_000)
        mouse(.leftMouseUp, p)
    case "scroll":
        // scroll <dy px per event> <events> <interval ms>; positive dy scrolls up (towards older posts).
        let dy = Int32(num(0)), count = Int(num(1)), interval = num(2)
        let start = now()
        for i in 0..<count {
            scroll(dy: dy, dx: 0)
            let target = start + UInt64(Double(i + 1) * interval * 1_000_000)
            let n = now()
            if target > n { usleep(UInt32((target - n) / 1000)) }
        }
    case "place":
        let pid = pid_t(num(0))
        guard let win = axWindow(pid) else { return "err no AX window for \(pid) (Accessibility permission?)" }
        var p = CGPoint(x: num(1), y: num(2))
        var s = CGSize(width: num(3), height: num(4))
        AXUIElementSetAttributeValue(win, kAXPositionAttribute as CFString, AXValueCreate(.cgPoint, &p)!)
        AXUIElementSetAttributeValue(win, kAXSizeAttribute as CFString, AXValueCreate(.cgSize, &s)!)
        AXUIElementSetAttributeValue(win, kAXPositionAttribute as CFString, AXValueCreate(.cgPoint, &p)!)
        usleep(200_000)
        let r = windows(pid).first ?? .zero
        return "ok \(Int(r.minX)) \(Int(r.minY)) \(Int(r.width)) \(Int(r.height))"
    case "windows":
        let list = windows(pid_t(num(0)))
        return "ok " + list.map { "\(Int($0.minX)),\(Int($0.minY)),\(Int($0.width)),\(Int($0.height))" }.joined(separator: " ")
    case "front":
        NSRunningApplication(processIdentifier: pid_t(num(0)))?.activate(options: [.activateAllWindows])
    case "sleep":
        usleep(UInt32(num(0) * 1000))
    case "quit":
        print("ok"); exit(0)
    default:
        return "err unknown command \(words[0])"
    }
    return "ok \(t)"
}

Thread {
    while let line = readLine() {
        var reply = ""
        DispatchQueue.main.sync { reply = handle(line.trimmingCharacters(in: .whitespaces)) }
        print(reply)
    }
    DispatchQueue.main.async { exit(0) }
}.start()

app.run()
