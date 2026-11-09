export async function installManualLogicalClock(page) {
  await page.addInitScript(() => {
    let now = 0;
    let remainingSteps = 1;
    globalThis.__arcweftNowMillis = () => {
      const consumed = Math.min(remainingSteps, 4);
      remainingSteps -= consumed;
      now += consumed * 16;
      return now;
    };
    globalThis.__arcweftGrantLogicalClockSteps = (count) => {
      if (!Number.isSafeInteger(count) || count < 0) {
        throw new Error(`invalid logical clock step count: ${count}`);
      }
      remainingSteps += count;
    };
    globalThis.__arcweftRemainingLogicalClockSteps = () => remainingSteps;
  });
}

export async function advanceLogicalClockSteps(page, steps) {
  const startTick = await page.evaluate(() =>
    window.__arcweftLastObservation?.logical_tick ?? null
  );
  if (!Number.isSafeInteger(startTick)) {
    throw new Error("runtime observation has no logical tick");
  }
  const targetTick = startTick + steps;
  await page.evaluate((count) => {
    globalThis.__arcweftGrantLogicalClockSteps(count);
  }, steps);
  await page.waitForFunction(
    (target) =>
      window.__arcweftLastObservation?.logical_tick >= target &&
      globalThis.__arcweftRemainingLogicalClockSteps() === 0,
    targetTick,
    { timeout: 10_000 },
  );
  const captureTick = await page.evaluate(() =>
    window.__arcweftLastObservation?.logical_tick ?? null
  );
  if (captureTick !== targetTick) {
    throw new Error(`logical clock overshot target ${targetTick}: ${captureTick}`);
  }
  return { startTick, captureTick };
}
