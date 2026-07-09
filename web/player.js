const DEFAULT_WASM_URL = "./pkg/arcweft_player_web.js";
const DEFAULT_WASM_BINARY_URL = "./pkg/arcweft_player_web_bg.wasm";
const DEFAULT_EDIT_CONTEXT_URL = "./player-editcontext.js";
const DEFAULT_FONT_URLS = [
  "./assets/noto-sans-jp-vf.ttf",
  "./assets/noto-emoji-regular.ttf",
  "./assets/arcweft-demo.ttf",
];
let wasmModulePromise = null;
let wasmModuleCacheKey = null;
let editContextModulePromise = null;
let editContextModuleCacheKey = null;
const runtimeTextInputs = new Map();
let runtimeCommandListenerInstalled = false;

export async function loadArcweftWasm(url = DEFAULT_WASM_URL, wasmBinaryUrl) {
  const cacheKey = `${url ?? ""}\0${wasmBinaryUrl ?? ""}`;
  if (
    globalThis.__arcweftWasmModule &&
    globalThis.__arcweftWasmModuleCacheKey === cacheKey
  ) {
    return globalThis.__arcweftWasmModule;
  }
  if (!wasmModulePromise || wasmModuleCacheKey !== cacheKey) {
    wasmModuleCacheKey = cacheKey;
    wasmModulePromise = import(url).then(async (module) => {
      await module.default(wasmBinaryUrl ? { module_or_path: wasmBinaryUrl } : undefined);
      globalThis.__arcweftWasmModule = module;
      globalThis.__arcweftWasmModuleCacheKey = cacheKey;
      return module;
    });
  }
  return wasmModulePromise;
}

export async function setupArcweftWebTextInput(options = {}) {
  markArcweftTextInputOwner();
  const params = new URLSearchParams(window.location.search);
  const editContextUrl = options.editContextUrl ??
    withAssetCachebust(DEFAULT_EDIT_CONTEXT_URL, assetCachebustParam(params));
  const { createArcweftEditContextPlayerGlue } = await loadEditContextGlue(editContextUrl);
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

async function loadEditContextGlue(url = DEFAULT_EDIT_CONTEXT_URL) {
  const cacheKey = String(url ?? DEFAULT_EDIT_CONTEXT_URL);
  if (!editContextModulePromise || editContextModuleCacheKey !== cacheKey) {
    editContextModuleCacheKey = cacheKey;
    editContextModulePromise = import(cacheKey);
  }
  return editContextModulePromise;
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
    const wasmUrls = resolveWasmUrls(options, params);
    const wasm = await loadArcweftWasm(wasmUrls.moduleUrl, wasmUrls.binaryUrl);
    const bundleUrl = withAssetCachebust(
      options.bundleUrl || params.get("bundle") || "./demo.awfb",
      assetCachebustParam(params),
    );
    const fontUrls = resolveFontUrls(options, params);
    const [bundleBytes, fontByteArrays] = await Promise.all([
      fetchBytes(bundleUrl, "Arcweft bundle"),
      Promise.all(
        fontUrls.map((fontUrl, index) => fetchBytes(fontUrl, `Arcweft font ${index + 1}`)),
      ),
    ]);
    const [fontBytes, ...additionalFontBytes] = fontByteArrays;
    window.__arcweftLoadedFontUrls = fontUrls;

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
        frameFit: resolveFrameFitOptions(options, params),
        additionalFontBytes,
      });
    } else {
      wasm.start_arcweft_player(canvasId, bundleBytes, fontBytes);
    }
    canvas?.focus();
  } catch (error) {
    showFatal(error);
  }
}

function resolveWasmUrls(options, params) {
  const cachebust = assetCachebustParam(params);
  const explicitModuleUrl = options.wasmUrl || params.get("wasmUrl") || params.get("wasm");
  const explicitBinaryUrl =
    options.wasmBinaryUrl || params.get("wasmBinaryUrl") || params.get("wasmBinary");
  return {
    moduleUrl: withAssetCachebust(explicitModuleUrl || DEFAULT_WASM_URL, cachebust),
    binaryUrl: explicitBinaryUrl
      ? withAssetCachebust(explicitBinaryUrl, cachebust)
      : explicitModuleUrl
        ? undefined
        : withAssetCachebust(DEFAULT_WASM_BINARY_URL, cachebust),
  };
}

function resolveFontUrls(options, params) {
  const cachebust = assetCachebustParam(params);
  const explicitFontUrls = urlListOption(options.fontUrls);
  if (explicitFontUrls.length > 0) {
    return explicitFontUrls.map((url) => withAssetCachebust(url, cachebust));
  }

  const queryFontUrls = urlListOption(params.get("fontUrls") ?? params.get("fonts"));
  if (queryFontUrls.length > 0) {
    return queryFontUrls.map((url) => withAssetCachebust(url, cachebust));
  }

  const primary = options.fontUrl || params.get("font");
  const additional =
    urlListOption(options.additionalFontUrls).length > 0
      ? urlListOption(options.additionalFontUrls)
      : urlListOption(params.get("additionalFontUrls") ?? params.get("additionalFonts"));
  if (primary) {
    return [primary, ...additional].map((url) => withAssetCachebust(url, cachebust));
  }
  return [...DEFAULT_FONT_URLS, ...additional].map((url) => withAssetCachebust(url, cachebust));
}

function urlListOption(value) {
  if (!value) {
    return [];
  }
  if (Array.isArray(value)) {
    return value.map((item) => String(item ?? "").trim()).filter(Boolean);
  }
  return String(value)
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

function resolveFrameFitOptions(options, params) {
  const explicit = options.frameFit ?? globalThis.__arcweftWebPlayerFrameFit;
  const fit = params.get("fit") ?? explicit?.fit;
  const designWidth = numberOption(params.get("designWidth")) ?? numberOption(explicit?.designWidth);
  const designHeight =
    numberOption(params.get("designHeight")) ?? numberOption(explicit?.designHeight);
  if (!fit && !designWidth && !designHeight) {
    return undefined;
  }
  return {
    fit: fit || "contain",
    designWidth: designWidth || 1280,
    designHeight: designHeight || 720,
  };
}

function assetCachebustParam(params) {
  return params.get("assetCachebust") ?? params.get("cachebust");
}

function withAssetCachebust(url, cachebust) {
  if (!cachebust) {
    return url;
  }
  try {
    const resolved = new URL(url, window.location.href);
    if (resolved.protocol === "data:" || resolved.protocol === "blob:") {
      return url;
    }
    resolved.searchParams.set("cachebust", cachebust);
    return resolved.href;
  } catch {
    return url;
  }
}

function numberOption(value) {
  const number = Number(value);
  return Number.isFinite(number) && number > 0 ? Math.round(number) : undefined;
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
    commandForKeyEvent(_hostId, event) {
      return wasm.arcweft_web_text_input_command_for_key_event?.(
        String(event?.key ?? ""),
        Boolean(event?.ctrlKey),
        Boolean(event?.metaKey),
        Boolean(event?.altKey),
        Boolean(event?.shiftKey),
      );
    },
    createEditContext(_hostId, initialText, secure) {
      return wasm.arcweft_web_text_input_create_edit_context?.(
        String(initialText ?? ""),
        Boolean(secure),
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
    fontUrls: dataListOption(canvas.dataset.arcweftFontUrls),
    additionalFontUrls: dataListOption(canvas.dataset.arcweftAdditionalFontUrls),
    wasmUrl: canvas.dataset.arcweftWasmUrl || undefined,
    wasmBinaryUrl: canvas.dataset.arcweftWasmBinaryUrl || undefined,
    textInput: canvas.dataset.arcweftTextInput === "false" ? false : undefined,
  };
}

function dataListOption(value) {
  const values = urlListOption(value);
  return values.length > 0 ? values : undefined;
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
