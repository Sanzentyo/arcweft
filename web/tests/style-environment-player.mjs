import { createReadStream } from "node:fs";
import { access, open } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const root = normalize(join(fileURLToPath(new URL(".", import.meta.url)), ".."));
const generatedRoot = join(root, "generated", "style-environment-player");
const modulePath = join(generatedRoot, "arcweft_player_web.js");
const wasmPath = join(generatedRoot, "arcweft_player_web_bg.wasm");
const fixturePath = join(root, "tests", "fixtures", "style-environment-player.awfb");
const fontPath = join(root, "assets", "noto-sans-jp-vf.ttf");

const contentTypes = new Map([
  [".awfb", "application/octet-stream"],
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".ttf", "font/ttf"],
  [".wasm", "application/wasm"],
]);

const harnessHtml = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <title>Arcweft style environment browser contract</title>
  <style>
    html, body { margin: 0; background: #111; }
    canvas { display: block; width: 320px; height: 180px; }
  </style>
</head>
<body>
  <canvas id="player-a"></canvas>
  <canvas id="player-b"></canvas>
  <canvas id="player-c"></canvas>
  <canvas id="player-d"></canvas>
  <canvas id="player-e"></canvas>
</body>
</html>`;

function fail(message) {
  throw new Error(message);
}

function staticPath(requestUrl) {
  const url = new URL(requestUrl, "http://127.0.0.1");
  const pathname = decodeURIComponent(url.pathname);
  if (pathname === "/favicon.ico") {
    return "";
  }
  if (pathname.includes("\0")) {
    return null;
  }
  const fullPath = normalize(join(root, pathname));
  return fullPath.startsWith(root) ? fullPath : null;
}

async function serve(request, response) {
  const url = new URL(request.url, "http://127.0.0.1");
  if (url.pathname === "/style-environment-player.html") {
    response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
    response.end(harnessHtml);
    return;
  }
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
    serve(request, response).catch((error) => {
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

async function requireArtifacts() {
  const paths = [modulePath, wasmPath, fixturePath, fontPath];
  await Promise.all(paths.map(async (path) => {
    try {
      await access(path);
    } catch {
      fail(`required browser artifact is missing: ${path}`);
    }
  }));
}

async function runBrowserContract(browser, baseUrl) {
  const page = await browser.newPage({ viewport: { width: 960, height: 900 } });
  const runtimeErrors = [];
  page.on("pageerror", (error) => runtimeErrors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") {
      runtimeErrors.push(message.text());
    }
  });
  try {
    await page.goto(`${baseUrl}/style-environment-player.html`);
    const hasWebGpu = await page.evaluate(() => Boolean(navigator.gpu));
    if (!hasWebGpu) {
      fail("navigator.gpu is unavailable in the real Chromium run");
    }
    const summary = await page.evaluate(async ({ baseUrl }) => {
      const wasm = await import(
        `${baseUrl}/generated/style-environment-player/arcweft_player_web.js`
      );
      await wasm.default({
        module_or_path:
          `${baseUrl}/generated/style-environment-player/arcweft_player_web_bg.wasm`,
      });
      const fetchBytes = async (url) => {
        const response = await fetch(url);
        if (!response.ok) {
          throw new Error(`failed to fetch ${url}: ${response.status}`);
        }
        return new Uint8Array(await response.arrayBuffer());
      };
      const [bundleBytes, fontBytes] = await Promise.all([
        fetchBytes(`${baseUrl}/tests/fixtures/style-environment-player.awfb`),
        fetchBytes(`${baseUrl}/assets/noto-sans-jp-vf.ttf`),
      ]);
      const passed = [];
      const assert = (condition, message) => {
        if (!condition) {
          throw new Error(message);
        }
      };
      const pass = (name) => passed.push(name);
      const baseSnapshot = () => ({
        colorScheme: "dark",
        contrast: "standard",
        reducedMotion: false,
        textScaleMilli: 1000,
      });
      const lightSnapshot = () => ({ ...baseSnapshot(), colorScheme: "light" });
      const expectError = (name, operation, code, field) => {
        let error;
        try {
          operation();
        } catch (caught) {
          error = caught;
        }
        assert(error !== undefined, `${name}: operation unexpectedly succeeded`);
        assert(error && typeof error === "object", `${name}: error is not a plain object`);
        assert(error.code === code, `${name}: expected ${code}, got ${String(error.code)}`);
        assert(typeof error.message === "string" && error.message.length > 0,
          `${name}: missing error message`);
        if (field !== undefined) {
          assert(error.field === field, `${name}: expected field ${field}, got ${error.field}`);
        }
        const allowed = new Set(["code", "message", "playerId", "field"]);
        assert(Object.keys(error).every((key) => allowed.has(key)),
          `${name}: unexpected error properties ${Object.keys(error).join(",")}`);
        pass(name);
        return error;
      };
      const assertReportShape = (name, report, playerId) => {
        assert(Object.getPrototypeOf(report) === Object.prototype,
          `${name}: result is not a plain object`);
        assert(
          Object.keys(report).sort().join(",") ===
            "changedFields,playerId,redrawRequested,revision",
          `${name}: result keys are not exact`,
        );
        assert(report.playerId === playerId, `${name}: player identity mismatch`);
        assert(typeof report.revision === "string" && /^\d+$/.test(report.revision),
          `${name}: revision is not a decimal string`);
        assert(Array.isArray(report.changedFields), `${name}: changedFields is not an array`);
        assert(typeof report.redrawRequested === "boolean",
          `${name}: redrawRequested is not boolean`);
      };
      const waitFor = async (predicate, label, timeoutMs = 20_000) => {
        const deadline = performance.now() + timeoutMs;
        while (!predicate()) {
          if (performance.now() >= deadline) {
            throw new Error(`timed out waiting for ${label}`);
          }
          await new Promise((resolve) => setTimeout(resolve, 16));
        }
      };
      let readyEvents = 0;
      const fatalEvents = [];
      document.addEventListener("arcweft-player-ready", () => {
        readyEvents += 1;
      });
      document.addEventListener("arcweft-player-fatal", (event) => {
        fatalEvents.push(String(event.detail));
      });
      const create = (canvasId) =>
        wasm.create_arcweft_player(canvasId, bundleBytes.slice(), fontBytes.slice());

      const firstReady = readyEvents;
      const first = create("player-a");
      assert(Number.isInteger(first.id) && first.id > 0, "first handle id is invalid");
      await waitFor(() => {
        if (fatalEvents.length > 0) {
          throw new Error(`player startup failed: ${fatalEvents.join(" | ")}`);
        }
        return readyEvents > firstReady;
      }, "first player GPU readiness");
      pass("create returns a nonzero handle identity");

      const secondReady = readyEvents;
      const second = create("player-b");
      assert(second.id === first.id + 1, "created handle ids are not monotonic");
      await waitFor(() => {
        if (fatalEvents.length > 0) {
          throw new Error(`player startup failed: ${fatalEvents.join(" | ")}`);
        }
        return readyEvents > secondReady;
      }, "second player GPU readiness");
      pass("two canvases have monotonic independent identities");

      expectError(
        "unknown field is rejected",
        () => first.setEnvironment({ ...baseSnapshot(), extra: true }),
        "style_environment.unknown_field",
        "extra",
      );
      const missing = baseSnapshot();
      delete missing.contrast;
      expectError(
        "missing field is rejected",
        () => first.setEnvironment(missing),
        "style_environment.missing_field",
        "contrast",
      );
      expectError(
        "null snapshot is rejected",
        () => first.setEnvironment(null),
        "style_environment.invalid_snapshot",
      );
      expectError(
        "array snapshot is rejected",
        () => first.setEnvironment([]),
        "style_environment.invalid_snapshot",
      );
      expectError(
        "custom prototype is rejected",
        () => first.setEnvironment(Object.create(baseSnapshot())),
        "style_environment.invalid_snapshot",
      );
      expectError(
        "nested value is rejected",
        () => first.setEnvironment({ ...baseSnapshot(), colorScheme: { value: "dark" } }),
        "style_environment.wrong_kind",
        "colorScheme",
      );
      expectError(
        "wrong enum value is rejected",
        () => first.setEnvironment({ ...baseSnapshot(), contrast: "less" }),
        "style_environment.wrong_kind",
        "contrast",
      );
      expectError(
        "NaN text scale is rejected",
        () => first.setEnvironment({ ...baseSnapshot(), textScaleMilli: Number.NaN }),
        "style_environment.wrong_kind",
        "textScaleMilli",
      );
      expectError(
        "fractional text scale is rejected",
        () => first.setEnvironment({ ...baseSnapshot(), textScaleMilli: 1000.5 }),
        "style_environment.wrong_kind",
        "textScaleMilli",
      );
      expectError(
        "unsafe text scale is rejected",
        () => first.setEnvironment({
          ...baseSnapshot(),
          textScaleMilli: Number.MAX_SAFE_INTEGER + 1,
        }),
        "style_environment.wrong_kind",
        "textScaleMilli",
      );
      expectError(
        "text scale below the closed range is rejected",
        () => first.setEnvironment({ ...baseSnapshot(), textScaleMilli: 499 }),
        "style_environment.text_scale_range",
        "textScaleMilli",
      );
      expectError(
        "text scale above the closed range is rejected",
        () => first.setEnvironment({ ...baseSnapshot(), textScaleMilli: 4001 }),
        "style_environment.text_scale_range",
        "textScaleMilli",
      );

      let accessorCalls = 0;
      const accessorSnapshot = baseSnapshot();
      Object.defineProperty(accessorSnapshot, "colorScheme", {
        configurable: true,
        enumerable: true,
        get() {
          accessorCalls += 1;
          throw new Error("the decoder must not invoke accessors");
        },
      });
      expectError(
        "accessor property is rejected without invocation",
        () => first.setEnvironment(accessorSnapshot),
        "style_environment.wrong_kind",
        "colorScheme",
      );
      assert(accessorCalls === 0, "snapshot accessor was invoked");
      pass("decode completes from data descriptors without callbacks");

      const nonEnumerable = baseSnapshot();
      Object.defineProperty(nonEnumerable, "contrast", {
        configurable: true,
        enumerable: false,
        value: "standard",
        writable: true,
      });
      expectError(
        "non-enumerable field is rejected",
        () => first.setEnvironment(nonEnumerable),
        "style_environment.wrong_kind",
        "contrast",
      );
      const symbolSnapshot = baseSnapshot();
      symbolSnapshot[Symbol("extra")] = true;
      expectError(
        "symbol field is rejected",
        () => first.setEnvironment(symbolSnapshot),
        "style_environment.unknown_field",
        "<symbol>",
      );
      const descriptorTrap = new Proxy(baseSnapshot(), {
        getOwnPropertyDescriptor() {
          throw new Error("descriptor trap");
        },
      });
      expectError(
        "throwing descriptor trap is rejected",
        () => first.setEnvironment(descriptorTrap),
        "style_environment.invalid_snapshot",
      );
      const duplicateKeys = new Proxy(baseSnapshot(), {
        ownKeys() {
          return [
            "colorScheme",
            "colorScheme",
            "contrast",
            "reducedMotion",
            "textScaleMilli",
          ];
        },
      });
      expectError(
        "duplicate-equivalent key inventory is rejected",
        () => first.setEnvironment(duplicateKeys),
        "style_environment.invalid_snapshot",
      );

      const same = first.setEnvironment(baseSnapshot());
      assertReportShape("same-value update", same, first.id);
      assert(same.changedFields.length === 0, "same-value update changed fields");
      assert(same.redrawRequested === false, "same-value update requested redraw");
      pass("same-value update preserves revision and redraw state");

      const changed = first.setEnvironment(lightSnapshot());
      assertReportShape("changed update", changed, first.id);
      assert(changed.changedFields.join(",") === "color_scheme",
        `changed update fields were ${changed.changedFields.join(",")}`);
      assert(changed.redrawRequested === true, "used environment change did not request redraw");
      assert(BigInt(changed.revision) > BigInt(same.revision),
        "changed update did not advance revision");
      pass("changed update reports exact field and redraw request");

      const secondSame = second.setEnvironment(baseSnapshot());
      assert(secondSame.changedFields.length === 0,
        "first player update leaked into second player");
      const secondChanged = second.setEnvironment({ ...baseSnapshot(), contrast: "more" });
      assert(secondChanged.changedFields.join(",") === "contrast",
        "second player did not retain independent environment state");
      const firstStillLight = first.setEnvironment(lightSnapshot());
      assert(firstStillLight.changedFields.length === 0,
        "second player update changed first player state");
      pass("multiple players retain independent environment state");

      expectError(
        "duplicate active canvas is rejected",
        () => create("player-a"),
        "style_environment.canvas_in_use",
      );

      const canvas = document.getElementById("player-a");
      const measuredWidth = canvas.clientWidth;
      let measurementAttempted = false;
      let reentrantError;
      Object.defineProperty(canvas, "clientWidth", {
        configurable: true,
        get() {
          if (!measurementAttempted) {
            measurementAttempted = true;
            try {
              first.setEnvironment({ ...lightSnapshot(), reducedMotion: true });
            } catch (error) {
              reentrantError = error;
            }
          }
          return measuredWidth;
        },
      });
      window.dispatchEvent(new Event("resize"));
      await waitFor(() => measurementAttempted, "synchronous canvas measurement callback");
      delete canvas.clientWidth;
      assert(reentrantError?.code === "style_environment.reentrant_update",
        `reentrant update returned ${String(reentrantError?.code)}`);
      assert(reentrantError.playerId === first.id, "reentrant error lost player identity");
      const afterReentry = first.setEnvironment(lightSnapshot());
      assert(afterReentry.changedFields.length === 0,
        "reentrant failure partially mutated environment state");
      pass("canvas measurement reentrancy is typed and atomic");

      let nextId = second.id + 1;
      const startResult = wasm.start_arcweft_player(
        "player-c",
        bundleBytes.slice(),
        fontBytes.slice(),
      );
      assert(startResult === undefined, "existing start export no longer returns unit");
      const retained = wasm.arcweft_player_handle(nextId);
      assert(retained.id === nextId, "registry lookup returned the wrong player");
      retained.free();
      wasm.stop_arcweft_player(nextId);
      expectError(
        "stopped registry player is no longer discoverable",
        () => wasm.arcweft_player_handle(nextId),
        "style_environment.unknown_player",
      );
      pass("start lookup and stop retain deterministic registry ownership");

      nextId += 1;
      const callerOwned = create("player-d");
      assert(callerOwned.id === nextId, "caller-owned id sequence changed");
      callerOwned.free();
      await Promise.resolve();
      nextId += 1;
      const afterFree = create("player-d");
      assert(afterFree.id === nextId, "free did not release the canvas for reuse");
      afterFree.shutdown();
      afterFree.free();
      pass("caller-owned free shuts down and releases canvas ownership");

      second.shutdown();
      expectError(
        "use after shutdown is rejected",
        () => second.setEnvironment(baseSnapshot()),
        "style_environment.player_closed",
      );
      second.free();
      first.shutdown();
      first.free();
      pass("explicit shutdown closes future updates");

      return {
        schema: "arcweft.style_environment.browser.v1",
        passed: passed.length,
        checks: passed,
      };
    }, { baseUrl });
    if (runtimeErrors.length > 0) {
      fail(`browser emitted runtime errors: ${runtimeErrors.join(" | ")}`);
    }
    return summary;
  } finally {
    await page.close();
  }
}

async function main() {
  await requireArtifacts();
  const { server, baseUrl } = await startServer();
  const args = ["--enable-unsafe-webgpu"];
  if (process.platform === "win32") {
    args.push("--use-angle=d3d11");
  }
  const launch = {
    channel: process.env.ARW_PLAYWRIGHT_CHANNEL || "chrome",
    headless: true,
    args,
  };
  const browser = await chromium.launch(launch);
  try {
    const summary = await runBrowserContract(browser, baseUrl);
    console.log(JSON.stringify(summary));
  } finally {
    await browser.close();
    await closeServer(server);
  }
}

await main().catch((error) => {
  console.error(error instanceof Error ? error.stack || error.message : error);
  process.exitCode = 1;
});
