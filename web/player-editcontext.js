const STATUS_EVENT = "arcweft-text-input-status";
const RENDER_EVENT = "arcweft-text-input-render";
const HOST_CLASS = "arcweft-editcontext-host";

export function createArcweftEditContextPlayerGlue(host, options = {}) {
  const resolvedHost = typeof host === "string" ? document.getElementById(host) : host;
  if (!resolvedHost) {
    throw new Error(`Arcweft text input host not found: ${String(host)}`);
  }
  return new ArcweftEditContextPlayerGlue(resolvedHost, options);
}

export class ArcweftEditContextPlayerGlue {
  #host;
  #editContext = null;
  #listeners = [];
  #statusTarget;
  #delegate;
  #mirror;
  #state;

  constructor(host, options = {}) {
    const initialText = String(options.initialText ?? "");
    const initialEnd = utf16Length(initialText);
    this.#host = host;
    this.#statusTarget = options.statusTarget ?? document;
    this.#delegate = options.delegate ?? null;
    this.#mirror = resolveMirror(options.mirror ?? null, host);
    this.#state = {
      hostId: host.id || options.hostId || "arcweft-editcontext-host",
      secure: Boolean(options.secure),
      text: initialText,
      selectionStart: initialEnd,
      selectionEnd: initialEnd,
      selectionAnchor: initialEnd,
      composing: false,
      compositionStart: initialEnd,
      compositionEnd: initialEnd,
      textFormats: [],
      installed: false,
      fallbackInstalled: false,
      pointerSelection: null,
      lastGeometry: null,
      lastCharacterBounds: [],
    };
  }

  async install() {
    if (!this.#isSupported()) {
      this.#emitStatus("unsupported_no_fallback", {
        api: "edit_context",
        message: "Web EditContext is unavailable; Arcweft did not install a DOM editing substitute.",
      });
      return this;
    }

    this.#host.classList.add(HOST_CLASS);
    if (!this.#host.hasAttribute("tabindex")) {
      this.#host.tabIndex = 0;
    }

    this.#editContext = new window.EditContext({ text: this.#state.secure ? "" : this.#state.text });
    this.#host.editContext = this.#editContext;
    this.#state.installed = true;

    this.#syncEditContextText();
    this.#syncEditContextGeometry();

    this.#installListener(this.#host, "focus", () => {
      if (typeof this.#editContext?.focus === "function") {
        this.#editContext.focus();
      }
      this.#syncEditContextGeometry();
      this.#emitStatus("focused");
    });
    this.#installListener(this.#host, "blur", () => this.#emitStatus("blurred"));
    this.#installListener(this.#host, "keydown", (event) => {
      this.#handleKeyDown(event).catch((error) => this.#emitError(error));
    });
    this.#installListener(this.#host, "pointerdown", (event) => {
      this.#handlePointerDown(event).catch((error) => this.#emitError(error));
    });
    this.#installListener(this.#host, "pointermove", (event) => {
      this.#handlePointerMove(event).catch((error) => this.#emitError(error));
    });
    this.#installListener(this.#host, "pointerup", (event) => {
      this.#handlePointerUp(event).catch((error) => this.#emitError(error));
    });
    this.#installListener(this.#host, "pointercancel", (event) => {
      this.#handlePointerCancel(event);
    });
    this.#installListener(this.#host, "copy", (event) => this.#handleCopy(event));
    this.#installListener(this.#host, "cut", (event) => {
      this.#handleCut(event).catch((error) => this.#emitError(error));
    });
    this.#installListener(this.#host, "paste", (event) => {
      this.#handlePaste(event).catch((error) => this.#emitError(error));
    });
    this.#installListener(window, "scroll", () => this.#syncEditContextGeometry(), { passive: true });
    this.#installListener(window, "resize", () => this.#syncEditContextGeometry(), { passive: true });

    this.#installListener(this.#editContext, "compositionstart", async () => {
      this.#state.composing = true;
      await this.#delegateCall("compositionStart", this.#state.hostId);
      this.#emitStatus("composition_start");
      this.#renderMirror();
    });
    this.#installListener(this.#editContext, "compositionend", async () => {
      this.#state.composing = false;
      this.#state.compositionStart = this.#state.selectionStart;
      this.#state.compositionEnd = this.#state.selectionEnd;
      await this.#delegateCall("compositionEnd", this.#state.hostId, false);
      this.#emitStatus("composition_end");
      this.#renderMirror();
    });
    this.#installListener(this.#editContext, "textupdate", (event) => {
      this.#handleTextUpdate(event).catch((error) => this.#emitError(error));
    });
    this.#installListener(this.#editContext, "textformatupdate", (event) => {
      this.#state.textFormats = typeof event.getTextFormats === "function" ? event.getTextFormats() : [];
      this.#emitStatus("format_update", { formatCount: this.#state.textFormats.length });
      this.#renderMirror();
    });
    this.#installListener(this.#editContext, "characterboundsupdate", (event) => {
      this.#handleCharacterBoundsUpdate(event);
    });

    this.#renderMirror();
    this.#emitStatus("ready", { api: "edit_context" });
    return this;
  }

  dispose() {
    for (const listener of this.#listeners.splice(0)) {
      listener.target.removeEventListener(listener.type, listener.callback, listener.options);
    }
    if (this.#host.editContext === this.#editContext) {
      this.#host.editContext = null;
    }
    this.#editContext = null;
    this.#state.installed = false;
    this.#emitStatus("deactivated");
  }

  updateFromRuntimeSnapshot(snapshot) {
    if (!snapshot) {
      return;
    }
    if (!this.#state.secure) {
      const text = String(snapshot.text ?? snapshot.surroundingText ?? "");
      this.#state.text = text;
      this.#state.selectionStart = numberOr(snapshot.selectionStart, utf16Length(text));
      this.#state.selectionEnd = numberOr(snapshot.selectionEnd, this.#state.selectionStart);
      this.#state.selectionAnchor = this.#state.selectionStart;
      this.#state.compositionStart = numberOr(snapshot.compositionStart, this.#state.selectionStart);
      this.#state.compositionEnd = numberOr(snapshot.compositionEnd, this.#state.selectionEnd);
    }
    this.#syncEditContextText();
    this.#syncEditContextGeometry();
    this.#renderMirror();
  }

  applyRuntimeCommand(command) {
    if (!command) {
      return;
    }
    switch (command.kind) {
      case "activate":
        this.updateFromRuntimeSnapshot(command.snapshot);
        this.#emitStatus("runtime_activated");
        break;
      case "update_snapshot":
        this.updateFromRuntimeSnapshot(command.snapshot);
        break;
      case "update_geometry":
        this.updateGeometry(command.geometry);
        break;
      case "commit_composition":
        this.#state.composing = false;
        this.#state.compositionStart = this.#state.selectionStart;
        this.#state.compositionEnd = this.#state.selectionEnd;
        this.#emitStatus("runtime_composition_commit", { session: command.session });
        this.#renderMirror();
        break;
      case "cancel_composition":
        this.#state.composing = false;
        this.#emitStatus("runtime_composition_cancel", { session: command.session });
        this.#renderMirror();
        break;
      case "deactivate":
        this.#state.installed = false;
        this.#state.composing = false;
        this.#emitStatus("runtime_deactivated", { session: command.session });
        this.#renderMirror();
        break;
      case "unsupported_no_fallback":
        this.#emitStatus("unsupported_no_fallback", {
          api: "edit_context",
          message: command.message,
        });
        break;
      default:
        this.#emitStatus("runtime_command_ignored", { kind: command.kind });
        break;
    }
  }

  updateGeometry(geometry) {
    this.#state.lastGeometry = normalizeRuntimeGeometry(geometry);
    this.#syncEditContextGeometry();
    this.#emitStatus("geometry_update");
  }

  status() {
    return {
      hostId: this.#state.hostId,
      state: this.#state.installed ? "ready" : "idle",
      secure: this.#state.secure,
      fallbackInstalled: false,
      selectionStart: this.#state.secure ? 0 : this.#state.selectionStart,
      selectionEnd: this.#state.secure ? 0 : this.#state.selectionEnd,
      composing: this.#state.composing,
      caretRect: this.#state.secure ? null : this.#selectionBounds(),
    };
  }

  #isSupported() {
    return typeof window.EditContext === "function" && "editContext" in this.#host;
  }

  #installListener(target, type, callback, options) {
    target.addEventListener(type, callback, options);
    this.#listeners.push({ target, type, callback, options });
  }

  async #handleTextUpdate(event) {
    const update = readTextUpdate(event);
    const observedTextBefore = this.#state.text;
    const composing = readComposing(this.#editContext, event, this.#state.composing);
    await this.#delegateCall("dispatchTextUpdate", this.#state.hostId, {
      updateRangeStart: update.updateRangeStart,
      updateRangeEnd: update.updateRangeEnd,
      text: update.text,
      selectionStart: update.selectionStart,
      selectionEnd: update.selectionEnd,
      observedTextBefore,
      composing,
    });

    this.#state.text = replaceUtf16Range(
      this.#state.text,
      update.updateRangeStart,
      update.updateRangeEnd,
      update.text,
    );
    this.#state.selectionStart = update.selectionStart;
    this.#state.selectionEnd = update.selectionEnd;
    this.#state.selectionAnchor = update.selectionStart;
    this.#state.composing = composing;
    this.#state.compositionStart = numberOr(event.compositionStart, update.selectionStart);
    this.#state.compositionEnd = numberOr(event.compositionEnd, update.selectionEnd);
    this.#syncEditContextText();
    this.#syncEditContextGeometry();
    this.#renderMirror();
    this.#emitStatus(composing ? "composition_update" : "committed_update");
  }

  #handleCharacterBoundsUpdate(event) {
    if (!this.#editContext || this.#state.secure) {
      this.#emitStatus("character_bounds_redacted");
      return;
    }
    const rangeStart = numberOr(event.rangeStart, this.#state.compositionStart);
    const rangeEnd = numberOr(event.rangeEnd, this.#state.compositionEnd);
    const bounds = this.#computeCharacterBounds(rangeStart, rangeEnd);
    this.#state.lastCharacterBounds = bounds;
    callIfPresent(this.#editContext, "updateCharacterBounds", rangeStart, bounds);
    this.#emitStatus("character_bounds_update", {
      requestedRangeStart: rangeStart,
      requestedRangeEnd: rangeEnd,
      nonOrigin: bounds.some((rect) => rect.x !== 0 || rect.y !== 0),
    });
  }

  async #handleKeyDown(event) {
    const command = commandForKeyEvent(event);
    if (!command) {
      return;
    }
    if (this.#state.composing && command.suppressedDuringComposition) {
      event.preventDefault();
      this.#emitStatus("shortcut_suppressed_during_composition", { command: command.name });
      return;
    }
    event.preventDefault();
    await this.#delegateCall("dispatchCommand", this.#state.hostId, command.name, Boolean(command.selecting));
    await this.#applyLocalCommand(command, event);
    this.#syncEditContextText();
    this.#syncEditContextGeometry();
    this.#renderMirror();
    this.#emitStatus("command", { command: command.name, selecting: Boolean(command.selecting) });
  }

  async #applyLocalCommand(command, event) {
    switch (command.name) {
      case "move_left":
        this.#moveCaret(previousGraphemeOffset(this.#state.text, caretPosition(this.#state)), command.selecting);
        await this.#dispatchSelectionUpdate();
        break;
      case "move_right":
        this.#moveCaret(nextGraphemeOffset(this.#state.text, caretPosition(this.#state)), command.selecting);
        await this.#dispatchSelectionUpdate();
        break;
      case "move_word_left":
        this.#moveCaret(previousWordOffset(this.#state.text, caretPosition(this.#state)), command.selecting);
        await this.#dispatchSelectionUpdate();
        break;
      case "move_word_right":
        this.#moveCaret(nextWordOffset(this.#state.text, caretPosition(this.#state)), command.selecting);
        await this.#dispatchSelectionUpdate();
        break;
      case "move_line_start":
        this.#moveCaret(lineStartOffset(this.#state.text, caretPosition(this.#state)), command.selecting);
        await this.#dispatchSelectionUpdate();
        break;
      case "move_line_end":
        this.#moveCaret(lineEndOffset(this.#state.text, caretPosition(this.#state)), command.selecting);
        await this.#dispatchSelectionUpdate();
        break;
      case "backspace":
        await this.#deleteBackward();
        break;
      case "delete":
        await this.#deleteForward();
        break;
      case "select_all":
        this.#setSelection(0, utf16Length(this.#state.text), 0);
        await this.#dispatchSelectionUpdate();
        break;
      case "copy":
        this.#writeClipboard(event);
        break;
      case "cut":
        this.#writeClipboard(event);
        await this.#replaceSelection("");
        break;
      case "paste":
        await this.#pasteFromEvent(event);
        break;
      case "submit":
      case "cancel":
        break;
      default:
        break;
    }
  }

  async #handlePointerDown(event) {
    if (this.#state.secure) {
      this.#emitStatus("pointer_geometry_redacted");
      return;
    }
    event.preventDefault();
    this.#host.focus();
    const offset = this.#offsetForClientPoint(event.clientX, event.clientY);
    this.#state.pointerSelection = { pointerId: event.pointerId, anchor: offset };
    this.#host.setPointerCapture?.(event.pointerId);
    this.#setSelection(offset, offset, offset);
    await this.#dispatchSelectionUpdate();
    this.#syncEditContextText();
    this.#syncEditContextGeometry();
    this.#renderMirror();
    this.#emitStatus("pointer_down", { offset });
  }

  async #handlePointerMove(event) {
    const active = this.#state.pointerSelection;
    if (!active || active.pointerId !== event.pointerId) {
      return;
    }
    event.preventDefault();
    const offset = this.#offsetForClientPoint(event.clientX, event.clientY);
    this.#setSelection(Math.min(active.anchor, offset), Math.max(active.anchor, offset), active.anchor);
    await this.#dispatchSelectionUpdate();
    this.#syncEditContextText();
    this.#syncEditContextGeometry();
    this.#renderMirror();
    this.#emitStatus("pointer_drag", { offset });
  }

  async #handlePointerUp(event) {
    const active = this.#state.pointerSelection;
    if (!active || active.pointerId !== event.pointerId) {
      return;
    }
    event.preventDefault();
    this.#host.releasePointerCapture?.(event.pointerId);
    this.#state.pointerSelection = null;
    await this.#dispatchSelectionUpdate();
    this.#emitStatus("pointer_up");
  }

  #handlePointerCancel(event) {
    if (this.#state.pointerSelection?.pointerId === event.pointerId) {
      this.#state.pointerSelection = null;
      this.#emitStatus("pointer_cancel");
    }
  }

  #handleCopy(event) {
    if (this.#state.secure) {
      event.preventDefault();
      this.#emitStatus("clipboard_redacted");
      return;
    }
    this.#writeClipboard(event);
  }

  async #handleCut(event) {
    if (this.#state.secure) {
      event.preventDefault();
      this.#emitStatus("clipboard_redacted");
      return;
    }
    this.#writeClipboard(event);
    await this.#replaceSelection("");
    this.#syncEditContextText();
    this.#syncEditContextGeometry();
    this.#renderMirror();
  }

  async #handlePaste(event) {
    if (this.#state.secure) {
      event.preventDefault();
      this.#emitStatus("clipboard_redacted");
      return;
    }
    await this.#pasteFromEvent(event);
    this.#syncEditContextText();
    this.#syncEditContextGeometry();
    this.#renderMirror();
  }

  #writeClipboard(event) {
    if (this.#state.secure) {
      event.preventDefault();
      return;
    }
    const text = selectedText(this.#state);
    event.clipboardData?.setData?.("text/plain", text);
    event.preventDefault();
    this.#emitStatus("clipboard_write", { textLength: utf16Length(text) });
  }

  async #pasteFromEvent(event) {
    event.preventDefault();
    const text = event.clipboardData?.getData?.("text/plain") ?? "";
    await this.#replaceSelection(text);
    this.#emitStatus("paste", { textLength: utf16Length(text) });
  }

  async #deleteBackward() {
    if (!selectionCollapsed(this.#state)) {
      await this.#replaceSelection("");
      return;
    }
    const caret = caretPosition(this.#state);
    const start = previousGraphemeOffset(this.#state.text, caret);
    await this.#replaceRange(start, caret, "");
  }

  async #deleteForward() {
    if (!selectionCollapsed(this.#state)) {
      await this.#replaceSelection("");
      return;
    }
    const caret = caretPosition(this.#state);
    const end = nextGraphemeOffset(this.#state.text, caret);
    await this.#replaceRange(caret, end, "");
  }

  async #replaceSelection(replacement) {
    const start = Math.min(this.#state.selectionStart, this.#state.selectionEnd);
    const end = Math.max(this.#state.selectionStart, this.#state.selectionEnd);
    await this.#replaceRange(start, end, replacement);
  }

  async #replaceRange(start, end, replacement) {
    const observedTextBefore = this.#state.text;
    await this.#delegateCall("dispatchTextUpdate", this.#state.hostId, {
      updateRangeStart: start,
      updateRangeEnd: end,
      text: replacement,
      selectionStart: start + utf16Length(replacement),
      selectionEnd: start + utf16Length(replacement),
      observedTextBefore,
      composing: false,
    });
    this.#state.text = replaceUtf16Range(this.#state.text, start, end, replacement);
    const caret = start + utf16Length(replacement);
    this.#setSelection(caret, caret, caret);
    this.#state.composing = false;
    this.#state.compositionStart = caret;
    this.#state.compositionEnd = caret;
  }

  async #dispatchSelectionUpdate() {
    if (this.#state.secure) {
      return;
    }
    await this.#delegateCall("dispatchTextUpdate", this.#state.hostId, {
      updateRangeStart: this.#state.selectionStart,
      updateRangeEnd: this.#state.selectionStart,
      text: "",
      selectionStart: this.#state.selectionStart,
      selectionEnd: this.#state.selectionEnd,
      observedTextBefore: this.#state.text,
      composing: this.#state.composing,
    });
  }

  #moveCaret(offset, selecting) {
    const bounded = clampOffset(this.#state.text, offset);
    if (selecting) {
      this.#setSelection(
        Math.min(this.#state.selectionAnchor, bounded),
        Math.max(this.#state.selectionAnchor, bounded),
        this.#state.selectionAnchor,
      );
    } else {
      this.#setSelection(bounded, bounded, bounded);
    }
  }

  #setSelection(start, end, anchor = start) {
    const textLength = utf16Length(this.#state.text);
    this.#state.selectionStart = Math.max(0, Math.min(textLength, start));
    this.#state.selectionEnd = Math.max(0, Math.min(textLength, end));
    this.#state.selectionAnchor = Math.max(0, Math.min(textLength, anchor));
  }

  #offsetForClientPoint(clientX, clientY) {
    const runtimeOffset = runtimeOffsetForPoint(this.#state.lastGeometry, clientX, clientY);
    if (runtimeOffset !== null) {
      return nearestGraphemeOffset(this.#state.text, runtimeOffset);
    }
    const rect = this.#host.getBoundingClientRect();
    const textLength = utf16Length(this.#state.text);
    if (textLength === 0) {
      return 0;
    }
    const advance = this.#characterAdvance(rect);
    const raw = Math.round((clientX - rect.left - this.#textInsetX()) / advance);
    return nearestGraphemeOffset(this.#state.text, Math.max(0, Math.min(textLength, raw)));
  }

  #syncEditContextText() {
    if (!this.#editContext || this.#state.secure) {
      return;
    }
    const current = String(this.#editContext.text ?? "");
    if (current !== this.#state.text) {
      callIfPresent(this.#editContext, "updateText", 0, utf16Length(current), this.#state.text);
    }
    callIfPresent(this.#editContext, "updateSelection", this.#state.selectionStart, this.#state.selectionEnd);
  }

  #syncEditContextGeometry() {
    if (!this.#editContext) {
      return;
    }
    const controlRect = this.#controlBounds();
    callIfPresent(this.#editContext, "updateControlBounds", controlRect);
    const selectionRect = this.#selectionBounds();
    callIfPresent(this.#editContext, "updateSelectionBounds", selectionRect);
    if (!this.#state.secure) {
      const rangeStart = Math.min(this.#state.compositionStart, this.#state.selectionStart, this.#state.selectionEnd);
      const rangeEnd = Math.max(this.#state.compositionEnd, this.#state.selectionStart, this.#state.selectionEnd, rangeStart + 1);
      const characterBounds = this.#computeCharacterBounds(rangeStart, rangeEnd);
      this.#state.lastCharacterBounds = characterBounds;
      callIfPresent(this.#editContext, "updateCharacterBounds", rangeStart, characterBounds);
    }
  }

  #controlBounds() {
    if (this.#state.lastGeometry?.controlRect) {
      return toDomRect(this.#state.lastGeometry.controlRect);
    }
    const rect = this.#host.getBoundingClientRect();
    return toDomRect(rect);
  }

  #selectionBounds() {
    const start = Math.min(this.#state.selectionStart, this.#state.selectionEnd);
    const end = Math.max(this.#state.selectionStart, this.#state.selectionEnd);
    if (start !== end) {
      const runtimeSelection = runtimeRectsForRange(this.#state.lastGeometry?.selectionRects, start, end);
      if (runtimeSelection.length > 0) {
        return unionRects(runtimeSelection.map((entry) => entry.rect));
      }
      return unionRects(this.#computeCharacterBounds(start, end));
    }
    if (this.#state.lastGeometry?.caretRect) {
      return toDomRect(this.#state.lastGeometry.caretRect);
    }
    return this.#rectForOffset(start);
  }

  #computeCharacterBounds(rangeStart, rangeEnd) {
    const runtimeBounds = runtimeRectsForRange(this.#state.lastGeometry?.characterBounds, rangeStart, rangeEnd);
    if (runtimeBounds.length > 0) {
      return runtimeBounds.map((entry) => toDomRect(entry.rect));
    }
    const start = clampOffset(this.#state.text, rangeStart);
    const end = clampOffset(this.#state.text, Math.max(rangeEnd, start));
    const offsets = graphemeOffsets(this.#state.text).filter((offset) => offset >= start && offset < end);
    if (offsets.length === 0) {
      return [this.#rectForOffset(start)];
    }
    return offsets.map((offset) => this.#rectForOffset(offset, this.#nextOffsetForBounds(offset)));
  }

  #nextOffsetForBounds(offset) {
    const next = nextGraphemeOffset(this.#state.text, offset);
    return next === offset ? offset + 1 : next;
  }

  #rectForOffset(offset, endOffset = offset) {
    const rect = this.#host.getBoundingClientRect();
    const advance = this.#characterAdvance(rect);
    const slot = graphemeSlotForOffset(this.#state.text, offset);
    const widthSlots = Math.max(1, graphemeSlotForOffset(this.#state.text, endOffset) - slot);
    return toDomRect({
      x: rect.left + this.#textInsetX() + slot * advance,
      y: rect.top + this.#textInsetY(),
      width: Math.max(1, widthSlots * advance),
      height: Math.max(1, rect.height - this.#textInsetY() * 2),
    });
  }

  #characterAdvance(rect = this.#host.getBoundingClientRect()) {
    const length = Math.max(1, graphemeOffsets(this.#state.text).length - 1);
    return Math.max(1, (rect.width - this.#textInsetX() * 2) / Math.max(8, length));
  }

  #textInsetX() {
    return numberOr(this.#state.lastGeometry?.textInsetX, 12);
  }

  #textInsetY() {
    return numberOr(this.#state.lastGeometry?.textInsetY, 8);
  }

  #renderMirror() {
    const detail = {
      secure: this.#state.secure,
      composing: this.#state.composing,
      textLength: this.#state.secure ? 0 : utf16Length(this.#state.text),
      selectionStart: this.#state.secure ? 0 : this.#state.selectionStart,
      selectionEnd: this.#state.secure ? 0 : this.#state.selectionEnd,
      caretRect: this.#state.secure ? null : this.#selectionBounds(),
    };
    if (!this.#state.secure) {
      detail.text = this.#state.text;
      detail.compositionStart = this.#state.compositionStart;
      detail.compositionEnd = this.#state.compositionEnd;
    }
    this.#statusTarget.dispatchEvent(new CustomEvent(RENDER_EVENT, { detail }));

    if (!this.#mirror || this.#state.secure) {
      return;
    }
    this.#mirror.committed.innerHTML = renderSelectionMarkup(this.#state);
    this.#mirror.composition.textContent = "";
    if (this.#mirror.caret) {
      const caretRect = this.#selectionBounds();
      const hostRect = this.#host.getBoundingClientRect();
      this.#mirror.caret.style.setProperty("--arcweft-caret-x", `${caretRect.x - hostRect.left}px`);
      this.#mirror.caret.style.setProperty("--arcweft-caret-y", `${caretRect.y - hostRect.top}px`);
      this.#mirror.caret.style.setProperty("--arcweft-caret-h", `${caretRect.height}px`);
    }
  }

  async #delegateCall(method, ...args) {
    const fn = this.#delegate?.[method];
    if (typeof fn !== "function") {
      return null;
    }
    return await fn(...args);
  }

  #emitStatus(state, extra = {}) {
    const detail = {
      owner: "arcweft-player",
      hostId: this.#state.hostId,
      state,
      secure: this.#state.secure,
      fallbackInstalled: false,
      composing: this.#state.composing,
      selectionStart: this.#state.secure ? 0 : this.#state.selectionStart,
      selectionEnd: this.#state.secure ? 0 : this.#state.selectionEnd,
      ...extra,
    };
    this.#statusTarget.dispatchEvent(new CustomEvent(STATUS_EVENT, { detail }));
  }

  #emitError(error) {
    this.#emitStatus("error", { message: String(error?.message ?? error) });
  }
}

function resolveMirror(mirror, host) {
  const committed = typeof mirror?.committedTextId === "string"
    ? document.getElementById(mirror.committedTextId)
    : mirror?.committed ?? host.querySelector?.(".committed-text");
  const composition = typeof mirror?.compositionTextId === "string"
    ? document.getElementById(mirror.compositionTextId)
    : mirror?.composition ?? host.querySelector?.(".composition-text");
  const caret = typeof mirror?.caretId === "string"
    ? document.getElementById(mirror.caretId)
    : mirror?.caret ?? host.querySelector?.(".caret");
  return committed && composition ? { committed, composition, caret } : null;
}

function readTextUpdate(event) {
  return {
    updateRangeStart: numberOr(event.updateRangeStart, 0),
    updateRangeEnd: numberOr(event.updateRangeEnd, 0),
    text: String(event.text ?? ""),
    selectionStart: numberOr(event.selectionStart, 0),
    selectionEnd: numberOr(event.selectionEnd, 0),
  };
}

function readComposing(editContext, event, fallback) {
  if (typeof event.compositionStart === "number" && typeof event.compositionEnd === "number") {
    return event.compositionStart !== event.compositionEnd;
  }
  if (typeof editContext?.isComposing === "boolean") {
    return editContext.isComposing;
  }
  return Boolean(fallback);
}

function normalizeRuntimeGeometry(geometry) {
  if (!geometry) {
    return null;
  }
  return {
    controlRect: geometry.controlRect ? toDomRectInit(geometry.controlRect) : null,
    caretRect: geometry.caretRect ? toDomRectInit(geometry.caretRect) : null,
    selectionRects: normalizeRuntimeRangeRects(geometry.selectionRects),
    compositionRects: normalizeRuntimeRangeRects(geometry.compositionRects),
    characterBounds: normalizeRuntimeRangeRects(geometry.characterBounds),
    textInsetX: geometry.textInsetX,
    textInsetY: geometry.textInsetY,
  };
}

function normalizeRuntimeRangeRects(rects = []) {
  return Array.isArray(rects) ? rects
    .map((entry) => ({
      start: numberOr(entry?.range?.start, numberOr(entry?.start, 0)),
      end: numberOr(entry?.range?.end, numberOr(entry?.end, 0)),
      rect: toDomRectInit(entry?.rect ?? entry?.bounds),
    }))
    .filter((entry) => entry.end >= entry.start)
    : [];
}

function runtimeRectsForRange(rects = [], start, end) {
  if (!Array.isArray(rects) || rects.length === 0) {
    return [];
  }
  const lo = Math.min(start, end);
  const hi = Math.max(start, end);
  return rects.filter((entry) => entry.start < hi && lo < entry.end);
}

function runtimeOffsetForPoint(geometry, x, y) {
  const bounds = geometry?.characterBounds ?? [];
  if (bounds.length === 0) {
    return null;
  }
  for (const entry of bounds) {
    const rect = entry.rect;
    if (y >= rect.y && y <= rect.y + rect.height && x <= rect.x + rect.width * 0.5) {
      return entry.start;
    }
    if (y >= rect.y && y <= rect.y + rect.height && x <= rect.x + rect.width) {
      return entry.end;
    }
  }
  const last = bounds[bounds.length - 1];
  return last?.end ?? null;
}

function commandForKeyEvent(event) {
  const selecting = Boolean(event.shiftKey);
  const shortcut = event.metaKey || event.ctrlKey;
  if (shortcut && !event.altKey) {
    switch (event.key.toLowerCase()) {
      case "a":
        return { name: "select_all", suppressedDuringComposition: true };
      case "c":
        return { name: "copy", suppressedDuringComposition: true };
      case "x":
        return { name: "cut", suppressedDuringComposition: true };
      case "v":
        return { name: "paste", suppressedDuringComposition: true };
      default:
        break;
    }
  }
  switch (event.key) {
    case "ArrowLeft":
      return { name: event.altKey || event.ctrlKey ? "move_word_left" : "move_left", selecting };
    case "ArrowRight":
      return { name: event.altKey || event.ctrlKey ? "move_word_right" : "move_right", selecting };
    case "Home":
      return { name: "move_line_start", selecting };
    case "End":
      return { name: "move_line_end", selecting };
    case "Backspace":
      return { name: "backspace" };
    case "Delete":
      return { name: "delete" };
    case "Enter":
      return { name: "submit" };
    case "Escape":
      return { name: "cancel", suppressedDuringComposition: true };
    default:
      return null;
  }
}

function replaceUtf16Range(text, start, end, replacement) {
  return text.slice(0, start) + replacement + text.slice(end);
}

function selectedText(state) {
  if (state.secure) {
    return "";
  }
  const start = Math.min(state.selectionStart, state.selectionEnd);
  const end = Math.max(state.selectionStart, state.selectionEnd);
  return state.text.slice(start, end);
}

function selectionCollapsed(state) {
  return state.selectionStart === state.selectionEnd;
}

function caretPosition(state) {
  return state.selectionEnd;
}

function utf16Length(text) {
  return String(text).length;
}

function clampOffset(text, offset) {
  return Math.max(0, Math.min(utf16Length(text), offset));
}

function graphemeOffsets(text) {
  const source = String(text);
  if (typeof Intl !== "undefined" && typeof Intl.Segmenter === "function") {
    const segmenter = new Intl.Segmenter(undefined, { granularity: "grapheme" });
    const offsets = [0];
    for (const segment of segmenter.segment(source)) {
      if (segment.index !== 0) {
        offsets.push(segment.index);
      }
    }
    const end = utf16Length(source);
    if (offsets[offsets.length - 1] !== end) {
      offsets.push(end);
    }
    return offsets;
  }
  const offsets = [0];
  for (let index = 0; index < source.length;) {
    const code = source.codePointAt(index);
    index += code > 0xffff ? 2 : 1;
    while (index < source.length && isCombiningMark(source.codePointAt(index))) {
      index += source.codePointAt(index) > 0xffff ? 2 : 1;
    }
    offsets.push(index);
  }
  return offsets;
}

function previousGraphemeOffset(text, offset) {
  const bounded = clampOffset(text, offset);
  return graphemeOffsets(text).filter((candidate) => candidate < bounded).pop() ?? 0;
}

function nextGraphemeOffset(text, offset) {
  const bounded = clampOffset(text, offset);
  return graphemeOffsets(text).find((candidate) => candidate > bounded) ?? utf16Length(text);
}

function nearestGraphemeOffset(text, offset) {
  const offsets = graphemeOffsets(text);
  return offsets.reduce((best, candidate) => (
    Math.abs(candidate - offset) < Math.abs(best - offset) ? candidate : best
  ), offsets[0] ?? 0);
}

function graphemeSlotForOffset(text, offset) {
  const nearest = nearestGraphemeOffset(text, offset);
  return graphemeOffsets(text).findIndex((candidate) => candidate === nearest);
}

function previousWordOffset(text, offset) {
  const source = String(text).slice(0, clampOffset(text, offset));
  const match = source.match(/[\p{Letter}\p{Number}_]+\W*$/u);
  return match ? source.length - match[0].length : 0;
}

function nextWordOffset(text, offset) {
  const source = String(text);
  const tail = source.slice(clampOffset(text, offset));
  const match = tail.match(/^\W*[\p{Letter}\p{Number}_]+/u);
  return match ? clampOffset(text, offset) + match[0].length : utf16Length(source);
}

function lineStartOffset(text, offset) {
  const source = String(text);
  const bounded = clampOffset(source, offset);
  const lineBreak = source.lastIndexOf("\n", bounded - 1);
  return lineBreak < 0 ? 0 : lineBreak + 1;
}

function lineEndOffset(text, offset) {
  const source = String(text);
  const bounded = clampOffset(source, offset);
  const lineBreak = source.indexOf("\n", bounded);
  return lineBreak < 0 ? utf16Length(source) : lineBreak;
}

function isCombiningMark(codePoint) {
  return (codePoint >= 0x0300 && codePoint <= 0x036f) ||
    (codePoint >= 0x1ab0 && codePoint <= 0x1aff) ||
    (codePoint >= 0x1dc0 && codePoint <= 0x1dff) ||
    (codePoint >= 0x20d0 && codePoint <= 0x20ff) ||
    (codePoint >= 0xfe20 && codePoint <= 0xfe2f);
}

function renderSelectionMarkup(state) {
  const start = Math.min(state.selectionStart, state.selectionEnd);
  const end = Math.max(state.selectionStart, state.selectionEnd);
  const compStart = Math.min(state.compositionStart, state.compositionEnd);
  const compEnd = Math.max(state.compositionStart, state.compositionEnd);
  let html = "";
  let cursor = 0;
  for (const boundary of sortedUnique([start, end, compStart, compEnd, utf16Length(state.text)])) {
    if (boundary < cursor) {
      continue;
    }
    const part = escapeHtml(state.text.slice(cursor, boundary));
    const selected = cursor < end && boundary > start;
    const composing = state.composing && cursor < compEnd && boundary > compStart;
    if (part) {
      const classes = [selected ? "arcweft-selection" : "", composing ? "arcweft-composition" : ""]
        .filter(Boolean)
        .join(" ");
      html += classes ? `<span class="${classes}">${part}</span>` : part;
    }
    cursor = boundary;
  }
  return html;
}

function sortedUnique(values) {
  return [...new Set(values.filter((value) => Number.isFinite(value)))].sort((a, b) => a - b);
}

function escapeHtml(value) {
  return String(value).replace(/[&<>"]/g, (ch) => ({
    "&": "&amp;",
    "<": "&lt;",
    ">": "&gt;",
    "\"": "&quot;",
  })[ch]);
}

function unionRects(rects) {
  if (!rects.length) {
    return toDomRect({ x: 0, y: 0, width: 1, height: 1 });
  }
  const left = Math.min(...rects.map((rect) => rect.x));
  const top = Math.min(...rects.map((rect) => rect.y));
  const right = Math.max(...rects.map((rect) => rect.x + rect.width));
  const bottom = Math.max(...rects.map((rect) => rect.y + rect.height));
  return toDomRect({ x: left, y: top, width: Math.max(1, right - left), height: Math.max(1, bottom - top) });
}

function numberOr(value, fallback) {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function callIfPresent(target, method, ...args) {
  if (typeof target?.[method] === "function") {
    target[method](...args);
  }
}

function toDomRectInit(rect = {}) {
  return {
    x: numberOr(rect.x, numberOr(rect.left, 0)),
    y: numberOr(rect.y, numberOr(rect.top, 0)),
    width: Math.max(0, numberOr(rect.width, 0)),
    height: Math.max(0, numberOr(rect.height, 0)),
  };
}

function toDomRect(rect = {}) {
  const init = toDomRectInit(rect);
  const DomRectType = typeof DOMRect === "function"
    ? DOMRect
    : typeof window !== "undefined" && typeof window.DOMRect === "function"
      ? window.DOMRect
      : null;
  return DomRectType
    ? new DomRectType(init.x, init.y, init.width, init.height)
    : init;
}
