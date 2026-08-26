import AppKit
import Foundation

private let nsNotFoundWire = UInt64.max

private struct WireRange: Codable, Equatable {
    var location: UInt64
    var length: UInt64

    static let notFound = WireRange(location: nsNotFoundWire, length: 0)

    var nsRange: NSRange {
        if location == nsNotFoundWire {
            return NSRange(location: NSNotFound, length: 0)
        }
        return NSRange(location: Int(clamping: location), length: Int(clamping: length))
    }

    static func from(_ range: NSRange) -> WireRange {
        if range.location == NSNotFound || range.location < 0 || range.length < 0 {
            return .notFound
        }
        return WireRange(location: UInt64(range.location), length: UInt64(range.length))
    }
}

private struct WireRect: Codable, Equatable {
    var x: Double
    var y: Double
    var width: Double
    var height: Double

    static let zero = WireRect(x: 0, y: 0, width: 1, height: 24)

    var nsRect: NSRect {
        NSRect(x: x, y: y, width: width, height: height)
    }
}

private struct WireCharacterBounds: Codable, Equatable {
    var range: WireRange
    var rect: WireRect
}

private struct BridgeState: Codable, Equatable {
    var session: UInt64
    var revision: UInt64
    var mode: String
    var displayText: String
    var selectedRange: WireRange
    var markedRange: WireRange
    var hasMarkedText: Bool
    var firstRect: WireRect
    var actualRange: WireRange
    var characterBounds: [WireCharacterBounds]
    var secure: Bool
    var diagnostics: [String]

    enum CodingKeys: String, CodingKey {
        case session
        case revision
        case mode
        case displayText = "display_text"
        case selectedRange = "selected_range"
        case markedRange = "marked_range"
        case hasMarkedText = "has_marked_text"
        case firstRect = "first_rect"
        case actualRange = "actual_range"
        case characterBounds = "character_bounds"
        case secure
        case diagnostics
    }

    static func initial(mode: String) -> BridgeState {
        BridgeState(
            session: 0,
            revision: 0,
            mode: mode,
            displayText: "",
            selectedRange: .notFound,
            markedRange: .notFound,
            hasMarkedText: false,
            firstRect: .zero,
            actualRange: .notFound,
            characterBounds: [],
            secure: mode == "secure-field",
            diagnostics: []
        )
    }
}

private final class JsonLineChannel {
    private let output = FileHandle.standardOutput
    private let lock = NSLock()

    func send(_ payload: [String: Any]) {
        lock.lock()
        defer { lock.unlock() }
        do {
            let data = try JSONSerialization.data(withJSONObject: payload, options: [])
            output.write(data)
            output.write(Data([0x0a]))
        } catch {
            let fallback = "{\"event\":\"bridge_error\",\"message\":\""
            let escaped = String(describing: error)
                .replacingOccurrences(of: "\\", with: "\\\\")
                .replacingOccurrences(of: "\"", with: "\\\"")
            output.write(Data((fallback + escaped + "\"}\n").utf8))
        }
    }
}

private enum TextInputBridgeError: Error, CustomStringConvertible {
    case unsupportedTextInputValue

    var description: String {
        switch self {
        case .unsupportedTextInputValue:
            return "unsupported_text_input_value"
        }
    }
}

final class ArcweftTextInputClientView: NSView, NSTextInputClient {
    private var bridgeState: BridgeState
    private let channel: JsonLineChannel
    private var trackingInstalled = false

    init(frame frameRect: NSRect, mode: String, channel: JsonLineChannel) {
        self.bridgeState = BridgeState.initial(mode: mode)
        self.channel = channel
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor
    }

    required init?(coder: NSCoder) {
        nil
    }

    override var acceptsFirstResponder: Bool { true }
    override var canBecomeKeyView: Bool { true }
    override var isFlipped: Bool { true }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        installWindowObserversIfNeeded()
        sendGeometryRefresh(event: "ready")
    }

    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        sendGeometryRefresh(event: "geometry_refresh")
    }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        sendGeometryRefresh(event: "geometry_refresh")
    }

    override func becomeFirstResponder() -> Bool {
        channel.send(["event": "focus"])
        return true
    }

    override func resignFirstResponder() -> Bool {
        channel.send(["event": "blur"])
        return true
    }

    override func keyDown(with event: NSEvent) {
        if inputContext?.handleEvent(event) != true {
            super.keyDown(with: event)
        }
    }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        let fieldRect = bounds.insetBy(dx: 16, dy: 20)
        NSColor.textBackgroundColor.setFill()
        NSBezierPath(rect: fieldRect).fill()
        NSColor.separatorColor.setStroke()
        let field = NSBezierPath(roundedRect: fieldRect, xRadius: 6, yRadius: 6)
        field.lineWidth = 1
        field.stroke()

        let visible = bridgeState.secure ? String(repeating: "•", count: bridgeState.displayText.count) : bridgeState.displayText
        let attrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 20),
            .foregroundColor: NSColor.labelColor,
        ]
        visible.draw(in: fieldRect.insetBy(dx: 12, dy: 8), withAttributes: attrs)
    }

    func apply(_ state: BridgeState) {
        bridgeState = state
        needsDisplay = true
    }

    func insertText(_ string: Any, replacementRange: NSRange) {
        guard case let .success(text) = Self.plainString(from: string) else {
            channel.send([
                "event": "bridge_error",
                "message": TextInputBridgeError.unsupportedTextInputValue.description,
            ])
            return
        }
        channel.send([
            "event": "insert_text",
            "text": text,
            "replacement_range": Self.jsonRange(replacementRange),
        ])
    }

    func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
        guard case let .success(text) = Self.plainString(from: string) else {
            channel.send([
                "event": "bridge_error",
                "message": TextInputBridgeError.unsupportedTextInputValue.description,
            ])
            return
        }
        channel.send([
            "event": "set_marked_text",
            "text": text,
            "selected_range": Self.jsonRange(selectedRange),
            "replacement_range": Self.jsonRange(replacementRange),
        ])
    }

    func unmarkText() {
        channel.send(["event": "unmark_text"])
    }

    func selectedRange() -> NSRange {
        bridgeState.secure ? NSRange(location: NSNotFound, length: 0) : bridgeState.selectedRange.nsRange
    }

    func markedRange() -> NSRange {
        bridgeState.secure ? NSRange(location: NSNotFound, length: 0) : bridgeState.markedRange.nsRange
    }

    func hasMarkedText() -> Bool {
        !bridgeState.secure && bridgeState.hasMarkedText
    }

    func attributedSubstring(forProposedRange range: NSRange, actualRange: NSRangePointer?) -> NSAttributedString? {
        guard !bridgeState.secure else {
            actualRange?.pointee = NSRange(location: NSNotFound, length: 0)
            return nil
        }
        guard let substring = substringForUtf16Range(range, in: bridgeState.displayText) else {
            actualRange?.pointee = NSRange(location: NSNotFound, length: 0)
            return nil
        }
        actualRange?.pointee = range
        return NSAttributedString(string: substring)
    }

    func firstRect(forCharacterRange range: NSRange, actualRange: NSRangePointer?) -> NSRect {
        if bridgeState.secure || range.location == NSNotFound {
            actualRange?.pointee = bridgeState.actualRange.nsRange
            return bridgeState.firstRect.nsRect
        }
        let proposed = WireRange.from(range)
        if let bound = bridgeState.characterBounds.first(where: { rangesOverlap($0.range, proposed) }) {
            actualRange?.pointee = bound.range.nsRange
            return bound.rect.nsRect
        }
        actualRange?.pointee = bridgeState.actualRange.nsRange
        return bridgeState.firstRect.nsRect
    }

    func characterIndex(for point: NSPoint) -> Int {
        guard !bridgeState.secure else {
            return NSNotFound
        }
        return bridgeState.characterBounds.first(where: { $0.rect.nsRect.contains(point) })
            .map { Int(clamping: $0.range.location) }
            ?? NSNotFound
    }

    func validAttributesForMarkedText() -> [NSAttributedString.Key] {
        []
    }

    func doCommand(by selector: Selector) {
        channel.send(["event": "command", "selector": NSStringFromSelector(selector)])
    }

    private func installWindowObserversIfNeeded() {
        guard !trackingInstalled, let window else {
            return
        }
        trackingInstalled = true
        let center = NotificationCenter.default
        center.addObserver(forName: NSWindow.didMoveNotification, object: window, queue: .main) { [weak self] _ in
            self?.sendGeometryRefresh(event: "geometry_refresh")
        }
        center.addObserver(forName: NSWindow.didResizeNotification, object: window, queue: .main) { [weak self] _ in
            self?.sendGeometryRefresh(event: "geometry_refresh")
        }
        center.addObserver(forName: NSWindow.didChangeScreenNotification, object: window, queue: .main) { [weak self] _ in
            self?.sendGeometryRefresh(event: "geometry_refresh")
        }
    }

    private func sendGeometryRefresh(event: String) {
        let screenHeight = window?.screen?.frame.height ?? NSScreen.main?.frame.height ?? 0
        let localRect = convert(bounds.insetBy(dx: 16, dy: 20), to: nil)
        let screenRect = window?.convertToScreen(localRect) ?? NSRect(x: 0, y: 0, width: 1, height: 24)
        channel.send([
            "event": event,
            "screen_height_points": screenHeight,
            "view_origin_x": screenRect.minX,
            "view_origin_y": screenHeight - screenRect.maxY,
        ])
    }

    private static func plainString(from value: Any) -> Result<String, TextInputBridgeError> {
        if let attributed = value as? NSAttributedString {
            return .success(attributed.string)
        }
        if let text = value as? String {
            return .success(text)
        }
        return .failure(.unsupportedTextInputValue)
    }

    private static func jsonRange(_ range: NSRange) -> [String: Any] {
        let wire = WireRange.from(range)
        return ["location": wire.location, "length": wire.length]
    }
}

private final class ArcweftTextInputAppDelegate: NSObject, NSApplicationDelegate {
    private let channel = JsonLineChannel()
    private var view: ArcweftTextInputClientView?

    func applicationDidFinishLaunching(_ notification: Notification) {
        let options = CommandLineBridgeOptions.parse()
        let window = NSWindow(
            contentRect: NSRect(x: 200, y: 200, width: 720, height: 180),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = options.title
        let view = ArcweftTextInputClientView(frame: window.contentView?.bounds ?? NSRect(x: 0, y: 0, width: 720, height: 180), mode: options.mode, channel: channel)
        view.autoresizingMask = [.width, .height]
        window.contentView = view
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        window.makeFirstResponder(view)
        self.view = view
        startStateReader(view: view)
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }

    private func startStateReader(view: ArcweftTextInputClientView) {
        DispatchQueue.global(qos: .userInitiated).async { [weak view] in
            let decoder = JSONDecoder()
            while let line = readLine() {
                guard let data = line.data(using: .utf8) else {
                    continue
                }
                do {
                    let state = try decoder.decode(BridgeState.self, from: data)
                    DispatchQueue.main.async {
                        view?.apply(state)
                    }
                } catch {
                    self.channel.send(["event": "bridge_error", "message": "invalid_state:\(error)"])
                }
            }
            DispatchQueue.main.async {
                NSApp.terminate(nil)
            }
        }
    }
}

private struct CommandLineBridgeOptions {
    var mode: String
    var title: String

    static func parse() -> CommandLineBridgeOptions {
        var mode = "text-field"
        var title = "Arcweft macOS IME Sample"
        var iterator = CommandLine.arguments.dropFirst().makeIterator()
        while let argument = iterator.next() {
            switch argument {
            case "--mode":
                mode = iterator.next() ?? mode
            case "--title":
                title = iterator.next() ?? title
            default:
                continue
            }
        }
        return CommandLineBridgeOptions(mode: mode, title: title)
    }
}

private func substringForUtf16Range(_ range: NSRange, in text: String) -> String? {
    guard range.location != NSNotFound else {
        return nil
    }
    let utf16 = text.utf16
    guard range.location >= 0,
          range.length >= 0,
          range.location <= utf16.count,
          range.location + range.length <= utf16.count else {
        return nil
    }
    let start = utf16.index(utf16.startIndex, offsetBy: range.location)
    let end = utf16.index(start, offsetBy: range.length)
    guard let startIndex = String.Index(start, within: text),
          let endIndex = String.Index(end, within: text) else {
        return nil
    }
    return String(text[startIndex..<endIndex])
}

private func rangesOverlap(_ left: WireRange, _ right: WireRange) -> Bool {
    if left.location == nsNotFoundWire || right.location == nsNotFoundWire {
        return false
    }
    let leftEnd = left.location.saturatingAdd(left.length)
    let rightEnd = right.location.saturatingAdd(right.length)
    return left.location < rightEnd && right.location < leftEnd
}

private extension UInt64 {
    func saturatingAdd(_ other: UInt64) -> UInt64 {
        let (value, overflow) = addingReportingOverflow(other)
        return overflow ? UInt64.max : value
    }
}

let app = NSApplication.shared
let delegate = ArcweftTextInputAppDelegate()
app.delegate = delegate
app.setActivationPolicy(.regular)
app.run()
