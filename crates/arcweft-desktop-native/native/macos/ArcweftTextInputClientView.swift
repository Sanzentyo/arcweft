// Reference host-side owner for AppKit. This file is intentionally outside
// Cargo's Rust compilation path. It shows the object that owns AppKit identity
// and converts AppKit values into the adapter-local Rust bridge ABI.
//
// The Rust overlay does not store NSView, NSTextInputContext, NSAttributedString,
// NSRange, Selector, or object identity in Sans I/O payloads.

import AppKit

final class ArcweftTextInputClientView: NSView, NSTextInputClient {
    private var selectedRangeValue = NSRange(location: NSNotFound, length: 0)
    private var markedRangeValue = NSRange(location: NSNotFound, length: 0)

    override var acceptsFirstResponder: Bool { true }

    override func keyDown(with event: NSEvent) {
        inputContext?.handleEvent(event)
    }

    func insertText(_ string: Any, replacementRange: NSRange) {
        let text = ArcweftTextInputClientView.plainString(from: string)
        arcweft_macos_text_input_insert_text(text, UInt64(replacementRange.location), UInt64(replacementRange.length))
    }

    func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
        let text = ArcweftTextInputClientView.plainString(from: string)
        arcweft_macos_text_input_set_marked_text(
            text,
            UInt64(selectedRange.location),
            UInt64(selectedRange.length),
            UInt64(replacementRange.location),
            UInt64(replacementRange.length)
        )
    }

    func unmarkText() {
        arcweft_macos_text_input_unmark_text()
    }

    func selectedRange() -> NSRange {
        selectedRangeValue
    }

    func markedRange() -> NSRange {
        markedRangeValue
    }

    func hasMarkedText() -> Bool {
        markedRangeValue.location != NSNotFound
    }

    func attributedSubstring(forProposedRange range: NSRange, actualRange: NSRangePointer?) -> NSAttributedString? {
        var actual = NSRange(location: NSNotFound, length: 0)
        guard let text = arcweft_macos_text_input_attributed_substring(UInt64(range.location), UInt64(range.length), &actual) else {
            actualRange?.pointee = NSRange(location: NSNotFound, length: 0)
            return nil
        }
        actualRange?.pointee = actual
        return NSAttributedString(string: text)
    }

    func firstRect(forCharacterRange range: NSRange, actualRange: NSRangePointer?) -> NSRect {
        var actual = NSRange(location: NSNotFound, length: 0)
        let rect = arcweft_macos_text_input_first_rect(UInt64(range.location), UInt64(range.length), &actual)
        actualRange?.pointee = actual
        return rect
    }

    func characterIndex(for point: NSPoint) -> Int {
        NSNotFound
    }

    func validAttributesForMarkedText() -> [NSAttributedString.Key] {
        []
    }

    private static func plainString(from value: Any) -> String {
        if let attributed = value as? NSAttributedString {
            return attributed.string
        }
        return String(describing: value)
    }
}
