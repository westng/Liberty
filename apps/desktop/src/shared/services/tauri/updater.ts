import { invoke } from "@tauri-apps/api/core";
import type { AppUpdateStatus } from "@/shared/types/meeting";

export function createLocalUpdaterService() {
  return {
    getStatus: () => invoke<AppUpdateStatus>("get_update_status"),
    checkForUpdates: (interactive = true) =>
      invoke<AppUpdateStatus>("check_for_updates", { interactive }),
    installUpdate: () => invoke<AppUpdateStatus>("install_update"),
    restartAfterUpdate: () => invoke<void>("restart_after_update"),
  };
}
