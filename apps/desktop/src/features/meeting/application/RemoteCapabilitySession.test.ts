import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createRemoteCapabilitySession } from "./RemoteCapabilitySession";

describe("RemoteCapabilitySession", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("shares an in-flight handshake and retries an initial failure", async () => {
    const connect = vi.fn()
      .mockRejectedValueOnce(new Error("offline"))
      .mockResolvedValue({ version: 1 });
    let cached: { version: number } | null = null;
    const onReady = vi.fn((capabilities) => { cached = capabilities; });
    const session = createRemoteCapabilitySession({
      retryDelaysMs: [100],
      connect,
      canRetry: () => true,
      cached: () => cached,
      onChecking: vi.fn(),
      onReady,
      onUnavailable: vi.fn(),
      onInvalidate: vi.fn(),
      onRetryReady: vi.fn(),
    });
    session.setEnabled(true);

    const first = session.request();
    const shared = session.request();
    expect(first).toBe(shared);
    await expect(first).rejects.toThrow("offline");
    expect(session.hasRetryTimer()).toBe(true);

    await vi.advanceTimersByTimeAsync(100);
    expect(connect).toHaveBeenCalledTimes(2);
    expect(onReady).toHaveBeenCalledWith({ version: 1 });
  });

  it("releases retry timers when the session is disabled", async () => {
    const session = createRemoteCapabilitySession({
      retryDelaysMs: [100],
      connect: () => Promise.reject(new Error("offline")),
      canRetry: () => true,
      cached: () => null,
      onChecking: vi.fn(),
      onReady: vi.fn(),
      onUnavailable: vi.fn(),
      onInvalidate: vi.fn(),
      onRetryReady: vi.fn(),
    });
    session.setEnabled(true);
    await expect(session.request()).rejects.toThrow("offline");
    session.setEnabled(false);

    expect(session.hasRetryTimer()).toBe(false);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("ignores degradation from an obsolete request generation", () => {
    const onUnavailable = vi.fn();
    const session = createRemoteCapabilitySession({
      retryDelaysMs: [],
      connect: () => Promise.resolve({ version: 1 }),
      canRetry: () => false,
      cached: () => null,
      onChecking: vi.fn(),
      onReady: vi.fn(),
      onUnavailable,
      onInvalidate: vi.fn(),
      onRetryReady: vi.fn(),
    });
    const generation = session.generation();
    session.reset();

    expect(session.degrade(new Error("late"), generation)).toBe(false);
    expect(onUnavailable).not.toHaveBeenCalled();
  });
});
