export class ArcweftClipboardAdapter {
  constructor(options = {}) {
    this.navigatorClipboard = options.navigatorClipboard ?? globalThis.navigator?.clipboard ?? null;
    this.secureContext = Boolean(options.secureContext ?? globalThis.isSecureContext);
    this.statusTarget = options.statusTarget ?? globalThis.document ?? null;
  }

  async apply(request, event = null) {
    switch (request?.operation) {
      case "copy":
      case "cut":
        return await this.writeText(request, event);
      case "paste":
        return await this.readText(request, event);
      case "clear":
        return this.failure(request, "unavailable", "web clipboard clear is not exposed as a stable text-control operation");
      default:
        return this.failure(request, "internal_failure", "unknown clipboard operation");
    }
  }

  async writeText(request, event) {
    const text = normalizeClipboardNewlines(String(request?.text ?? ""));
    if (event?.clipboardData?.setData) {
      event.clipboardData.setData("text/plain", text);
      event.preventDefault?.();
      this.emit("text_clipboard.host_result", request, { status: "event_write" });
      return { requestId: request.requestId, kind: "write_committed" };
    }
    if (!this.secureContext || !this.navigatorClipboard?.writeText) {
      return this.failure(request, "unavailable", "navigator.clipboard.writeText unavailable");
    }
    try {
      await this.navigatorClipboard.writeText(text);
      this.emit("text_clipboard.host_result", request, { status: "api_write" });
      return { requestId: request.requestId, kind: "write_committed" };
    } catch (error) {
      return this.failure(request, webClipboardErrorKind(error), String(error?.name ?? error));
    }
  }

  async readText(request, event) {
    if (event?.clipboardData?.getData) {
      event.preventDefault?.();
      const text = normalizeClipboardNewlines(event.clipboardData.getData("text/plain") ?? "");
      this.emit("text_clipboard.host_result", request, { status: "event_read" });
      return { requestId: request.requestId, kind: "read_committed", text };
    }
    if (!this.secureContext || !this.navigatorClipboard?.readText) {
      return this.failure(request, "unavailable", "navigator.clipboard.readText unavailable");
    }
    try {
      const text = normalizeClipboardNewlines(await this.navigatorClipboard.readText());
      this.emit("text_clipboard.host_result", request, { status: "api_read" });
      return { requestId: request.requestId, kind: "read_committed", text };
    } catch (error) {
      return this.failure(request, webClipboardErrorKind(error), String(error?.name ?? error));
    }
  }

  failure(request, kind, diagnostic) {
    this.emit("text_clipboard.host_result", request, { status: "failed", kind });
    return { requestId: request?.requestId ?? 0, kind: "failed", error: { kind, diagnostic } };
  }

  emit(type, request, detail = {}) {
    this.statusTarget?.dispatchEvent?.(new CustomEvent(type, {
      detail: {
        owner: "arcweft-player",
        requestId: request?.requestId ?? 0,
        operation: request?.operation ?? "unknown",
        origin: request?.origin ?? "unknown",
        secure: Boolean(request?.secure),
        ...detail,
      },
    }));
  }
}

export function normalizeClipboardNewlines(text) {
  return String(text).replace(/\r\n/g, "\n").replace(/\r/g, "\n");
}

function webClipboardErrorKind(error) {
  switch (error?.name) {
    case "NotAllowedError":
    case "SecurityError":
      return "denied";
    case "NotFoundError":
      return "unsupported_format";
    default:
      return "internal_failure";
  }
}
