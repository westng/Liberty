import { afterEach, describe, expect, it } from "vitest";
import { AppCommandError, localizeAppError, normalizeAppError } from "./appError";

describe("app error transport", () => {
  const previousDocument = globalThis.document;

  afterEach(() => {
    Object.defineProperty(globalThis, "document", {
      configurable: true,
      value: previousDocument,
    });
  });

  it("normalizes a structured command rejection without exposing source text", () => {
    const error = normalizeAppError({
      code: "ai_credential_operation_failed",
      category: "credentials",
      retryable: true,
      params: {},
      source: "secret provider body",
    });
    expect(error).toBeInstanceOf(AppCommandError);
    expect(error.message).not.toContain("secret provider body");
  });

  it("localizes high-risk errors in both supported languages", () => {
    const error = new AppCommandError({
      code: "remote_service_unavailable",
      category: "network",
      retryable: true,
      params: {},
    });
    Object.defineProperty(globalThis, "document", {
      configurable: true,
      value: { documentElement: { lang: "zh-CN" } },
    });
    expect(localizeAppError(error)).toContain("远端会议服务");
    Object.defineProperty(globalThis, "document", {
      configurable: true,
      value: { documentElement: { lang: "en-US" } },
    });
    expect(localizeAppError(error)).toContain("remote meeting service");
  });
});
