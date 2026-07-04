import { createReadStream } from "node:fs";
import { mkdir, open, readFile, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { createHash } from "node:crypto";
import { deflateSync } from "node:zlib";
import { execFileSync } from "node:child_process";
import { dirname, extname, isAbsolute, join, normalize, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const webRoot = normalize(join(fileURLToPath(new URL(".", import.meta.url)), ".."));
const repoRoot = normalize(join(webRoot, ".."));
const WIDTH = 320;
const HEIGHT = 180;
const METRICS = "psnr,ssim,mse,mae,maxae";
const WEBGPU_ENV = "ARW_SEQ06_13E1_INSET_SHADOW_WEBGPU";
const REQUIRED_ENV = "ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_REQUIRED";
const PINNED_ENV = "ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_PINNED";

const contentTypes = new Map([
  [".css", "text/css; charset=utf-8"],
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".json", "application/json; charset=utf-8"],
  [".png", "image/png"],
  [".wasm", "application/wasm"],
]);

function fail(message) {
  throw new Error(message);
}

function expect(condition, message) {
  if (!condition) {
    fail(message);
  }
}

function parseArgs(argv) {
  const args = {
    root: repoRoot,
    outDir: join(repoRoot, "target/seq06.13e.1-inset-box-shadow-golden"),
    browserChannel: process.env.ARW_PLAYWRIGHT_CHANNEL || "chrome",
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    switch (arg) {
      case "--root":
        args.root = resolve(nextArg(argv, ++index, arg));
        break;
      case "--out-dir":
        args.outDir = resolveMaybeRoot(args.root, nextArg(argv, ++index, arg));
        break;
      case "--browser-channel":
        args.browserChannel = nextArg(argv, ++index, arg);
        break;
      case "--help":
      case "-h":
        args.help = true;
        break;
      default:
        fail(`unknown argument: ${arg}`);
    }
  }
  return args;
}

function nextArg(argv, index, name) {
  const value = argv[index];
  if (!value) {
    fail(`${name} requires a value`);
  }
  return value;
}

function resolveMaybeRoot(root, value) {
  return isAbsolute(value) ? normalize(value) : normalize(join(root, value));
}

function printHelp() {
  console.log(`seq06.13e.1 Web exact PNG capture\n\nUsage:\n  node web/tests/seq06-13e1-inset-shadow-exact-capture.mjs --root . --out-dir target/seq06.13e.1-inset-box-shadow-golden [--browser-channel chrome]\n\nThe visual source is the wasm-exported Arcweft WGPU compositor readback. This script never uses DOM/CSS screenshots, SVG filters, Canvas 2D fallback, or CPU raster fallback.`);
}

function staticPath(requestUrl) {
  const url = new URL(requestUrl, "http://127.0.0.1");
  let pathname = decodeURIComponent(url.pathname);
  if (pathname === "/") {
    pathname = "/seq06-13e1-inset-shadow-capture.html";
  }
  if (pathname === "/favicon.ico") {
    return "";
  }
  if (pathname.includes("\0")) {
    return null;
  }
  const fullPath = normalize(join(webRoot, pathname));
  return fullPath.startsWith(webRoot) ? fullPath : null;
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
    "content-type": contentTypes.get(extname(fullPath).toLowerCase()) ?? "application/octet-stream",
    "cache-control": "no-store",
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
  return new Promise((resolveServer) => {
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      resolveServer({ server, baseUrl: `http://127.0.0.1:${address.port}` });
    });
  });
}

function closeServer(server) {
  return new Promise((resolveClose, reject) => {
    server.close((error) => (error ? reject(error) : resolveClose()));
  });
}

function launchArgs() {
  const args = ["--enable-unsafe-webgpu"];
  if (process.platform === "win32") {
    args.push("--use-angle=d3d11");
  }
  return args;
}

async function collectConsoleErrors(page) {
  const errors = [];
  page.on("console", (message) => {
    if (message.type() === "error") {
      errors.push(message.text());
    }
  });
  page.on("pageerror", (error) => errors.push(error.message));
  return errors;
}

async function captureFromBrowser(page, baseUrl) {
  const errors = await collectConsoleErrors(page);
  await page.goto(`${baseUrl}/seq06-13e1-inset-shadow-capture.html`);
  await page.waitForFunction(() => globalThis.__arcweftSeq0613e1CaptureHostReady === true);
  expect(await page.evaluate(() => Boolean(navigator.gpu)), "navigator.gpu is unavailable");
  const result = await page.evaluate(async () => {
    const module = await import("/pkg/arcweft_player_web.js");
    await module.default();
    const capture = await module.capture_seq06_13e1_inset_box_shadow_exact_png();
    const canvas = document.querySelector("#seq06-13e1-canvas-fingerprint");
    return {
      width: capture.width,
      height: capture.height,
      format: capture.format,
      rgba: Array.from(capture.rgba),
      observeJson: capture.observe_json,
      adapterInfoJson: capture.adapter_info_json,
      page: {
        userAgent: navigator.userAgent,
        language: navigator.language,
        secureContext: globalThis.isSecureContext,
        webgpuAvailable: Boolean(navigator.gpu),
        devicePixelRatio: window.devicePixelRatio,
        canvasWidth: canvas?.width ?? null,
        canvasHeight: canvas?.height ?? null,
        canvasClientWidth: canvas?.clientWidth ?? null,
        canvasClientHeight: canvas?.clientHeight ?? null,
      },
    };
  });
  expect(errors.length === 0, `console errors: ${errors.join("\n")}`);
  expect(result.width === WIDTH, `unexpected capture width: ${result.width}`);
  expect(result.height === HEIGHT, `unexpected capture height: ${result.height}`);
  expect(result.format === "rgba8unorm", `unexpected capture format: ${result.format}`);
  expect(result.rgba.length === WIDTH * HEIGHT * 4, `unexpected rgba byte count: ${result.rgba.length}`);
  return result;
}

async function writeCapturePacket(args, browser, result) {
  const webDir = join(args.outDir, "web");
  await mkdir(webDir, { recursive: true });
  const candidatePath = join(webDir, "seq06_13e1_inset_box_shadow.candidate.png");
  const observePath = join(webDir, "seq06_13e1_inset_box_shadow.observe.json");
  const environmentPath = join(webDir, "seq06_13e1_inset_box_shadow.environment.json");
  const referencePath = join(
    args.root,
    "fixtures/visual-smoke-goldens/seq06.13e.1-inset-box-shadow/web/seq06_13e1_inset_box_shadow.png",
  );

  const rgba = Buffer.from(result.rgba);
  await writeFile(candidatePath, encodePngRgba(WIDTH, HEIGHT, rgba));

  const observe = JSON.parse(result.observeJson);
  observe.candidate_png = slashPath(relative(args.root, candidatePath));
  observe.browser = browserFingerprint(browser, result.page, args.browserChannel);
  await writeFile(observePath, `${JSON.stringify(observe, null, 2)}\n`);

  const environment = await environmentJson(args, browser, result, candidatePath, referencePath);
  await writeFile(environmentPath, `${JSON.stringify(environment, null, 2)}\n`);

  console.log(`wrote seq06.13e.1 Web candidate ${candidatePath}`);
  console.log(`wrote seq06.13e.1 Web observe ${observePath}`);
  console.log(`wrote seq06.13e.1 Web environment ${environmentPath}`);
}

function browserFingerprint(browser, page, channel) {
  return {
    name: browser.browserType().name(),
    version: browser.version(),
    channel,
    user_agent: page.userAgent,
    language: page.language,
    secure_context: page.secureContext,
  };
}

async function environmentJson(args, browser, result, candidatePath, referencePath) {
  const adapter = JSON.parse(result.adapterInfoJson);
  return {
    schema: "arcweft.seq06.13e1.inset_box_shadow.web_environment.v1",
    generated_unix_seconds: Math.floor(Date.now() / 1000),
    target: "web",
    status: "artifacts_complete",
    classification_code: "ready_for_packet_validation",
    message: "browser WebGPU exact readback capture wrote candidate PNG, observe JSON, and same-run Web fingerprint",
    environment: {
      required: Object.prototype.hasOwnProperty.call(process.env, REQUIRED_ENV),
      pinned: Object.prototype.hasOwnProperty.call(process.env, PINNED_ENV),
      os: {
        family: process.platform,
        arch: process.arch,
        version_family: process.version,
      },
      runtime: {
        name: "Playwright browser test harness",
        version: npmCommand(args.root, ["exec", "--", "playwright", "--version"]),
        node_version: process.version,
        npm_version: npmCommand(args.root, ["--version"]),
      },
      browser: browserFingerprint(browser, result.page, args.browserChannel),
      webgpu: {
        required_env: process.env[WEBGPU_ENV] ?? null,
        available: result.page.webgpuAvailable,
        adapter_label: adapter.name,
        adapter_vendor: adapter.vendor,
        adapter_device: adapter.device,
        adapter_device_type: adapter.device_type,
        backend: adapter.backend,
        driver: adapter.driver,
        driver_info: adapter.driver_info,
      },
      canvas: {
        width: result.page.canvasWidth,
        height: result.page.canvasHeight,
        client_width: result.page.canvasClientWidth,
        client_height: result.page.canvasClientHeight,
        device_pixel_ratio: result.page.devicePixelRatio,
      },
      feature_flags: [
        "WebGPU enabled",
        "deterministic Arcweft WGPU texture copy/readback enabled",
        "DOM/CSS screenshot path disabled for this fixture",
      ],
      imq: {
        available: commandAvailable("imq", ["--version"]),
        version: commandOutput("imq", ["--version"]),
        metrics: METRICS,
      },
      arcweft: {
        commit: commandOutput("git", ["-C", args.root, "rev-parse", "HEAD"]),
        dirty: Boolean(commandOutput("git", ["-C", args.root, "status", "--short"])),
      },
      fixture: {
        source_hash: gitHashObject(args.root, join(args.root, "docs/fixtures/web/seq06_13e1_inset_box_shadow_exact_golden.json")),
        policy_hash: gitHashObject(args.root, join(args.root, "fixtures/visual-smoke-goldens/seq06.13e.1-inset-box-shadow-exact-png-policy.json")),
      },
      reference: {
        path: slashPath(relative(args.root, referencePath)),
        exists: await exists(referencePath),
        hash: await sha256FileOrNull(referencePath),
      },
      candidate: {
        path: slashPath(relative(args.root, candidatePath)),
        exists: await exists(candidatePath),
        hash: await sha256FileOrNull(candidatePath),
      },
    },
    artifacts: {
      candidate_png: slashPath(relative(args.root, candidatePath)),
      reference_png: slashPath(relative(args.root, referencePath)),
      observation_json: "target/seq06.13e.1-inset-box-shadow-golden/web/seq06_13e1_inset_box_shadow.observe.json",
      imq_json: "target/seq06.13e.1-inset-box-shadow-golden/web/seq06_13e1_inset_box_shadow.imq.json",
      command_logs: "target/seq06.13e.1-inset-box-shadow-golden/web/command-logs/",
    },
  };
}

function npmCommand(root, args) {
  return commandOutput("npm", args, { cwd: join(root, "web") });
}

function commandAvailable(command, args) {
  return commandOutput(command, args) !== null;
}

function commandOutput(command, args, options = {}) {
  try {
    return execFileSync(command, args, {
      cwd: options.cwd,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    }).trim() || null;
  } catch {
    return null;
  }
}

function gitHashObject(root, path) {
  if (!commandAvailable("git", ["--version"])) {
    return null;
  }
  return commandOutput("git", ["-C", root, "hash-object", path]);
}

async function exists(path) {
  try {
    const file = await open(path, "r");
    await file.close();
    return true;
  } catch {
    return false;
  }
}

async function sha256FileOrNull(path) {
  if (!(await exists(path))) {
    return null;
  }
  const bytes = await readFile(path);
  return createHash("sha256").update(bytes).digest("hex");
}

function slashPath(path) {
  return path.split("\\").join("/");
}

function encodePngRgba(width, height, rgba) {
  const rowBytes = width * 4;
  const raw = Buffer.alloc((rowBytes + 1) * height);
  for (let y = 0; y < height; y += 1) {
    const rawRow = y * (rowBytes + 1);
    raw[rawRow] = 0;
    rgba.copy(raw, rawRow + 1, y * rowBytes, (y + 1) * rowBytes);
  }
  const chunks = [
    pngChunk("IHDR", ihdr(width, height)),
    pngChunk("IDAT", deflateSync(raw, { level: 9 })),
    pngChunk("IEND", Buffer.alloc(0)),
  ];
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), ...chunks]);
}

function ihdr(width, height) {
  const data = Buffer.alloc(13);
  data.writeUInt32BE(width, 0);
  data.writeUInt32BE(height, 4);
  data[8] = 8;
  data[9] = 6;
  data[10] = 0;
  data[11] = 0;
  data[12] = 0;
  return data;
}

function pngChunk(type, data) {
  const typeBytes = Buffer.from(type, "ascii");
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length, 0);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([typeBytes, data])), 0);
  return Buffer.concat([length, typeBytes, data, crc]);
}

let crcTable = null;
function crc32(bytes) {
  if (!crcTable) {
    crcTable = new Uint32Array(256);
    for (let n = 0; n < 256; n += 1) {
      let c = n;
      for (let k = 0; k < 8; k += 1) {
        c = (c & 1) ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      }
      crcTable[n] = c >>> 0;
    }
  }
  let c = 0xffffffff;
  for (const byte of bytes) {
    c = crcTable[(c ^ byte) & 0xff] ^ (c >>> 8);
  }
  return (c ^ 0xffffffff) >>> 0;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) {
    printHelp();
    return;
  }
  const { server, baseUrl } = await startServer();
  const browser = await chromium.launch({
    channel: args.browserChannel,
    headless: true,
    args: launchArgs(),
  });
  try {
    const page = await browser.newPage({
      viewport: { width: WIDTH, height: HEIGHT },
      deviceScaleFactor: 1,
    });
    try {
      const result = await captureFromBrowser(page, baseUrl);
      await writeCapturePacket(args, browser, result);
    } finally {
      await page.close();
    }
  } finally {
    await browser.close();
    await closeServer(server);
  }
}

await main().catch((error) => {
  console.error(error instanceof Error ? error.stack || error.message : error);
  process.exitCode = 1;
});
