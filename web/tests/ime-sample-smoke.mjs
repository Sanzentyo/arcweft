import { createReadStream } from "node:fs";
import { open } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const root = normalize(join(fileURLToPath(new URL(".", import.meta.url)), ".."));
const contentTypes = new Map([
  [".awfb", "application/octet-stream"],
  [".css", "text/css; charset=utf-8"],
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".json", "application/json; charset=utf-8"],
  [".png", "image/png"],
  [".ttf", "font/ttf"],
  [".wasm", "application/wasm"],
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

function containsControlId(value, id) {
  if (typeof value === "string") {
    return value.includes(id);
  }
  if (Array.isArray(value)) {
    return value.some((entry) => containsControlId(entry, id));
  }
  if (value && typeof value === "object") {
    return Object.values(value).some((entry) => containsControlId(entry, id));
  }
  return false;
}

function geometryCommands(commands) {
  return commands.flatMap((envelope) => envelope?.commands ?? [])
    .filter((command) => command.kind === "activate" || command.kind === "update_geometry");
}

const { server, baseUrl } = await startServer();
const browser = await chromium.launch();
try {
  const page = await browser.newPage({ viewport: { width: 960, height: 640 } });
  const errors = [];
  page.on("console", (message) => {
    if (message.type() === "error") {
      errors.push(message.text());
    }
  });
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(`${baseUrl}/ime-sample.html`);

  const sourceShape = await page.evaluate(() => ({
    canvasCount: document.querySelectorAll("canvas#arcweft-canvas").length,
    forbiddenActiveNodeCount: document.querySelectorAll(
      "input, textarea, [contenteditable], [role='textbox'], .caret, .committed-text, .composition-text, #ime-sample-status, #ime-sample-selection, #ime-sample-fonts",
    ).length,
    sample: window.__arcweftImeSample,
  }));
  if (sourceShape.canvasCount !== 1 || sourceShape.forbiddenActiveNodeCount !== 0) {
    throw new Error(`active sample is not canvas-only: ${JSON.stringify(sourceShape)}`);
  }
  if (sourceShape.sample?.boundary !== "player-rendered") {
    throw new Error(`sample did not declare player-rendered boundary: ${JSON.stringify(sourceShape.sample)}`);
  }

  await page.waitForFunction(() => (
    Boolean(window.__arcweftFatal) ||
    Boolean(window.__arcweftImeSample?.fatal) ||
    Boolean(window.__arcweftLastFrameObservation) ||
    Boolean(window.__arcweftImeSample?.unsupportedNoFallback)
  ), null, { timeout: 15_000 });

  const fatal = await page.evaluate(() => window.__arcweftFatal ?? window.__arcweftImeSample?.fatal ?? null);
  if (fatal) {
    console.log(JSON.stringify({
      sample: "web-ime-player-rendered-smoke",
      status: "environment_blocked",
      fatal,
      forbiddenActiveNodeCount: sourceShape.forbiddenActiveNodeCount,
    }));
  } else {
    await page.locator("#arcweft-canvas").click({ position: { x: 96, y: 96 } });
    await page.waitForTimeout(250);
    await page.keyboard.type("abc");
    await page.waitForTimeout(250);

    const evidence = await page.evaluate(() => ({
      owner: window.__arcweftImeSampleGlueOwner,
      fallbackInstalled: window.__arcweftImeSampleFallbackInstalled,
      frame: window.__arcweftLastFrameObservation,
      commands: window.__arcweftImeSample?.runtimeCommands ?? [],
    }));
    const geometry = geometryCommands(evidence.commands);
    const caretRects = geometry
      .map((command) => command.geometry?.caretRect ?? command.snapshot?.caretRect)
      .filter(Boolean);

    if (evidence.fallbackInstalled !== false) {
      throw new Error("sample installed a forbidden fallback");
    }
    if (!containsControlId(evidence.frame, "input.jp_text_field")) {
      throw new Error("frame observation did not include the Japanese text field target");
    }
    if (caretRects.length > 0 && !caretRects.every((rect) => rect.height > 8 && rect.width >= 0)) {
      throw new Error(`invalid caret geometry evidence: ${JSON.stringify(caretRects)}`);
    }
    if (errors.length > 0) {
      throw new Error(`browser console errors:\n${errors.join("\n")}`);
    }
    console.log(JSON.stringify({
      sample: "web-ime-player-rendered-smoke",
      status: "passed",
      owner: evidence.owner,
      geometryCommandCount: geometry.length,
      caretRects,
    }));
  }
} finally {
  await browser.close();
  await closeServer(server).catch(() => {});
}
