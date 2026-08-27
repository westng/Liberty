import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { handleEditorWindowCloseRequested } from "./window";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

describe("editor window close guard", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("destroys a clean editor through the scoped Rust command", async () => {
    const preventDefault = vi.fn();
    const confirmDiscard = vi.fn();

    await handleEditorWindowCloseRequested({ preventDefault }, false, confirmDiscard);

    expect(preventDefault).toHaveBeenCalledOnce();
    expect(confirmDiscard).not.toHaveBeenCalled();
    expect(invokeMock).toHaveBeenCalledWith("destroy_current_window");
  });

  it("keeps a dirty editor open when discard is cancelled", async () => {
    const preventDefault = vi.fn();
    const confirmDiscard = vi.fn().mockResolvedValue(false);

    await handleEditorWindowCloseRequested({ preventDefault }, true, confirmDiscard);

    expect(preventDefault).toHaveBeenCalledOnce();
    expect(confirmDiscard).toHaveBeenCalledOnce();
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("destroys a dirty editor after discard is confirmed", async () => {
    const preventDefault = vi.fn();
    const confirmDiscard = vi.fn().mockResolvedValue(true);

    await handleEditorWindowCloseRequested({ preventDefault }, true, confirmDiscard);

    expect(preventDefault).toHaveBeenCalledOnce();
    expect(confirmDiscard).toHaveBeenCalledOnce();
    expect(invokeMock).toHaveBeenCalledWith("destroy_current_window");
  });
});
