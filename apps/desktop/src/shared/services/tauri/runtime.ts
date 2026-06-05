import { invoke } from "@tauri-apps/api/core";
import type { ManagedRuntimeStatus } from "@/shared/types/meeting";

export interface RuntimeDownloadSourceOption {
  id: string;
  label: string;
}

export function createLocalRuntimeService() {
  return {
    getStatus: () => invoke<ManagedRuntimeStatus>("get_runtime_status"),
    listDownloadSources: () => invoke<RuntimeDownloadSourceOption[]>("list_runtime_download_sources"),
    detectSystem: () => invoke<ManagedRuntimeStatus>("detect_system_runtime"),
    install: () => invoke<ManagedRuntimeStatus>("install_runtime"),
    getInstallLog: () => invoke<string>("get_runtime_install_log"),
  };
}
