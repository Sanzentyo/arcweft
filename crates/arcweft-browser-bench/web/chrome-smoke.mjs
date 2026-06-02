import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { get } from "node:http";
import { join, resolve } from "node:path";

const port = readPort();
const benchUrl = `http://127.0.0.1:${port}/`;
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
  const index = process.argv.indexOf("--port");
  if (index === -1) {
    return 8787;
  }
  const value = Number.parseInt(process.argv[index + 1], 10);
  if (!Number.isInteger(value) || value <= 0) {
    throw new Error("--port requires a positive integer");
  }
  return value;
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
      if (Date.now() - started > 60_000) {
        throw error;
      }
    }
    if (Date.now() - started > 60_000) {
      throw new Error("browser bench timed out");
    }
    await delay(250);
  }
}

function summarize(report) {
  const failedCases = report.cases.filter(
    (entry) => entry.fallback_reason === null && !entry.correctness.passed,
  );
  return {
    schema_version: report.schema_version,
    webgpu_available: report.run.webgpu.available,
    webgpu_fallback_reason: report.run.webgpu.fallback_reason,
    cases: report.cases.length,
    measured_cases: report.cases.filter((entry) => entry.median_ms !== null).length,
    skipped_cases: report.cases.filter((entry) => entry.fallback_reason !== null).length,
    failed_cases: failedCases.length,
    representative: report.cases
      .filter((entry) => entry.median_ms !== null)
      .slice(0, 8)
      .map((entry) => ({
        case_id: entry.case_id,
        mode: entry.mode,
        median_ms: entry.median_ms,
        correct: entry.correctness.passed,
      })),
  };
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
