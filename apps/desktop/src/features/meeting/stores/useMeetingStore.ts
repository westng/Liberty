import { useSyncExternalStore } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  defaultSettings,
  isManagedRuntimeReady,
  normalizeSettings,
} from "@/features/meeting/application/settingsPolicy";
import { hasActiveJobs, mergeJobSnapshot } from "@/features/meeting/application/jobSnapshots";
import { createPollingScheduler } from "@/features/meeting/application/polling";
import { createJobQueryController } from "@/features/meeting/application/JobQueryController";
import { createRemoteCapabilitySession } from "@/features/meeting/application/RemoteCapabilitySession";
import { createRuntimeInstallController } from "@/features/meeting/application/RuntimeInstallController";
import { createSettingsSaveCoordinator } from "@/features/meeting/application/SettingsSaveCoordinator";
import { applyAppearance } from "@/shared/services/ui/appearance";
import { runAppStatusAction } from "@/shared/services/ui/statusNotifications";
import { publishEntityChanged } from "@/shared/services/ui/windows";
import { createLocalAiService } from "@/shared/services/tauri/ai";
import { createLocalMeetingService } from "@/shared/services/tauri/meeting";
import { applyLocalPetWorkflowEvent } from "@/shared/services/tauri/pet";
import { createLocalRuntimeService } from "@/shared/services/tauri/runtime";
import {
  createLocalSettingsService,
  SettingsConflictError,
} from "@/shared/services/tauri/settings";
import {
  createMeetingApi,
  type RemoteMeetingCapabilities,
  type RemoteMeetingOperation,
} from "@/shared/services/remote/meetingApi";
import type {
  ManagedRuntimeStatus,
  MeetingJob,
  MeetingJobRef,
  NewMeetingJobInput,
  ProcessingMode,
  RuntimeComponentId,
  RuntimeComponentState,
  RuntimeSource,
  SettingsCredentialUpdate,
  SettingsState,
} from "@/shared/types/meeting";
import type { RuntimeDownloadSourceOption } from "@/shared/services/tauri/runtime";

type MeetingState = {
  jobs: MeetingJob[];
  settings: SettingsState;
  runtimeStatus: ManagedRuntimeStatus;
  runtimeInstallLog: string;
  runtimeDownloadSources: RuntimeDownloadSourceOption[];
  settingsLoaded: boolean;
  settingsLoadError: string | null;
  remoteStatus: "idle" | "checking" | "ready" | "unavailable";
  remoteCapabilities: RemoteMeetingCapabilities | null;
  remoteError: string | null;
};

type SettingsSaveIntent = {
  base: SettingsState;
  next: SettingsState;
  credential: SettingsCredentialUpdate;
};

const CLIENT_REMOTE_OPERATIONS = new Set<RemoteMeetingOperation>([
  "jobs.list",
  "jobs.read",
  "jobs.result.read",
  "jobs.retry",
  "jobs.delete",
  "transcript.speakers.rename",
]);

const initialRuntimeStatus: ManagedRuntimeStatus = {
  platformId: "",
  runtimeVersion: "",
  pythonVersion: "",
  status: "missing",
  updatedAt: "",
  python: initialRuntimeComponent("python", "managed"),
  ffmpeg: initialRuntimeComponent("ffmpeg", "managed"),
  models: initialRuntimeComponent("model"),
  shellReady: false,
};

function initialRuntimeComponent(
  component: RuntimeComponentId,
  source?: RuntimeSource,
): RuntimeComponentState {
  return {
    component,
    source,
    availability: "unavailable" as const,
    operation: {
      kind: "idle" as const,
      generation: 0,
      phase: "idle",
    },
    updatedAt: "",
  };
}

let state: MeetingState = {
  jobs: [],
  settings: { ...defaultSettings },
  runtimeStatus: initialRuntimeStatus,
  runtimeInstallLog: "",
  runtimeDownloadSources: [],
  settingsLoaded: false,
  settingsLoadError: null,
  remoteStatus: "idle",
  remoteCapabilities: null,
  remoteError: null,
};

const listeners = new Set<() => void>();
const localAiService = createLocalAiService();
const localMeetingService = createLocalMeetingService();
const localRuntimeService = createLocalRuntimeService();
const localSettingsService = createLocalSettingsService();
const remoteMeetingApi = createMeetingApi();
let settingsLoadPromise: Promise<void> | null = null;
let globalServicesPromise: Promise<void> | null = null;
let globalServicesInitialized = false;
const hydratedJobIds: Record<ProcessingMode, Set<string>> = {
  local: new Set<string>(),
  remote: new Set<string>(),
};
const localJobPolling = createPollingScheduler();
let globalPollingEnabled = false;
const jobQueries = createJobQueryController();

const REMOTE_HANDSHAKE_RETRY_DELAYS_MS = [2_000, 5_000, 15_000] as const;

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function setState(patch: Partial<MeetingState>) {
  state = { ...state, ...patch };
  for (const listener of listeners) {
    listener();
  }
}

function getSnapshot() {
  return state;
}

function isMainWindow() {
  return getCurrentWindow().label === "main";
}

function capabilityUnavailable(message: string) {
  const error = new Error(`capability_unavailable: ${message}`);
  error.name = "CapabilityUnavailableError";
  return error;
}

function getLocalMode() {
  return state.settings.processingMode === "local";
}

function invalidateJobSource(source: ProcessingMode, clearHydratedJobs = false) {
  jobQueries.invalidateSource(source);
  if (clearHydratedJobs) {
    hydratedJobIds[source].clear();
  }
}

function resetRemoteConnection() {
  remoteCapabilities.reset();
  setState({
    remoteStatus: "idle",
    remoteCapabilities: null,
    remoteError: null,
  });
  syncJobPolling();
}

function remoteErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

async function requestRemoteCapabilities(force = false, resetRetryBudget = false) {
  await ensureSettingsLoaded();
  if (!isMainWindow()) {
    throw capabilityUnavailable("独立窗口不允许连接远端会议服务。");
  }
  if (resetRetryBudget) {
    remoteCapabilities.reset();
  }
  if (!state.settings.backendUrl.trim()) {
    remoteCapabilities.reset();
    const message = "capability_unavailable: 远端模式未配置在线后端地址。";
    setState({
      remoteStatus: "unavailable",
      remoteCapabilities: null,
      remoteError: message,
    });
    syncJobPolling();
    throw new Error(message);
  }
  return remoteCapabilities.request(force, resetRetryBudget);
}

async function ensureRemoteCapabilities(force = false) {
  return requestRemoteCapabilities(force, force);
}

async function runRemoteOperation<T>(
  operation: RemoteMeetingOperation,
  request: (capabilities: RemoteMeetingCapabilities) => Promise<T>,
): Promise<T>;
async function runRemoteOperation<T>(
  operation: RemoteMeetingOperation,
  request: (capabilities: RemoteMeetingCapabilities) => Promise<T>,
  isCurrent: () => boolean,
): Promise<T | undefined>;
async function runRemoteOperation<T>(
  operation: RemoteMeetingOperation,
  request: (capabilities: RemoteMeetingCapabilities) => Promise<T>,
  isCurrent?: () => boolean,
): Promise<T | undefined> {
  const capabilities = await requireRemoteOperation(operation);
  if (isCurrent && !isCurrent()) {
    return undefined;
  }
  const requestSequence = remoteCapabilities.generation();
  try {
    return await request(capabilities);
  } catch (error) {
    remoteCapabilities.degrade(error, requestSequence);
    throw error;
  }
}

async function requireRemoteOperation(operation: RemoteMeetingOperation) {
  const capabilities = await ensureRemoteCapabilities();
  if (
    !CLIENT_REMOTE_OPERATIONS.has(operation)
    || !capabilities.operations.includes(operation)
  ) {
    throw capabilityUnavailable(`远端服务当前不支持 ${operation} 操作。`);
  }
  return capabilities;
}

function canRemoteOperation(operation: RemoteMeetingOperation) {
  return CLIENT_REMOTE_OPERATIONS.has(operation)
    && state.remoteStatus === "ready"
    && Boolean(state.remoteCapabilities?.operations.includes(operation));
}

function requireJobSource(job: MeetingJob) {
  if (job.source !== "local" && job.source !== "remote") {
    throw new Error("任务缺少明确的数据源，请刷新任务列表后重试。");
  }
  return job.source;
}

function withJobSource(job: MeetingJob, source: "local" | "remote"): MeetingJob {
  return { ...job, source };
}

async function refreshPolledJobs() {
  try {
    await refreshJobs();
  } catch {
    // Keep the last known state when background polling fails.
  }
}

function applyJobListSnapshot(incomingJobs: MeetingJob[], source: "local" | "remote") {
  const existingById = new Map(
    state.jobs
      .filter((job) => job.source === source)
      .map((job) => [job.id, job]),
  );
  setState({
    jobs: [
      ...incomingJobs.map((job) => mergeJobSnapshot(
        existingById.get(job.id),
        withJobSource(job, source),
        hydratedJobIds[source],
      )),
      ...state.jobs.filter((job) => job.source !== source),
    ],
  });
  syncJobPolling();
}

function syncJobPolling() {
  const shouldPoll = globalPollingEnabled
    && state.settingsLoaded
    && (
      state.settings.processingMode === "local"
      || canRemoteOperation("jobs.list")
    );
  const pollingIntervalMs = hasActiveJobs(
    state.jobs.filter((job) => job.source === state.settings.processingMode),
  ) ? 1500 : 15000;
  localJobPolling.sync(shouldPoll, pollingIntervalMs, () => {
    return refreshPolledJobs();
  });
}

function setGlobalEffectsEnabled(enabled: boolean) {
  globalPollingEnabled = enabled;
  remoteCapabilities.setEnabled(enabled);
  syncJobPolling();
  syncRuntimePolling();
  if (enabled) {
    void ensureGlobalServicesInitialized().catch(() => undefined);
  }
}

function syncRuntimePolling() {
  runtimeInstall.sync(globalPollingEnabled && state.settingsLoaded, state.runtimeStatus);
}

function settingsLoadFailure(error: unknown): Error {
  const detail = error instanceof Error ? error.message : String(error);
  const message = `完整设置加载失败，已阻止任务操作：${detail}`;
  setState({ settingsLoaded: false, settingsLoadError: message });
  globalServicesInitialized = false;
  syncJobPolling();
  syncRuntimePolling();
  return new Error(message);
}

async function loadMainSettingsSnapshot() {
  try {
    return normalizeSettings(await localSettingsService.getSettings());
  } catch (error) {
    throw settingsLoadFailure(error);
  }
}

async function ensureSettingsLoaded(force = false) {
  if (state.settingsLoaded && !force) {
    return;
  }

  if (settingsLoadPromise) {
    return settingsLoadPromise;
  }

  if (isMainWindow() && state.settingsLoadError && !force) {
    throw new Error(state.settingsLoadError);
  }

  if (force) {
    setState({ settingsLoaded: false, settingsLoadError: null });
  }

  settingsLoadPromise = (async () => {
    if (isMainWindow()) {
      const settings = await loadMainSettingsSnapshot();
      setState({ settings, settingsLoaded: true, settingsLoadError: null });
    } else {
      try {
        const preferences = await localSettingsService.getUiPreferences();
        setState({
          settings: normalizeSettings({ ...defaultSettings, ...preferences }),
          settingsLoaded: true,
          settingsLoadError: null,
        });
      } catch {
        setState({
          settings: normalizeSettings({ ...defaultSettings }),
          settingsLoaded: true,
          settingsLoadError: null,
        });
      }
    }

    applyAppearance(state.settings);
    syncJobPolling();
    syncRuntimePolling();
  })().finally(() => {
    settingsLoadPromise = null;
  });

  return settingsLoadPromise;
}

async function ensureGlobalServicesInitialized() {
  if (!globalPollingEnabled || globalServicesInitialized) {
    return;
  }
  if (globalServicesPromise) {
    return globalServicesPromise;
  }

  globalServicesPromise = (async () => {
    await ensureSettingsLoaded();
    if (!globalPollingEnabled) {
      return;
    }
    await refreshRuntimeStatus();
    await refreshRuntimeDownloadSources();
    await detectSelectedSystemComponents();
    if (state.settings.processingMode === "remote") {
      await ensureRemoteCapabilities();
    }
    await refreshJobs();
    globalServicesInitialized = true;
    syncJobPolling();
    syncRuntimePolling();
  })().finally(() => {
    globalServicesPromise = null;
  });

  return globalServicesPromise;
}

async function detectSelectedSystemComponents() {
  for (const component of ["python", "ffmpeg"] as const) {
    const source = component === "python"
      ? state.settings.pythonRuntimeSource
      : state.settings.ffmpegRuntimeSource;
    const componentState = state.runtimeStatus[component];
    if (
      source === "system"
      && componentState.availability !== "ready"
      && componentState.operation.kind !== "detecting"
    ) {
      setState({ runtimeStatus: await localRuntimeService.detectComponent(component) });
    }
  }
}

function replaceJob(job: MeetingJob) {
  setState({
    jobs: [
      job,
      ...state.jobs.filter((item) => item.id !== job.id || item.source !== job.source),
    ],
  });
  return job;
}

async function refreshRuntimeStatus() {
  const previousStatus = state.runtimeStatus;
  try {
    const runtimeStatus = await localRuntimeService.getStatus();
    const detectionFinished = (["python", "ffmpeg"] as const).some((component) =>
      previousStatus[component].operation.kind === "detecting"
      && runtimeStatus[component].operation.kind !== "detecting");
    if (detectionFinished && isMainWindow()) {
      const settings = await loadMainSettingsSnapshot();
      setState({ runtimeStatus, settings });
    } else {
      setState({ runtimeStatus });
    }
  } catch {
    setState({ runtimeStatus: initialRuntimeStatus });
  }

  syncJobPolling();
  syncRuntimePolling();

  return state.runtimeStatus;
}

async function refreshRuntimeInstallLog() {
  try {
    setState({ runtimeInstallLog: await localRuntimeService.getInstallLog() });
  } catch {
    setState({ runtimeInstallLog: "" });
  }

  return state.runtimeInstallLog;
}

async function refreshRuntimeDownloadSources() {
  try {
    setState({ runtimeDownloadSources: await localRuntimeService.listDownloadSources() });
  } catch {
    setState({ runtimeDownloadSources: [] });
  }

  return state.runtimeDownloadSources;
}

async function beginManagedRuntimeInstall() {
  return runtimeInstall.install();
}

const runtimeInstall = createRuntimeInstallController({
  pollingIntervalMs: 1500,
  isOperationActive: (status: ManagedRuntimeStatus) => [
    status.python,
    status.ffmpeg,
    status.models,
  ].some((component) => [
    "detecting",
    "downloading",
    "installing",
    "validating",
  ].includes(component.operation.kind)),
  install: async () => {
    setState({ runtimeStatus: await localRuntimeService.install() });
    await refreshRuntimeInstallLog();
    syncJobPolling();
    syncRuntimePolling();
    return state.runtimeStatus;
  },
  refresh: () => Promise.all([
    refreshRuntimeStatus(),
    refreshRuntimeInstallLog(),
  ]).then(() => undefined),
});

const remoteCapabilities = createRemoteCapabilitySession({
  retryDelaysMs: REMOTE_HANDSHAKE_RETRY_DELAYS_MS,
  connect: () => remoteMeetingApi.getCapabilities(),
  canRetry: () => globalPollingEnabled
    && state.settings.processingMode === "remote"
    && state.remoteStatus === "unavailable",
  cached: () => state.remoteStatus === "ready" ? state.remoteCapabilities : null,
  onChecking: () => {
    setState({ remoteStatus: "checking", remoteCapabilities: null, remoteError: null });
    syncJobPolling();
  },
  onReady: (capabilities) => {
    setState({ remoteStatus: "ready", remoteCapabilities: capabilities, remoteError: null });
    syncJobPolling();
  },
  onUnavailable: (error) => {
    setState({
      remoteStatus: "unavailable",
      remoteCapabilities: null,
      remoteError: remoteErrorMessage(error),
    });
    syncJobPolling();
  },
  onInvalidate: () => invalidateJobSource("remote"),
  onRetryReady: () => refreshJobs().then(() => undefined),
});

function sleep(ms: number) {
  return new Promise((resolve) => globalThis.setTimeout(resolve, ms));
}

async function refreshJobs() {
  await ensureSettingsLoaded();
  if (!isMainWindow()) {
    throw capabilityUnavailable("独立窗口不允许读取任务列表。");
  }
  const processingMode = state.settings.processingMode;
  const isCurrentRequest = jobQueries.beginList(processingMode);
  const isCurrent = () => processingMode === state.settings.processingMode && isCurrentRequest();
  const incomingJobs = processingMode === "local"
    ? await localMeetingService.listJobs()
    : await runRemoteOperation(
        "jobs.list",
        (capabilities) => remoteMeetingApi.listJobs(capabilities),
        isCurrent,
      );

  if (!incomingJobs || !isCurrent()) {
    return state.jobs;
  }

  applyJobListSnapshot(incomingJobs, processingMode);
  return state.jobs;
}

function resolveJobRef(reference: string | MeetingJobRef): {
  jobId: string;
  source: ProcessingMode;
  windowScopeToken?: string;
} {
  if (isMainWindow()) {
    if (typeof reference !== "string") {
      if (!reference.jobId || (reference.source !== "local" && reference.source !== "remote")) {
        throw new Error("任务引用无效。");
      }
      return reference;
    }
    const existing = getJobById(reference, state.settings.processingMode)
      ?? state.jobs.find((job) => job.id === reference);
    return {
      jobId: reference,
      source: existing?.source ?? state.settings.processingMode,
    };
  }

  const params = new URLSearchParams(window.location.search);
  const jobId = params.get("jobId") ?? "";
  const source = params.get("source");
  const windowScopeToken = params.get("scopeToken") ?? "";
  const requestedJobId = typeof reference === "string" ? reference : reference.jobId;
  const requestedSource = typeof reference === "string" ? source : reference.source;
  const requestedScopeToken = typeof reference === "string"
    ? windowScopeToken
    : reference.windowScopeToken;
  if (
    jobId !== requestedJobId
    || source !== requestedSource
    || !windowScopeToken
    || windowScopeToken !== requestedScopeToken
    || (source !== "local" && source !== "remote")
  ) {
    throw capabilityUnavailable("独立窗口需要有效的任务作用域。");
  }
  return { jobId, source, windowScopeToken };
}

async function refreshJobFromSource(reference: string | MeetingJobRef, resultOnly: boolean) {
  await ensureSettingsLoaded();
  const {
    jobId: id,
    source: explicitSource,
    windowScopeToken,
  } = resolveJobRef(reference);
  if (!isMainWindow() && explicitSource === "remote") {
    throw capabilityUnavailable("远端任务尚无安全的独立窗口后端代理。");
  }
  const source = explicitSource;
  const fence = jobQueries.beginRequest({ jobId: id, source });
  const applyRefreshedJob = (refreshed: MeetingJob) => {
    if (!jobQueries.isRequestCurrent(fence)) {
      return getJobById(id, source);
    }
    hydratedJobIds[source].add(id);
    return replaceJob(mergeJobSnapshot(
      getJobById(id, source),
      withJobSource(refreshed, source),
      hydratedJobIds[source],
    ));
  };

  if (source === "local") {
    const refreshed = resultOnly
      ? await localMeetingService.getJobResult(id, windowScopeToken)
      : await localMeetingService.getJob(id);
    return applyRefreshedJob(refreshed);
  }

  if (resultOnly) {
    await requireRemoteOperation("jobs.read");
  }
  const refreshed = resultOnly
    ? await runRemoteOperation(
        "jobs.result.read",
        (capabilities) => remoteMeetingApi.getResult(capabilities, id),
        () => jobQueries.isRequestCurrent(fence),
      )
    : await runRemoteOperation(
        "jobs.read",
        (capabilities) => remoteMeetingApi.getJob(capabilities, id),
        () => jobQueries.isRequestCurrent(fence),
      );
  if (!refreshed) {
    return getJobById(id, source);
  }
  return applyRefreshedJob(refreshed);
}

async function refreshJob(reference: string | MeetingJobRef) {
  return refreshJobFromSource(reference, false);
}

async function refreshJobResult(reference: string | MeetingJobRef) {
  return refreshJobFromSource(reference, true);
}

async function refreshJobRuns(reference: string | MeetingJobRef) {
  await ensureSettingsLoaded();

  await refreshJobResult(reference);
}

async function createJobOperation(input: NewMeetingJobInput) {
  await ensureSettingsLoaded();
  const sourceGeneration = jobQueries.sourceGeneration("local");

  if (getLocalMode() && !isManagedRuntimeReady(state.runtimeStatus)) {
    await ensureManagedRuntimeReadyForLocalWork();
  }

  if (getLocalMode()) {
    const firstFile = input.files[0];

    if (!firstFile?.path) {
      throw new Error("本地模式只支持带本地路径的单个文件。");
    }

    const created = await localMeetingService.createJob({
      ...input,
      files: [firstFile],
    });

    void applyLocalPetWorkflowEvent({
      eventType: "job_created",
      metadata: created.id,
    }).catch(() => undefined);

    syncJobPolling();
    const sourcedJob = withJobSource(created, "local");
    if (
      sourceGeneration === jobQueries.sourceGeneration("local")
      && state.settings.processingMode === "local"
    ) {
      hydratedJobIds.local.add(created.id);
      replaceJob(sourcedJob);
    }
    return sourcedJob;
  }

  await requireRemoteOperation("jobs.create");
  throw capabilityUnavailable("安全的远端分块上传协议尚未实现，已阻止创建任务。");
}

function resolveExistingJob(reference: MeetingJobRef) {
  const resolved = resolveJobRef(reference);
  const job = getJobById(resolved.jobId, resolved.source);
  if (!job) {
    throw new Error("没有找到这个任务。");
  }
  return { reference: resolved, job };
}

async function retryJobOperation(reference: MeetingJobRef) {
  await ensureSettingsLoaded();
  const { reference: jobRef, job } = resolveExistingJob(reference);
  const { jobId: id, source } = jobRef;
  requireJobSource(job);
  const fence = jobQueries.beginMutation(jobRef);

  if (source === "local" && !isManagedRuntimeReady(state.runtimeStatus)) {
    await ensureManagedRuntimeReadyForLocalWork();
  }
  if (!jobQueries.isMutationCurrent(fence)) {
    return;
  }

  if (source === "local") {
    const updated = await localMeetingService.retryJob(id);
    if (!jobQueries.commitMutation(fence)) {
      return;
    }
    syncJobPolling();
    hydratedJobIds[source].add(updated.id);
    return replaceJob(withJobSource(updated, source));
  }

  const updated = await runRemoteOperation(
    "jobs.retry",
    (capabilities) => remoteMeetingApi.retryJob(capabilities, id),
    () => jobQueries.isMutationCurrent(fence),
  );
  if (!updated || !jobQueries.commitMutation(fence)) {
    return;
  }
  hydratedJobIds[source].add(updated.id);
  return replaceJob(withJobSource(updated, source));
}

async function deleteJobOperation(reference: MeetingJobRef) {
  await ensureSettingsLoaded();
  const { reference: jobRef, job } = resolveExistingJob(reference);
  const { jobId: id, source } = jobRef;
  requireJobSource(job);
  const fence = jobQueries.beginMutation(jobRef);

  if (source === "local") {
    await localMeetingService.deleteJob(id);
  } else {
    const completed = await runRemoteOperation(
      "jobs.delete",
      async (capabilities) => {
        await remoteMeetingApi.deleteJob(capabilities, id);
        return true;
      },
      () => jobQueries.isMutationCurrent(fence),
    );
    if (!completed) {
      return false;
    }
  }

  if (!jobQueries.commitMutation(fence)) {
    return false;
  }
  hydratedJobIds[source].delete(id);
  setState({
    jobs: state.jobs.filter((item) => item.id !== id || item.source !== source),
  });
  return true;
}

async function renameSpeakerOperation(
  reference: MeetingJobRef,
  fromSpeaker: string,
  toSpeaker: string,
) {
  await ensureSettingsLoaded();
  const { reference: jobRef, job } = resolveExistingJob(reference);
  const { jobId: id, source } = jobRef;
  requireJobSource(job);
  const fence = jobQueries.beginMutation(jobRef);

  const normalizedTarget = toSpeaker.trim();

  if (!normalizedTarget) {
    throw new Error("讲话人名称不能为空。");
  }

  if (source === "local") {
    const updated = await localMeetingService.renameSpeaker(id, fromSpeaker, normalizedTarget);
    if (!jobQueries.commitMutation(fence)) {
      return;
    }
    hydratedJobIds[source].add(updated.id);
    return replaceJob(withJobSource(updated, source));
  }

  const updated = await runRemoteOperation(
    "transcript.speakers.rename",
    (capabilities) => remoteMeetingApi.renameSpeaker(
      capabilities,
      id,
      fromSpeaker,
      normalizedTarget,
    ),
    () => jobQueries.isMutationCurrent(fence),
  );
  if (!updated || !jobQueries.commitMutation(fence)) {
    return;
  }
  hydratedJobIds[source].add(updated.id);
  return replaceJob(withJobSource(updated, source));
}

function rebaseSettingsIntent(intent: SettingsSaveIntent, current: SettingsState) {
  const choose = <Key extends keyof SettingsState>(key: Key): SettingsState[Key] =>
    Object.is(intent.base[key], intent.next[key]) ? current[key] : intent.next[key];

  return normalizeSettings({
    ...current,
    themeMode: choose("themeMode"),
    liquidGlassStyle: choose("liquidGlassStyle"),
    accentColor: choose("accentColor"),
    locale: choose("locale"),
    backendUrl: choose("backendUrl"),
    processingMode: choose("processingMode"),
    defaultHotwords: choose("defaultHotwords"),
    summaryTemplate: choose("summaryTemplate"),
    concurrency: choose("concurrency"),
    localAsrDevice: choose("localAsrDevice"),
    localAsrThreads: choose("localAsrThreads"),
    localAsrBatchSizeSeconds: choose("localAsrBatchSizeSeconds"),
    runtimeDownloadSource: choose("runtimeDownloadSource"),
    pythonPath: current.pythonPath,
    ffmpegPath: current.ffmpegPath,
    pythonRuntimeSource: current.pythonRuntimeSource,
    ffmpegRuntimeSource: current.ffmpegRuntimeSource,
    runnerScriptPath: current.runnerScriptPath,
    apiTokenConfigured: current.apiTokenConfigured,
    settingsRevision: current.settingsRevision,
  });
}

async function saveSettingsOperation(intent: SettingsSaveIntent) {
  await ensureSettingsLoaded();
  let current = state.settings;

  for (let attempt = 0; attempt < 4; attempt += 1) {
    const normalized = rebaseSettingsIntent(intent, current);
    try {
      const saved = await localSettingsService.saveSettings(normalized, intent.credential);
      await applySettingsSnapshot(saved, intent.credential.action !== "keep");
      return state.settings;
    } catch (error) {
      if (!(error instanceof SettingsConflictError)) {
        throw error;
      }
      current = normalizeSettings(error.current);
      await applySettingsSnapshot(error.current);
      if (attempt === 3) {
        throw error;
      }
    }
  }

  return state.settings;
}

async function applySettingsSnapshot(
  snapshot: SettingsState,
  remoteCredentialChanged = false,
) {
  const persisted = normalizeSettings(snapshot);
  const dataSourceChanged = persisted.processingMode !== state.settings.processingMode
    || persisted.backendUrl !== state.settings.backendUrl
    || persisted.apiTokenConfigured !== state.settings.apiTokenConfigured
    || remoteCredentialChanged;
  if (dataSourceChanged) {
    invalidateJobSource("local", true);
    invalidateJobSource("remote", true);
    resetRemoteConnection();
  }
  setState({ settings: persisted, ...(dataSourceChanged ? { jobs: [] } : {}) });
  applyAppearance(persisted);
  await refreshRuntimeStatus();
  syncJobPolling();
  syncRuntimePolling();

  if (persisted.processingMode === "remote") {
    void ensureRemoteCapabilities()
      .then(() => refreshJobs())
      .catch(() => undefined);
  } else {
    void refreshJobs().catch(() => undefined);
  }
}

async function installManagedRuntimeOperation() {
  await ensureSettingsLoaded();
  return beginManagedRuntimeInstall();
}

async function setRuntimeComponentSourceOperation(component: "python" | "ffmpeg", source: RuntimeSource) {
  await ensureSettingsLoaded();
  const runtimeStatus = await localRuntimeService.setComponentSource(component, source);
  const settings = await loadMainSettingsSnapshot();
  setState({
    runtimeStatus,
    settings,
  });
  syncRuntimePolling();
  return runtimeStatus;
}

async function detectRuntimeComponentOperation(component: "python" | "ffmpeg") {
  await ensureSettingsLoaded();
  setState({ runtimeStatus: await localRuntimeService.detectComponent(component) });
  syncRuntimePolling();
  return state.runtimeStatus;
}

async function installRuntimeComponentOperation(component: RuntimeComponentId) {
  await ensureSettingsLoaded();
  setState({ runtimeStatus: await localRuntimeService.installComponent(component) });
  await refreshRuntimeInstallLog();
  syncRuntimePolling();
  return state.runtimeStatus;
}

function createJob(input: NewMeetingJobInput) {
  return runAppStatusAction("createJob", async () => {
    const job = await createJobOperation(input);
    if (getJobById(job.id, job.source)) {
      await publishEntityChanged({ entity: "job", id: job.id, action: "saved" }).catch(() => undefined);
    }
    return job;
  });
}

function retryJob(reference: MeetingJobRef) {
  return runAppStatusAction("retryJob", async () => {
    const job = await retryJobOperation(reference);
    if (job) {
      await publishEntityChanged({ entity: "job", id: job.id, action: "saved" }).catch(() => undefined);
    }
    return job;
  });
}

function deleteJob(reference: MeetingJobRef) {
  return runAppStatusAction("deleteJob", async () => {
    const deleted = await deleteJobOperation(reference);
    if (deleted) {
      await publishEntityChanged({ entity: "job", id: reference.jobId, action: "deleted" }).catch(() => undefined);
    }
  });
}

function renameSpeaker(
  reference: MeetingJobRef,
  fromSpeaker: string,
  toSpeaker: string,
) {
  return runAppStatusAction("renameSpeaker", async () => {
    const job = await renameSpeakerOperation(reference, fromSpeaker, toSpeaker);
    if (job) {
      await publishEntityChanged({ entity: "job", id: job.id, action: "saved" }).catch(() => undefined);
    }
    return job;
  });
}

function saveSettings(
  next: SettingsState,
  credential: SettingsCredentialUpdate = { action: "keep" },
) {
  const base = settingsSaves.pendingCount() === 0
    ? state.settings
    : settingsSaves.projected();
  const intent: SettingsSaveIntent = {
    base,
    next: normalizeSettings(next),
    credential,
  };
  return settingsSaves.enqueue(intent);
}

const settingsSaves = createSettingsSaveCoordinator({
  current: () => state.settings,
  project: rebaseSettingsIntent,
  execute: (intent: SettingsSaveIntent) => runAppStatusAction(
    "saveSettings",
    () => saveSettingsOperation(intent),
  ),
});

function installManagedRuntime() {
  return runAppStatusAction("downloadRuntime", installManagedRuntimeOperation, { completedAsStarted: true });
}

function setRuntimeComponentSource(component: "python" | "ffmpeg", source: RuntimeSource) {
  return runAppStatusAction(
    "updateRuntimeSource",
    () => setRuntimeComponentSourceOperation(component, source),
  );
}

function detectRuntimeComponent(component: "python" | "ffmpeg") {
  return runAppStatusAction("detectRuntime", () => detectRuntimeComponentOperation(component));
}

function installRuntimeComponent(component: RuntimeComponentId) {
  return runAppStatusAction(
    "downloadRuntime",
    () => installRuntimeComponentOperation(component),
    { completedAsStarted: true },
  );
}

async function ensureManagedRuntimeReadyForLocalWork() {
  const timeoutAt = Date.now() + 10 * 60 * 1000;

  if (state.runtimeStatus.status === "missing" || state.runtimeStatus.status === "repair_required") {
    await installManagedRuntime();
  }

  while (!isManagedRuntimeReady(state.runtimeStatus)) {
    if (state.runtimeStatus.status === "unsupported") {
      throw new Error(state.runtimeStatus.lastError || "当前平台不支持内置本地运行环境。");
    }

    if (state.runtimeStatus.status === "failed") {
      throw new Error(state.runtimeStatus.lastError || "本地运行环境自动安装失败。");
    }

    if (Date.now() > timeoutAt) {
      throw new Error("等待本地运行环境就绪超时，请稍后重试。");
    }

    await sleep(1500);
    await refreshRuntimeStatus();
  }
}

function getJobById(id: string, source?: ProcessingMode) {
  if (source) {
    return state.jobs.find((job) => job.id === id && job.source === source);
  }
  return state.jobs.find((job) =>
    job.id === id && job.source === state.settings.processingMode)
    ?? state.jobs.find((job) => job.id === id);
}

async function setActiveSummaryRun(reference: MeetingJobRef, runId: string) {
  await ensureSettingsLoaded();
  const { reference: jobRef, job } = resolveExistingJob(reference);
  const fence = jobQueries.beginMutation(jobRef);

  if (job && requireJobSource(job) === "local") {
    await localAiService.setActiveSummaryRun(
      "local",
      jobRef.jobId,
      runId,
      jobRef.windowScopeToken,
    );
    if (!jobQueries.commitMutation(fence)) {
      return;
    }
    await refreshJobRuns(jobRef);
    if (!jobQueries.isMutationCurrent(fence)) {
      return;
    }
    await publishEntityChanged({ entity: "summary", id: jobRef.jobId, action: "saved" }).catch(() => undefined);
    return;
  }

  throw new Error("远端总结版本选择接口尚不可用。");
}

async function deleteSummaryRun(reference: MeetingJobRef, runId: string) {
  await ensureSettingsLoaded();
  const { reference: jobRef, job } = resolveExistingJob(reference);
  const fence = jobQueries.beginMutation(jobRef);

  if (job && requireJobSource(job) === "local") {
    await localAiService.deleteSummaryRun(
      "local",
      jobRef.jobId,
      runId,
      jobRef.windowScopeToken,
    );
    if (!jobQueries.commitMutation(fence)) {
      return;
    }
    await refreshJobRuns(jobRef);
    if (!jobQueries.isMutationCurrent(fence)) {
      return;
    }
    await publishEntityChanged({ entity: "summary", id: jobRef.jobId, action: "deleted" }).catch(() => undefined);
    return;
  }

  throw new Error("远端总结版本删除接口尚不可用。");
}

const actions = {
  ensureSettingsLoaded,
  ensureRemoteCapabilities,
  canRemoteOperation,
  refreshRuntimeStatus,
  refreshRuntimeInstallLog,
  refreshRuntimeDownloadSources,
  refreshJobs,
  refreshJob,
  refreshJobResult,
  refreshJobRuns,
  createJob,
  deleteJob,
  installManagedRuntime,
  setRuntimeComponentSource,
  detectRuntimeComponent,
  installRuntimeComponent,
  renameSpeaker,
  retryJob,
  saveSettings,
  setActiveSummaryRun,
  deleteSummaryRun,
  getJobById,
  setGlobalEffectsEnabled,
};

export function useMeetingStore() {
  if (!state.settingsLoaded && !state.settingsLoadError) {
    void ensureSettingsLoaded().catch(() => undefined);
  }
  const snapshot = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);

  return {
    ...snapshot,
    localMode: snapshot.settings.processingMode === "local",
    ...actions,
  };
}

export type MeetingStore = ReturnType<typeof useMeetingStore>;
