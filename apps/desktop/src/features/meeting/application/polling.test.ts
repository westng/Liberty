// @vitest-environment jsdom

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createPollingScheduler } from "./polling";

describe("polling scheduler", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("runs at the configured interval and stops cleanly", async () => {
    const task = vi.fn();
    const scheduler = createPollingScheduler();

    scheduler.sync(true, 1000, task);
    expect(scheduler.isRunning()).toBe(true);
    await vi.advanceTimersByTimeAsync(1000);
    expect(task).toHaveBeenCalledTimes(1);

    scheduler.stop();
    await vi.advanceTimersByTimeAsync(5000);
    expect(task).toHaveBeenCalledTimes(1);
    expect(scheduler.isRunning()).toBe(false);
  });

  it("does not overlap an in-flight task", async () => {
    let finishTask: (() => void) | undefined;
    const task = vi.fn(() => new Promise<void>((resolve) => {
      finishTask = resolve;
    }));
    const scheduler = createPollingScheduler();

    scheduler.sync(true, 100, task);
    await vi.advanceTimersByTimeAsync(100);
    scheduler.sync(true, 50, task);
    await vi.advanceTimersByTimeAsync(1000);
    expect(task).toHaveBeenCalledTimes(1);

    finishTask?.();
    await Promise.resolve();
    await vi.advanceTimersByTimeAsync(0);
    expect(task).toHaveBeenCalledTimes(2);
    scheduler.stop();
  });
});
