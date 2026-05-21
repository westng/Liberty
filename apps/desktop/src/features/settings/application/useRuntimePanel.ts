import { useMemo } from "react";
import type { MessageTree } from "@/shared/i18n";
import type { ManagedRuntimeStatus } from "@/shared/types/meeting";
import type { MeetingStore } from "@/features/meeting/stores/useMeetingStore";

export function useRuntimePanel(
  store: MeetingStore,
  messages: MessageTree["settings"],
  shellMessages: MessageTree["shell"],
  commonMessages: MessageTree["common"],
) {
  const runtimeModeLabel = store.localMode
    ? shellMessages.localMode
    : store.settings.backendUrl
      ? shellMessages.remoteMode
      : shellMessages.mockModeShort;
  const runtimeStatus = store.runtimeStatus;
  const runtimeInstallLog = store.runtimeInstallLog;
  const runtimeInstallLogReversed = useMemo(() => {
    const lines = runtimeInstallLog
      .split(/\r?\n/)
      .map((line) => line.trimEnd())
      .filter(Boolean);

    return lines.reverse().join("\n");
  }, [runtimeInstallLog]);
  const runtimeActionLabel = runtimeStatus.status === "unsupported"
    ? messages.runtimeStatusUnsupported
    : runtimeStatus.status === "ready"
      ? messages.runtimeReinstallAction
      : runtimeStatus.status === "installing"
        ? messages.runtimeStatusInstalling
        : messages.runtimeInstallAction;
  const runtimeStatusLabel = labelForRuntimeStatus(runtimeStatus, messages);
  const runtimeStatusDescription = runtimeDescription(runtimeStatus, messages);
  const runtimeBusy = runtimeStatus.status === "installing" || runtimeStatus.status === "unsupported";
  const runtimeInstalledAtLabel = formatRuntimeDate(runtimeStatus.installedAt);
  const runtimeInstallProgress = runtimeProgress(runtimeStatus, runtimeInstallLog, messages);

  async function refreshRuntimePanel() {
    await store.refreshRuntimeStatus();
    await store.refreshRuntimeInstallLog();
  }

  function formatRuntimeDate(value?: string) {
    const normalized = value?.trim();
    if (!normalized) {
      return commonMessages.dash;
    }

    const fromMillis = Number(normalized);
    const date = Number.isFinite(fromMillis) && fromMillis > 0 ? new Date(fromMillis) : new Date(normalized);
    return Number.isNaN(date.getTime()) ? normalized : date.toLocaleString(store.settings.locale);
  }

  return {
    runtimeModeLabel,
    runtimeStatus,
    runtimeInstallLog,
    runtimeInstallLogReversed,
    runtimeActionLabel,
    runtimeStatusLabel,
    runtimeStatusDescription,
    runtimeBusy,
    runtimeInstalledAtLabel,
    runtimeInstallProgress,
    refreshRuntimePanel,
  };
}

function labelForRuntimeStatus(status: ManagedRuntimeStatus, messages: MessageTree["settings"]) {
  switch (status.status) {
    case "installing":
      return messages.runtimeStatusInstalling;
    case "ready":
      return messages.runtimeStatusReady;
    case "failed":
      return messages.runtimeStatusFailed;
    case "repair_required":
      return messages.runtimeStatusRepair;
    case "unsupported":
      return messages.runtimeStatusUnsupported;
    default:
      return messages.runtimeStatusMissing;
  }
}

function runtimeDescription(status: ManagedRuntimeStatus, messages: MessageTree["settings"]) {
  if (status.lastError?.trim()) {
    return status.lastError.trim();
  }

  switch (status.status) {
    case "ready":
      return messages.runtimeDescriptionReady;
    case "installing":
      return messages.runtimeDescriptionInstalling;
    case "failed":
    case "repair_required":
    case "unsupported":
      return messages.runtimeDescriptionFailed;
    default:
      return messages.runtimeDescriptionMissing;
  }
}

function runtimeProgress(status: ManagedRuntimeStatus, log: string, messages: MessageTree["settings"]) {
  const normalized = log.trim();

  if (status.status === "ready" || normalized.includes("[runtime] install completed.")) {
    return {
      percent: 100,
      label: messages.runtimeInstallCompleted,
    };
  }

  let percent = status.status === "installing" ? 4 : 0;
  let label = messages.runtimeInstallPreparing;
  const lastStageProgress = Array.from(
    log.matchAll(/\[runtime\] staging progress .*?\(([\d.]+)%\)/g),
  ).at(-1)?.[1];

  if (lastStageProgress) {
    percent = Math.max(percent, Math.min(52, Math.round(Number(lastStageProgress) * 0.52)));
    label = messages.runtimeInstallDownload;
  } else if (normalized.includes("[runtime] staging bundled ")) {
    percent = Math.max(percent, 16);
    label = messages.runtimeInstallDownload;
  }

  const stageWeights = [
    ["[runtime] locating bundled runtime resources", 8, messages.runtimeInstallPreparing],
    ["[runtime] staging bundled Python runtime", 22, messages.runtimeInstallDownload],
    ["[runtime] verifying bundled asset checksum", 32, messages.runtimeInstallVerify],
    ["[runtime] extracting python runtime archive", 44, messages.runtimeInstallExtract],
    ["[runtime] resolved python=", 54, messages.runtimeInstallResolvePython],
    ["Validating bundled Python runtime", 66, messages.runtimeInstallBootstrapPip],
    ["[runtime] staging bundled FFmpeg runtime", 76, messages.runtimeInstallUpgradePip],
    ["Validating ffmpeg runtime", 84, messages.runtimeInstallPytorch],
    ["Downloading default ASR models", 94, messages.runtimeInstallModels],
    ["Downloading default Sherpa-ONNX model", 94, messages.runtimeInstallModels],
  ] as const;

  for (const [pattern, stagePercent, stageLabel] of stageWeights) {
    if (normalized.includes(pattern)) {
      percent = Math.max(percent, stagePercent);
      label = stageLabel;
    }
  }

  return {
    percent: Math.max(0, Math.min(99, percent)),
    label,
  };
}
