import { readFile } from "node:fs/promises";
import { join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const root = normalize(join(fileURLToPath(new URL(".", import.meta.url)), ".."));
const html = await readFile(join(root, "ime-sample.html"), "utf8");
const css = await readFile(join(root, "ime-sample.css"), "utf8");
const sampleJs = await readFile(join(root, "ime-sample.js"), "utf8");
const playerJs = await readFile(join(root, "player.js"), "utf8");
const editContextJs = await readFile(join(root, "player-editcontext.js"), "utf8");

const hits = [];

function forbid(label, text, patterns) {
  for (const pattern of patterns) {
    if (text.includes(pattern)) {
      hits.push(`${label} contains forbidden active-sample pattern ${JSON.stringify(pattern)}`);
    }
  }
}

forbid("ime-sample.html", html, [
  'role="textbox"',
  "arcweft-ime-surface",
  "arcweft-ime-text",
  "arcweft-ime-composition",
  "committed-text",
  "composition-text",
  'class="caret"',
  "ime-sample-status",
  "ime-sample-selection",
  "ime-sample-fonts",
  "<input",
  "<textarea",
  "contenteditable",
]);

forbid("ime-sample.css", css, [
  ".ime-surface",
  ".committed-text",
  ".composition-text",
  ".caret",
  "--arcweft-caret",
  ".sample-grid",
  ".status-line",
  ".metric",
  "caret-color",
]);

forbid("ime-sample.js", sampleJs, [
  "setupArcweftWebTextInput",
  "new window.EditContext",
  "new EditContext",
  "committedTextId",
  "compositionTextId",
  "innerHTML",
  "textContent =",
  'querySelector("#ime-sample-status',
  "querySelector('#ime-sample-status",
]);

forbid("player.js", playerJs, [
  'document.createElement("textarea")',
  "document.createElement('textarea')",
  "contenteditable",
  "installKeyboardFallback",
]);

forbid("player-editcontext.js", editContextJs, [
  'document.createElement("textarea")',
  "document.createElement('textarea')",
  'document.createElement("input")',
  "document.createElement('input')",
  "contenteditable",
  "installKeyboardFallback",
]);

if (!html.includes('id="arcweft-canvas"')) {
  hits.push("ime-sample.html must host the normal Arcweft canvas");
}
if (!sampleJs.includes("__arcweftWebPlayerAutostartOptions")) {
  hits.push("ime-sample.js must configure the normal player autostart options");
}
if (!sampleJs.includes('await import("./player.js")')) {
  hits.push("ime-sample.js must enter through web/player.js");
}
if (!playerJs.includes("__arcweftWebPlayerAutostartOptions")) {
  hits.push("player.js must consume thin-host autostart options");
}

if (hits.length > 0) {
  throw new Error(hits.join("\n"));
}

console.log(JSON.stringify({ gate: "ime-sample-player-rendered-source", status: "passed" }));
