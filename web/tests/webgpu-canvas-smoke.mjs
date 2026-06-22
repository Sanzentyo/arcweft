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
  [".gif", "image/gif"],
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".json", "application/json; charset=utf-8"],
  [".png", "image/png"],
  [".ttf", "font/ttf"],
  [".wasm", "application/wasm"],
  [".webp", "image/webp"],
]);

function fail(message) {
  throw new Error(message);
}

function expect(condition, message) {
  if (!condition) {
    fail(message);
  }
}

function staticPath(requestUrl) {
  const url = new URL(requestUrl, "http://127.0.0.1");
  let pathname = decodeURIComponent(url.pathname);
  if (pathname === "/") {
    pathname = "/index.html";
  }
  if (pathname === "/favicon.ico") {
    return "";
  }
  if (pathname.includes("\0")) {
    return null;
  }
  const fullPath = normalize(join(root, pathname));
  return fullPath.startsWith(root) ? fullPath : null;
}

async function serveFile(request, response) {
  const fullPath = staticPath(request.url);
  if (fullPath === "") {
    response.writeHead(204);
    response.end();
    return;
  }
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
    "content-type": contentTypes.get(extname(fullPath).toLowerCase()) ??
      "application/octet-stream",
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
    server.close((error) => (error ? reject(error) : resolve()));
  });
}

function collectConsoleErrors(page) {
  const errors = [];
  page.on("console", (message) => {
    if (message.type() === "error") {
      errors.push(message.text());
    }
  });
  page.on("pageerror", (error) => errors.push(error.message));
  return errors;
}

async function openReady(browser, baseUrl) {
  const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
  const errors = collectConsoleErrors(page);
  await page.goto(`${baseUrl}/index.html`);
  expect(await page.evaluate(() => Boolean(navigator.gpu)), "navigator.gpu is unavailable");
  try {
    await page.waitForFunction(
      () => document.querySelector("#arcweft-canvas")?.dataset.arcweftReady === "true",
      null,
      { timeout: 10_000 },
    );
  } catch (error) {
    const state = await page.evaluate(() => ({
      fatal: document.querySelector("#arcweft-fatal")?.textContent ?? null,
      loadingHidden: document.querySelector("#arcweft-loading")?.hidden ?? null,
      ready: document.querySelector("#arcweft-canvas")?.dataset.arcweftReady ?? null,
      observation: window.__arcweftLastObservation ?? null,
    }));
    throw new Error(`${error.message}\nstate=${JSON.stringify(state)}`);
  }
  try {
    await page.waitForFunction(
      () =>
        window.__arcweftLastObservation?.choice_count > 0 &&
        window.__arcweftLastObservation?.image_count >= 4,
      null,
      { timeout: 10_000 },
    );
  } catch (error) {
    const state = await page.evaluate(() => ({
      fatal: document.querySelector("#arcweft-fatal")?.textContent ?? null,
      ready: document.querySelector("#arcweft-canvas")?.dataset.arcweftReady ?? null,
      observation: window.__arcweftLastObservation ?? null,
    }));
    throw new Error(`${error.message}\nstate=${JSON.stringify(state)}`);
  }
  return { page, errors };
}

async function runSmoke(name, test) {
  const started = performance.now();
  try {
    await test();
    console.log(`ok ${name} (${Math.round(performance.now() - started)}ms)`);
  } catch (error) {
    console.error(`not ok ${name}`);
    throw error;
  }
}

async function main() {
  const { server, baseUrl } = await startServer();
  const webGpuArgs = ["--enable-unsafe-webgpu"];
  if (process.platform === "win32") {
    webGpuArgs.push("--use-angle=d3d11");
  }

  const browser = await chromium.launch({
    channel: process.env.ARW_PLAYWRIGHT_CHANNEL || "chrome",
    headless: true,
    args: webGpuArgs,
  });

  try {
    await runSmoke("dialogue and choice are WebGPU canvas content, not DOM game UI", async () => {
      const { page, errors } = await openReady(browser, baseUrl);
      try {
        expect(await page.locator("canvas#arcweft-canvas").isVisible(), "canvas is not visible");
        expect(await page.locator("button").count() === 0, "DOM button renderer is present");
        const domText = await page
          .locator("[data-arcweft-speaker], [data-arcweft-dialogue], [data-arcweft-choice]")
          .count();
        expect(domText === 0, "DOM game text renderer is present");
        expect(errors.length === 0, `console errors: ${errors.join("\n")}`);
      } finally {
        await page.close();
      }
    });

    await runSmoke("pointer hit-test selects a canvas choice and advances runtime", async () => {
      const { page, errors } = await openReady(browser, baseUrl);
      try {
        const box = await page.locator("#arcweft-canvas").boundingBox();
        expect(Boolean(box), "canvas has no bounding box");
        await page.mouse.click(box.x + box.width * 0.5, box.y + box.height * 0.39);
        await page.waitForFunction(() => window.__arcweftLastObservation?.finished === true);
        expect(errors.length === 0, `console errors: ${errors.join("\n")}`);
      } finally {
        await page.close();
      }
    });

    await runSmoke("keyboard focus navigation and activation select a canvas choice", async () => {
      const { page, errors } = await openReady(browser, baseUrl);
      try {
        await page.locator("#arcweft-canvas").focus();
        await page.keyboard.press("ArrowDown");
        await page.keyboard.press("Enter");
        await page.waitForFunction(() => window.__arcweftLastObservation?.finished === true);
        expect(errors.length === 0, `console errors: ${errors.join("\n")}`);
      } finally {
        await page.close();
      }
    });

    await runSmoke("resize keeps WebGPU viewport and hit-test geometry aligned", async () => {
      const { page, errors } = await openReady(browser, baseUrl);
      try {
        await page.setViewportSize({ width: 960, height: 540 });
        await page.waitForTimeout(100);
        const box = await page.locator("#arcweft-canvas").boundingBox();
        expect(Boolean(box), "canvas has no bounding box after resize");
        await page.mouse.click(box.x + box.width * 0.5, box.y + box.height * 0.39);
        await page.waitForFunction(() => window.__arcweftLastObservation?.finished === true);
        expect(errors.length === 0, `console errors: ${errors.join("\n")}`);
      } finally {
        await page.close();
      }
    });

    await runSmoke("missing WebGPU produces a structured fatal bootstrap error", async () => {
      const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
      try {
        await page.addInitScript(() => {
          Object.defineProperty(Navigator.prototype, "gpu", {
            configurable: true,
            get: () => undefined,
          });
        });
        await page.goto(`${baseUrl}/index.html`);
        await page.waitForSelector("#arcweft-fatal:not([hidden])");
        const fatalText = await page.locator("#arcweft-fatal").textContent();
        expect(
          fatalText?.includes("WebGPU is unsupported"),
          `unexpected fatal text: ${fatalText}`,
        );
        expect(await page.locator("button").count() === 0, "DOM button renderer is present");
      } finally {
        await page.close();
      }
    });
  } finally {
    await browser.close();
    await closeServer(server);
  }
}

await main().catch((error) => {
  console.error(error instanceof Error ? error.stack || error.message : error);
  process.exitCode = 1;
});
