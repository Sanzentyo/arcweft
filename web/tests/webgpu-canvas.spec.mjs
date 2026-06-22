import { expect, test } from "@playwright/test";

function collectConsoleErrors(page) {
  const errors = [];
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(message.text());
  });
  page.on("pageerror", (error) => errors.push(error.message));
  return errors;
}

async function openReady(page) {
  await page.goto("/index.html");
  expect(await page.evaluate(() => Boolean(navigator.gpu))).toBe(true);
  await expect(page.locator("#arcweft-canvas")).toHaveAttribute("data-arcweft-ready", "true");
  await page.waitForFunction(() => window.__arcweftLastObservation?.choice_count > 0);
}

test("dialogue and choice are WebGPU canvas content, not DOM game UI", async ({ page }) => {
  const errors = collectConsoleErrors(page);
  await openReady(page);

  await expect(page.locator("canvas#arcweft-canvas")).toBeVisible();
  await expect(page.locator("button")).toHaveCount(0);
  await expect(page.locator("[data-arcweft-speaker], [data-arcweft-dialogue], [data-arcweft-choice]"))
    .toHaveCount(0);
  expect(errors).toEqual([]);
});

test("pointer hit-test selects a canvas choice and advances runtime", async ({ page }) => {
  const errors = collectConsoleErrors(page);
  await openReady(page);

  const canvas = page.locator("#arcweft-canvas");
  const box = await canvas.boundingBox();
  expect(box).not.toBeNull();
  await page.mouse.click(box.x + box.width * 0.5, box.y + box.height * 0.39);
  await page.waitForFunction(() => window.__arcweftLastObservation?.finished === true);
  expect(errors).toEqual([]);
});

test("keyboard focus navigation and activation select a canvas choice", async ({ page }) => {
  const errors = collectConsoleErrors(page);
  await openReady(page);

  await page.locator("#arcweft-canvas").focus();
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Enter");
  await page.waitForFunction(() => window.__arcweftLastObservation?.finished === true);
  expect(errors).toEqual([]);
});

test("resize keeps WebGPU viewport and hit-test geometry aligned", async ({ page }) => {
  const errors = collectConsoleErrors(page);
  await openReady(page);
  await page.setViewportSize({ width: 960, height: 540 });
  await page.waitForTimeout(100);

  const box = await page.locator("#arcweft-canvas").boundingBox();
  await page.mouse.click(box.x + box.width * 0.5, box.y + box.height * 0.39);
  await page.waitForFunction(() => window.__arcweftLastObservation?.finished === true);
  expect(errors).toEqual([]);
});

test("missing WebGPU produces a structured fatal bootstrap error", async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(Navigator.prototype, "gpu", {
      configurable: true,
      get: () => undefined,
    });
  });
  await page.goto("/index.html");
  await expect(page.locator("#arcweft-fatal")).toBeVisible();
  await expect(page.locator("#arcweft-fatal")).toContainText("WebGPU is unsupported");
  await expect(page.locator("button")).toHaveCount(0);
});
