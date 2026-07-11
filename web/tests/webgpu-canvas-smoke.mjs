import { createReadStream } from "node:fs";
import { mkdir } from "node:fs/promises";
import { open } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const root = normalize(join(fileURLToPath(new URL(".", import.meta.url)), ".."));
const maxBlockedAdvances = 16;

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

async function openReady(browser, baseUrl, options = {}) {
  const { page, errors } = await openBooted(browser, baseUrl, options);
  await advanceDialogueToChoices(page);
  try {
    await page.waitForFunction(
      () =>
        window.__arcweftLastObservation?.choice_count > 0 &&
        window.__arcweftLastObservation?.image_count >= 4 &&
        window.__arcweftLastFrameObservation?.choice_count === 2 &&
        window.__arcweftLastFrameObservation?.image_count === 4,
      null,
      { timeout: 10_000 },
    );
    await assertFrameObservation(page, await canvasViewport(page));
  } catch (error) {
    const state = await page.evaluate(() => ({
      fatal: document.querySelector("#arcweft-fatal")?.textContent ?? null,
      ready: document.querySelector("#arcweft-canvas")?.dataset.arcweftReady ?? null,
      observation: window.__arcweftLastObservation ?? null,
      frameObservation: window.__arcweftLastFrameObservation ?? null,
    }));
    throw new Error(`${error.message}\nstate=${JSON.stringify(state)}`);
  }
  return { page, errors };
}

async function openBooted(browser, baseUrl, options = {}) {
  const viewport = options.viewport ?? { width: 1280, height: 720 };
  const page = await browser.newPage({
    viewport,
    deviceScaleFactor: options.deviceScaleFactor ?? 1,
  });
  if (options.deterministicClock) {
    await installDeterministicClock(page);
  }
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
  return { page, errors };
}

async function advanceToDialoguePage(page, instance, pageIndex) {
  await page.locator("#arcweft-canvas").focus();
  for (let attempt = 0; attempt < maxBlockedAdvances; attempt += 1) {
    const reached = await page.evaluate(
      ({ expectedInstance, expectedPage }) => {
        const dialogue = window.__arcweftLastObservation?.dialogue;
        return dialogue?.instance === expectedInstance && dialogue?.page_index === expectedPage;
      },
      { expectedInstance: instance, expectedPage: pageIndex },
    );
    if (reached) {
      return;
    }
    await page.keyboard.press("Enter");
    await page.waitForTimeout(50);
  }
  fail(`dialogue instance ${instance} did not reach page ${pageIndex}`);
}

async function advanceDialogueToChoices(page) {
  await page.locator("#arcweft-canvas").focus();
  for (let attempt = 0; attempt < maxBlockedAdvances; attempt += 1) {
    const state = await page.evaluate(() => ({
      dialoguePresent: Boolean(window.__arcweftLastObservation?.dialogue),
      choiceCount: window.__arcweftLastObservation?.choice_count ?? 0,
      stopReason: window.__arcweftLastObservation?.stop_reason ?? null,
    }));
    if (state.choiceCount > 0) {
      return;
    }
    if (!state.dialoguePresent || state.stopReason !== "Blocked") {
      return;
    }
    await page.keyboard.press("Enter");
    await page.waitForTimeout(50);
  }
}

async function finishBlockedDialogue(page) {
  await page.locator("#arcweft-canvas").focus();
  for (let attempt = 0; attempt < maxBlockedAdvances; attempt += 1) {
    const state = await page.evaluate(() => ({
      dialoguePresent: Boolean(window.__arcweftLastObservation?.dialogue),
      choiceCount: window.__arcweftLastObservation?.choice_count ?? 0,
      finished: window.__arcweftLastObservation?.finished === true,
      stopReason: window.__arcweftLastObservation?.stop_reason ?? null,
    }));
    if (state.finished) {
      return;
    }
    if (!state.dialoguePresent || state.choiceCount > 0 || state.stopReason !== "Blocked") {
      break;
    }
    await page.keyboard.press("Enter");
    await page.waitForTimeout(50);
  }
  await page.waitForFunction(() => window.__arcweftLastObservation?.finished === true);
}

async function installDeterministicClock(page) {
  await page.addInitScript(() => {
    let now = 0;
    globalThis.__arcweftNowMillis = () => {
      now = Math.min(now + 16, 160);
      return now;
    };
  });
}

async function assertFrameObservation(page, expected) {
  const frame = await page.evaluate(() => window.__arcweftLastFrameObservation);
  expect(
    frame?.schema_version === "arcweft.web_frame_observation.v3",
    "unexpected frame observation schema",
  );
  expect(
    frame.viewport.logical_width_milli === expected.logicalWidth * 1_000,
    "logical width mismatch",
  );
  expect(
    frame.viewport.logical_height_milli === expected.logicalHeight * 1_000,
    "logical height mismatch",
  );
  expect(frame.viewport.physical_width === expected.physicalWidth, "physical width mismatch");
  expect(frame.viewport.physical_height === expected.physicalHeight, "physical height mismatch");
  expect(
    frame.viewport.scale_factor_milli === Math.round(expected.scaleFactor * 1_000),
    "scale factor mismatch",
  );
  expect(
    frame.images.map((image) => image.id).join(",") ===
      [
        "image.generated.background",
        "image.generated.character_stand",
        "image.generated.gif_pulse",
        "image.generated.webp_pulse",
      ].join(","),
    `unexpected image ids: ${JSON.stringify(frame.images)}`,
  );
  const expectedChoices = expectedChoiceGeometry(expected);
  expect(
    frame.choices.length === expectedChoices.length &&
      frame.choices.every(
        (choice, index) =>
          choice.option_id === expectedChoices[index].optionId &&
          Math.abs(choice.bounds.y_milli - expectedChoices[index].yMilli) <= 1,
      ),
    `unexpected choice geometry: ${JSON.stringify(frame.choices)}`,
  );
}

function expectedChoiceGeometry(expected) {
  const referenceWidth = 1280;
  const referenceHeight = 720;
  const scale = Math.min(
    expected.logicalWidth / referenceWidth,
    expected.logicalHeight / referenceHeight,
  );
  const offsetY = (expected.logicalHeight - referenceHeight * scale) / 2;
  const width = Math.min(Math.max(referenceWidth * 0.52, 360), 760);
  const itemHeight = 60;
  const gap = 12;
  const total = 2 * (itemHeight + gap) - gap;
  const margin = Math.max(referenceWidth * 0.045, 24);
  const panelHeight = Math.min(Math.max(referenceHeight * 0.28, 180), 320);
  const panelY = referenceHeight - panelHeight - margin;
  const top = Math.max(panelY - total - 22, 36);
  return [
    {
      optionId: "choice.web_demo.continue",
      yMilli: Math.round((offsetY + top * scale) * 1_000),
    },
    {
      optionId: "choice.web_demo.alternate",
      yMilli: Math.round((offsetY + (top + itemHeight + gap) * scale) * 1_000),
    },
  ];
}

async function canvasViewport(page) {
  return await page.evaluate(() => {
    const canvas = document.querySelector("#arcweft-canvas");
    return {
      logicalWidth: canvas.clientWidth,
      logicalHeight: canvas.clientHeight,
      physicalWidth: canvas.width,
      physicalHeight: canvas.height,
      scaleFactor: window.devicePixelRatio,
    };
  });
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
    await runSmoke("[p] advances within the same dialogue line before the next line", async () => {
      const { page, errors } = await openBooted(browser, baseUrl);
      try {
        await page.waitForFunction(
          () =>
            window.__arcweftLastObservation?.dialogue?.page_count === 2 &&
            window.__arcweftLastObservation?.dialogue?.page_index === 0,
        );
        const first = await page.evaluate(() => window.__arcweftLastObservation.dialogue);
        await advanceToDialoguePage(page, first.instance, 1);
        const second = await page.evaluate(() => window.__arcweftLastObservation.dialogue);
        expect(second.instance === first.instance, "[p] replaced the dialogue occurrence");
        expect(second.stage_index > first.stage_index, "[p] did not advance the dialogue stage");
        const visibleText = await page.evaluate(() => {
          const frame = window.__arcweftLastFrameObservation;
          return [
            ...(frame?.text?.map((item) => item.text) ?? []),
            ...(frame?.styled_paragraphs?.map((item) => item.text) ?? []),
          ].join("");
        });
        expect(visibleText.includes("同じdialogue lineの2ページ目"), "second page is not visible");
        expect(!visibleText.includes("語り手は明朝体"), "first page remained visible after [p]");
        expect(errors.length === 0, `console errors: ${errors.join("\n")}`);
      } finally {
        await page.close();
      }
    });

    await runSmoke("dialogue and choice are WebGPU canvas content, not DOM game View", async () => {
      const { page, errors } = await openReady(browser, baseUrl, {
        deterministicClock: Boolean(process.env.ARW_WEB_PARITY_DIR),
      });
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

    await runSmoke("canvas keeps a 16:9 aspect ratio inside non-16:9 viewports", async () => {
      const { page, errors } = await openReady(browser, baseUrl, {
        viewport: { width: 1000, height: 800 },
      });
      try {
        const metrics = await canvasViewport(page);
        expect(metrics.logicalWidth === 1000, `unexpected canvas width: ${metrics.logicalWidth}`);
        expect(metrics.logicalHeight < 800, `canvas did not letterbox: ${metrics.logicalHeight}`);
        expect(
          Math.abs(metrics.logicalWidth / metrics.logicalHeight - 16 / 9) < 0.005,
          `canvas is not 16:9: ${JSON.stringify(metrics)}`,
        );
        expect(errors.length === 0, `console errors: ${errors.join("\n")}`);
      } finally {
        await page.close();
      }
    });

    if (process.env.ARW_WEB_PARITY_DIR) {
      await runSmoke("native/web visual parity checkpoints are capturable", async () => {
        await writeCanvasParityScreenshots(browser, baseUrl);
      });
    }

    await runSmoke("pointer hit-test selects a canvas choice and advances runtime", async () => {
      const { page, errors } = await openReady(browser, baseUrl);
      try {
        const point = await choiceCenter(page, 0);
        await page.mouse.click(point.x, point.y);
        await finishBlockedDialogue(page);
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
        await finishBlockedDialogue(page);
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
        await assertFrameObservation(page, await canvasViewport(page));
        const point = await choiceCenter(page, 0);
        await page.mouse.click(point.x, point.y);
        await finishBlockedDialogue(page);
        expect(errors.length === 0, `console errors: ${errors.join("\n")}`);
      } finally {
        await page.close();
      }
    });

    await runSmoke("missing WebGPU produces a structured fatal bootstrap error", async () => {
      const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
      await installDeterministicClock(page);
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

async function writeCanvasParityScreenshots(browser, baseUrl) {
  const directory = process.env.ARW_WEB_PARITY_DIR;
  if (!directory) {
    return;
  }
  await mkdir(directory, { recursive: true });
  const names = (process.env.ARW_WEB_PARITY_CHECKPOINTS ??
    "focus-first-choice,hover-second-choice,press-first-choice,compact-focus-first-choice")
    .split(",")
    .map((name) => name.trim())
    .filter(Boolean);
  for (const name of names) {
    const checkpoint = parityCheckpoint(name);
    const { page, errors } = await openReady(browser, baseUrl, {
      deterministicClock: true,
      viewport: checkpoint.viewport,
      deviceScaleFactor: checkpoint.deviceScaleFactor,
    });
    try {
      await checkpoint.apply(page);
      expect(errors.length === 0, `console errors: ${errors.join("\n")}`);
      await page.locator("#arcweft-canvas").screenshot({
        path: join(directory, `web-${name}.png`),
        scale: "css",
      });
    } finally {
      await page.close();
    }
  }
}

function parityCheckpoint(name) {
  const base = {
    viewport: { width: 1280, height: 720 },
    deviceScaleFactor: 1,
  };
  switch (name) {
    case "focus-first-choice":
      return { ...base, apply: async () => {} };
    case "hover-first-choice":
      return { ...base, apply: hoverFirstChoice };
    case "hover-second-choice":
      return { ...base, apply: hoverSecondChoice };
    case "press-first-choice":
      return { ...base, apply: pressFirstChoice };
    case "compact-focus-first-choice":
      return {
        viewport: { width: 960, height: 540 },
        deviceScaleFactor: 1,
        apply: async () => {},
      };
    case "hidpi-focus-first-choice":
      return {
        viewport: { width: 640, height: 360 },
        deviceScaleFactor: 2,
        apply: async () => {},
      };
    default:
      throw new Error(`unknown parity checkpoint: ${name}`);
  }
}

async function hoverFirstChoice(page) {
  const point = await choiceCenter(page, 0);
  await page.mouse.move(point.x, point.y);
  await page.waitForTimeout(100);
}

async function hoverSecondChoice(page) {
  const point = await choiceCenter(page, 1);
  await page.mouse.move(point.x, point.y);
  await page.waitForTimeout(100);
}

async function pressFirstChoice(page) {
  const point = await choiceCenter(page, 0);
  await page.mouse.move(point.x, point.y);
  await page.mouse.down();
  await page.waitForTimeout(100);
}

async function choiceCenter(page, index) {
  const frame = await page.evaluate(() => window.__arcweftLastFrameObservation);
  const choice = frame?.choices?.[index];
  expect(Boolean(choice), `choice ${index} is not available`);
  const box = await page.locator("#arcweft-canvas").boundingBox();
  expect(Boolean(box), "canvas has no bounding box");
  return {
    x: box.x + (choice.bounds.x_milli + choice.bounds.width_milli / 2) / 1_000,
    y: box.y + (choice.bounds.y_milli + choice.bounds.height_milli / 2) / 1_000,
  };
}

await main().catch((error) => {
  console.error(error instanceof Error ? error.stack || error.message : error);
  process.exitCode = 1;
});
