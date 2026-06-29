import { readFile } from "node:fs/promises";
import { join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const root = normalize(join(fileURLToPath(new URL(".", import.meta.url)), "..", ".."));
const fixture = JSON.parse(await readFile(
  join(root, "fixtures", "web-ime-player-rendered", "ui-runtime-text-controls.json"),
  "utf8",
));

const hits = [];
const ids = new Set(fixture.controls.map((control) => control.id));
for (const required of ["input.jp_text_field", "input.long_latin_area", "input.secret_secure_field"]) {
  if (!ids.has(required)) {
    hits.push(`missing required control id ${required}`);
  }
}
for (const kind of ["text_field", "text_area", "secure_field"]) {
  if (!fixture.controls.some((control) => control.kind === kind)) {
    hits.push(`missing control kind ${kind}`);
  }
}
const secure = fixture.controls.find((control) => control.kind === "secure_field");
if (!secure?.secure || secure?.evidence?.plaintextInObservation !== false) {
  hits.push("secure field must declare redacted observation evidence");
}
if (!fixture.fontStacks?.some((stack) => stack.id === "jp-serif")) {
  hits.push("fixture must record the Japanese serif font stack");
}
if (!fixture.fontStacks?.some((stack) => stack.id === "ui-sans")) {
  hits.push("fixture must record the UI sans font stack");
}

if (hits.length > 0) {
  throw new Error(hits.join("\n"));
}

console.log(JSON.stringify({ gate: "ime-player-rendered-fixture-source", status: "passed" }));
