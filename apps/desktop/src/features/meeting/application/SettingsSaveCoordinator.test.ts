import { describe, expect, it } from "vitest";
import { createSettingsSaveCoordinator } from "./SettingsSaveCoordinator";

describe("SettingsSaveCoordinator", () => {
  it("serializes writes and maintains the optimistic projection", async () => {
    let current = { revision: 0, value: "initial" };
    const order: string[] = [];
    const coordinator = createSettingsSaveCoordinator({
      current: () => current,
      project: (intent: string, projection) => ({ ...projection, value: intent }),
      execute: async (intent: string) => {
        order.push(intent);
        current = { revision: current.revision + 1, value: intent };
        return current;
      },
    });

    const first = coordinator.enqueue("first");
    const second = coordinator.enqueue("second");
    expect(coordinator.projected().value).toBe("second");
    await Promise.all([first, second]);

    expect(order).toEqual(["first", "second"]);
    expect(coordinator.pendingCount()).toBe(0);
    expect(coordinator.projected()).toEqual(current);
  });

  it("continues queued writes after a rejected conflicting write", async () => {
    let current = { revision: 3, value: "current" };
    const attempts: string[] = [];
    const coordinator = createSettingsSaveCoordinator({
      current: () => current,
      project: (intent: string, projection) => ({ ...projection, value: intent }),
      execute: async (intent: string) => {
        attempts.push(intent);
        if (intent === "conflict") {
          throw new Error("settings revision conflict");
        }
        current = { revision: current.revision + 1, value: intent };
        return current;
      },
    });

    const conflicting = coordinator.enqueue("conflict");
    const following = coordinator.enqueue("following");

    await expect(conflicting).rejects.toThrow("settings revision conflict");
    await expect(following).resolves.toEqual({ revision: 4, value: "following" });
    expect(attempts).toEqual(["conflict", "following"]);
    expect(coordinator.pendingCount()).toBe(0);
    expect(coordinator.projected()).toEqual(current);
  });
});
