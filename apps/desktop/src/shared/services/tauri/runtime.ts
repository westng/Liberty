import { invoke } from "@tauri-apps/api/core";
import type { ManagedRuntimeStatus } from "@/shared/types/meeting";

export function createLocalRuntimeService() {
  return {
    getStatus: () => invoke<ManagedRuntimeStatus>("get_runtime_status"),
    detectSystem: () => invoke<ManagedRuntimeStatus>("detect_system_runtime"),
    install: () => invoke<ManagedRuntimeStatus>("install_runtime"),
    getInstallLog: () => invoke<string>("get_runtime_install_log"),
  };
}
