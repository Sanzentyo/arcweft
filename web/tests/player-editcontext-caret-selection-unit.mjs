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
  const page = await browser.newPage({ viewport: { width: 760, height: 420 } });
  await page.addInitScript(() => {
    function assertDomRect(bounds) {
      if (!(bounds instanceof DOMRect)) {
        throw new Error(`EditContext geometry requires DOMRect, got ${Object.prototype.toString.call(bounds)}`);
      }
    }

    class FakeEditContext extends EventTarget {
      constructor({ text = "" } = {}) {
        super();
        this.text = text;
        this.selectionStart = text.length;
        this.selectionEnd = text.length;
        this.isComposing = false;
        this.geometryCalls = [];
      }
      updateText(start, end, text) {
        this.text = this.text.slice(0, start) + text + this.text.slice(end);
      }
      updateSelection(start, end) {
        this.selectionStart = start;
        this.selectionEnd = end;
      }
      updateControlBounds(bounds) {
        assertDomRect(bounds);
        this.controlBounds = bounds;
        this.geometryCalls.push(["control", bounds]);
      }
      updateSelectionBounds(bounds) {
        assertDomRect(bounds);
        this.selectionBounds = bounds;
        this.geometryCalls.push(["selection", bounds]);
      }
      updateCharacterBounds(rangeStart, bounds) {
        bounds.forEach(assertDomRect);
        this.characterBounds = { rangeStart, bounds };
        this.geometryCalls.push(["character", { rangeStart, bounds }]);
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
  await page.waitForSelector("#ime-sample-status[data-state='ready']");
  const result = await page.evaluate(async () => {
    const host = document.getElementById("arcweft-ime-surface");
    const statusStates = [];
    document.addEventListener("arcweft-text-input-status", (event) => statusStates.push(event.detail.state));
    const update = new Event("textupdate");
    update.updateRangeStart = 0;
    update.updateRangeEnd = 0;
    update.text = "abcd";
    update.selectionStart = 4;
    update.selectionEnd = 4;
    host.editContext.dispatchEvent(update);
    await new Promise((resolve) => setTimeout(resolve, 0));

    host.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowLeft", bubbles: true }));
    host.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowLeft", shiftKey: true, bubbles: true }));
    await new Promise((resolve) => setTimeout(resolve, 0));

    const caret = host.querySelectorAll(".caret");
    const selectionMarkup = host.querySelectorAll(".arcweft-selection");
    return {
      text: document.getElementById("arcweft-ime-text").textContent,
      selectionStart: host.editContext.selectionStart,
      selectionEnd: host.editContext.selectionEnd,
      selectionBounds: host.editContext.selectionBounds,
      characterBounds: host.editContext.characterBounds,
      geometryCalls: host.editContext.geometryCalls.length,
      caretCount: caret.length,
      selectionMarkupCount: selectionMarkup.length,
      statusStates,
    };
  });

  if (result.text !== "abcd") {
    throw new Error(`expected committed text abcd, got ${JSON.stringify(result.text)}`);
  }
  if (result.caretCount !== 1) {
    throw new Error(`expected one Arcweft caret, got ${result.caretCount}`);
  }
  if (result.selectionStart === result.selectionEnd) {
    throw new Error(`expected ranged selection after Shift+ArrowLeft: ${result.selectionStart}..${result.selectionEnd}`);
  }
  if (!result.selectionBounds || (result.selectionBounds.x === 0 && result.selectionBounds.y === 0)) {
    throw new Error(`candidate/selection bounds stayed at origin: ${JSON.stringify(result.selectionBounds)}`);
  }
  if (!result.characterBounds?.bounds?.length) {
    throw new Error("EditContext character bounds were not pumped");
  }
  if (result.selectionMarkupCount < 1) {
    throw new Error("ranged selection was not rendered by the Arcweft mirror");
  }
  if (!result.statusStates.includes("command")) {
    throw new Error(`keyboard commands did not route through player glue: ${result.statusStates.join(",")}`);
  }
  console.log(JSON.stringify({ test: "player-editcontext-caret-selection-unit", result }));
} finally {
  await browser.close();
  await closeServer(server);
}
