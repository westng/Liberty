import { listen } from "@tauri-apps/api/event";
import { computed, reactive, toRefs } from "vue";
import { applyAppearance } from "@/shared/services/ui/appearance";
import { createEmptyMeetingSummary, summaryResultToMeetingSummary } from "@/shared/services/ai/storage";
import { createLocalAiService } from "@/shared/services/tauri/ai";
import { createLocalMeetingService } from "@/shared/services/tauri/meeting";
import { createLocalPetService } from "@/shared/services/tauri/pet";
import { createLocalRuntimeService } from "@/shared/services/tauri/runtime";
import { createLocalSettingsService } from "@/shared/services/tauri/settings";
import { createLocalUpdaterService } from "@/shared/services/tauri/updater";
import { createMeetingApi } from "@/shared/services/remote/meetingApi";
import type {
  AppUpdateStatus,
  AiSummaryRun,
  ManagedRuntimeStatus,
  MeetingJob,
  NewMeetingJobInput,
  SettingsState,
} from "@/shared/types/meeting";

const defaultSettings: SettingsState = {
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
};

const defaultUpdateStatus: AppUpdateStatus = {
  status: "idle",
  platform: "unsupported",
  channel: "GitHub Releases",
  currentVersion: "",
  feedUrl: "",
  canAutoInstall: false,
};

const state = reactive({
  jobs: [] as MeetingJob[],
  settings: { ...defaultSettings } as SettingsState,
  runtimeStatus: {
    platformId: "",
    runtimeVersion: "",
    pythonVersion: "",
    status: "missing",
    updatedAt: "",
  } as ManagedRuntimeStatus,
  runtimeInstallLog: "",
  updateStatus: { ...defaultUpdateStatus } as AppUpdateStatus,
  settingsLoaded: false,
});

const localAiService = createLocalAiService();
const localPetService = createLocalPetService();
const localRuntimeService = createLocalRuntimeService();
const localSettingsService = createLocalSettingsService();
const localUpdaterService = createLocalUpdaterService();
let localPollingId: number | null = null;
let settingsLoadPromise: Promise<void> | null = null;
let runtimePollingId: number | null = null;
let runtimeInstallPromise: Promise<ManagedRuntimeStatus> | null = null;
let runtimeAutoInstallAttempted = false;
let updateListenerAttached = false;
const hydratedJobIds = new Set<string>();

function normalizeSettings(settings?: Partial<SettingsState> | null): SettingsState {
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
  };
}

function normalizeUpdateStatus(status?: Partial<AppUpdateStatus> | null): AppUpdateStatus {
  return {
    ...defaultUpdateStatus,
    ...(status ?? {}),
    channel: status?.channel?.trim() || defaultUpdateStatus.channel,
    currentVersion: status?.currentVersion?.trim() || defaultUpdateStatus.currentVersion,
    feedUrl: status?.feedUrl?.trim() || defaultUpdateStatus.feedUrl,
    canAutoInstall: Boolean(status?.canAutoInstall),
    platform:
      status?.platform === "macos" || status?.platform === "windows"
        ? status.platform
        : "unsupported",
  };
}

function hasManualPythonOverride(settings: SettingsState) {
  return Boolean(settings.pythonPath.trim());
}

function shouldUseLocalDataSource(settings: SettingsState) {
  return !settings.backendUrl.trim();
}

function isManagedRuntimeReady(runtimeStatus: ManagedRuntimeStatus) {
  return runtimeStatus.status === "ready" && Boolean(runtimeStatus.pythonExecutablePath?.trim());
}

function shouldAutoInstallManagedRuntime(
  settings: SettingsState,
  runtimeStatus: ManagedRuntimeStatus,
) {
  if (settings.backendUrl.trim()) {
    return false;
  }

  if (hasManualPythonOverride(settings)) {
    return false;
  }

  return runtimeStatus.status === "missing" || runtimeStatus.status === "repair_required";
}

async function refreshLocalJobs() {
  try {
    applyJobListSnapshot(await createLocalMeetingService().listJobs());
  } catch {
    // Keep the last known local state when polling fails.
  }
}

function mergeJobSnapshot(existing: MeetingJob | undefined, incoming: MeetingJob) {
  if (!existing || !hydratedJobIds.has(incoming.id)) {
    return incoming;
  }

  return {
    ...existing,
    ...incoming,
    transcriptSegments: existing.transcriptSegments,
    speakerSegments: existing.speakerSegments,
    summaryRuns: existing.summaryRuns,
    processLog: existing.processLog,
    summary: existing.summary,
    activeSummaryRunId: existing.activeSummaryRunId,
  } satisfies MeetingJob;
}

function applyJobListSnapshot(incomingJobs: MeetingJob[]) {
  const existingById = new Map(state.jobs.map((job) => [job.id, job]));
  state.jobs = incomingJobs.map((job) => mergeJobSnapshot(existingById.get(job.id), job));
  syncLocalPolling();
}

function syncLocalPolling() {
  if (typeof window === "undefined") {
    return;
  }

  const shouldPoll = shouldUseLocalDataSource(state.settings);
  const hasActiveLocalJobs = state.jobs.some((job) =>
    ["queued", "transcribing", "speaker_processing", "summarizing"].includes(job.overallStatus),
  );
  const pollingIntervalMs = hasActiveLocalJobs ? 1500 : 15000;

  if (shouldPoll && localPollingId === null) {
    localPollingId = window.setInterval(() => {
      void refreshLocalJobs();
    }, pollingIntervalMs);
    return;
  }

  if (!shouldPoll && localPollingId !== null) {
    window.clearInterval(localPollingId);
    localPollingId = null;
    return;
  }

  if (shouldPoll && localPollingId !== null) {
    window.clearInterval(localPollingId);
    localPollingId = window.setInterval(() => {
      void refreshLocalJobs();
    }, pollingIntervalMs);
  }
}

function syncRuntimePolling() {
  if (typeof window === "undefined") {
    return;
  }

  const shouldPoll = state.runtimeStatus.status === "installing";

  if (shouldPoll && runtimePollingId === null) {
    runtimePollingId = window.setInterval(() => {
      void refreshRuntimeStatus();
      void refreshRuntimeInstallLog();
    }, 1500);
    return;
  }

  if (!shouldPoll && runtimePollingId !== null) {
    window.clearInterval(runtimePollingId);
    runtimePollingId = null;
  }
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
      state.settings = normalizeSettings(loaded);
    } catch {
      state.settings = normalizeSettings();
    }

    await refreshRuntimeStatus();
    await refreshUpdateStatus();
    state.settingsLoaded = true;
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

async function ensureUpdateListener() {
  if (updateListenerAttached || typeof window === "undefined") {
    return;
  }

  updateListenerAttached = true;
  await listen<AppUpdateStatus>("liberty://update-status", (event) => {
    state.updateStatus = normalizeUpdateStatus(event.payload);
  });
}

function replaceJob(job: MeetingJob) {
  state.jobs = [job, ...state.jobs.filter((item) => item.id !== job.id)];
  return job;
}

async function refreshRuntimeStatus() {
  try {
    state.runtimeStatus = await localRuntimeService.getStatus();
  } catch {
    state.runtimeStatus = {
      platformId: "",
      runtimeVersion: "",
      pythonVersion: "",
      status: "missing",
      updatedAt: "",
    };
  }

  syncLocalPolling();
  syncRuntimePolling();
  maybeStartRuntimeAutoInstall();

  return state.runtimeStatus;
}

async function refreshRuntimeInstallLog() {
  try {
    state.runtimeInstallLog = await localRuntimeService.getInstallLog();
  } catch {
    state.runtimeInstallLog = "";
  }

  return state.runtimeInstallLog;
}

async function refreshUpdateStatus() {
  try {
    state.updateStatus = normalizeUpdateStatus(await localUpdaterService.getStatus());
  } catch {
    state.updateStatus = normalizeUpdateStatus(state.updateStatus);
  }

  return state.updateStatus;
}

function maybeStartRuntimeAutoInstall() {
  if (!state.settingsLoaded) {
    return;
  }

  if (runtimeAutoInstallAttempted) {
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
    state.runtimeStatus = await localRuntimeService.install();
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

void ensureSettingsLoaded();
void ensureUpdateListener();

export function useMeetingStore() {
  syncLocalPolling();
  void ensureSettingsLoaded();
  void ensureUpdateListener();

  const api = computed(() =>
    state.settings.backendUrl
      ? createMeetingApi(state.settings.backendUrl, state.settings.apiToken)
      : null,
  );
  const localMode = computed(
    () => isManagedRuntimeReady(state.runtimeStatus) || hasManualPythonOverride(state.settings),
  );

  async function refreshJobs() {
    await ensureSettingsLoaded();

    if (shouldUseLocalDataSource(state.settings)) {
      applyJobListSnapshot(await createLocalMeetingService().listJobs());
      return state.jobs;
    }

    if (!api.value) {
      return state.jobs;
    }

    try {
      state.jobs = await api.value.listJobs();
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
      replaceJob(refreshed);
      return refreshed;
    }

    if (!api.value) {
      return getJobById(id);
    }

    try {
      const refreshed = await api.value.getJob(id);
      hydratedJobIds.add(id);
      replaceJob(refreshed);
      return refreshed;
    } catch {
      return getJobById(id);
    }
  }

  async function refreshJobRuns(id: string) {
    await ensureSettingsLoaded();

    if (!shouldUseLocalDataSource(state.settings)) {
      return;
    }

    const job = state.jobs.find((item) => item.id === id);
    if (!job) {
      return;
    }

    const refreshed = await createLocalMeetingService().getJob(id);
    hydratedJobIds.add(id);
    Object.assign(job, refreshed);
  }

  async function createJob(input: NewMeetingJobInput) {
    await ensureSettingsLoaded();

    if (!localMode.value && !api.value) {
      await ensureManagedRuntimeReadyForLocalWork();
    }

    if (localMode.value) {
      const firstFile = input.files[0];

      if (!firstFile?.path) {
        throw new Error("本地模式只支持带本地路径的单个文件。");
      }

      const created = await createLocalMeetingService().createJob({
        ...input,
        files: [firstFile],
      });

      void localPetService.applyWorkflowEvent({
        eventType: "job_created",
        metadata: created.title,
      }).catch(() => undefined);

      syncLocalPolling();
      hydratedJobIds.add(created.id);
      return replaceJob(created);
    }

    if (api.value) {
      const created = await api.value.createJob(input);
      void localPetService.applyWorkflowEvent({
        eventType: "job_created",
        metadata: created.title,
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

    if (!localMode.value && !api.value) {
      await ensureManagedRuntimeReadyForLocalWork();
    }

    if (localMode.value) {
      const updated = await createLocalMeetingService().retryJob(id);
      syncLocalPolling();
      hydratedJobIds.add(updated.id);
      return replaceJob(updated);
    }

    job.failureReason = undefined;
    job.overallStatus = "queued";
    job.asrStatus = "queued";
    job.summaryStatus = "idle";

    if (api.value) {
      const updated = await api.value.retryJob(id);
      Object.assign(job, updated);
      return;
    }

    throw new Error("当前未安装本地运行环境，也未配置在线后端，无法重试任务。");
  }

  async function deleteJob(id: string) {
    await ensureSettingsLoaded();

    if (localMode.value) {
      await createLocalMeetingService().deleteJob(id);
    }

    state.jobs = state.jobs.filter((job) => job.id !== id);
    hydratedJobIds.delete(id);
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

    if (localMode.value) {
      const updated = await createLocalMeetingService().renameSpeaker(
        id,
        fromSpeaker,
        normalizedTarget,
      );
      hydratedJobIds.add(updated.id);
      return replaceJob(updated);
    }

    const normalizedSource = fromSpeaker.trim();
    const updateSegments = (segments: typeof job.speakerSegments) =>
      segments.map((segment) => {
        const currentSpeaker = (segment.speaker ?? "").trim();
        const matches = normalizedSource
          ? currentSpeaker === normalizedSource
          : !currentSpeaker;

        return matches
          ? {
              ...segment,
              speaker: normalizedTarget,
            }
          : segment;
      });

    job.speakerSegments = updateSegments(job.speakerSegments);
    return job;
  }

  async function saveSettings(next: SettingsState) {
    const normalized = normalizeSettings(next);
    state.settings = normalized;
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

  async function checkForUpdates(interactive = true) {
    await ensureSettingsLoaded();
    state.updateStatus = normalizeUpdateStatus(
      await localUpdaterService.checkForUpdates(interactive),
    );
    return state.updateStatus;
  }

  async function installUpdate() {
    await ensureSettingsLoaded();
    state.updateStatus = normalizeUpdateStatus(await localUpdaterService.installUpdate());
    return state.updateStatus;
  }

  async function restartAfterUpdate() {
    await localUpdaterService.restartAfterUpdate();
  }

  async function ensureManagedRuntimeReadyForLocalWork() {
    if (api.value || hasManualPythonOverride(state.settings)) {
      return;
    }

    const timeoutAt = Date.now() + 10 * 60 * 1000;

    if (state.runtimeStatus.status === "missing" || state.runtimeStatus.status === "repair_required") {
      runtimeAutoInstallAttempted = true;
      await installManagedRuntime();
    }

    while (!isManagedRuntimeReady(state.runtimeStatus)) {
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
      void localPetService.applyWorkflowEvent({
        eventType: "ai_summary_completed",
        metadata: run.jobId,
      }).catch(() => undefined);
    }
    await refreshJobRuns(run.jobId);
  }

  async function setActiveSummaryRun(jobId: string, runId: string) {
    await ensureSettingsLoaded();

    if (localMode.value) {
      await localAiService.setActiveSummaryRun(jobId, runId);
      await refreshJobRuns(jobId);
      return;
    }

    const job = state.jobs.find((item) => item.id === jobId);
    const run = job?.summaryRuns.find((item) => item.id === runId);

    if (!job || !run) {
      return;
    }

    job.activeSummaryRunId = run.id;
    job.summary = run.result ? summaryResultToMeetingSummary(run.result) : createEmptyMeetingSummary(job.title);
  }

  async function deleteSummaryRun(jobId: string, runId: string) {
    await ensureSettingsLoaded();

    if (localMode.value) {
      await localAiService.deleteSummaryRun(jobId, runId);
      await refreshJobRuns(jobId);
      return;
    }

    const job = state.jobs.find((item) => item.id === jobId);

    if (!job) {
      return;
    }

    job.summaryRuns = job.summaryRuns.filter((run) => run.id !== runId);
    const nextActiveRun = job.summaryRuns.find((run) => run.id === job.activeSummaryRunId)
      ?? job.summaryRuns.find((run) => run.status === "completed" && run.result)
      ?? job.summaryRuns[0];
    job.activeSummaryRunId = nextActiveRun?.id;
    job.summary = nextActiveRun?.result
      ? summaryResultToMeetingSummary(nextActiveRun.result)
      : createEmptyMeetingSummary(job.title);
  }

  return {
    ...toRefs(state),
    api,
    localMode,
    ensureSettingsLoaded,
    refreshRuntimeStatus,
    refreshRuntimeInstallLog,
    refreshUpdateStatus,
    refreshJobs,
    refreshJob,
    refreshJobRuns,
    checkForUpdates,
    createJob,
    deleteJob,
    installManagedRuntime,
    installUpdate,
    renameSpeaker,
    restartAfterUpdate,
    retryJob,
    saveSettings,
    saveSummaryRun,
    setActiveSummaryRun,
    deleteSummaryRun,
    getJobById,
  };
}
