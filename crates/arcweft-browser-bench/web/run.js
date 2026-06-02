import init, {
  run_arcweft_browser_math_bench,
} from "./arcweft_browser_webgpu.js";

const DEFAULT_CONFIG = {
  warmup_iters: 1,
  sample_iters: 3,
  seed: 2900693030,
  async_batch_depth: 4,
  add_lengths: [0, 1, 255, 256, 257, 4096],
  matmul_shapes: [
    { rows: 1, shared: 1, cols: 1 },
    { rows: 2, shared: 3, cols: 4 },
    { rows: 17, shared: 19, cols: 23 },
    { rows: 64, shared: 64, cols: 64 },
  ],
  modes: [
    "cpu_wasm",
    "web_gpu_one_shot",
    "web_gpu_prepared_upload",
    "web_gpu_prepared_resident",
    "web_gpu_prepared_resident_async",
    "web_gpu_prepared_resident_pipelined",
  ],
};

const PERF_CONFIG = {
  warmup_iters: 2,
  sample_iters: 5,
  seed: 2900693030,
  async_batch_depth: 4,
  add_lengths: [65536, 262144, 1048576, 4194304],
  matmul_shapes: [
    { rows: 64, shared: 64, cols: 64 },
    { rows: 128, shared: 128, cols: 128 },
    { rows: 256, shared: 256, cols: 256 },
  ],
  modes: [
    "cpu_wasm",
    "web_gpu_prepared_upload",
    "web_gpu_prepared_resident",
    "web_gpu_prepared_resident_async",
    "web_gpu_prepared_resident_pipelined",
  ],
};

const statusElement = document.querySelector("#status");
const casesElement = document.querySelector("#cases");
const jsonElement = document.querySelector("#json");

try {
  await init();
  const report = await run_arcweft_browser_math_bench(
    JSON.stringify(readConfig()),
  );
  globalThis.__arcweftBrowserBenchReport = report;
  render(report);
} catch (error) {
  statusElement.textContent = `Benchmark failed: ${String(error)}`;
  statusElement.classList.add("error");
  throw error;
}

function readConfig() {
  const params = new URLSearchParams(globalThis.location.search);
  const encoded = params.get("config");
  if (!encoded) {
    return params.get("preset") === "perf" ? PERF_CONFIG : DEFAULT_CONFIG;
  }
  return JSON.parse(encoded);
}

function render(report) {
  const webgpu = report.run.webgpu.available
    ? "WebGPU available"
    : `WebGPU skipped: ${report.run.webgpu.fallback_reason ?? "unknown"}`;
  statusElement.textContent = `${webgpu}; ${report.cases.length} case(s) recorded`;
  casesElement.replaceChildren(
    ...report.cases.map((entry) => {
      const row = document.createElement("tr");
      row.append(
        cell(entry.case_id),
        cell(entry.mode),
        cell(formatNumber(entry.median_ms)),
        cell(entry.dispatches),
        cell(entry.bytes_uploaded),
        cell(entry.bytes_readback),
        cell(entry.correctness.passed ? "yes" : "no"),
        cell(entry.fallback_reason ?? ""),
      );
      return row;
    }),
  );
  jsonElement.textContent = JSON.stringify(report, null, 2);
}

function cell(value) {
  const element = document.createElement("td");
  element.textContent = String(value);
  return element;
}

function formatNumber(value) {
  return typeof value === "number" ? value.toFixed(4) : "";
}
