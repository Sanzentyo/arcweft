import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const html = await readFile(join(root, "ime-sample.html"), "utf8");
const css = await readFile(join(root, "ime-sample.css"), "utf8");
const js = await readFile(join(root, "ime-sample.js"), "utf8");

for (const property of [
  "filter:",
  "backdrop-filter:",
  "clip-path:",
  "mix-blend-mode:",
  "mask:",
]) {
  if (css.includes(property)) {
    throw new Error(`IME sample CSS must stay direct-wgpu-compatible; found ${property}`);
  }
}

if (!js.includes("window.EditContext") || !js.includes("surface.editContext")) {
  throw new Error("IME sample must exercise the Web EditContext API");
}

if ((css.match(/font-family:/g) ?? []).length < 3) {
  throw new Error("IME sample must declare multiple font stacks");
}

if (!html.includes("role=\"textbox\"")) {
  throw new Error("IME sample must expose a textbox role");
}

console.log(JSON.stringify({ sample: "web-ime", status: "ok" }));
