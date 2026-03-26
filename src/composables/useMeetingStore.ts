import { computed, reactive, toRefs } from "vue";
import { applyAppearance } from "@/services/appearance";
import { createEmptyMeetingSummary, summaryResultToMeetingSummary } from "@/services/aiStorage";
import { createLocalAiService } from "@/services/localAi";
import { createLocalMeetingService } from "@/services/localMeeting";
import { createLocalSettingsService } from "@/services/localSettings";
import { createMeetingApi } from "@/services/api";
import type { AiSummaryRun, MeetingJob, NewMeetingJobInput, SettingsState } from "@/types/meeting";

const defaultSettings: SettingsState = {
  themeMode: "auto",
  liquidGlassStyle: "transparent",
  accentColor: "#2f6dff",
  locale: "zh-CN",
  backendUrl: "",
  apiToken: "",
  defaultHotwords: "SeACo-Paraformer, FunASR, 会议纪要",
  summaryTemplate: "默认会议纪要模板",
  concurrency: 2,
  pythonPath: "",
  runnerScriptPath: "",
  localAsrDevice: "auto",
  localAsrThreads: 0,
  localAsrBatchSizeSeconds: 300,
};

const state = reactive({
  jobs: [] as MeetingJob[],
  settings: { ...defaultSettings } as SettingsState,
  settingsLoaded: false,
});

const localAiService = createLocalAiService();
const localSettingsService = createLocalSettingsService();
let localPollingId: number | null = null;
let settingsLoadPromise: Promise<void> | null = null;

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
    summaryTemplate: merged.summaryTemplate.trim() || defaultSettings.summaryTemplate,
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

function hasLocalRunnerSettings(settings: SettingsState) {
  return Boolean(settings.pythonPath.trim());
}

async function refreshLocalJobs() {
  try {
    state.jobs = await createLocalMeetingService().listJobs();
  } catch {
    // Keep the last known local state when polling fails.
  }
}

function syncLocalPolling() {
  if (typeof window === "undefined") {
    return;
  }

  const shouldPoll = hasLocalRunnerSettings(state.settings);

  if (shouldPoll && localPollingId === null) {
    localPollingId = window.setInterval(() => {
      void refreshLocalJobs();
    }, 1500);
    return;
  }

  if (!shouldPoll && localPollingId !== null) {
    window.clearInterval(localPollingId);
    localPollingId = null;
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

    state.settingsLoaded = true;
    applyAppearance(state.settings);
    syncLocalPolling();

    if (hasLocalRunnerSettings(state.settings)) {
      await refreshLocalJobs();
    }
  })().finally(() => {
    settingsLoadPromise = null;
  });

  return settingsLoadPromise;
}

function replaceJob(job: MeetingJob) {
  state.jobs = [job, ...state.jobs.filter((item) => item.id !== job.id)];
  return job;
}

void ensureSettingsLoaded();

export function useMeetingStore() {
  syncLocalPolling();
  void ensureSettingsLoaded();

  const api = computed(() =>
    state.settings.backendUrl
      ? createMeetingApi(state.settings.backendUrl, state.settings.apiToken)
      : null,
  );
  const localMode = computed(() => hasLocalRunnerSettings(state.settings));

  async function refreshJobs() {
    await ensureSettingsLoaded();

    if (localMode.value) {
      state.jobs = await createLocalMeetingService().listJobs();
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

  async function refreshJobRuns(id: string) {
    await ensureSettingsLoaded();

    if (!localMode.value) {
      return;
    }

    const job = state.jobs.find((item) => item.id === id);
    if (!job) {
      return;
    }

    const refreshed = await createLocalMeetingService().getJob(id);
    Object.assign(job, refreshed);
  }

  async function createJob(input: NewMeetingJobInput) {
    await ensureSettingsLoaded();

    if (localMode.value) {
      const firstFile = input.files[0];

      if (!firstFile?.path) {
        throw new Error("本地模式只支持带本地路径的单个文件。");
      }

      const created = await createLocalMeetingService().createJob(
        {
          ...input,
          files: [firstFile],
        },
        state.settings,
      );

      syncLocalPolling();
      return replaceJob(created);
    }

    if (api.value) {
      const created = await api.value.createJob(input);
      return replaceJob(created);
    }

    throw new Error("当前未配置可用的本地 Python 环境或在线后端，无法创建任务。");
  }

  async function retryJob(id: string) {
    await ensureSettingsLoaded();
    const job = state.jobs.find((item) => item.id === id);

    if (!job) {
      return;
    }

    if (localMode.value) {
      const updated = await createLocalMeetingService().retryJob(id, state.settings);
      syncLocalPolling();
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

    throw new Error("当前未配置可用的本地 Python 环境或在线后端，无法重试任务。");
  }

  async function deleteJob(id: string) {
    await ensureSettingsLoaded();

    if (localMode.value) {
      await createLocalMeetingService().deleteJob(id);
    }

    state.jobs = state.jobs.filter((job) => job.id !== id);
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

    if (hasLocalRunnerSettings(normalized)) {
      await refreshLocalJobs();
    }
  }

  function getJobById(id: string) {
    return state.jobs.find((job) => job.id === id);
  }

  async function saveSummaryRun(run: AiSummaryRun) {
    await localAiService.saveSummaryRun(run);
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
    refreshJobs,
    refreshJobRuns,
    createJob,
    deleteJob,
    renameSpeaker,
    retryJob,
    saveSettings,
    saveSummaryRun,
    setActiveSummaryRun,
    deleteSummaryRun,
    getJobById,
  };
}
