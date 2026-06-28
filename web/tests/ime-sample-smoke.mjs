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
  [".png", "image/png"],
  [".ttf", "font/ttf"],
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
  const page = await browser.newPage({ viewport: { width: 960, height: 640 } });
  const errors = [];
  page.on("console", (message) => {
    if (message.type() === "error") {
      errors.push(message.text());
    }
  });
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(`${baseUrl}/ime-sample.html`);
  await page.locator("#arcweft-ime-surface").focus();
  await page.waitForSelector("#ime-sample-status[data-state]");
  await page.waitForFunction(() => {
    const state = document.getElementById("ime-sample-status")?.dataset.state;
    return state === "ready" || state === "unsupported";
  });
  const result = await page.evaluate(() => ({
    state: document.getElementById("ime-sample-status").dataset.state,
    fonts: document.getElementById("ime-sample-fonts").textContent,
    owner: window.__arcweftImeSampleGlueOwner,
    fallbackInstalled: window.__arcweftImeSampleFallbackInstalled,
  }));
  if (!["ready", "unsupported"].includes(result.state)) {
    throw new Error(`unexpected IME sample state: ${result.state}`);
  }
  if (result.owner !== "arcweft-player") {
    throw new Error(`sample was not installed by Arcweft player glue: ${result.owner}`);
  }
  if (result.fallbackInstalled !== false) {
    throw new Error("sample installed a forbidden fallback");
  }
  if (!result.fonts.includes("Arcweft Demo") || !result.fonts.includes("Noto Sans JP")) {
    throw new Error(`font stack status did not include expected fonts: ${result.fonts}`);
  }
  if (errors.length > 0) {
    throw new Error(`browser console errors:\n${errors.join("\n")}`);
  }
  console.log(JSON.stringify({ sample: "web-ime-player-owned-smoke", ...result }));
} finally {
  await browser.close();
  await closeServer(server);
}
