const surface = document.querySelector("#arcweft-ime-surface");
const committedText = document.querySelector("#arcweft-ime-text");
const compositionText = document.querySelector("#arcweft-ime-composition");
const statusLine = document.querySelector("#ime-sample-status");
const selectionLine = document.querySelector("#ime-sample-selection");
const fontsLine = document.querySelector("#ime-sample-fonts");

let modelText = "";
let composing = false;

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

function render(text, composition = "") {
  committedText.textContent = text;
  compositionText.textContent = composition;
}

function utf16SelectionLabel(selectionStart, selectionEnd) {
  return `selection ${selectionStart}..${selectionEnd} utf16`;
}

function applyUpdate(updateRangeStart, updateRangeEnd, text, selectionStart, selectionEnd) {
  modelText =
    modelText.slice(0, updateRangeStart) + text + modelText.slice(updateRangeEnd);
  selectionLine.value = utf16SelectionLabel(selectionStart, selectionEnd);
  if (!composing) {
    render(modelText);
  }
}

function installKeyboardFallbackForUnsupportedBrowser() {
  surface.addEventListener("beforeinput", (event) => {
    if (typeof event.data !== "string") {
      return;
    }
    modelText += event.data;
    render(modelText);
  });
  surface.addEventListener("keydown", (event) => {
    if (event.key === "Backspace") {
      event.preventDefault();
      modelText = Array.from(modelText).slice(0, -1).join("");
      render(modelText);
    }
  });
  setStatus("EditContext unavailable: sample is in keyboard-only fallback mode", "unsupported");
}

function installEditContext() {
  if (typeof window.EditContext !== "function" || !("editContext" in surface)) {
    installKeyboardFallbackForUnsupportedBrowser();
    return;
  }

  const editContext = new window.EditContext({ text: "" });
  surface.editContext = editContext;
  surface.addEventListener("focus", () => editContext.focus?.());

  editContext.addEventListener("textupdate", (event) => {
    composing = Boolean(event.compositionStart !== event.compositionEnd);
    applyUpdate(
      event.updateRangeStart,
      event.updateRangeEnd,
      event.text,
      event.selectionStart,
      event.selectionEnd,
    );
    if (composing) {
      render(
        modelText.slice(0, event.compositionStart),
        modelText.slice(event.compositionStart, event.compositionEnd),
      );
    }
    setStatus(composing ? "composition update" : "committed update", "ready");
  });

  editContext.addEventListener("compositionend", () => {
    composing = false;
    render(modelText);
    setStatus("composition committed", "ready");
  });

  editContext.addEventListener("textformatupdate", () => {
    setStatus("format spans updated", "ready");
  });

  setStatus("EditContext ready", "ready");
}

render("");
updateFontStatus();
installEditContext();
