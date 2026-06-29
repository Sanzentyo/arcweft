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
      delegate: {},
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
    return {
      fallbackInstalled: glue.status().fallbackInstalled,
      text: host.editContext.text,
      statuses: statuses.map((status) => status.state).filter(Boolean),
      mirrorNodeCount: document.querySelectorAll(".committed-text,.composition-text,.caret").length,
      forbiddenActiveNodeCount: document.querySelectorAll("input, textarea, [contenteditable], [role='textbox']").length,
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
  console.log(JSON.stringify({ test: "player-editcontext-invisible-glue-unit", result }));
} finally {
  await browser.close();
  await closeServer(server);
}
