import { createReadStream } from "node:fs";
import { open } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const root = normalize(join(fileURLToPath(new URL(".", import.meta.url)), ".."));
const contentTypes = new Map([
  [".css", "text/css; charset=utf-8"],
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
]);

function staticPath(requestUrl) {
  const url = new URL(requestUrl, "http://127.0.0.1");
  const pathname = decodeURIComponent(url.pathname);
  if (pathname.includes("\0")) {
    return null;
  }
  const fullPath = normalize(join(root, pathname === "/" ? "/ime-sample.html" : pathname));
  return fullPath.startsWith(root) ? fullPath : null;
}

async function serveFile(request, response) {
  const fullPath = staticPath(request.url);
  if (!fullPath) {
    response.writeHead(400);
    response.end("bad path");
    return;
  }
  try {
    const file = await open(fullPath, "r");
    await file.close();
  } catch {
    response.writeHead(404);
    response.end("not found");
    return;
  }
  response.writeHead(200, {
    "content-type": contentTypes.get(extname(fullPath).toLowerCase()) ?? "application/octet-stream",
  });
  createReadStream(fullPath).pipe(response);
}

function startServer() {
  const server = createServer((request, response) => {
    serveFile(request, response).catch((error) => {
      response.writeHead(500);
      response.end(String(error?.stack ?? error));
    });
  });
  return new Promise((resolve) => {
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      resolve({ server, baseUrl: `http://127.0.0.1:${address.port}` });
    });
  });
}

function closeServer(server) {
  return new Promise((resolve, reject) => {
    server.close((error) => error ? reject(error) : resolve());
  });
}

const { server, baseUrl } = await startServer();
const browser = await chromium.launch();
try {
  const page = await browser.newPage({ viewport: { width: 640, height: 360 } });
  await page.addInitScript(() => {
    class FakeEditContext extends EventTarget {
      constructor({ text = "" } = {}) {
        super();
        this.text = text;
        this.selectionStart = text.length;
        this.selectionEnd = text.length;
        this.isComposing = false;
      }
      updateText(start, end, text) {
        this.text = this.text.slice(0, start) + text + this.text.slice(end);
      }
      updateSelection(start, end) {
        this.selectionStart = start;
        this.selectionEnd = end;
      }
      updateControlBounds(bounds) {
        if (!(bounds instanceof DOMRect)) {
          throw new Error("control bounds must be DOMRect");
        }
        this.controlBounds = bounds;
      }
      updateSelectionBounds(bounds) {
        if (!(bounds instanceof DOMRect)) {
          throw new Error("selection bounds must be DOMRect");
        }
        this.selectionBounds = bounds;
      }
      updateCharacterBounds(rangeStart, bounds) {
        for (const bound of bounds) {
          if (!(bound instanceof DOMRect)) {
            throw new Error("character bound must be DOMRect");
          }
        }
        this.characterBounds = { rangeStart, bounds };
      }
    }
    window.EditContext = FakeEditContext;
    Object.defineProperty(HTMLElement.prototype, "editContext", {
      configurable: true,
      get() {
        return this.__arcweftEditContext ?? null;
      },
      set(value) {
        this.__arcweftEditContext = value;
      },
    });
  });

  await page.goto(`${baseUrl}/ime-sample.html`);
  const result = await page.evaluate(async () => {
    const module = await import("./player-editcontext.js");
    const host = document.createElement("div");
    host.id = "unit-editcontext-host";
    host.tabIndex = 0;
    host.style.cssText = "position:absolute;left:8px;top:8px;width:320px;height:48px";
    document.body.append(host);
    const statusTarget = new EventTarget();
    const statuses = [];
    statusTarget.addEventListener("arcweft-text-input-status", (event) => statuses.push(event.detail));
    statusTarget.addEventListener("arcweft-text-input-render", (event) => statuses.push({ render: event.detail }));
    const glue = module.createArcweftEditContextPlayerGlue(host, {
      hostId: host.id,
      initialText: "",
      statusTarget,
      delegate: {
        createEditContext(_hostId, initialText) {
          return new window.EditContext({ text: initialText });
        },
      },
    });
    await glue.install();
    host.editContext.dispatchEvent(new Event("compositionstart"));
    const update = new Event("textupdate");
    update.updateRangeStart = 0;
    update.updateRangeEnd = 0;
    update.text = "にほんご";
    update.selectionStart = 4;
    update.selectionEnd = 4;
    update.compositionStart = 0;
    update.compositionEnd = 4;
    host.editContext.dispatchEvent(update);
    host.editContext.dispatchEvent(new Event("compositionend"));
    await new Promise((resolve) => setTimeout(resolve, 0));

    const keydownHost = document.createElement("div");
    keydownHost.id = "unit-editcontext-keydown-host";
    keydownHost.tabIndex = 0;
    keydownHost.style.cssText = "position:absolute;left:8px;top:72px;width:320px;height:48px";
    document.body.append(keydownHost);
    const keydownUpdates = [];
    const keydownStatuses = [];
    const commandCalls = [];
    const keydownStatusTarget = new EventTarget();
    keydownStatusTarget.addEventListener(
      "arcweft-text-input-status",
      (event) => keydownStatuses.push(event.detail.state),
    );
    const keydownGlue = module.createArcweftEditContextPlayerGlue(keydownHost, {
      hostId: keydownHost.id,
      initialText: "ab",
      statusTarget: keydownStatusTarget,
      delegate: {
        createEditContext(_hostId, initialText) {
          return new window.EditContext({ text: initialText });
        },
        commandForKeyEvent(_hostId, event) {
          commandCalls.push({
            key: event.key,
            ctrlKey: event.ctrlKey,
            altKey: event.altKey,
            metaKey: event.metaKey,
          });
          return null;
        },
        dispatchTextUpdate(hostId, payload) {
          keydownUpdates.push({ hostId, payload });
        },
      },
    });
    await keydownGlue.install();
    keydownGlue.updateFromRuntimeSnapshot({ text: "ab", selectionStart: 1, selectionEnd: 2 });
    const printableKey = new KeyboardEvent("keydown", { key: "Z", bubbles: true, cancelable: true });
    const printableDispatchAccepted = keydownHost.dispatchEvent(printableKey);
    await new Promise((resolve) => setTimeout(resolve, 0));
    const shortcutKey = new KeyboardEvent("keydown", {
      key: "c",
      ctrlKey: true,
      bubbles: true,
      cancelable: true,
    });
    const shortcutDispatchAccepted = keydownHost.dispatchEvent(shortcutKey);
    await new Promise((resolve) => setTimeout(resolve, 0));
    keydownGlue.updateFromRuntimeSnapshot({ text: "aZ", selectionStart: 2, selectionEnd: 2, multiline: true });
    const multilineEnterKey = new KeyboardEvent("keydown", {
      key: "Enter",
      bubbles: true,
      cancelable: true,
    });
    const multilineEnterDispatchAccepted = keydownHost.dispatchEvent(multilineEnterKey);
    await new Promise((resolve) => setTimeout(resolve, 0));
    const updatesBeforeInactiveKey = keydownUpdates.length;
    keydownGlue.applyRuntimeCommand({ kind: "deactivate", session: 7 });
    const inactiveKey = new KeyboardEvent("keydown", { key: "X", bubbles: true, cancelable: true });
    const inactiveDispatchAccepted = keydownHost.dispatchEvent(inactiveKey);
    await new Promise((resolve) => setTimeout(resolve, 0));

    const pointerHost = document.createElement("div");
    pointerHost.id = "unit-editcontext-pointer-host";
    pointerHost.tabIndex = 0;
    pointerHost.style.cssText = "position:absolute;left:0;top:0;width:640px;height:360px";
    pointerHost.setPointerCapture = () => {};
    pointerHost.releasePointerCapture = () => {};
    document.body.append(pointerHost);
    const pointerUpdates = [];
    const pointerStatuses = [];
    const pointerStatusTarget = new EventTarget();
    pointerStatusTarget.addEventListener(
      "arcweft-text-input-status",
      (event) => pointerStatuses.push(event.detail.state),
    );
    const pointerGlue = module.createArcweftEditContextPlayerGlue(pointerHost, {
      hostId: pointerHost.id,
      initialText: "abcd",
      statusTarget: pointerStatusTarget,
      delegate: {
        createEditContext(_hostId, initialText) {
          return new window.EditContext({ text: initialText });
        },
        dispatchTextUpdate(hostId, payload) {
          pointerUpdates.push({ hostId, payload });
        },
      },
    });
    await pointerGlue.install();
    pointerGlue.updateFromRuntimeSnapshot({ text: "abcd", selectionStart: 0, selectionEnd: 0 });
    pointerGlue.updateGeometry({
      controlRect: { x: 50, y: 50, width: 100, height: 32 },
      caretRect: { x: 50, y: 50, width: 1, height: 32 },
    });
    const outsidePointer = new PointerEvent("pointerdown", {
      pointerId: 41,
      clientX: 240,
      clientY: 140,
      bubbles: true,
      cancelable: true,
    });
    const outsideDispatchAccepted = pointerHost.dispatchEvent(outsidePointer);
    await new Promise((resolve) => setTimeout(resolve, 0));
    const updatesAfterOutsidePointer = pointerUpdates.length;
    const insidePointer = new PointerEvent("pointerdown", {
      pointerId: 42,
      clientX: 64,
      clientY: 60,
      bubbles: true,
      cancelable: true,
    });
    const insideDispatchAccepted = pointerHost.dispatchEvent(insidePointer);
    await new Promise((resolve) => setTimeout(resolve, 0));

    return {
      fallbackInstalled: glue.status().fallbackInstalled,
      text: host.editContext.text,
      statuses: statuses.map((status) => status.state).filter(Boolean),
      mirrorNodeCount: document.querySelectorAll(".committed-text,.composition-text,.caret").length,
      forbiddenActiveNodeCount: document.querySelectorAll("input, textarea, [contenteditable], [role='textbox']").length,
      keydown: {
        text: keydownHost.editContext.text,
        updates: keydownUpdates,
        statuses: keydownStatuses,
        commandCalls,
        printableDefaultPrevented: printableKey.defaultPrevented,
        printableDispatchAccepted,
        shortcutDefaultPrevented: shortcutKey.defaultPrevented,
        shortcutDispatchAccepted,
        multilineEnterDefaultPrevented: multilineEnterKey.defaultPrevented,
        multilineEnterDispatchAccepted,
        inactiveDefaultPrevented: inactiveKey.defaultPrevented,
        inactiveDispatchAccepted,
        inactiveUpdateCount: keydownUpdates.length - updatesBeforeInactiveKey,
      },
      pointer: {
        updates: pointerUpdates,
        statuses: pointerStatuses,
        outsideDefaultPrevented: outsidePointer.defaultPrevented,
        outsideDispatchAccepted,
        updatesAfterOutsidePointer,
        insideDefaultPrevented: insidePointer.defaultPrevented,
        insideDispatchAccepted,
      },
    };
  });

  if (result.fallbackInstalled !== false) {
    throw new Error("fallback flag must remain false");
  }
  if (result.mirrorNodeCount !== 0 || result.forbiddenActiveNodeCount !== 0) {
    throw new Error(`forbidden DOM nodes: ${JSON.stringify(result)}`);
  }
  if (!result.statuses.includes("composition_update") || !result.statuses.includes("composition_end")) {
    throw new Error(`missing composition statuses: ${result.statuses.join(", ")}`);
  }
  if (result.keydown.text !== "aZ\n") {
    throw new Error(`printable keydown did not replace selected text: ${JSON.stringify(result.keydown)}`);
  }
  if (!result.keydown.printableDefaultPrevented || result.keydown.printableDispatchAccepted !== false) {
    throw new Error(`printable keydown was not synchronously claimed: ${JSON.stringify(result.keydown)}`);
  }
  if (result.keydown.shortcutDefaultPrevented || result.keydown.shortcutDispatchAccepted !== true) {
    throw new Error(`shortcut-like keydown should remain command-owned: ${JSON.stringify(result.keydown)}`);
  }
  if (result.keydown.updates.length !== 2) {
    throw new Error(`printable keydown and multiline Enter should emit two text updates: ${JSON.stringify(result.keydown)}`);
  }
  const update = result.keydown.updates[0];
  if (
    update.hostId !== "unit-editcontext-keydown-host" ||
    update.payload.updateRangeStart !== 1 ||
    update.payload.updateRangeEnd !== 2 ||
    update.payload.text !== "Z" ||
    update.payload.selectionStart !== 2 ||
    update.payload.selectionEnd !== 2 ||
    update.payload.observedTextBefore !== "ab" ||
    update.payload.composing !== false
  ) {
    throw new Error(`unexpected printable keydown payload: ${JSON.stringify(result.keydown)}`);
  }
  const newlineUpdate = result.keydown.updates[1];
  if (
    newlineUpdate.hostId !== "unit-editcontext-keydown-host" ||
    newlineUpdate.payload.updateRangeStart !== 2 ||
    newlineUpdate.payload.updateRangeEnd !== 2 ||
    newlineUpdate.payload.text !== "\n" ||
    newlineUpdate.payload.selectionStart !== 3 ||
    newlineUpdate.payload.selectionEnd !== 3 ||
    newlineUpdate.payload.observedTextBefore !== "aZ" ||
    newlineUpdate.payload.composing !== false
  ) {
    throw new Error(`unexpected multiline Enter payload: ${JSON.stringify(result.keydown)}`);
  }
  if (!result.keydown.multilineEnterDefaultPrevented || result.keydown.multilineEnterDispatchAccepted !== false) {
    throw new Error(`multiline Enter should be claimed as a newline edit: ${JSON.stringify(result.keydown)}`);
  }
  if (!result.keydown.statuses.includes("newline")) {
    throw new Error(`multiline Enter should report newline status: ${JSON.stringify(result.keydown)}`);
  }
  if (
    result.keydown.commandCalls.length !== 1 ||
    result.keydown.commandCalls[0].key !== "c" ||
    result.keydown.commandCalls[0].ctrlKey !== true
  ) {
    throw new Error(`plain printable keydown should bypass command lookup: ${JSON.stringify(result.keydown)}`);
  }
  if (result.keydown.inactiveDefaultPrevented || result.keydown.inactiveDispatchAccepted !== true) {
    throw new Error(`inactive keydown should pass through to the player: ${JSON.stringify(result.keydown)}`);
  }
  if (result.keydown.inactiveUpdateCount !== 0 || !result.keydown.statuses.includes("keydown_ignored_inactive")) {
    throw new Error(`inactive keydown should not dispatch text updates: ${JSON.stringify(result.keydown)}`);
  }
  if (result.pointer.outsideDefaultPrevented || result.pointer.outsideDispatchAccepted !== true) {
    throw new Error(`outside pointerdown should pass through to the player: ${JSON.stringify(result.pointer)}`);
  }
  if (result.pointer.updatesAfterOutsidePointer !== 0) {
    throw new Error(`outside pointerdown should not dispatch text updates: ${JSON.stringify(result.pointer)}`);
  }
  if (!result.pointer.statuses.includes("pointer_ignored_outside_control")) {
    throw new Error(`outside pointerdown should report an ignored text-control hit: ${JSON.stringify(result.pointer)}`);
  }
  if (!result.pointer.insideDefaultPrevented || result.pointer.insideDispatchAccepted !== false) {
    throw new Error(`inside pointerdown should remain a text-control selection: ${JSON.stringify(result.pointer)}`);
  }
  if (result.pointer.updates.length !== 1) {
    throw new Error(`inside pointerdown should dispatch one text selection update: ${JSON.stringify(result.pointer)}`);
  }
  console.log(JSON.stringify({ test: "player-editcontext-invisible-glue-unit", result }));
} finally {
  await browser.close();
  await closeServer(server);
}
