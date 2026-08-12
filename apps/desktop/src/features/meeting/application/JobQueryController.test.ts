import { describe, expect, it } from "vitest";
import { createJobQueryController } from "./JobQueryController";

describe("JobQueryController", () => {
  it("rejects stale detail responses after a mutation", () => {
    const controller = createJobQueryController();
    const request = controller.beginRequest({ source: "local", jobId: "job-1" });
    const mutation = controller.beginMutation({ source: "local", jobId: "job-1" });

    expect(controller.isRequestCurrent(request)).toBe(false);
    expect(controller.commitMutation(mutation)).toBe(true);
  });

  it("fences late delete and retry responses independently", () => {
    const controller = createJobQueryController();
    const deleteFence = controller.beginMutation({ source: "local", jobId: "job-1" });
    const retryFence = controller.beginMutation({ source: "local", jobId: "job-1" });

    expect(controller.commitMutation(deleteFence)).toBe(false);
    expect(controller.commitMutation(retryFence)).toBe(true);
  });

  it("invalidates list responses when the source changes", () => {
    const controller = createJobQueryController();
    const isCurrent = controller.beginList("remote");
    controller.invalidateSource("remote");
    expect(isCurrent()).toBe(false);
  });
});
