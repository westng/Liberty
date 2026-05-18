import { computed } from "vue";
import type { ComputedRef } from "vue";
import type { Messages } from "@/shared/i18n";
import type { ManagedRuntimeStatus } from "@/shared/types/meeting";
import type { MeetingStore } from "@/features/meeting/stores/useMeetingStore";

export function useRuntimePanel(
  store: MeetingStore,
  messages: ComputedRef<Messages["settings"]>,
  shellMessages: ComputedRef<Messages["shell"]>,
  commonMessages: ComputedRef<Messages["common"]>,
) {
  const runtimeModeLabel = computed(() => {
    if (store.localMode.value) {
      return shellMessages.value.localMode;
    }

    if (store.settings.value.backendUrl) {
      return shellMessages.value.remoteMode;
    }

    return shellMessages.value.mockModeShort;
  });
  const runtimeStatus = computed(() => store.runtimeStatus.value);
  const runtimeInstallLog = computed(() => store.runtimeInstallLog.value);
  const runtimeInstallLogReversed = computed(() => {
    const lines = runtimeInstallLog.value
      .split(/\r?\n/)
      .map((line) => line.trimEnd())
      .filter(Boolean);

    return lines.reverse().join("\n");
  });
  const runtimeActionLabel = computed(() =>
    runtimeStatus.value.status === "unsupported"
      ? messages.value.runtimeStatusUnsupported
      : runtimeStatus.value.status === "ready"
        ? messages.value.runtimeReinstallAction
        : runtimeStatus.value.status === "installing"
          ? messages.value.runtimeStatusInstalling
          : messages.value.runtimeInstallAction,
  );
  const runtimeStatusLabel = computed(() => labelForRuntimeStatus(runtimeStatus.value, messages.value));
  const runtimeStatusDescription = computed(() => runtimeDescription(runtimeStatus.value, messages.value));
  const runtimeBusy = computed(() => runtimeStatus.value.status === "installing" || runtimeStatus.value.status === "unsupported");
  const runtimeInstalledAtLabel = computed(() => formatRuntimeDate(runtimeStatus.value.installedAt));
  const runtimeInstallProgress = computed(() => runtimeProgress(runtimeStatus.value, runtimeInstallLog.value, messages.value));

  async function refreshRuntimePanel() {
    await store.refreshRuntimeStatus();
    await store.refreshRuntimeInstallLog();
  }

  function formatRuntimeDate(value?: string) {
    const normalized = value?.trim();
    if (!normalized) {
      return commonMessages.value.dash;
    }

    const fromMillis = Number(normalized);
    const date = Number.isFinite(fromMillis) && fromMillis > 0 ? new Date(fromMillis) : new Date(normalized);
    return Number.isNaN(date.getTime()) ? normalized : date.toLocaleString(store.settings.value.locale);
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

function labelForRuntimeStatus(status: ManagedRuntimeStatus, messages: Messages["settings"]) {
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

function runtimeDescription(status: ManagedRuntimeStatus, messages: Messages["settings"]) {
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

function runtimeProgress(status: ManagedRuntimeStatus, log: string, messages: Messages["settings"]) {
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
