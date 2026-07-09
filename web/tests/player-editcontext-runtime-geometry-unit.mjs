import { strict as assert } from "node:assert";
import test from "node:test";
import { createArcweftEditContextPlayerGlue } from "../player-editcontext.js";

if (typeof globalThis.CustomEvent !== "function") {
  globalThis.CustomEvent = class CustomEvent extends Event {
    constructor(type, init = {}) {
      super(type, init);
      this.detail = init.detail;
    }
  };
}

test("runtime geometry overrides DOM-estimated selection bounds", async () => {
  const host = fakeHost();
  const glue = createArcweftEditContextPlayerGlue(host, {
    initialText: "A日本",
    delegate: {},
    statusTarget: new EventTarget(),
  });

  glue.updateFromRuntimeSnapshot({
    text: "A日本",
    selectionStart: 1,
    selectionEnd: 4,
  });
  glue.updateGeometry({
    caretRect: { x: 410, y: 88, width: 1, height: 18 },
    selectionRects: [
      { range: { start: 1, end: 4 }, rect: { x: 120, y: 40, width: 22, height: 18 } },
    ],
    characterBounds: [
      { range: { start: 0, end: 1 }, rect: { x: 100, y: 40, width: 8, height: 18 } },
      { range: { start: 1, end: 4 }, rect: { x: 120, y: 40, width: 22, height: 18 } },
    ],
  });

  assert.equal(glue.status().caretRect.x, 120);
  assert.equal(glue.status().caretRect.width, 22);
});

function fakeHost() {
  return {
    id: "runtime-geometry-host",
    classList: { add() {} },
    style: {},
    hasAttribute() { return true; },
    addEventListener() {},
    removeEventListener() {},
    getBoundingClientRect() {
      return { left: 0, top: 0, x: 0, y: 0, width: 20, height: 20 };
    },
  };
}
