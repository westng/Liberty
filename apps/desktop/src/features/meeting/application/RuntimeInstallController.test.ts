// @vitest-environment jsdom

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createRuntimeInstallController } from "./RuntimeInstallController";

describe("RuntimeInstallController", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("deduplicates installs and polls only while work is active", async () => {
    let resolveInstall: ((status: string) => void) | undefined;
    const install = vi.fn(() => new Promise<string>((resolve) => {
      resolveInstall = resolve;
    }));
    const refresh = vi.fn();
    const controller = createRuntimeInstallController({
      pollingIntervalMs: 100,
      isOperationActive: (status: string) => status === "installing",
      install,
      refresh,
    });

    const first = controller.install();
    const second = controller.install();
    expect(first).toBe(second);
    controller.sync(true, "installing");
    await vi.advanceTimersByTimeAsync(100);
    expect(refresh).toHaveBeenCalledTimes(1);

    controller.sync(true, "ready");
    await vi.advanceTimersByTimeAsync(500);
    expect(refresh).toHaveBeenCalledTimes(1);
    resolveInstall?.("ready");
    await first;
    expect(controller.isInstalling()).toBe(false);
  });

  it("clears polling on dispose", () => {
    const controller = createRuntimeInstallController({
      pollingIntervalMs: 100,
      isOperationActive: () => true,
      install: () => Promise.resolve("ready"),
      refresh: () => Promise.resolve(),
    });
    controller.sync(true, "installing");
    controller.dispose();
    expect(controller.isPolling()).toBe(false);
    expect(vi.getTimerCount()).toBe(0);
  });
});
