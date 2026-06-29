import { createArcweftEditContextPlayerGlue } from "./player-editcontext.js";

const DEFAULT_WASM_URL = "./pkg/arcweft_player_web.js";
let wasmModulePromise = null;
const runtimeTextInputs = new Map();
let runtimeCommandListenerInstalled = false;

export async function loadArcweftWasm(url = DEFAULT_WASM_URL) {
  if (globalThis.__arcweftWasmModule) {
    return globalThis.__arcweftWasmModule;
  }
  if (!wasmModulePromise) {
    wasmModulePromise = import(url).then(async (module) => {
      await module.default();
      return module;
    });
  }
  return wasmModulePromise;
}

export async function setupArcweftWebTextInput(options = {}) {
  markArcweftTextInputOwner();
  const hostId = options.hostId || "arcweft-canvas";
  const host = options.host || document.getElementById(hostId);
  if (!host) {
    throw new Error(`Arcweft text input host not found: ${hostId}`);
  }
  const wasm = options.wasm ?? (options.wasmUrl ? await maybeLoadWasm(options.wasmUrl) : null);
  const delegate = options.delegate ?? wasmDelegate(wasm);
  const glue = createArcweftEditContextPlayerGlue(host, {
    hostId,
    secure: Boolean(options.secure),
    initialText: options.initialText ?? "",
    mirror: options.mirror,
    statusTarget: options.statusTarget ?? document,
    delegate,
  });
  const installed = await glue.install();
  markArcweftTextInputOwner();
  runtimeTextInputs.set(hostId, installed);
  globalThis.__arcweftWebTextInputs = runtimeTextInputs;
  installRuntimeCommandListener(options.statusTarget ?? document);
  globalThis.__arcweftWebTextInput = installed;
  return installed;
}

export async function startArcweftWebPlayer(options = {}) {
  const canvasId = options.canvasId || "arcweft-canvas";
  const canvas = document.getElementById(canvasId);
  const loading = document.getElementById("arcweft-loading");
  const fatal = document.getElementById("arcweft-fatal");
  const params = new URLSearchParams(window.location.search);

  const showFatal = (error) => {
    const message = error instanceof Error ? error.message : String(error);
    if (loading) {
      loading.hidden = true;
    }
    if (fatal) {
      fatal.hidden = false;
      fatal.textContent = `Arcweft player could not start.\n${message}`;
    }
    window.__arcweftFatal = { kind: "unsupported_or_fatal", message };
    document.dispatchEvent(
      new CustomEvent("arcweft-player-fatal", {
        detail: message,
      }),
    );
  };

  try {
    if (!globalThis.isSecureContext) {
      throw new Error("WebGPU requires a secure context (HTTPS or localhost)");
    }
    if (!navigator.gpu) {
      throw new Error("WebGPU is unsupported: navigator.gpu is unavailable");
    }
    const wasm = await loadArcweftWasm(options.wasmUrl);
    const bundleUrl = options.bundleUrl || params.get("bundle") || "./demo.awfb";
    const fontUrl = options.fontUrl || params.get("font") || "./assets/arcweft-demo.ttf";
    const [bundleBytes, fontBytes] = await Promise.all([
      fetchBytes(bundleUrl, "Arcweft bundle"),
      fetchBytes(fontUrl, "Arcweft font"),
    ]);

    if (options.textInput !== false) {
      await setupArcweftWebTextInput({
        hostId: canvasId,
        host: canvas,
        wasm,
        secure: false,
        statusTarget: document,
      });
    }

    if (typeof wasm.start_arcweft_player_with_options === "function") {
      wasm.start_arcweft_player_with_options(canvasId, bundleBytes, fontBytes, {
        textInput: options.textInput ?? true,
      });
    } else {
      wasm.start_arcweft_player(canvasId, bundleBytes, fontBytes);
    }
    canvas?.focus();
  } catch (error) {
    showFatal(error);
  }
}

function wasmDelegate(wasm) {
  if (!wasm) {
    return null;
  }
  return {
    dispatchTextUpdate(hostId, payload) {
      return wasm.arcweft_web_text_input_runtime_dispatch_text_update?.(hostId, payload);
    },
    compositionStart(hostId) {
      return wasm.arcweft_web_text_input_runtime_composition_start?.(hostId);
    },
    compositionEnd(hostId, cancelled) {
      return wasm.arcweft_web_text_input_runtime_composition_end?.(hostId, Boolean(cancelled));
    },
    dispatchCommand(hostId, command, selecting) {
      return wasm.arcweft_web_text_input_runtime_dispatch_command?.(
        hostId,
        command,
        Boolean(selecting),
      );
    },
  };
}

function installRuntimeCommandListener(target = document) {
  if (runtimeCommandListenerInstalled) {
    return;
  }
  runtimeCommandListenerInstalled = true;
  target.addEventListener("arcweft-text-input-runtime-command", (event) => {
    const detail = parseRuntimeCommandDetail(event.detail);
    const glue = runtimeTextInputs.get(detail.hostId) ?? globalThis.__arcweftWebTextInput;
    if (!glue) {
      return;
    }
    for (const command of detail.commands ?? []) {
      glue.applyRuntimeCommand(command);
    }
  });
}

function parseRuntimeCommandDetail(detail) {
  if (typeof detail === "string") {
    try {
      return JSON.parse(detail);
    } catch {
      return { hostId: "arcweft-canvas", commands: [] };
    }
  }
  return detail ?? { hostId: "arcweft-canvas", commands: [] };
}

function markArcweftTextInputOwner() {
  globalThis.__arcweftImeSampleGlueOwner = "arcweft-player";
  globalThis.__arcweftImeSampleFallbackInstalled = false;
}

async function maybeLoadWasm(url) {
  try {
    return await loadArcweftWasm(url);
  } catch {
    return null;
  }
}

async function fetchBytes(url, label) {
  const response = await fetch(url, { cache: "no-store" });
  if (!response.ok) {
    throw new Error(`${label} fetch failed (${response.status} ${response.statusText})`);
  }
  return new Uint8Array(await response.arrayBuffer());
}

document.addEventListener("arcweft-player-ready", () => {
  const loading = document.getElementById("arcweft-loading");
  const canvas = document.getElementById("arcweft-canvas");
  if (loading) {
    loading.hidden = true;
  }
  if (canvas) {
    canvas.dataset.arcweftReady = "true";
  }
});

document.addEventListener("arcweft-runtime-observation", (event) => {
  try {
    window.__arcweftLastObservation = JSON.parse(event.detail);
  } catch (error) {
    console.error("Arcweft observation JSON was invalid", error);
  }
});

document.addEventListener("arcweft-frame-observation", (event) => {
  try {
    window.__arcweftLastFrameObservation = JSON.parse(event.detail);
  } catch (error) {
    console.error("Arcweft frame observation JSON was invalid", error);
  }
});

function autostartOptionsFromCanvas(canvas) {
  return {
    canvasId: canvas.id || "arcweft-canvas",
    bundleUrl: canvas.dataset.arcweftBundleUrl || undefined,
    fontUrl: canvas.dataset.arcweftFontUrl || undefined,
    textInput: canvas.dataset.arcweftTextInput === "false" ? false : undefined,
  };
}

function shouldAutostartArcweftWebPlayer(canvas) {
  return Boolean(canvas) && globalThis.__arcweftWebPlayerAutostart !== false;
}

const autostartCanvas = document.getElementById("arcweft-canvas");
if (shouldAutostartArcweftWebPlayer(autostartCanvas)) {
  startArcweftWebPlayer({
    ...autostartOptionsFromCanvas(autostartCanvas),
    ...(globalThis.__arcweftWebPlayerAutostartOptions ?? {}),
  });
}
