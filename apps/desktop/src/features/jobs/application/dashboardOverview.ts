import type {
  DashboardJobSummary,
  DashboardMetrics,
  DashboardOverview,
  DashboardRange,
  DashboardTrendPoint,
  MeetingJob,
  ProcessingMode,
} from "@/shared/types/meeting";

const ACTIVE_STATUSES = new Set(["queued", "transcribing", "speaker_processing", "summarizing"]);

export function ratioPercent(value: number, total: number) {
  if (total <= 0) {
    return null;
  }
  return Math.round((Math.max(0, value) / total) * 100);
}

export function selectVisibleDashboardOverview(
  processingMode: ProcessingMode,
  localOverview: DashboardOverview | undefined,
  remoteOverview: DashboardOverview,
) {
  return processingMode === "local" ? localOverview : remoteOverview;
}

export function buildRemoteDashboardOverview(
  jobs: MeetingJob[],
  range: DashboardRange,
  now = new Date(),
): DashboardOverview {
  const startAt = rangeStart(range, now);
  const filtered = jobs
    .filter((job) => !startAt || new Date(job.createdAt).getTime() >= startAt.getTime())
    .sort((left, right) => right.createdAt.localeCompare(left.createdAt));
  const completed = filtered.filter((job) => job.overallStatus === "completed");
  const metrics: DashboardMetrics = {
    totalJobs: filtered.length,
    mediaDurationMinutes: sum(filtered, (job) => job.durationMinutes),
    processingDurationSeconds: sum(filtered, (job) => job.processingDurationSeconds ?? 0),
    activeJobs: filtered.filter((job) => ACTIVE_STATUSES.has(job.overallStatus)).length,
    completedJobs: completed.length,
    failedJobs: filtered.filter((job) => job.overallStatus === "failed").length,
    transcriptReadyJobs: completed.filter((job) => job.asrStatus === "completed").length,
    speakerEligibleJobs: completed.filter((job) => job.enableSpeaker).length,
    speakerReadyJobs: completed.filter((job) => job.diarizationStatus === "completed").length,
    summaryReadyJobs: completed.filter((job) => (
      job.summaryRuns.some((run) => run.status === "completed" && Boolean(run.result))
      || Boolean(job.summary.overview)
    )).length,
    exportedJobs: completed.filter((job) => Boolean(job.lastExportedAt)).length,
    warningJobs: filtered.filter((job) => job.warnings.length > 0).length,
  };
  const jobSummary = (job: MeetingJob): DashboardJobSummary => ({
    id: job.id,
    title: job.title,
    createdAt: job.createdAt,
    durationMinutes: job.durationMinutes,
    overallStatus: job.overallStatus,
    diarizationStatus: job.diarizationStatus,
    warningCount: job.warnings.length,
    hasSummary: job.summaryRuns.some((run) => run.status === "completed" && Boolean(run.result))
      || Boolean(job.summary.overview),
    lastExportedAt: job.lastExportedAt,
  });

  return {
    range,
    trendGranularity: range === "today" ? "hour" : range === "all" ? "month" : "day",
    metrics,
    trend: buildTrend(filtered, range),
    attentionJobs: filtered.filter((job) => (
      ACTIVE_STATUSES.has(job.overallStatus)
      || job.overallStatus === "failed"
      || job.warnings.length > 0
      || (job.overallStatus === "completed" && !jobSummary(job).hasSummary)
      || (job.overallStatus === "completed" && !job.lastExportedAt)
    )).slice(0, 6).map(jobSummary),
    recentResults: completed.slice(0, 6).map(jobSummary),
    resources: { aiModels: 0, enabledAiModels: 0, templates: 0, members: 0 },
  };
}

function buildTrend(jobs: MeetingJob[], range: DashboardRange): DashboardTrendPoint[] {
  const hourly = range === "today";
  const monthly = range === "all";
  const points = new Map<string, DashboardTrendPoint>();
  for (const job of jobs) {
    const createdAt = new Date(job.createdAt);
    const period = hourly
      ? `${String(createdAt.getHours()).padStart(2, "0")}:00`
      : monthly
      ? `${createdAt.getFullYear()}-${String(createdAt.getMonth() + 1).padStart(2, "0")}`
      : `${createdAt.getFullYear()}-${String(createdAt.getMonth() + 1).padStart(2, "0")}-${String(createdAt.getDate()).padStart(2, "0")}`;
    const point = points.get(period) ?? {
      period,
      totalJobs: 0,
      completedJobs: 0,
      failedJobs: 0,
      mediaDurationMinutes: 0,
      processingDurationSeconds: 0,
    };
    point.totalJobs += 1;
    point.completedJobs += Number(job.overallStatus === "completed");
    point.failedJobs += Number(job.overallStatus === "failed");
    point.mediaDurationMinutes += job.durationMinutes;
    point.processingDurationSeconds += job.processingDurationSeconds ?? 0;
    points.set(period, point);
  }
  return [...points.values()]
    .sort((left, right) => left.period.localeCompare(right.period))
    .slice(hourly ? -24 : monthly ? -36 : -31);
}

function rangeStart(range: DashboardRange, now: Date) {
  if (range === "all") {
    return null;
  }
  const days = range === "today" ? 0 : range === "7d" ? 6 : 29;
  const start = new Date(now);
  start.setHours(0, 0, 0, 0);
  start.setDate(start.getDate() - days);
  return start;
}

function sum<T>(values: T[], value: (item: T) => number) {
  return values.reduce((total, item) => total + Math.max(0, value(item)), 0);
}
