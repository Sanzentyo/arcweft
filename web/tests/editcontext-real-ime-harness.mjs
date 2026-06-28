import { createReadStream } from "node:fs";
import { mkdir, open, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import process from "node:process";
import { chromium } from "playwright";

const webRoot = normalize(join(fileURLToPath(new URL(".", import.meta.url)), ".."));
const repoRoot = normalize(join(webRoot, ".."));
const defaultFixtureDir = join(repoRoot, "fixtures", "web-editcontext-real-ime");
const mode = argValue("--mode") ?? process.env.ARCWEFT_REAL_IME_MODE ?? "manual";
const outputDir = resolve(argValue("--output-dir") ?? process.env.ARCWEFT_REAL_IME_OUTPUT_DIR ?? defaultFixtureDir);
const allowText = flag("--allow-text") || process.env.ARCWEFT_REAL_IME_ALLOW_TEXT === "1";
const expectedCommit = argValue("--expected-commit") ?? process.env.ARCWEFT_REAL_IME_EXPECTED_COMMIT ?? "日本語";
const keepOpen = flag("--keep-open") || process.env.ARCWEFT_REAL_IME_KEEP_OPEN === "1";
const recordVideo = flag("--video") || process.env.ARCWEFT_REAL_IME_RECORD_VIDEO === "1";
const forceUnsupported = mode === "unsupported" || flag("--unsupported");
const contentTypes = new Map([
  [".css", "text/css; charset=utf-8"],
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".json", "application/json; charset=utf-8"],
  [".png", "image/png"],
  [".ttf", "font/ttf"],
]);

function argValue(name) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : null;
}

function flag(name) {
  return process.argv.includes(name);
}

function staticPath(requestUrl) {
  const url = new URL(requestUrl, "http://127.0.0.1");
  const pathname = decodeURIComponent(url.pathname);
  if (pathname.includes("\0")) {
    return null;
  }
  const fullPath = normalize(join(webRoot, pathname === "/" ? "/ime-sample.html" : pathname));
  return fullPath.startsWith(webRoot) ? fullPath : null;
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
  return new Promise((resolveServer) => {
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      resolveServer({ server, baseUrl: `http://127.0.0.1:${address.port}` });
    });
  });
}

function closeServer(server) {
  return new Promise((resolveClose, reject) => {
    server.close((error) => error ? reject(error) : resolveClose());
  });
}

function browserLaunchOptions() {
  const args = [
    "--disable-background-timer-throttling",
    "--disable-renderer-backgrounding",
    "--enable-blink-features=EditContext",
  ];
  const options = {
    headless: forceUnsupported,
    args,
  };
  const executablePath = argValue("--browser") ?? process.env.ARCWEFT_REAL_IME_BROWSER;
  if (executablePath) {
    options.executablePath = executablePath;
    return options;
  }
  const channel = argValue("--channel") ?? process.env.ARCWEFT_REAL_IME_CHANNEL;
  if (channel) {
    options.channel = channel;
    return options;
  }
  return options;
}

const recorderInit = ({ allowText, expectedCommit, forceUnsupported }) => {
  const redactText = (value) => {
    const text = String(value ?? "");
    const summary = {
      redacted: !allowText,
      utf16Length: text.length,
      hasJapanese: /[\u3040-\u30ff\u3400-\u9fff]/u.test(text),
      hasAscii: /[A-Za-z]/u.test(text),
    };
    if (allowText) {
      summary.value = text;
    }
    return summary;
  };
  const rect = (value) => {
    if (!value) {
      return null;
    }
    const x = number(value.x ?? value.left, 0);
    const y = number(value.y ?? value.top, 0);
    const width = number(value.width, Math.max(0, number(value.right, x) - x));
    const height = number(value.height, Math.max(0, number(value.bottom, y) - y));
    return {
      x,
      y,
      width,
      height,
      nonOrigin: x !== 0 || y !== 0,
    };
  };
  const rects = (values) => Array.from(values ?? []).map(rect).filter(Boolean);
  const number = (value, fallback) => Number.isFinite(Number(value)) ? Number(value) : fallback;
  const trace = {
    schemaVersion: "arcweft.web-editcontext-real-ime.trace.v1",
    capturedAt: new Date().toISOString(),
    mode: forceUnsupported ? "unsupported" : "manual",
    expectedCommit: redactText(expectedCommit),
    redaction: {
      rawTextAllowed: allowText,
      secureFieldPolicy: "secure traces must redact surrounding text, event text, clipboard text, and selection-derived geometry",
    },
    events: [],
  };
  const record = (kind, detail = {}) => {
    trace.events.push({
      seq: trace.events.length + 1,
      timeMs: Math.round(performance.now() * 1000) / 1000,
      kind,
      detail,
    });
  };
  const summarizeStatus = (detail = {}) => ({
    owner: detail.owner,
    state: detail.state,
    secure: Boolean(detail.secure),
    fallbackInstalled: Boolean(detail.fallbackInstalled),
    composing: Boolean(detail.composing),
    selectionStart: number(detail.selectionStart, 0),
    selectionEnd: number(detail.selectionEnd, 0),
    command: detail.command,
    selecting: Boolean(detail.selecting),
    nonOrigin: detail.nonOrigin,
    requestedRangeStart: detail.requestedRangeStart,
    requestedRangeEnd: detail.requestedRangeEnd,
    message: detail.message,
  });
  const summarizeRender = (detail = {}) => {
    const secure = Boolean(detail.secure);
    return {
      secure,
      composing: Boolean(detail.composing),
      textLength: number(detail.textLength, 0),
      selectionStart: number(detail.selectionStart, 0),
      selectionEnd: number(detail.selectionEnd, 0),
      compositionStart: secure ? 0 : detail.compositionStart,
      compositionEnd: secure ? 0 : detail.compositionEnd,
      text: secure ? { redacted: true, utf16Length: 0 } : redactText(detail.text),
      caretRect: secure ? null : rect(detail.caretRect),
    };
  };
  const summarizeTextUpdate = (event) => ({
    updateRangeStart: number(event.updateRangeStart, 0),
    updateRangeEnd: number(event.updateRangeEnd, 0),
    selectionStart: number(event.selectionStart, 0),
    selectionEnd: number(event.selectionEnd, 0),
    compositionStart: typeof event.compositionStart === "number" ? event.compositionStart : null,
    compositionEnd: typeof event.compositionEnd === "number" ? event.compositionEnd : null,
    text: redactText(event.text),
  });
  const summarizeMethod = (name, args) => {
    switch (name) {
      case "updateText":
        return {
          updateRangeStart: number(args[0], 0),
          updateRangeEnd: number(args[1], 0),
          text: redactText(args[2]),
        };
      case "updateSelection":
        return {
          selectionStart: number(args[0], 0),
          selectionEnd: number(args[1], 0),
        };
      case "updateControlBounds":
      case "updateSelectionBounds":
        return { rect: rect(args[0]) };
      case "updateCharacterBounds": {
        const bounds = rects(args[1]);
        return {
          rangeStart: number(args[0], 0),
          bounds,
          nonOrigin: bounds.some((candidate) => candidate.nonOrigin),
        };
      }
      default:
        return { argCount: args.length };
    }
  };
  const installFinishOverlay = () => {
    const panel = document.createElement("aside");
    panel.id = "arcweft-real-ime-recorder-panel";
    panel.style.cssText = [
      "position:fixed",
      "z-index:2147483647",
      "right:16px",
      "bottom:16px",
      "max-width:360px",
      "padding:12px",
      "border:1px solid rgb(100 210 200 / 70%)",
      "border-radius:8px",
      "background:rgb(13 17 23 / 95%)",
      "color:white",
      "font:13px/1.45 system-ui,sans-serif",
      "box-shadow:0 8px 32px rgb(0 0 0 / 35%)",
    ].join(";");
    panel.innerHTML = `
      <strong>Arcweft real IME recorder</strong>
      <ol style="padding-left:1.2em;margin:8px 0">
        <li>Focus the Arcweft text surface.</li>
        <li>Use the OS Japanese IME: type <code>nihongo</code>, convert to <code>日本語</code>, then commit.</li>
        <li>While composition is active, move the caret/selection with ArrowLeft and Shift+ArrowLeft.</li>
        <li>Click and drag in the text surface, then test Backspace, Delete, and Select All.</li>
      </ol>
      <button id="arcweft-real-ime-finish" type="button">Finish trace</button>
    `;
    document.body.append(panel);
    document.getElementById("arcweft-real-ime-finish")?.addEventListener("click", () => {
      window.__arcweftRealImeFinished = true;
      record("manual_finish", { source: "button" });
    });
  };
  const instrumentEditContextInstance = (editContext) => {
    for (const name of [
      "updateText",
      "updateSelection",
      "updateControlBounds",
      "updateSelectionBounds",
      "updateCharacterBounds",
    ]) {
      const original = editContext?.[name];
      if (typeof original !== "function") {
        record("editcontext_method_missing", { method: name });
        continue;
      }
      try {
        Object.defineProperty(editContext, name, {
          configurable: true,
          value: (...args) => {
            record("editcontext_method", { method: name, ...summarizeMethod(name, args) });
            return Reflect.apply(original, editContext, args);
          },
        });
      } catch (error) {
        record("editcontext_method_patch_failed", { method: name, message: String(error?.message ?? error) });
      }
    }
    for (const type of ["compositionstart", "compositionend"]) {
      editContext.addEventListener(type, () => record(type, {}));
    }
    editContext.addEventListener("textupdate", (event) => record("textupdate", summarizeTextUpdate(event)));
    editContext.addEventListener("textformatupdate", (event) => {
      const formats = typeof event.getTextFormats === "function" ? event.getTextFormats() : [];
      record("textformatupdate", { formatCount: formats.length });
    });
    editContext.addEventListener("characterboundsupdate", (event) => record("characterboundsupdate", {
      rangeStart: number(event.rangeStart, 0),
      rangeEnd: number(event.rangeEnd, 0),
    }));
  };
  const installConstructorRecorder = () => {
    if (forceUnsupported) {
      try {
        Object.defineProperty(window, "EditContext", { configurable: true, writable: true, value: undefined });
      } catch (error) {
        record("force_unsupported_failed", { target: "window.EditContext", message: String(error?.message ?? error) });
      }
      record("forced_unsupported", {
        editContextType: typeof window.EditContext,
        elementHasEditContext: "editContext" in HTMLElement.prototype,
      });
      return;
    }
    const NativeEditContext = window.EditContext;
    if (typeof NativeEditContext !== "function") {
      record("unsupported_feature_detection", {
        editContextType: typeof NativeEditContext,
        elementHasEditContext: "editContext" in HTMLElement.prototype,
      });
      return;
    }
    try {
      function InstrumentedEditContext(options = {}) {
        const editContext = new NativeEditContext(options);
        record("editcontext_construct", {
          initialText: redactText(options?.text),
          hasUpdateSelection: typeof editContext.updateSelection === "function",
          hasUpdateSelectionBounds: typeof editContext.updateSelectionBounds === "function",
          hasUpdateCharacterBounds: typeof editContext.updateCharacterBounds === "function",
        });
        instrumentEditContextInstance(editContext);
        return editContext;
      }
      Object.setPrototypeOf(InstrumentedEditContext, NativeEditContext);
      InstrumentedEditContext.prototype = NativeEditContext.prototype;
      Object.defineProperty(window, "EditContext", {
        configurable: true,
        writable: true,
        value: InstrumentedEditContext,
      });
      record("editcontext_constructor_instrumented", {
        elementHasEditContext: "editContext" in HTMLElement.prototype,
      });
    } catch (error) {
      record("editcontext_constructor_patch_failed", { message: String(error?.message ?? error) });
    }
  };
  window.__arcweftRealImeTrace = trace;
  window.__arcweftRealImeRecord = record;
  window.__arcweftRealImeFinished = false;
  installConstructorRecorder();
  document.addEventListener("arcweft-text-input-status", (event) => record("arcweft_status", summarizeStatus(event.detail)), true);
  document.addEventListener("arcweft-text-input-render", (event) => record("arcweft_render", summarizeRender(event.detail)), true);
  window.addEventListener("keydown", (event) => record("keydown", {
    key: event.key,
    code: event.code,
    shiftKey: event.shiftKey,
    ctrlKey: event.ctrlKey,
    altKey: event.altKey,
    metaKey: event.metaKey,
    isComposing: event.isComposing,
  }), true);
  window.addEventListener("pointerdown", (event) => record("pointerdown", { clientX: event.clientX, clientY: event.clientY, pointerId: event.pointerId }), true);
  window.addEventListener("pointermove", (event) => {
    if (event.buttons) {
      record("pointermove", { clientX: event.clientX, clientY: event.clientY, pointerId: event.pointerId });
    }
  }, true);
  window.addEventListener("pointerup", (event) => record("pointerup", { clientX: event.clientX, clientY: event.clientY, pointerId: event.pointerId }), true);
  document.addEventListener("DOMContentLoaded", installFinishOverlay, { once: true });
};

function distanceBetweenCenters(a, b) {
  if (!a || !b) {
    return Number.POSITIVE_INFINITY;
  }
  const ax = a.x + a.width / 2;
  const ay = a.y + a.height / 2;
  const bx = b.x + b.width / 2;
  const by = b.y + b.height / 2;
  return Math.hypot(ax - bx, ay - by);
}

function analyzeTrace(trace, snapshot, forceUnsupported) {
  const events = trace.events ?? [];
  const methodEvents = events.filter((event) => event.kind === "editcontext_method");
  const statuses = events.filter((event) => event.kind === "arcweft_status").map((event) => event.detail);
  const renders = events.filter((event) => event.kind === "arcweft_render").map((event) => event.detail);
  const selectionBounds = methodEvents
    .filter((event) => event.detail.method === "updateSelectionBounds")
    .map((event) => event.detail.rect)
    .filter(Boolean);
  const characterBounds = methodEvents
    .filter((event) => event.detail.method === "updateCharacterBounds")
    .flatMap((event) => event.detail.bounds ?? []);
  const caretRects = renders.map((event) => event.caretRect).filter(Boolean);
  const geometryTracksCaret = selectionBounds.some((selectionRect) => (
    caretRects.some((caretRect) => distanceBetweenCenters(selectionRect, caretRect) <= 96)
  ));
  const commandNames = new Set(statuses.filter((status) => status.state === "command").map((status) => status.command));
  const support = snapshot.support;
  const hiddenFallbackCount = snapshot.hiddenFallbackCount;
  const unsupportedSuccess = forceUnsupported && snapshot.sampleState === "unsupported" && hiddenFallbackCount === 0 && snapshot.fallbackInstalled === false;
  const checks = {
    editContextSupported: support.editContextType === "function" && support.hostHasEditContext === true,
    noHiddenFallback: hiddenFallbackCount === 0 && snapshot.fallbackInstalled === false,
    compositionStarted: events.some((event) => event.kind === "compositionstart"),
    textUpdated: events.some((event) => event.kind === "textupdate" && event.detail.text?.utf16Length > 0),
    compositionEnded: events.some((event) => event.kind === "compositionend"),
    selectionBoundsNonOrigin: selectionBounds.some((candidate) => candidate.nonOrigin),
    characterBoundsNonOrigin: characterBounds.some((candidate) => candidate.nonOrigin),
    geometryTracksCaret,
    oneArcweftCaret: snapshot.arcweftCaretCount === 1,
    nativeCaretHidden: snapshot.caretColor === "rgba(0, 0, 0, 0)" || snapshot.caretColor === "transparent",
    keyboardMovement: commandNames.has("move_left") || commandNames.has("move_right"),
    rangedSelection: [...commandNames].some((command) => command?.startsWith("move_")) && statuses.some((status) => status.selectionStart !== status.selectionEnd),
    pointerSelection: statuses.some((status) => ["pointer_down", "pointer_drag", "pointer_up"].includes(status.state)),
    deleteBackspace: commandNames.has("backspace") && commandNames.has("delete"),
    selectAll: commandNames.has("select_all"),
    unsupportedSuccess,
  };
  const required = forceUnsupported
    ? ["unsupportedSuccess", "noHiddenFallback"]
    : [
      "editContextSupported",
      "noHiddenFallback",
      "compositionStarted",
      "textUpdated",
      "compositionEnded",
      "selectionBoundsNonOrigin",
      "characterBoundsNonOrigin",
      "geometryTracksCaret",
      "oneArcweftCaret",
      "nativeCaretHidden",
      "keyboardMovement",
      "rangedSelection",
      "pointerSelection",
      "deleteBackspace",
      "selectAll",
    ];
  const failures = required.filter((name) => !checks[name]);
  return {
    status: failures.length === 0 ? "passed" : support.editContextType === "function" && !forceUnsupported ? "failed" : "blocked",
    checks,
    failures,
    commandNames: [...commandNames].sort(),
    selectionBoundsCount: selectionBounds.length,
    characterBoundsCount: characterBounds.length,
  };
}

async function snapshotPage(page) {
  return await page.evaluate(() => {
    const host = document.getElementById("arcweft-ime-surface");
    const style = host ? getComputedStyle(host) : null;
    const hiddenFallbacks = [...document.querySelectorAll("textarea,input,[contenteditable]")]
      .map((element) => ({
        tagName: element.tagName,
        type: element.getAttribute("type"),
        contenteditable: element.getAttribute("contenteditable"),
        hidden: element.hidden || getComputedStyle(element).display === "none" || getComputedStyle(element).visibility === "hidden",
      }));
    return {
      title: document.title,
      url: location.href,
      sampleState: document.getElementById("ime-sample-status")?.dataset.state ?? null,
      statusText: document.getElementById("ime-sample-status")?.value ?? document.getElementById("ime-sample-status")?.textContent ?? null,
      fallbackInstalled: Boolean(window.__arcweftImeSampleFallbackInstalled),
      glueOwner: window.__arcweftImeSampleGlueOwner,
      arcweftCaretCount: host?.querySelectorAll(".caret")?.length ?? 0,
      caretColor: style?.caretColor ?? null,
      hiddenFallbackCount: hiddenFallbacks.length,
      hiddenFallbacks,
      support: {
        editContextType: typeof window.EditContext,
        hostHasEditContext: host ? "editContext" in host : false,
        prototypeHasEditContext: "editContext" in HTMLElement.prototype,
        userAgent: navigator.userAgent,
        platform: navigator.platform,
      },
    };
  });
}

function traceFileName(kind) {
  const timestamp = new Date().toISOString().replace(/[:.]/g, "-");
  return join(outputDir, `${timestamp}-${kind}.json`);
}

await mkdir(outputDir, { recursive: true });
const { server, baseUrl } = await startServer();
const browser = await chromium.launch(browserLaunchOptions());
const context = await browser.newContext({
  viewport: { width: 1120, height: 760 },
  recordVideo: recordVideo ? { dir: outputDir } : undefined,
});
try {
  const page = await context.newPage();
  const consoleErrors = [];
  page.on("console", (message) => {
    if (message.type() === "error") {
      consoleErrors.push(message.text());
    }
  });
  page.on("pageerror", (error) => consoleErrors.push(error.message));
  await page.addInitScript(recorderInit, { allowText, expectedCommit, forceUnsupported });
  await page.goto(`${baseUrl}/ime-sample.html`, { waitUntil: "domcontentloaded" });
  await page.locator("#arcweft-ime-surface").focus();
  await page.waitForFunction(() => {
    const state = document.getElementById("ime-sample-status")?.dataset.state;
    return state === "ready" || state === "unsupported";
  });
  const initialSnapshot = await snapshotPage(page);
  if (!forceUnsupported && initialSnapshot.sampleState !== "ready") {
    const trace = await page.evaluate(() => window.__arcweftRealImeTrace);
    trace.result = initialSnapshot;
    trace.analysis = analyzeTrace(trace, initialSnapshot, forceUnsupported);
    trace.analysis.status = "blocked";
    trace.analysis.reason = "EditContext-capable browser required for real IME validation";
    const output = traceFileName("blocked");
    await writeFile(output, JSON.stringify(trace, null, 2));
    console.log(JSON.stringify({ test: "editcontext-real-ime-harness", status: "blocked", output }));
    process.exitCode = 2;
  } else if (forceUnsupported) {
    const snapshot = await snapshotPage(page);
    const screenshotPath = join(outputDir, `${new Date().toISOString().replace(/[:.]/g, "-")}-unsupported.png`);
    await page.screenshot({ path: screenshotPath, fullPage: true });
    const trace = await page.evaluate(() => window.__arcweftRealImeTrace);
    trace.result = snapshot;
    trace.artifacts = { screenshots: [screenshotPath] };
    trace.analysis = analyzeTrace(trace, snapshot, forceUnsupported);
    trace.consoleErrors = consoleErrors;
    const output = traceFileName("unsupported");
    await writeFile(output, JSON.stringify(trace, null, 2));
    if (trace.analysis.status !== "passed") {
      throw new Error(`unsupported browser assertion failed: ${trace.analysis.failures.join(", ")}`);
    }
    console.log(JSON.stringify({ test: "editcontext-real-ime-harness", status: "passed", mode: "unsupported", output }));
  } else {
    console.log(`Arcweft real IME recorder is open at ${baseUrl}/ime-sample.html`);
    console.log("Use a real Japanese IME to type and commit 日本語, then exercise ArrowLeft, Shift+ArrowLeft, pointer drag, Backspace, Delete, and Select All. Click Finish trace in the page.");
    await page.waitForFunction(() => window.__arcweftRealImeFinished === true, null, { timeout: 0 });
    const screenshotPath = join(outputDir, `${new Date().toISOString().replace(/[:.]/g, "-")}-real-ime.png`);
    await page.screenshot({ path: screenshotPath, fullPage: true });
    const snapshot = await snapshotPage(page);
    const trace = await page.evaluate(() => window.__arcweftRealImeTrace);
    trace.result = snapshot;
    trace.artifacts = { screenshots: [screenshotPath] };
    trace.analysis = analyzeTrace(trace, snapshot, forceUnsupported);
    trace.consoleErrors = consoleErrors;
    const output = traceFileName("real-ime");
    await writeFile(output, JSON.stringify(trace, null, 2));
    if (trace.analysis.status !== "passed") {
      throw new Error(`real IME validation failed: ${trace.analysis.failures.join(", ")}. Trace written to ${output}`);
    }
    console.log(JSON.stringify({ test: "editcontext-real-ime-harness", status: "passed", mode: "manual", output }));
    if (keepOpen) {
      await page.waitForTimeout(2147483647);
    }
  }
} finally {
  await context.close();
  await browser.close();
  await closeServer(server);
}
