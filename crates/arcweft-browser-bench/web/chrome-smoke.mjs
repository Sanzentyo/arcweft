import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { get } from "node:http";
import { join, resolve } from "node:path";

const port = readPort();
const preset = readOption("--preset");
const timeoutMs = readPositiveInteger("--timeout-ms", 60_000);
const benchUrl = buildBenchUrl(port, preset);
const cdpPort = port + 1000;
const server = spawn(hostToolPath(), ["serve", "--port", String(port)], {
  stdio: ["ignore", "pipe", "pipe"],
});
const chrome = spawn(chromePath(), chromeArgs(cdpPort), {
  stdio: ["ignore", "pipe", "pipe"],
});

try {
  await waitForHttp(benchUrl, 20_000);
  await waitForHttp(`http://127.0.0.1:${cdpPort}/json/version`, 20_000);
  const target = await requestJson(
    `http://127.0.0.1:${cdpPort}/json/new?${encodeURIComponent("about:blank")}`,
    "PUT",
  );
  const report = await readBenchReport(target.webSocketDebuggerUrl, benchUrl);
  const summary = summarize(report);
  console.log(JSON.stringify(summary, null, 2));
  process.exitCode = summary.failed_cases === 0 ? 0 : 1;
} finally {
  chrome.kill();
  server.kill();
}

function readPort() {
  return readPositiveInteger("--port", 8787);
}

function readPositiveInteger(name, fallback) {
  const value = readOption(name);
  if (value === null) {
    return fallback;
  }
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`${name} requires a positive integer`);
  }
  return parsed;
}

function readOption(name) {
  const index = process.argv.indexOf(name);
  return index === -1 ? null : process.argv[index + 1];
}

function buildBenchUrl(port, preset) {
  const url = new URL(`http://127.0.0.1:${port}/`);
  if (preset) {
    url.searchParams.set("preset", preset);
  }
  return url.toString();
}

function hostToolPath() {
  const exe = process.platform === "win32" ? "browser_bench_host.exe" : "browser_bench_host";
  return join("target", "debug", exe);
}

function chromePath() {
  for (const value of [process.env.CHROME, process.env.CHROME_BIN]) {
    if (value && existsSync(value)) {
      return value;
    }
  }
  if (process.platform === "win32") {
    for (const root of [process.env.ProgramFiles, process.env["ProgramFiles(x86)"]]) {
      if (!root) {
        continue;
      }
      for (const relative of [
        ["Google", "Chrome", "Application", "chrome.exe"],
        ["Microsoft", "Edge", "Application", "msedge.exe"],
      ]) {
        const candidate = join(root, ...relative);
        if (existsSync(candidate)) {
          return candidate;
        }
      }
    }
  }
  return process.platform === "darwin" ? "google-chrome" : "google-chrome";
}

function chromeArgs(port) {
  return [
    "--headless=new",
    "--remote-debugging-address=127.0.0.1",
    `--remote-debugging-port=${port}`,
    "--remote-allow-origins=*",
    "--no-first-run",
    "--no-default-browser-check",
    "--enable-unsafe-webgpu",
    "--disable-background-networking",
    "--disable-sync",
    "--disable-extensions",
    `--user-data-dir=${resolve("target/chrome-webgpu-bench")}`,
    "about:blank",
  ];
}

async function readBenchReport(webSocketUrl, benchUrl) {
  const socket = new WebSocket(webSocketUrl);
  let nextId = 1;
  const pending = new Map();
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    if (message.id && pending.has(message.id)) {
      const { resolve, reject } = pending.get(message.id);
      pending.delete(message.id);
      if (message.error) {
        reject(new Error(message.error.message));
      } else {
        resolve(message.result);
      }
    }
  });
  await new Promise((resolve, reject) => {
    socket.addEventListener("open", resolve, { once: true });
    socket.addEventListener("error", reject, { once: true });
  });
  const send = (method, params = {}) =>
    new Promise((resolve, reject) => {
      const id = nextId++;
      pending.set(id, { resolve, reject });
      socket.send(JSON.stringify({ id, method, params }));
    });
  await waitForCommand(send, "Page.enable");
  await waitForCommand(send, "Runtime.enable");
  await waitForCommand(send, "Page.navigate", { url: benchUrl });
  await waitForRuntimeContext(send);
  const result = await pollBenchReport(send);
  socket.close();
  return result;
}

async function waitForCommand(send, method, params = {}) {
  const started = Date.now();
  for (;;) {
    try {
      await send(method, params);
      return;
    } catch (error) {
      if (Date.now() - started > 20_000) {
        throw error;
      }
      await delay(100);
    }
  }
}

async function waitForRuntimeContext(send) {
  const started = Date.now();
  for (;;) {
    try {
      await send("Runtime.evaluate", {
        expression: "1",
        returnByValue: true,
      });
      return;
    } catch (error) {
      if (Date.now() - started > 20_000) {
        throw error;
      }
      await delay(100);
    }
  }
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function pollBenchReport(send) {
  const started = Date.now();
  for (;;) {
    try {
      const result = await send("Runtime.evaluate", {
        expression:
          "globalThis.__arcweftBrowserBenchReport ? JSON.stringify(globalThis.__arcweftBrowserBenchReport) : null",
        returnByValue: true,
      });
      const value = result.result.value;
      if (typeof value === "string") {
        return JSON.parse(value);
      }
    } catch (error) {
      if (Date.now() - started > timeoutMs) {
        throw error;
      }
    }
    if (Date.now() - started > timeoutMs) {
      throw new Error("browser bench timed out");
    }
    await delay(250);
  }
}

function summarize(report) {
  const failedCases = report.cases.filter(
    (entry) => entry.fallback_reason === null && !entry.correctness.passed,
  );
  const speedups = summarizeSpeedups(report);
  return {
    schema_version: report.schema_version,
    webgpu_available: report.run.webgpu.available,
    webgpu_fallback_reason: report.run.webgpu.fallback_reason,
    cases: report.cases.length,
    measured_cases: report.cases.filter((entry) => entry.median_ms !== null).length,
    skipped_cases: report.cases.filter((entry) => entry.fallback_reason !== null).length,
    failed_cases: failedCases.length,
    recommendations: report.recommendations?.length ?? 0,
    stability: summarizeStability(report).slice(0, 12),
    best_speedups: speedups.slice(0, 12),
    selected_modes: (report.recommendations ?? []).slice(0, 8).map((entry) => ({
      op: entry.op,
      shape: entry.shape,
      selected_mode: entry.selected_mode,
      speedup: entry.speedup,
      reason: entry.reason,
      capacity: entry.selected_capacity,
    })),
    representative: report.cases
      .filter((entry) => entry.median_ms !== null)
      .slice(0, 8)
      .map((entry) => ({
        case_id: entry.case_id,
        mode: entry.mode,
        median_ms: entry.median_ms,
        effective_gflops: entry.effective_gflops,
        submit_ms: entry.submit_median_ms,
        readback_ms: entry.readback_median_ms,
        submit_share: entry.submit_median_share,
        readback_share: entry.readback_median_share,
        correct: entry.correctness.passed,
      })),
  };
}

function summarizeStability(report) {
  return (report.stability ?? [])
    .filter((entry) => entry.measured_rounds > 1)
    .map((entry) => ({
      op: entry.op,
      shape: entry.shape,
      mode: entry.mode,
      rounds: entry.measured_rounds,
      median_ms: entry.median_of_medians_ms,
      min_ms: entry.min_median_ms,
      max_ms: entry.max_median_ms,
      mad_ms: entry.median_mad_ms,
      spread_ratio: entry.spread_ratio,
    }))
    .sort((lhs, rhs) => (rhs.spread_ratio ?? 0) - (lhs.spread_ratio ?? 0));
}

function summarizeSpeedups(report) {
  const stabilityRows = summarizeStabilitySpeedups(report);
  if (stabilityRows.length > 0) {
    return stabilityRows;
  }
  return summarizeCaseSpeedups(report);
}

function summarizeStabilitySpeedups(report) {
  const groups = new Map();
  for (const entry of report.stability ?? []) {
    if (
      entry.measured_rounds <= 1 ||
      typeof entry.median_of_medians_ms !== "number" ||
      entry.median_of_medians_ms === 0
    ) {
      continue;
    }
    const key = shapeKey(entry.op, entry.shape);
    if (!groups.has(key)) {
      groups.set(key, {});
    }
    groups.get(key)[entry.mode] = entry;
  }
  const rows = [];
  for (const [key, modes] of groups) {
    const cpu = modes.cpu_wasm;
    if (!cpu || typeof cpu.median_of_medians_ms !== "number") {
      continue;
    }
    for (const entry of Object.values(modes)) {
      if (
        entry.mode === "cpu_wasm" ||
        typeof entry.median_of_medians_ms !== "number" ||
        entry.median_of_medians_ms === 0
      ) {
        continue;
      }
      const representative = representativeCase(
        report,
        entry.op,
        entry.shape,
        entry.mode,
      );
      rows.push({
        case: key,
        mode: entry.mode,
        cpu_ms: cpu.median_of_medians_ms,
        gpu_ms: entry.median_of_medians_ms,
        speedup: cpu.median_of_medians_ms / entry.median_of_medians_ms,
        effective_gflops:
          representative?.estimated_flops && entry.median_of_medians_ms > 0
            ? representative.estimated_flops / (entry.median_of_medians_ms * 1_000_000)
            : null,
        submit_ms: representative?.submit_median_ms ?? null,
        readback_ms: representative?.readback_median_ms ?? null,
        submit_share: representative?.submit_median_share ?? null,
        readback_share: representative?.readback_median_share ?? null,
        workgroups: representative?.workgroups ?? null,
        estimated_flops: representative?.estimated_flops ?? null,
        rounds: entry.measured_rounds,
        spread_ratio: entry.spread_ratio,
      });
    }
  }
  return rows.sort((lhs, rhs) => rhs.speedup - lhs.speedup);
}

function summarizeCaseSpeedups(report) {
  const groups = new Map();
  for (const entry of report.cases) {
    const key = shapeKey(entry.op, entry.shape);
    if (!groups.has(key)) {
      groups.set(key, {});
    }
    groups.get(key)[entry.mode] = entry;
  }
  const rows = [];
  for (const [key, modes] of groups) {
    const cpu = modes.cpu_wasm;
    if (!cpu || typeof cpu.median_ms !== "number" || cpu.median_ms === 0) {
      continue;
    }
    for (const entry of Object.values(modes)) {
      if (
        entry.mode === "cpu_wasm" ||
        typeof entry.median_ms !== "number" ||
        entry.median_ms === 0
      ) {
        continue;
      }
      rows.push({
        case: key,
        mode: entry.mode,
        cpu_ms: cpu.median_ms,
        gpu_ms: entry.median_ms,
        speedup: cpu.median_ms / entry.median_ms,
        effective_gflops: entry.effective_gflops,
        submit_ms: entry.submit_median_ms,
        readback_ms: entry.readback_median_ms,
        submit_share: entry.submit_median_share,
        readback_share: entry.readback_median_share,
        workgroups: entry.workgroups,
        estimated_flops: entry.estimated_flops,
      });
    }
  }
  return rows.sort((lhs, rhs) => rhs.speedup - lhs.speedup);
}

function representativeCase(report, op, shape, mode) {
  return report.cases.find(
    (entry) =>
      entry.op === op &&
      entry.mode === mode &&
      JSON.stringify(entry.shape) === JSON.stringify(shape),
  );
}

function shapeKey(op, shape) {
  if (shape.len !== undefined) {
    return `${op}_len${shape.len.len}`;
  }
  const matmul = shape.matmul;
  return `${op}_m${matmul.rows}_k${matmul.shared}_n${matmul.cols}`;
}

function waitForHttp(url, timeoutMs) {
  const started = Date.now();
  return new Promise((resolve, reject) => {
    const tick = () => {
      request(url)
        .then(resolve)
        .catch((error) => {
          if (Date.now() - started > timeoutMs) {
            reject(error);
          } else {
            setTimeout(tick, 250);
          }
        });
    };
    tick();
  });
}

function requestJson(url, method = "GET") {
  return request(url, method).then((body) => JSON.parse(body));
}

function request(url, method = "GET") {
  return new Promise((resolve, reject) => {
    const request = get(url, { method }, (response) => {
      const chunks = [];
      response.on("data", (chunk) => chunks.push(chunk));
      response.on("end", () => {
        const body = Buffer.concat(chunks).toString("utf8");
        if (response.statusCode >= 200 && response.statusCode < 300) {
          resolve(body);
        } else {
          reject(new Error(`HTTP ${response.statusCode}: ${body}`));
        }
      });
    });
    request.on("error", reject);
  });
}
