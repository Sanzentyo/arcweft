const DEFAULT_BUNDLE_URL = "./ime-player-rendered.awfb";
const DEFAULT_FONT_URL = "./assets/arcweft-demo.ttf";
const params = new URLSearchParams(window.location.search);
const bundleUrl = params.get("bundle") || DEFAULT_BUNDLE_URL;
const fontUrl = params.get("font") || DEFAULT_FONT_URL;

const diagnostic = {
  sample: "web-ime-player-rendered",
  boundary: "player-rendered",
  visibleDomTextUi: false,
  hostId: "arcweft-canvas",
  bundleUrl,
  fontUrl,
  equivalentUrl: `index.html?bundle=${bundleUrl}`,
  events: [],
  runtimeCommands: [],
  frameObservations: [],
  unsupportedNoFallback: false,
};

globalThis.__arcweftImeSample = diagnostic;
globalThis.__arcweftWebPlayerAutostartOptions = {
  canvasId: "arcweft-canvas",
  bundleUrl,
  fontUrl,
  textInput: true,
};

function parseDetail(detail) {
  if (typeof detail !== "string") {
    return detail ?? null;
  }
  try {
    return JSON.parse(detail);
  } catch {
    return detail;
  }
}

function pushLimited(array, value, limit = 80) {
  array.push(value);
  if (array.length > limit) {
    array.splice(0, array.length - limit);
  }
}

function record(name, detail) {
  const parsed = parseDetail(detail);
  pushLimited(diagnostic.events, { name, detail: parsed });
  return parsed;
}

document.addEventListener("arcweft-text-input-status", (event) => {
  const detail = record("arcweft-text-input-status", event.detail);
  if (detail?.state === "unsupported_no_fallback") {
    diagnostic.unsupportedNoFallback = true;
  }
});

document.addEventListener("arcweft-text-input-runtime-command", (event) => {
  const detail = record("arcweft-text-input-runtime-command", event.detail);
  pushLimited(diagnostic.runtimeCommands, detail);
});

document.addEventListener("arcweft-frame-observation", (event) => {
  const detail = record("arcweft-frame-observation", event.detail);
  pushLimited(diagnostic.frameObservations, detail, 16);
});

document.addEventListener("arcweft-runtime-observation", (event) => {
  record("arcweft-runtime-observation", event.detail);
});

document.addEventListener("arcweft-player-fatal", (event) => {
  diagnostic.fatal = String(event.detail ?? "unknown fatal");
});

await import("./player.js");
