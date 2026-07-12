import { createReadStream } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { open } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

// Drives deterministic prepared-text checkpoints through the browser player.

const root = normalize(join(fileURLToPath(new URL(".", import.meta.url)), ".."));
const sample = process.env.ARW_TEXT_PARITY_SAMPLE ?? "css-style-parity";
const outputDir = process.env.ARW_TEXT_PARITY_DIR ??
  normalize(join(root, "..", "target", "css-style-parity"));
const checkpoints = (process.env.ARW_TEXT_PARITY_CHECKPOINTS ?? "default,compact")
  .split(",")
  .map((name) => name.trim())
  .filter(Boolean);
const bundleUrl = process.env.ARW_TEXT_PARITY_BUNDLE_URL ??
  "./local/css-style-parity.awfb";
const fontUrl = process.env.ARW_TEXT_PARITY_FONT_URL ??
  "./assets/arcweft-demo.ttf";
const additionalFontUrls = (
  process.env.ARW_TEXT_PARITY_ADDITIONAL_FONT_URLS ??
    "./assets/noto-sans-jp-css-style-parity.ttf"
)
  .split(",")
  .map((url) => url.trim())
  .filter(Boolean);
const defaultVisualTimeMillis = Number.parseInt(
  process.env.ARW_TEXT_PARITY_VISUAL_TIME_MILLIS ?? "9000",
  10,
);
const visualTimes = assignmentMap(
  process.env.ARW_TEXT_PARITY_VISUAL_TIMES,
  (value) => Number.parseInt(value, 10),
);
const advanceCounts = assignmentMap(
  process.env.ARW_TEXT_PARITY_ADVANCE_COUNTS,
  (value) => Number.parseInt(value, 10),
);
const globalRequiredText = splitValues(
  process.env.ARW_TEXT_PARITY_REQUIRED_TEXT ?? "DSL-styled text|wave motion",
  "|",
);
const requiredTextByCheckpoint = assignmentMap(
  process.env.ARW_TEXT_PARITY_REQUIRED_TEXT_BY_CHECKPOINT,
  (value) => splitValues(value, "|"),
  ";",
);
const minimumTextCount = Number.parseInt(
  process.env.ARW_TEXT_PARITY_MIN_TEXT_COUNT ?? "2",
  10,
);
const expectedImageCount = Number.parseInt(
  process.env.ARW_TEXT_PARITY_IMAGE_COUNT ?? "0",
  10,
);

function splitValues(value, separator) {
  return value.split(separator).map((item) => item.trim()).filter(Boolean);
}

function assignmentMap(raw, parseValue, separator = ",") {
  const values = new Map();
  for (const assignment of splitValues(raw ?? "", separator)) {
    const equals = assignment.indexOf("=");
    expect(equals > 0, `bad checkpoint assignment: ${assignment}`);
    const key = assignment.slice(0, equals).trim();
    const value = parseValue(assignment.slice(equals + 1).trim());
    expect(key.length > 0, `empty checkpoint assignment key: ${assignment}`);
    values.set(key, value);
  }
  return values;
}

function checkpointVisualTime(checkpoint) {
  return visualTimes.get(checkpoint) ?? defaultVisualTimeMillis;
}

function checkpointAdvanceCount(checkpoint) {
  return advanceCounts.get(checkpoint) ?? 0;
}

function checkpointRequiredText(checkpoint) {
  return requiredTextByCheckpoint.get(checkpoint) ?? globalRequiredText;
}

const contentTypes = new Map([
  [".awfb", "application/octet-stream"],
  [".css", "text/css; charset=utf-8"],
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".otf", "font/otf"],
  [".png", "image/png"],
  [".ttf", "font/ttf"],
  [".wasm", "application/wasm"],
  [".woff", "font/woff"],
  [".woff2", "font/woff2"],
]);

function expect(condition, message) {
  if (!condition) {
    throw new Error(message);
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

async function installDeterministicClock(page, visualTimeMillis) {
  await page.addInitScript(() => {
    let now = 0;
    let remainingSteps = 1;
    globalThis.__arcweftNowMillis = () => {
      const consumed = Math.min(remainingSteps, 4);
      remainingSteps -= consumed;
      now += consumed * 16;
      return now;
    };
    globalThis.__arcweftGrantLogicalClockSteps = (count) => {
      if (!Number.isSafeInteger(count) || count < 0) {
        throw new Error(`invalid logical clock step count: ${count}`);
      }
      remainingSteps += count;
    };
    globalThis.__arcweftRemainingLogicalClockSteps = () => remainingSteps;
  });
  await page.addInitScript((value) => {
    globalThis.__arcweftDialogueVisualTimeMillis = value;
  }, visualTimeMillis);
}

async function advanceLogicalClockSteps(page, steps) {
  const startTick = await page.evaluate(() =>
    window.__arcweftLastObservation?.logical_tick ?? null
  );
  expect(Number.isSafeInteger(startTick), "runtime observation has no logical tick");
  const targetTick = startTick + steps;
  await page.evaluate((count) => {
    globalThis.__arcweftGrantLogicalClockSteps(count);
  }, steps);
  await page.waitForFunction(
    (target) =>
      window.__arcweftLastObservation?.logical_tick >= target &&
      globalThis.__arcweftRemainingLogicalClockSteps() === 0,
    targetTick,
    { timeout: 10_000 },
  );
  const captureTick = await page.evaluate(() =>
    window.__arcweftLastObservation?.logical_tick ?? null
  );
  expect(
    captureTick === targetTick,
    `logical clock overshot target ${targetTick}: ${captureTick}`,
  );
  return { startTick, captureTick };
}

async function advanceLogicalClockDuration(page, elapsedMillis) {
  const elapsedSteps = Math.ceil(elapsedMillis / 16);
  const { startTick, captureTick } = await advanceLogicalClockSteps(page, elapsedSteps);
  return {
    quantum_millis: 16,
    activation_tick: startTick,
    capture_tick: captureTick,
    elapsed_steps: elapsedSteps,
    elapsed_millis: elapsedSteps * 16,
  };
}

function checkpointOptions(name) {
  switch (name) {
    case "default":
      return { viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1 };
    case "compact":
      return { viewport: { width: 960, height: 540 }, deviceScaleFactor: 1 };
    case "hidpi":
      return { viewport: { width: 640, height: 360 }, deviceScaleFactor: 2 };
    default:
      return { viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1 };
  }
}

function frameText(frame) {
  return frame?.text?.map((item) => item.visible_text ?? item.text).join("") ?? "";
}

async function openReady(browser, baseUrl, checkpoint) {
  const options = checkpointOptions(checkpoint);
  const page = await browser.newPage(options);
  const visualTimeMillis = checkpointVisualTime(checkpoint);
  const requiredText = checkpointRequiredText(checkpoint);
  await installDeterministicClock(page, visualTimeMillis);
  const errors = collectConsoleErrors(page);
  const search = new URLSearchParams({
    bundle: bundleUrl,
    fonts: [...additionalFontUrls, fontUrl].join(","),
  });
  await page.goto(`${baseUrl}/index.html?${search}`);
  expect(await page.evaluate(() => Boolean(navigator.gpu)), "navigator.gpu is unavailable");
  await page.waitForFunction(
    () => document.querySelector("#arcweft-canvas")?.dataset.arcweftReady === "true",
    null,
    { timeout: 10_000 },
  );
  await page.waitForFunction(
    () => Boolean(window.__arcweftFatal) ||
      Boolean(window.__arcweftLastFrameObservation?.text?.some((item) =>
        item.owner?.kind?.startsWith("dialogue:")
      )),
    null,
    { timeout: 10_000 },
  );
  await advanceDialoguePages(page, checkpointAdvanceCount(checkpoint));
  const logicalClock = await advanceLogicalClockDuration(page, visualTimeMillis);
  try {
    await page.waitForFunction(
      ({ requiredText, expectedImageCount }) =>
        Boolean(window.__arcweftFatal) ||
        (Boolean(window.__arcweftLastObservation?.dialogue) &&
        window.__arcweftLastFrameObservation?.image_count === expectedImageCount &&
        (() => {
          const frame = window.__arcweftLastFrameObservation;
          const text = frame?.text
            ?.map((item) => item.visible_text ?? item.text)
            .join("") ?? "";
          return requiredText.every((token) => text.includes(token));
        })()),
      { requiredText, expectedImageCount },
      { timeout: 10_000 },
    );
    const fatal = await page.evaluate(() => window.__arcweftFatal ?? null);
    if (fatal) {
      throw new Error(`Arcweft player fatal: ${fatal.message}`);
    }
  } catch (error) {
    const state = await page.evaluate(() => ({
      fatal: document.querySelector("#arcweft-fatal")?.textContent ?? null,
      observation: window.__arcweftLastObservation ?? null,
      frameObservation: window.__arcweftLastFrameObservation ?? null,
    }));
    throw new Error(
      `${error.message}\nconsoleErrors=${JSON.stringify(errors, null, 2)}\nstate=${JSON.stringify(state, null, 2)}`,
    );
  }
  return { page, errors, logicalClock };
}

async function advanceDialoguePages(page, count) {
  for (let index = 0; index < count; index += 1) {
    const before = await page.evaluate(() => {
      const item = window.__arcweftLastFrameObservation?.text?.find((candidate) =>
        candidate.owner?.kind?.startsWith("dialogue:")
      );
      return item
        ? { identity: item.owner.kind, text: item.text, visibleText: item.visible_text }
        : null;
    });
    expect(before, `missing dialogue body before advance ${index + 1}`);
    if (before.visibleText !== before.text) {
      await page.keyboard.press("Enter");
      await page.waitForFunction(
        (text) => Boolean(window.__arcweftFatal) ||
          window.__arcweftLastFrameObservation?.text?.some((item) =>
            item.owner?.kind?.startsWith("dialogue:") &&
            item.text === text &&
            item.visible_text === item.text
          ),
        before.text,
        { timeout: 2_000 },
      );
    }
    await page.keyboard.press("Enter");
    await advanceLogicalClockSteps(page, 1);
    await page.waitForFunction(
      (identity) => Boolean(window.__arcweftFatal) ||
        window.__arcweftLastFrameObservation?.text?.some((item) =>
          item.owner?.kind?.startsWith("dialogue:") && item.owner.kind !== identity
        ),
      before.identity,
      { timeout: 2_000 },
    );
  }
}

async function assertCanvasOnlySample(page, checkpoint) {
  expect(await page.locator("canvas#arcweft-canvas").isVisible(), "canvas is not visible");
  expect(await page.locator("button").count() === 0, "DOM button renderer is present");
  const domGameText = await page
    .locator("[data-arcweft-speaker], [data-arcweft-dialogue], [data-arcweft-choice]")
    .count();
  expect(domGameText === 0, "DOM game text renderer is present");
  const frame = await page.evaluate(() => window.__arcweftLastFrameObservation);
  expect(frame?.schema_version === "arcweft.web_frame_observation.v3", "bad frame schema");
  expect(
    frame.image_count === expectedImageCount,
    `expected ${expectedImageCount} image assets, got ${frame.image_count}`,
  );
  expect(
    frame.text_count >= minimumTextCount,
    `expected canonical prepared text evidence, got ${frame.text_count}`,
  );
  const text = frameText(frame);
  for (const token of checkpointRequiredText(checkpoint)) {
    expect(text.includes(token), `missing ${JSON.stringify(token)} for ${checkpoint}`);
  }
  const paragraph = frame.text?.find((item) =>
    item.owner?.kind?.startsWith("dialogue:")
  );
  expect(paragraph?.lines?.length > 0, `missing prepared text lines for ${checkpoint}`);
  expect(paragraph?.runs?.length > 0, `missing prepared text runs for ${checkpoint}`);
  expect(paragraph?.glyphs?.length > 0, `missing prepared text glyphs for ${checkpoint}`);
  expect(
    paragraph.glyphs.every((glyph) =>
      Number.isInteger(glyph.source_start) &&
      Number.isInteger(glyph.source_end) &&
      glyph.bounds &&
      glyph.ink_bounds &&
      typeof glyph.visible === "boolean" &&
      glyph.rgba?.length === 4 &&
      glyph.transform?.matrix_milli?.length === 4 &&
      glyph.transform?.translate_milli?.length === 2
    ),
    `bad canonical prepared glyph evidence for ${checkpoint}`,
  );
  expect(
    paragraph.runs.every((run) =>
      run.style?.rgba?.length === 4 &&
      run.style?.font_families?.length > 0 &&
      typeof run.style?.writing_mode === "string"
    ),
    `bad canonical prepared run evidence for ${checkpoint}`,
  );
}

async function fontFingerprint(url) {
  const fullPath = staticPath(url);
  expect(fullPath && fullPath !== "", `font path is outside served fixture root: ${url}`);
  const bytes = await readFile(fullPath);
  return {
    url,
    path: fullPath,
    byte_len: bytes.byteLength,
    fnv1a64: fnv1a64(bytes),
  };
}

function fnv1a64(bytes) {
  let hash = 0xcbf29ce484222325n;
  const prime = 0x00000100000001b3n;
  const mask = 0xffffffffffffffffn;
  for (const byte of bytes) {
    hash ^= BigInt(byte);
    hash = (hash * prime) & mask;
  }
  return hash.toString(16).padStart(16, "0");
}

async function main() {
  await mkdir(outputDir, { recursive: true });
  const fonts = await Promise.all([...additionalFontUrls, fontUrl].map(fontFingerprint));
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
    for (const checkpoint of checkpoints) {
      const { page, errors, logicalClock } = await openReady(browser, baseUrl, checkpoint);
      try {
        await assertCanvasOnlySample(page, checkpoint);
        expect(errors.length === 0, `console errors: ${errors.join("\n")}`);
        const frame = await page.evaluate(() => window.__arcweftLastFrameObservation);
        const frameEvidence = {
          ...frame,
          checkpoint,
          visual_time_millis: checkpointVisualTime(checkpoint),
          logical_clock: logicalClock,
          execution_path: {
            layout: "web-player-scene",
            raster: "web-shared-wgpu-canvas",
          },
          fonts,
        };
        await writeFile(
          join(outputDir, `web-${checkpoint}.frame.json`),
          `${JSON.stringify(frameEvidence, null, 2)}\n`,
        );
        await page.locator("#arcweft-canvas").screenshot({
          path: join(outputDir, `web-${checkpoint}.png`),
          scale: "device",
        });
        console.log(JSON.stringify({
          sample,
          checkpoint,
          advanceCount: checkpointAdvanceCount(checkpoint),
          textCount: frame.text_count,
          choiceCount: frame.choice_count,
          imageCount: frame.image_count,
          visualTimeMillis: checkpointVisualTime(checkpoint),
          fontHashes: fonts.map((font) => font.fnv1a64),
        }));
      } finally {
        await page.close();
      }
    }
  } finally {
    await browser.close();
    await closeServer(server);
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
