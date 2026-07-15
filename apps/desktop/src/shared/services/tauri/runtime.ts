import { invoke } from "@tauri-apps/api/core";
import type {
  ManagedRuntimeStatus,
  RuntimeComponentId,
  RuntimeSource,
} from "@/shared/types/meeting";

export interface RuntimeDownloadSourceOption {
  id: string;
  label: string;
}

export function createLocalRuntimeService() {
  return {
    getStatus: () => invoke<ManagedRuntimeStatus>("get_runtime_status"),
    listDownloadSources: () => invoke<RuntimeDownloadSourceOption[]>("list_runtime_download_sources"),
    setComponentSource: (component: "python" | "ffmpeg", source: RuntimeSource) =>
      invoke<ManagedRuntimeStatus>("set_runtime_component_source", { component, source }),
    detectComponent: (component: "python" | "ffmpeg") =>
      invoke<ManagedRuntimeStatus>("detect_runtime_component", { component }),
    installComponent: (component: RuntimeComponentId) =>
      invoke<ManagedRuntimeStatus>("install_runtime_component", { component }),
    getComponentLog: (component: RuntimeComponentId) =>
      invoke<string>("get_runtime_component_log", { component }),
    install: () => invoke<ManagedRuntimeStatus>("install_runtime"),
    getInstallLog: () => invoke<string>("get_runtime_install_log"),
  };
}
