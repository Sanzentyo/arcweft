// Browser-real harness skeleton. Run after building the fixture bundle and wasm
// package in a checkout with an EditContext-capable browser.
import { chromium } from "playwright";

const url = process.env.ARCWEFT_WEB_PLAYER_URL ??
  "http://127.0.0.1:8080/web/index.html?bundle=../fixtures/web-editcontext-runtime-bridge/web-editcontext-runtime-bridge.awfb";

const browser = await chromium.launch({ headless: false });
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
const evidence = [];
for (const eventName of [
  "arcweft-text-input-status",
  "arcweft-text-input-render",
  "arcweft-text-input-runtime-command",
]) {
  await page.exposeFunction(`record_${eventName.replaceAll("-", "_")}`, (detail) => {
    evidence.push({ eventName, detail, at: Date.now() });
  });
  await page.addInitScript((name) => {
    document.addEventListener(name, (event) => {
      globalThis[`record_${name.replaceAll("-", "_")}`](event.detail);
    });
  }, eventName);
}

await page.goto(url);
await page.waitForFunction(() => document.getElementById("arcweft-canvas")?.dataset.arcweftReady === "true");
await page.locator("#arcweft-canvas").click({ position: { x: 440, y: 210 } });

console.log(JSON.stringify({
  url,
  unsupported: evidence.some((entry) => String(entry.detail).includes("unsupported_no_fallback")),
  eventsCaptured: evidence.length,
  evidence,
}, null, 2));

await browser.close();
