import { invoke } from "@tauri-apps/api/core";
import type {
  ManagedRuntimeStateV1,
  RuntimeComponentStateV1,
} from "@liberty-contracts/runtime-v1";
import type {
  ManagedRuntimeStatus,
  RuntimeComponentId,
  RuntimeComponentState,
  RuntimeSource,
} from "@/shared/types/meeting";

export interface RuntimeDownloadSourceOption {
  id: string;
  label: string;
}

function normalizeComponent(component: RuntimeComponentStateV1): RuntimeComponentState {
  return {
    ...component,
    source: component.source ?? undefined,
    activeArtifact: component.activeArtifact ?? undefined,
    operation: {
      ...component.operation,
      progress: component.operation.progress ?? undefined,
      lastError: component.operation.lastError ?? undefined,
    },
  };
}

function normalizeRuntime(status: ManagedRuntimeStateV1): ManagedRuntimeStatus {
  return {
    ...status,
    pythonExecutablePath: status.pythonExecutablePath ?? undefined,
    modelsRoot: status.modelsRoot ?? undefined,
    installRoot: status.installRoot ?? undefined,
    ffmpegPath: status.ffmpegPath ?? undefined,
    lastError: status.lastError ?? undefined,
    installedAt: status.installedAt ?? undefined,
    lastLogPath: status.lastLogPath ?? undefined,
    python: normalizeComponent(status.python),
    ffmpeg: normalizeComponent(status.ffmpeg),
    models: normalizeComponent(status.models),
  };
}

export function createLocalRuntimeService() {
  return {
    getStatus: () => invoke<ManagedRuntimeStateV1>("get_runtime_status").then(normalizeRuntime),
    listDownloadSources: () => invoke<RuntimeDownloadSourceOption[]>("list_runtime_download_sources"),
    setComponentSource: (component: "python" | "ffmpeg", source: RuntimeSource) =>
      invoke<ManagedRuntimeStateV1>("set_runtime_component_source", { component, source }).then(normalizeRuntime),
    detectComponent: (component: "python" | "ffmpeg") =>
      invoke<ManagedRuntimeStateV1>("detect_runtime_component", { component }).then(normalizeRuntime),
    installComponent: (component: RuntimeComponentId) =>
      invoke<ManagedRuntimeStateV1>("install_runtime_component", { component }).then(normalizeRuntime),
    getComponentLog: (component: RuntimeComponentId) =>
      invoke<string>("get_runtime_component_log", { component }),
    install: () => invoke<ManagedRuntimeStateV1>("install_runtime").then(normalizeRuntime),
    getInstallLog: () => invoke<string>("get_runtime_install_log"),
  };
}
