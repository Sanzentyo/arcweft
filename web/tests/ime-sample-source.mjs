import { readFile } from "node:fs/promises";
import { join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const root = normalize(join(fileURLToPath(new URL(".", import.meta.url)), ".."));
const html = await readFile(join(root, "ime-sample.html"), "utf8");
const css = await readFile(join(root, "ime-sample.css"), "utf8");

const productionFiles = [
  "player.js",
  "player-editcontext.js",
  "ime-sample.js",
];
const hiddenFallbackPatterns = [
  "textarea",
  "contenteditable",
  "HtmlTextAreaElement",
  "HtmlInputElement",
  "installKeyboardFallback",
  "beforeinput",
];
const sampleOwnershipPatterns = [
  "new window.EditContext",
  "new EditContext",
  "addEventListener(\"textupdate\"",
  "addEventListener('textupdate'",
  "addEventListener(\"compositionend\"",
  "addEventListener('compositionend'",
  "let modelText",
  "modelText =",
  "applyUpdate(",
];

const hits = [];
for (const property of [
  "filter:",
  "backdrop-filter:",
  "clip-path:",
  "mix-blend-mode:",
  "mask:",
]) {
  if (css.includes(property)) {
    hits.push(`ime-sample.css must stay direct-wgpu-compatible; found ${property}`);
  }
}
if (!html.includes("role=\"textbox\"")) {
  hits.push("ime-sample.html must expose a textbox role");
}
for (const fontToken of [
  "--font-ui",
  "--font-editor",
  "Arcweft Demo",
  "Yu Gothic",
  "Noto Sans JP",
]) {
  if (!css.includes(fontToken)) {
    hits.push(`ime-sample.css must keep the multi-font stack token ${JSON.stringify(fontToken)}`);
  }
}
for (const file of productionFiles) {
  const text = await readFile(join(root, file), "utf8");
  for (const pattern of hiddenFallbackPatterns) {
    if (text.includes(pattern)) {
      hits.push(`${file} contains hidden-fallback pattern ${JSON.stringify(pattern)}`);
    }
  }
}
const sample = await readFile(join(root, "ime-sample.js"), "utf8");
for (const pattern of sampleOwnershipPatterns) {
  if (sample.includes(pattern)) {
    hits.push(`ime-sample.js still owns browser IME glue via ${JSON.stringify(pattern)}`);
  }
}

if (hits.length > 0) {
  throw new Error(hits.join("\n"));
}

console.log(JSON.stringify({ gate: "ime-sample-source", status: "passed" }));
