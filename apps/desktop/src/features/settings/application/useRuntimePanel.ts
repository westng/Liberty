import { useMemo } from "react";
import type { MessageTree } from "@/shared/i18n";
import type {
  ManagedRuntimeStatus,
  RuntimeComponentState,
  RuntimeOperationKind,
  RuntimeSource,
} from "@/shared/types/meeting";
import type { MeetingStore } from "@/features/meeting/stores/useMeetingStore";

export type RuntimeResourceId = "python" | "ffmpeg" | "model";
export type RuntimeResourceSource = RuntimeSource;

export interface RuntimeResourceRow {
  id: RuntimeResourceId;
  name: string;
  percent: number;
  statusLabel: string;
  actionLabel: string;
  actionKind: "detect" | "install";
  disabled: boolean;
  busy: boolean;
  indeterminate: boolean;
  source: RuntimeResourceSource;
  sourceSelectable: boolean;
}

export function useRuntimePanel(
  store: MeetingStore,
  messages: MessageTree["settings"],
  shellMessages: MessageTree["shell"],
) {
  const runtimeModeLabel = store.localMode
    ? shellMessages.localMode
    : shellMessages.remoteMode;
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
    : runtimeStatus.status === "ready" || runtimeStatus.status === "system_ready"
      ? messages.runtimeReinstallAction
      : runtimeStatus.status === "installing"
        ? messages.runtimeStatusInstalling
        : messages.runtimeInstallAction;
  const runtimeStatusLabel = labelForRuntimeStatus(runtimeStatus, messages);
  const runtimeStatusDescription = runtimeDescription(runtimeStatus, messages);
  const runtimeBusy = runtimeStatus.status === "unsupported";
  const runtimeSelectedSourceId = store.settings.runtimeDownloadSource.trim();
  const runtimeDownloadSourceOptions = store.runtimeDownloadSources;
  const runtimeSourceRequired = runtimeDownloadSourceOptions.length > 0 && !runtimeSelectedSourceId;
  const runtimeResourceRows = runtimeResources(
    runtimeStatus,
    messages,
    runtimeSourceRequired,
    store.settings.pythonRuntimeSource,
    store.settings.ffmpegRuntimeSource,
  );

  async function refreshRuntimePanel() {
    await store.refreshRuntimeDownloadSources();
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
    case "system_ready":
      return messages.runtimeStatusSystemReady;
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
    case "system_ready":
      return messages.runtimeDescriptionSystemReady;
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
  messages: MessageTree["settings"],
  sourceRequired: boolean,
  pythonSource: RuntimeSource,
  ffmpegSource: RuntimeSource,
): RuntimeResourceRow[] {
  return [
    runtimeResource("python", messages.runtimeResourcePython, status.python, messages, sourceRequired, pythonSource),
    runtimeResource("ffmpeg", messages.runtimeResourceFfmpeg, status.ffmpeg, messages, sourceRequired, ffmpegSource),
    runtimeResource("model", messages.runtimeResourceFunasr, status.models, messages, sourceRequired, "managed"),
  ];
}

function runtimeResource(
  id: RuntimeResourceId,
  name: string,
  component: RuntimeComponentState,
  messages: MessageTree["settings"],
  sourceRequired: boolean,
  selectedSource: RuntimeSource,
): RuntimeResourceRow {
  const sourceSelectable = id !== "model";
  const source = sourceSelectable ? selectedSource : "managed";
  const busy = isOperationActive(component.operation.kind);
  const progress = component.operation.progress;
  const percent = component.availability === "ready" && !busy
    ? 100
    : progress ?? (busy ? 36 : 0);
  const indeterminate = busy && progress === undefined;
  const actionKind = source === "system" ? "detect" : "install";
  const disabled = busy || (source === "managed" && sourceRequired);

  return {
    id,
    name,
    percent,
    statusLabel: componentStatusLabel(component, source, messages, sourceRequired),
    actionLabel: actionKind === "detect"
      ? component.availability === "ready"
        ? messages.runtimeResourceDetectAgain
        : messages.runtimeResourceDetect
      : component.availability === "ready"
        ? messages.runtimeResourceRedownload
        : messages.runtimeResourceDownload,
    actionKind,
    disabled,
    busy,
    indeterminate,
    source,
    sourceSelectable,
  };
}

function componentStatusLabel(
  component: RuntimeComponentState,
  source: RuntimeSource,
  messages: MessageTree["settings"],
  sourceRequired: boolean,
) {
  if (component.availability === "unsupported") {
    return messages.runtimeStatusUnsupported;
  }
  switch (component.operation.kind) {
    case "detecting":
      return messages.runtimeResourceDetecting;
    case "waiting_for_python":
      return messages.runtimeResourceWaitingPython;
    case "downloading":
    case "installing":
      return messages.runtimeResourceDownloading;
    case "validating":
      return messages.runtimeResourceValidating;
    case "failed":
      return component.operation.lastError?.trim()
        || (source === "system"
          ? messages.runtimeResourceNotDetected
          : messages.runtimeResourceFailed);
    default:
      break;
  }
  if (component.availability === "ready") {
    return source === "system"
      ? messages.runtimeResourceSystemSelected
      : messages.runtimeResourceReady;
  }
  if (source === "managed" && sourceRequired) {
    return messages.runtimeDownloadSourceRequired;
  }
  return source === "system"
    ? messages.runtimeResourceNotDetected
    : messages.runtimeResourcePending;
}

function isOperationActive(kind: RuntimeOperationKind) {
  return kind !== "idle" && kind !== "failed";
}
