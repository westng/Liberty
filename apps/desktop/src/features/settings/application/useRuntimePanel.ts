import { useMemo } from "react";
import type { MessageTree } from "@/shared/i18n";
import type { ManagedRuntimeStatus } from "@/shared/types/meeting";
import type { MeetingStore } from "@/features/meeting/stores/useMeetingStore";

const runtimeDownloadSources = [
  { sourceId: "aliyun", nameZh: "阿里云镜像", nameEn: "Aliyun Mirror" },
  { sourceId: "tencent", nameZh: "腾讯云镜像", nameEn: "Tencent Cloud Mirror" },
  { sourceId: "huawei", nameZh: "华为云镜像", nameEn: "Huawei Cloud Mirror" },
  { sourceId: "github", nameZh: "官方源", nameEn: "Official Source" },
] as const;

type RuntimeResourceId = "python" | "ffmpeg" | "model";

export interface RuntimeResourceRow {
  id: RuntimeResourceId;
  name: string;
  percent: number;
  statusLabel: string;
  actionLabel: string;
  disabled: boolean;
}

export function useRuntimePanel(
  store: MeetingStore,
  messages: MessageTree["settings"],
  shellMessages: MessageTree["shell"],
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
  const runtimeSelectedSourceId = store.settings.runtimeDownloadSource.trim();
  const runtimeDownloadSourceOptions = runtimeDownloadSources.map((source) => ({
    id: source.sourceId,
    label: store.settings.locale === "en-US" ? source.nameEn : source.nameZh,
  }));
  const runtimeSourceRequired = !runtimeSelectedSourceId;
  const runtimeResourceRows = runtimeResources(
    runtimeStatus,
    runtimeInstallLog,
    messages,
    runtimeSourceRequired,
    runtimeBusy,
  );

  async function refreshRuntimePanel() {
    await store.refreshRuntimeStatus();
    await store.refreshRuntimeInstallLog();
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
    runtimeSelectedSourceId,
    runtimeDownloadSourceOptions,
    runtimeSourceRequired,
    runtimeResourceRows,
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

function runtimeResources(
  status: ManagedRuntimeStatus,
  log: string,
  messages: MessageTree["settings"],
  sourceRequired: boolean,
  runtimeBusy: boolean,
): RuntimeResourceRow[] {
  return [
    runtimeResource("python", messages.runtimeResourcePython, status, log, messages, sourceRequired, runtimeBusy),
    runtimeResource("ffmpeg", messages.runtimeResourceFfmpeg, status, log, messages, sourceRequired, runtimeBusy),
    runtimeResource("model", messages.runtimeResourceFunasr, status, log, messages, sourceRequired, runtimeBusy),
  ];
}

function runtimeResource(
  id: RuntimeResourceId,
  name: string,
  status: ManagedRuntimeStatus,
  log: string,
  messages: MessageTree["settings"],
  sourceRequired: boolean,
  runtimeBusy: boolean,
): RuntimeResourceRow {
  const completed = isResourceCompleted(id, status, log);
  const active = isResourceActive(id, status, log);
  const progress = active ? activeResourceProgress(log) : 0;
  const percent = completed ? 100 : progress;
  const disabled = sourceRequired || runtimeBusy;

  return {
    id,
    name,
    percent,
    statusLabel: resourceStatusLabel(status, messages, sourceRequired, completed, active),
    actionLabel: completed ? messages.runtimeResourceRedownload : messages.runtimeResourceDownload,
    disabled,
  };
}

function resourceStatusLabel(
  status: ManagedRuntimeStatus,
  messages: MessageTree["settings"],
  sourceRequired: boolean,
  completed: boolean,
  active: boolean,
) {
  if (sourceRequired) {
    return messages.runtimeDownloadSourceRequired;
  }

  if (completed) {
    return messages.runtimeResourceReady;
  }

  if (status.status === "unsupported") {
    return messages.runtimeStatusUnsupported;
  }

  if (status.status === "failed" || status.status === "repair_required") {
    return messages.runtimeResourceFailed;
  }

  if (active) {
    return messages.runtimeResourceDownloading;
  }

  return messages.runtimeResourcePending;
}

function activeResourceProgress(log: string) {
  const lastStageProgress = Array.from(
    log.matchAll(/\[runtime\] (?:staging|download) progress .*?\(([\d.]+)%\)/g),
  ).at(-1)?.[1];

  if (!lastStageProgress) {
    return 18;
  }

  return Math.max(18, Math.min(96, Math.round(Number(lastStageProgress))));
}

function isResourceCompleted(id: RuntimeResourceId, status: ManagedRuntimeStatus, log: string) {
  if (status.status === "ready" || log.includes("[runtime] install completed.")) {
    return true;
  }

  if (id === "python") {
    return log.includes("[runtime] resolved python=") || log.includes("Validating Python runtime");
  }

  if (id === "ffmpeg") {
    return log.includes("[runtime] resolved ffmpeg=") || log.includes("Validating ffmpeg runtime");
  }

  return false;
}

function isResourceActive(id: RuntimeResourceId, status: ManagedRuntimeStatus, log: string) {
  if (status.status !== "installing") {
    return false;
  }

  if (id === "python") {
    return !isResourceCompleted("python", status, log);
  }

  if (id === "ffmpeg") {
    return isResourceCompleted("python", status, log) && !isResourceCompleted("ffmpeg", status, log);
  }

  return isResourceCompleted("ffmpeg", status, log) && !isResourceCompleted("model", status, log);
}
