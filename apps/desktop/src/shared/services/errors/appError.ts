import { getCurrentMessages } from "@/shared/i18n";

export type AppErrorCategory =
  | "validation"
  | "not_found"
  | "conflict"
  | "authorization"
  | "network"
  | "credentials"
  | "runtime"
  | "protocol"
  | "infrastructure";

export type AppErrorDto = {
  code: string;
  category: AppErrorCategory;
  retryable: boolean;
  params: Record<string, string>;
};

export class AppCommandError extends Error {
  readonly detail: AppErrorDto;

  constructor(detail: AppErrorDto) {
    super(detail.code);
    this.name = "AppCommandError";
    this.detail = detail;
  }
}

export function isAppErrorDto(value: unknown): value is AppErrorDto {
  if (!value || typeof value !== "object") {
    return false;
  }
  const candidate = value as Partial<AppErrorDto>;
  return typeof candidate.code === "string"
    && typeof candidate.category === "string"
    && typeof candidate.retryable === "boolean"
    && Boolean(candidate.params)
    && typeof candidate.params === "object";
}

export function normalizeAppError(error: unknown, fallback?: AppErrorDto): Error {
  if (error instanceof AppCommandError) {
    return error;
  }
  if (isAppErrorDto(error)) {
    return new AppCommandError(error);
  }
  if (fallback) {
    return new AppCommandError(fallback);
  }
  return error instanceof Error ? error : new Error(String(error));
}

export function appError(
  code: string,
  category: AppErrorCategory,
  retryable: boolean,
  params: Record<string, string> = {},
) {
  return new AppCommandError({ code, category, retryable, params });
}

export function localizeAppError(error: unknown): string {
  if (!(error instanceof AppCommandError)) {
    return error instanceof Error ? error.message : String(error);
  }
  const messages = getCurrentMessages().errors;
  const message = (() => {
    switch (error.detail.code) {
      case "remote_service_unavailable":
        return messages.remoteServiceUnavailable;
      case "ai_credential_operation_failed":
        return messages.aiCredentialOperationFailed;
      case "validation_error":
        return messages.validationError;
      case "not_found":
        return messages.notFound;
      case "conflict":
        return messages.conflict;
      case "job_window_scope_invalid":
        return messages.jobWindowScopeInvalid;
      case "infrastructure_error":
        return messages.infrastructureError;
      default:
        return messages.unexpected;
    }
  })();
  return error.detail.retryable ? `${message} ${messages.retryableHint}` : message;
}
