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

test("runtime commands drive snapshot, geometry, and deactivate without app IME glue", () => {
  const host = fakeHost();
  const statusTarget = new EventTarget();
  const statuses = [];
  statusTarget.addEventListener("arcweft-text-input-status", (event) => statuses.push(event.detail.state));
  const glue = createArcweftEditContextPlayerGlue(host, {
    delegate: {},
    statusTarget,
  });

  glue.applyRuntimeCommand({
    kind: "activate",
    snapshot: {
      session: 4,
      revision: 1,
      target: "target.web.runtime",
      secure: false,
      text: "A日本",
      surroundingText: "A日本",
      selectionStart: 4,
      selectionEnd: 4,
      compositionStart: 4,
      compositionEnd: 4,
      controlRect: { x: 100, y: 40, width: 240, height: 32 },
      caretRect: { x: 130, y: 44, width: 1, height: 24 },
      characterBounds: [],
    },
  });
  glue.applyRuntimeCommand({
    kind: "update_geometry",
    geometry: {
      session: 4,
      revision: 1,
      controlRect: { x: 100, y: 40, width: 240, height: 32 },
      caretRect: { x: 130, y: 44, width: 1, height: 24 },
      selectionRects: [],
      compositionRects: [],
      characterBounds: [
        { range: { start: 0, end: 1 }, rect: { x: 100, y: 44, width: 10, height: 24 } },
        { range: { start: 1, end: 4 }, rect: { x: 110, y: 44, width: 28, height: 24 } },
      ],
    },
  });

  assert.equal(glue.status().caretRect.x, 130);
  assert.equal(glue.status().selectionStart, 4);
  assert.deepEqual(statuses.slice(0, 2), ["runtime_activated", "geometry_update"]);

  glue.applyRuntimeCommand({ kind: "deactivate", session: 4 });
  assert.ok(statuses.includes("runtime_deactivated"));
});

function fakeHost() {
  return {
    id: "runtime-bridge-host",
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
