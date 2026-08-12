import { invoke } from "@tauri-apps/api/core";
import type { SettingsSnapshotV1 } from "@liberty-contracts/settings-v1";
import type {
  SettingsCommandError,
  SettingsCredentialUpdate,
  SettingsSnapshot,
  SettingsState,
  UiPreferences,
} from "@/shared/types/meeting";

export class SettingsConflictError extends Error {
  readonly current: SettingsSnapshot;

  constructor(message: string, current: SettingsSnapshot) {
    super(message);
    this.name = "SettingsConflictError";
    this.current = current;
  }
}

function isSettingsCommandError(error: unknown): error is SettingsCommandError {
  if (!error || typeof error !== "object") {
    return false;
  }
  const candidate = error as Partial<SettingsCommandError>;
  return (candidate.code === "settings_conflict" || candidate.code === "settings_save_failed")
    && typeof candidate.message === "string";
}

function normalizeSettingsCommandError(error: unknown): Error {
  if (isSettingsCommandError(error)) {
    if (error.code === "settings_conflict" && error.current) {
      return new SettingsConflictError(error.message, error.current);
    }
    return new Error(error.message);
  }
  return error instanceof Error ? error : new Error(String(error));
}

export function createLocalSettingsService() {
  return {
    getSettings: () => invoke<SettingsSnapshotV1>("get_settings"),
    getUiPreferences: () => invoke<UiPreferences>("get_ui_preferences"),
    saveSettings: async (
      settings: SettingsState,
      credential: SettingsCredentialUpdate = { action: "keep" },
    ): Promise<SettingsSnapshot> => {
      try {
        return await invoke<SettingsSnapshotV1>("save_settings", {
          settings: { ...settings, credential },
        });
      } catch (error) {
        throw normalizeSettingsCommandError(error);
      }
    },
  };
}
