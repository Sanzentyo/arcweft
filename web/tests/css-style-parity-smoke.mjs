import { createReadStream } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { open } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const root = normalize(join(fileURLToPath(new URL(".", import.meta.url)), ".."));
const outputDir = process.env.ARW_CSS_STYLE_PARITY_DIR ??
  normalize(join(root, "..", "target", "css-style-parity"));
const checkpoints = (process.env.ARW_CSS_STYLE_PARITY_CHECKPOINTS ?? "default,compact")
  .split(",")
  .map((name) => name.trim())
  .filter(Boolean);
const bundleUrl = process.env.ARW_CSS_STYLE_PARITY_BUNDLE_URL ??
  "./local/css-style-parity.awfb";
const fontUrl = process.env.ARW_CSS_STYLE_PARITY_FONT_URL ??
  "./assets/arcweft-demo.ttf";
const additionalFontUrls = (
  process.env.ARW_CSS_STYLE_PARITY_ADDITIONAL_FONT_URLS ??
    "./assets/noto-sans-jp-css-style-parity.ttf"
)
  .split(",")
  .map((url) => url.trim())
  .filter(Boolean);
const visualTimeMillis = Number.parseInt(
  process.env.ARW_CSS_STYLE_PARITY_VISUAL_TIME_MILLIS ?? "9000",
  10,
);

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

async function installDeterministicClock(page, maxMillis) {
  await page.addInitScript(() => {
    let now = 0;
    globalThis.__arcweftNowMillis = () => {
      now = Math.min(now + 16, globalThis.__arcweftCssStyleParityMaxMillis ?? 9000);
      return now;
    };
  });
  await page.addInitScript((value) => {
    globalThis.__arcweftCssStyleParityMaxMillis = value;
    globalThis.__arcweftDialogueVisualTimeMillis = value;
  }, maxMillis);
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
      throw new Error(`unknown CSS style parity checkpoint: ${name}`);
  }
}

function frameText(frame) {
  const textBlocks = frame?.text?.map((item) => item.text) ?? [];
  const preparedText =
    frame?.text?.map((item) => item.visible_text ?? item.text) ?? [];
  return [...textBlocks, ...preparedText].join("");
}

async function openReady(browser, baseUrl, checkpoint) {
  const options = checkpointOptions(checkpoint);
  const page = await browser.newPage(options);
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
  try {
    await page.waitForFunction(
      () =>
        Boolean(window.__arcweftFatal) ||
        (Boolean(window.__arcweftLastObservation?.dialogue) &&
        window.__arcweftLastFrameObservation?.image_count === 0 &&
        (() => {
          const frame = window.__arcweftLastFrameObservation;
          const textBlocks = frame?.text?.map((item) => item.text) ?? [];
          const preparedText =
            frame?.text?.map((item) => item.visible_text ?? item.text) ?? [];
          const text = [...textBlocks, ...preparedText].join("");
          return text.includes("DSL-styled text") && text.includes("wave motion");
        })()),
      null,
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
  return { page, errors };
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
  expect(frame.image_count === 0, "CSS style parity sample should have no image assets");
  expect(frame.text_count >= 2, `expected styled text evidence, got ${frame.text_count}`);
  expect(
    frame.text_count >= 2,
    `expected canonical prepared text evidence, got ${frame.text_count}`,
  );
  const text = frameText(frame);
  expect(text.includes("DSL-styled text"), `missing styled sample text for ${checkpoint}`);
  if (checkpoint === "default") {
    expect(text.includes("color"), `missing color sample text for ${checkpoint}`);
    expect(text.includes("wave motion"), `missing wave sample text for ${checkpoint}`);
  }
  const paragraph = frame.text?.find((item) => item.owner?.kind?.endsWith(":body"));
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
      const { page, errors } = await openReady(browser, baseUrl, checkpoint);
      try {
        await assertCanvasOnlySample(page, checkpoint);
        expect(errors.length === 0, `console errors: ${errors.join("\n")}`);
        const frame = await page.evaluate(() => window.__arcweftLastFrameObservation);
        const frameEvidence = {
          ...frame,
          checkpoint,
          visual_time_millis: visualTimeMillis,
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
          sample: "css-style-parity",
          checkpoint,
          textCount: frame.text_count,
          choiceCount: frame.choice_count,
          imageCount: frame.image_count,
          visualTimeMillis,
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
