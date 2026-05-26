import { useSyncExternalStore } from "react";
import {
  defaultSettings,
  hasManualPythonOverride,
  isManagedRuntimeReady,
  normalizeSettings,
  shouldAutoInstallManagedRuntime,
  shouldUseLocalDataSource,
} from "@/features/meeting/application/settingsPolicy";
import { hasActiveLocalJobs, mergeJobSnapshot } from "@/features/meeting/application/jobSnapshots";
import { createPollingScheduler } from "@/features/meeting/application/polling";
import { applyAppearance } from "@/shared/services/ui/appearance";
import { createEmptyMeetingSummary, summaryResultToMeetingSummary } from "@/shared/services/ai/storage";
import { createLocalAiService } from "@/shared/services/tauri/ai";
import { createLocalMeetingService } from "@/shared/services/tauri/meeting";
import { applyLocalPetWorkflowEvent } from "@/shared/services/tauri/pet";
import { createLocalRuntimeService } from "@/shared/services/tauri/runtime";
import { createLocalSettingsService } from "@/shared/services/tauri/settings";
import { createMeetingApi } from "@/shared/services/remote/meetingApi";
import type {
  AiSummaryRun,
  ManagedRuntimeStatus,
  MeetingJob,
  NewMeetingJobInput,
  SettingsState,
} from "@/shared/types/meeting";

type MeetingState = {
  jobs: MeetingJob[];
  settings: SettingsState;
  runtimeStatus: ManagedRuntimeStatus;
  runtimeInstallLog: string;
  settingsLoaded: boolean;
};

const initialRuntimeStatus: ManagedRuntimeStatus = {
  platformId: "",
  runtimeVersion: "",
  pythonVersion: "",
  status: "missing",
  updatedAt: "",
};

let state: MeetingState = {
  jobs: [],
  settings: { ...defaultSettings },
  runtimeStatus: initialRuntimeStatus,
  runtimeInstallLog: "",
  settingsLoaded: false,
};

const listeners = new Set<() => void>();
const localAiService = createLocalAiService();
const localRuntimeService = createLocalRuntimeService();
const localSettingsService = createLocalSettingsService();
let settingsLoadPromise: Promise<void> | null = null;
let runtimeInstallPromise: Promise<ManagedRuntimeStatus> | null = null;
let runtimeAutoInstallAttempted = false;
const hydratedJobIds = new Set<string>();
const localJobPolling = createPollingScheduler();
const runtimeInstallPolling = createPollingScheduler();

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

function getApi() {
  return state.settings.backendUrl
    ? createMeetingApi(state.settings.backendUrl, state.settings.apiToken)
    : null;
}

function getLocalMode() {
  return isManagedRuntimeReady(state.runtimeStatus) || hasManualPythonOverride(state.settings);
}

async function refreshLocalJobs() {
  try {
    applyJobListSnapshot(await createLocalMeetingService().listJobs());
  } catch {
    // Keep the last known local state when polling fails.
  }
}

function applyJobListSnapshot(incomingJobs: MeetingJob[]) {
  const existingById = new Map(state.jobs.map((job) => [job.id, job]));
  setState({
    jobs: incomingJobs.map((job) => mergeJobSnapshot(existingById.get(job.id), job, hydratedJobIds)),
  });
  syncLocalPolling();
}

function syncLocalPolling() {
  const shouldPoll = shouldUseLocalDataSource(state.settings);
  const pollingIntervalMs = hasActiveLocalJobs(state.jobs) ? 1500 : 15000;
  localJobPolling.sync(shouldPoll, pollingIntervalMs, () => {
    void refreshLocalJobs();
  });
}

function syncRuntimePolling() {
  const shouldPoll = state.runtimeStatus.status === "installing";
  runtimeInstallPolling.sync(shouldPoll, 1500, () => {
    void refreshRuntimeStatus();
    void refreshRuntimeInstallLog();
  });
}

async function ensureSettingsLoaded(force = false) {
  if (state.settingsLoaded && !force) {
    return;
  }

  if (settingsLoadPromise && !force) {
    return settingsLoadPromise;
  }

  settingsLoadPromise = (async () => {
    try {
      const loaded = await localSettingsService.getSettings();
      setState({ settings: normalizeSettings(loaded) });
    } catch {
      setState({ settings: normalizeSettings() });
    }

    await refreshRuntimeStatus();
    setState({ settingsLoaded: true });
    applyAppearance(state.settings);
    syncLocalPolling();
    syncRuntimePolling();
    maybeStartRuntimeAutoInstall();

    if (shouldUseLocalDataSource(state.settings)) {
      await refreshLocalJobs();
    }
  })().finally(() => {
    settingsLoadPromise = null;
  });

  return settingsLoadPromise;
}

function replaceJob(job: MeetingJob) {
  setState({ jobs: [job, ...state.jobs.filter((item) => item.id !== job.id)] });
  return job;
}

async function refreshRuntimeStatus() {
  try {
    setState({ runtimeStatus: await localRuntimeService.getStatus() });
  } catch {
    setState({ runtimeStatus: initialRuntimeStatus });
  }

  syncLocalPolling();
  syncRuntimePolling();
  maybeStartRuntimeAutoInstall();

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

function maybeStartRuntimeAutoInstall() {
  if (!state.settingsLoaded || runtimeAutoInstallAttempted) {
    return;
  }

  if (!shouldAutoInstallManagedRuntime(state.settings, state.runtimeStatus)) {
    return;
  }

  runtimeAutoInstallAttempted = true;
  void beginManagedRuntimeInstall().catch(() => {
    runtimeAutoInstallAttempted = false;
  });
}

async function beginManagedRuntimeInstall() {
  if (runtimeInstallPromise) {
    return runtimeInstallPromise;
  }

  runtimeInstallPromise = (async () => {
    setState({ runtimeStatus: await localRuntimeService.install() });
    await refreshRuntimeInstallLog();
    syncLocalPolling();
    syncRuntimePolling();
    return state.runtimeStatus;
  })().finally(() => {
    runtimeInstallPromise = null;
  });

  return runtimeInstallPromise;
}

function sleep(ms: number) {
  return new Promise((resolve) => globalThis.setTimeout(resolve, ms));
}

async function refreshJobs() {
  await ensureSettingsLoaded();

  if (shouldUseLocalDataSource(state.settings)) {
    applyJobListSnapshot(await createLocalMeetingService().listJobs());
    return state.jobs;
  }

  const api = getApi();
  if (!api) {
    return state.jobs;
  }

  try {
    setState({ jobs: await api.listJobs() });
    return state.jobs;
  } catch {
    return state.jobs;
  }
}

async function refreshJob(id: string) {
  await ensureSettingsLoaded();

  if (shouldUseLocalDataSource(state.settings)) {
    const refreshed = await createLocalMeetingService().getJob(id);
    hydratedJobIds.add(id);
    return replaceJob(refreshed);
  }

  const api = getApi();
  if (!api) {
    return getJobById(id);
  }

  try {
    const refreshed = await api.getJob(id);
    hydratedJobIds.add(id);
    return replaceJob(refreshed);
  } catch {
    return getJobById(id);
  }
}

async function refreshJobRuns(id: string) {
  await ensureSettingsLoaded();

  if (!shouldUseLocalDataSource(state.settings)) {
    return;
  }

  const refreshed = await createLocalMeetingService().getJob(id);
  hydratedJobIds.add(id);
  replaceJob(refreshed);
}

async function createJob(input: NewMeetingJobInput) {
  await ensureSettingsLoaded();

  if (!getLocalMode() && !getApi()) {
    await ensureManagedRuntimeReadyForLocalWork();
  }

  if (getLocalMode()) {
    const firstFile = input.files[0];

    if (!firstFile?.path) {
      throw new Error("本地模式只支持带本地路径的单个文件。");
    }

    const created = await createLocalMeetingService().createJob({
      ...input,
      files: [firstFile],
    });

    void applyLocalPetWorkflowEvent({
      eventType: "job_created",
      metadata: created.id,
    }).catch(() => undefined);

    syncLocalPolling();
    hydratedJobIds.add(created.id);
    return replaceJob(created);
  }

  const api = getApi();
  if (api) {
    const created = await api.createJob(input);
    void applyLocalPetWorkflowEvent({
      eventType: "job_created",
      metadata: created.id,
    }).catch(() => undefined);
    hydratedJobIds.add(created.id);
    return replaceJob(created);
  }

  throw new Error("当前未安装本地运行环境，也未配置在线后端，无法创建任务。");
}

async function retryJob(id: string) {
  await ensureSettingsLoaded();
  const job = state.jobs.find((item) => item.id === id);

  if (!job) {
    return;
  }

  if (!getLocalMode() && !getApi()) {
    await ensureManagedRuntimeReadyForLocalWork();
  }

  if (getLocalMode()) {
    const updated = await createLocalMeetingService().retryJob(id);
    syncLocalPolling();
    hydratedJobIds.add(updated.id);
    return replaceJob(updated);
  }

  const api = getApi();
  if (api) {
    const updated = await api.retryJob(id);
    return replaceJob(updated);
  }

  throw new Error("当前未安装本地运行环境，也未配置在线后端，无法重试任务。");
}

async function deleteJob(id: string) {
  await ensureSettingsLoaded();

  if (getLocalMode()) {
    await createLocalMeetingService().deleteJob(id);
  }

  hydratedJobIds.delete(id);
  setState({ jobs: state.jobs.filter((job) => job.id !== id) });
}

async function renameSpeaker(id: string, fromSpeaker: string, toSpeaker: string) {
  await ensureSettingsLoaded();
  const job = state.jobs.find((item) => item.id === id);

  if (!job) {
    throw new Error("没有找到这个任务。");
  }

  const normalizedTarget = toSpeaker.trim();

  if (!normalizedTarget) {
    throw new Error("讲话人名称不能为空。");
  }

  if (getLocalMode()) {
    const updated = await createLocalMeetingService().renameSpeaker(id, fromSpeaker, normalizedTarget);
    hydratedJobIds.add(updated.id);
    return replaceJob(updated);
  }

  const normalizedSource = fromSpeaker.trim();
  const updateSegments = (segments: typeof job.speakerSegments) =>
    segments.map((segment) => {
      const currentSpeaker = (segment.speaker ?? "").trim();
      const matches = normalizedSource ? currentSpeaker === normalizedSource : !currentSpeaker;

      return matches ? { ...segment, speaker: normalizedTarget } : segment;
    });

  return replaceJob({
    ...job,
    speakerSegments: updateSegments(job.speakerSegments),
    transcriptSegments: updateSegments(job.transcriptSegments),
  });
}

async function saveSettings(next: SettingsState) {
  const normalized = normalizeSettings(next);
  setState({ settings: normalized });
  applyAppearance(normalized);
  await localSettingsService.saveSettings(normalized);
  syncLocalPolling();
  syncRuntimePolling();
  runtimeAutoInstallAttempted = hasManualPythonOverride(normalized) || Boolean(normalized.backendUrl.trim());
  maybeStartRuntimeAutoInstall();

  if (shouldUseLocalDataSource(normalized)) {
    await refreshLocalJobs();
  }
}

async function installManagedRuntime() {
  await ensureSettingsLoaded();
  return beginManagedRuntimeInstall();
}

async function ensureManagedRuntimeReadyForLocalWork() {
  const api = getApi();
  if (api || hasManualPythonOverride(state.settings)) {
    return;
  }

  const timeoutAt = Date.now() + 10 * 60 * 1000;

  if (state.runtimeStatus.status === "missing" || state.runtimeStatus.status === "repair_required") {
    runtimeAutoInstallAttempted = true;
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

function getJobById(id: string) {
  return state.jobs.find((job) => job.id === id);
}

async function saveSummaryRun(run: AiSummaryRun) {
  await localAiService.saveSummaryRun(run);
  if (run.status === "completed") {
    void applyLocalPetWorkflowEvent({
      eventType: "ai_summary_completed",
      metadata: run.jobId,
    }).catch(() => undefined);
  }
  await refreshJobRuns(run.jobId);
}

async function setActiveSummaryRun(jobId: string, runId: string) {
  await ensureSettingsLoaded();

  if (getLocalMode()) {
    await localAiService.setActiveSummaryRun(jobId, runId);
    await refreshJobRuns(jobId);
    return;
  }

  const job = state.jobs.find((item) => item.id === jobId);
  const run = job?.summaryRuns.find((item) => item.id === runId);

  if (!job || !run) {
    return;
  }

  replaceJob({
    ...job,
    activeSummaryRunId: run.id,
    summary: run.result ? summaryResultToMeetingSummary(run.result) : createEmptyMeetingSummary(job.title),
  });
}

async function deleteSummaryRun(jobId: string, runId: string) {
  await ensureSettingsLoaded();

  if (getLocalMode()) {
    await localAiService.deleteSummaryRun(jobId, runId);
    await refreshJobRuns(jobId);
    return;
  }

  const job = state.jobs.find((item) => item.id === jobId);

  if (!job) {
    return;
  }

  const summaryRuns = job.summaryRuns.filter((run) => run.id !== runId);
  const nextActiveRun = summaryRuns.find((run) => run.id === job.activeSummaryRunId)
    ?? summaryRuns.find((run) => run.status === "completed" && run.result)
    ?? summaryRuns[0];
  replaceJob({
    ...job,
    summaryRuns,
    activeSummaryRunId: nextActiveRun?.id,
    summary: nextActiveRun?.result
      ? summaryResultToMeetingSummary(nextActiveRun.result)
      : createEmptyMeetingSummary(job.title),
  });
}

const actions = {
  ensureSettingsLoaded,
  refreshRuntimeStatus,
  refreshRuntimeInstallLog,
  refreshJobs,
  refreshJob,
  refreshJobRuns,
  createJob,
  deleteJob,
  installManagedRuntime,
  renameSpeaker,
  retryJob,
  saveSettings,
  saveSummaryRun,
  setActiveSummaryRun,
  deleteSummaryRun,
  getJobById,
};

void ensureSettingsLoaded();

export function useMeetingStore() {
  syncLocalPolling();
  void ensureSettingsLoaded();
  const snapshot = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);

  return {
    ...snapshot,
    api: getApi(),
    localMode: isManagedRuntimeReady(snapshot.runtimeStatus) || hasManualPythonOverride(snapshot.settings),
    ...actions,
  };
}

export type MeetingStore = ReturnType<typeof useMeetingStore>;
