import { setupArcweftWebTextInput } from "./player.js";

const statusLine = document.querySelector("#ime-sample-status");
const selectionLine = document.querySelector("#ime-sample-selection");
const fontsLine = document.querySelector("#ime-sample-fonts");

function setStatus(text, state) {
  statusLine.value = text;
  statusLine.dataset.state = state;
}

function updateFontStatus() {
  const fonts = [
    "Arcweft Demo",
    "Yu Gothic",
    "Hiragino Sans",
    "Noto Sans JP",
  ];
  fontsLine.value = `fonts ${fonts.join(" / ")}`;
}

function selectionLabel(detail) {
  return `selection ${detail.selectionStart ?? 0}..${detail.selectionEnd ?? 0} utf16`;
}

document.addEventListener("arcweft-text-input-status", (event) => {
  const detail = event.detail ?? {};
  const ready = [
    "ready",
    "focused",
    "composition_start",
    "composition_update",
    "composition_end",
    "committed_update",
    "format_update",
    "geometry_update",
    "runtime_update",
  ].includes(detail.state);
  const unsupported = detail.state === "unsupported_no_fallback";
  const state = unsupported ? "unsupported" : ready ? "ready" : "initializing";
  setStatus(`${detail.owner ?? "arcweft-player"}: ${detail.state ?? "initializing"}`, state);
  selectionLine.value = selectionLabel(detail);
});

updateFontStatus();
setStatus("Arcweft player-owned EditContext initializing", "initializing");

setupArcweftWebTextInput({
  hostId: "arcweft-ime-surface",
  mirror: {
    committedTextId: "arcweft-ime-text",
    compositionTextId: "arcweft-ime-composition",
  },
  initialText: "",
}).catch((error) => {
  setStatus(`Arcweft player-owned text input failed: ${error.message}`, "unsupported");
});
