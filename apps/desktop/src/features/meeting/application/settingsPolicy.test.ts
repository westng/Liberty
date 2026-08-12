import { describe, expect, it } from "vitest";
import type { ManagedRuntimeStatus } from "@/shared/types/meeting";
import {
  defaultSettings,
  hasManualPythonOverride,
  isManagedRuntimeReady,
  normalizeSettings,
  shouldUseLocalDataSource,
} from "./settingsPolicy";

describe("settings policy", () => {
  it("normalizes invalid and out-of-range values", () => {
    const normalized = normalizeSettings({
      accentColor: "invalid",
      concurrency: 99,
      localAsrThreads: -1,
      localAsrBatchSizeSeconds: 5,
      locale: "zh-CN",
      backendUrl: "  https://example.com  ",
    });

    expect(normalized.accentColor).toBe(defaultSettings.accentColor);
    expect(normalized.concurrency).toBe(8);
    expect(normalized.localAsrThreads).toBe(0);
    expect(normalized.localAsrBatchSizeSeconds).toBe(30);
    expect(normalized.backendUrl).toBe("https://example.com");
  });

  it("selects local data and manual Python from normalized settings", () => {
    const local = normalizeSettings({ pythonPath: " /python ", processingMode: "local" });
    const remote = normalizeSettings({ processingMode: "remote" });

    expect(hasManualPythonOverride(local)).toBe(true);
    expect(shouldUseLocalDataSource(local)).toBe(true);
    expect(shouldUseLocalDataSource(remote)).toBe(false);
  });

  it("uses shell readiness as the managed runtime boundary", () => {
    expect(isManagedRuntimeReady({ shellReady: true } as ManagedRuntimeStatus)).toBe(true);
    expect(isManagedRuntimeReady({ shellReady: false } as ManagedRuntimeStatus)).toBe(false);
  });
});
