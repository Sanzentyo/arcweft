import init, { start_arcweft_player } from "./pkg/arcweft_player_web.js";

const canvas = document.getElementById("arcweft-canvas");
const loading = document.getElementById("arcweft-loading");
const fatal = document.getElementById("arcweft-fatal");

function showFatal(error) {
  const message = error instanceof Error ? error.message : String(error);
  loading.hidden = true;
  fatal.hidden = false;
  fatal.textContent = `Arcweft player could not start.\n${message}`;
  window.__arcweftFatal = { kind: "unsupported_or_fatal", message };
}

async function fetchBytes(url, label) {
  const response = await fetch(url, { cache: "no-store" });
  if (!response.ok) {
    throw new Error(`${label} fetch failed (${response.status} ${response.statusText})`);
  }
  return new Uint8Array(await response.arrayBuffer());
}

async function boot() {
  if (!globalThis.isSecureContext) {
    throw new Error("WebGPU requires a secure context (HTTPS or localhost)");
  }
  if (!navigator.gpu) {
    throw new Error("WebGPU is unsupported: navigator.gpu is unavailable");
  }

  await init();
  const [bundleBytes, fontBytes] = await Promise.all([
    fetchBytes("./demo.awfb", "Arcweft bundle"),
    fetchBytes("./assets/arcweft-demo.ttf", "Arcweft font"),
  ]);
  start_arcweft_player("arcweft-canvas", bundleBytes, fontBytes);
  canvas.focus();
}

document.addEventListener("arcweft-player-ready", () => {
  loading.hidden = true;
  canvas.dataset.arcweftReady = "true";
});

document.addEventListener("arcweft-player-fatal", (event) => {
  showFatal(event.detail || "unknown WebGPU player failure");
});

document.addEventListener("arcweft-runtime-observation", (event) => {
  try {
    window.__arcweftLastObservation = JSON.parse(event.detail);
  } catch (error) {
    console.error("Arcweft observation JSON was invalid", error);
  }
});

boot().catch(showFatal);
