import { invoke } from "@tauri-apps/api/core";
import type { SettingsState } from "@/types/meeting";

export function createLocalSettingsService() {
  return {
    getSettings: () => invoke<SettingsState>("get_settings"),
    saveSettings: (settings: SettingsState) => invoke<void>("save_settings", { settings }),
  };
}
