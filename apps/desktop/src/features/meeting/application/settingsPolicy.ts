import type { ManagedRuntimeStatus, SettingsState } from "@/shared/types/meeting";

export const defaultSettings: SettingsState = {
  themeMode: "auto",
  liquidGlassStyle: "transparent",
  accentColor: "#2f6dff",
  locale: "zh-CN",
  backendUrl: "",
  apiToken: "",
  defaultHotwords: "SeACo-Paraformer, FunASR, 会议纪要",
  summaryTemplate: "表格版会议纪要",
  concurrency: 2,
  pythonPath: "",
  runnerScriptPath: "",
  localAsrDevice: "auto",
  localAsrThreads: 0,
  localAsrBatchSizeSeconds: 300,
  runtimeDownloadSource: "",
};

export function normalizeSettings(settings?: Partial<SettingsState> | null): SettingsState {
  const merged = {
    ...defaultSettings,
    ...(settings ?? {}),
  };

  return {
    ...merged,
    themeMode: merged.themeMode === "light" || merged.themeMode === "dark" ? merged.themeMode : "auto",
    liquidGlassStyle: merged.liquidGlassStyle === "tinted" ? "tinted" : "transparent",
    locale: merged.locale === "en-US" ? "en-US" : "zh-CN",
    accentColor: /^#[0-9a-fA-F]{6}$/.test(merged.accentColor.trim())
      ? merged.accentColor.trim().toLowerCase()
      : defaultSettings.accentColor,
    backendUrl: merged.backendUrl.trim(),
    apiToken: merged.apiToken.trim(),
    defaultHotwords: merged.defaultHotwords.trim() || defaultSettings.defaultHotwords,
    summaryTemplate:
      merged.summaryTemplate.trim() === "默认会议纪要模板"
        ? defaultSettings.summaryTemplate
        : merged.summaryTemplate.trim() || defaultSettings.summaryTemplate,
    concurrency: Math.min(8, Math.max(1, Number(merged.concurrency) || defaultSettings.concurrency)),
    pythonPath: merged.pythonPath.trim(),
    runnerScriptPath: merged.runnerScriptPath.trim(),
    localAsrDevice:
      merged.localAsrDevice === "cpu" || merged.localAsrDevice === "mps" || merged.localAsrDevice === "cuda"
        ? merged.localAsrDevice
        : "auto",
    localAsrThreads: Math.min(32, Math.max(0, Number(merged.localAsrThreads) || 0)),
    localAsrBatchSizeSeconds: Math.min(
      1200,
      Math.max(30, Number(merged.localAsrBatchSizeSeconds) || defaultSettings.localAsrBatchSizeSeconds),
    ),
    runtimeDownloadSource: merged.runtimeDownloadSource.trim(),
  };
}

export function hasManualPythonOverride(settings: SettingsState) {
  return Boolean(settings.pythonPath.trim());
}

export function shouldUseLocalDataSource(settings: SettingsState) {
  return !settings.backendUrl.trim();
}

export function isManagedRuntimeReady(runtimeStatus: ManagedRuntimeStatus) {
  return (
    (runtimeStatus.status === "ready" || runtimeStatus.status === "system_ready") &&
    Boolean(runtimeStatus.pythonExecutablePath?.trim())
  );
}

export function shouldAutoInstallManagedRuntime(
  settings: SettingsState,
  runtimeStatus: ManagedRuntimeStatus,
) {
  if (settings.backendUrl.trim()) {
    return false;
  }

  if (hasManualPythonOverride(settings)) {
    return false;
  }

  if (!settings.runtimeDownloadSource.trim()) {
    return false;
  }

  if (runtimeStatus.status === "system_ready") {
    return false;
  }

  if (runtimeStatus.status === "unsupported") {
    return false;
  }

  return runtimeStatus.status === "missing" || runtimeStatus.status === "repair_required";
}
