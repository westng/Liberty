import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";
import { Link } from "@/app/router/RouterContext";
import {
  buildRemoteDashboardOverview,
  ratioPercent,
  selectVisibleDashboardOverview,
} from "@/features/jobs/application/dashboardOverview";
import { isManagedRuntimeReady } from "@/features/meeting/application/settingsPolicy";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import StatusBadge from "@/shared/components/StatusBadge";
import { Button, Tabs } from "@/shared/components/ui";
import { getMessages } from "@/shared/i18n";
import { createLocalMeetingService } from "@/shared/services/tauri/meeting";
import { openJobWorkbenchWindow } from "@/shared/services/ui/windows";
import type {
  DashboardJobSummary,
  DashboardOverview,
  DashboardRange,
  DashboardTrendPoint,
  ProcessingMode,
} from "@/shared/types/meeting";
import { jobDetailPath } from "./jobRoutes";
import "./WorkHubViews.css";

type TrendMetric = "jobs" | "media" | "processing";
type DashboardSection = "activity" | "insights";
type BarStyle = CSSProperties & { "--bar-height": string };

const localMeetingService = createLocalMeetingService();

export default function WorkDashboardView() {
  const store = useMeetingStore();
  const isEnglish = store.settings.locale === "en-US";
  const isLocal = store.settings.processingMode === "local";
  const operationUnavailable = getMessages(store.settings.locale).workbench.remoteOperationUnavailable;
  const [range, setRange] = useState<DashboardRange>("7d");
  const [section, setSection] = useState<DashboardSection>("activity");
  const [trendMetric, setTrendMetric] = useState<TrendMetric>("jobs");
  const [localOverview, setLocalOverview] = useState<DashboardOverview>();
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState<string>();
  const refreshRequestId = useRef(0);
  const copy = useMemo(() => createCopy(isEnglish), [isEnglish]);

  const remoteOverview = useMemo(
    () => buildRemoteDashboardOverview(
      store.jobs.filter((job) => job.source === "remote"),
      range,
    ),
    [range, store.jobs],
  );
  const overview = selectVisibleDashboardOverview(
    store.settings.processingMode,
    localOverview,
    remoteOverview,
  );
  const canOpenResults = isLocal || (
    store.canRemoteOperation("jobs.read")
    && store.canRemoteOperation("jobs.result.read")
  );

  const refreshOverview = useCallback(async () => {
    const requestId = refreshRequestId.current + 1;
    refreshRequestId.current = requestId;
    setIsLoading(true);
    setLoadError(undefined);
    try {
      if (isLocal) {
        const nextOverview = await localMeetingService.getDashboardOverview(range);
        if (refreshRequestId.current === requestId) {
          setLocalOverview(nextOverview);
        }
      } else {
        await store.refreshJobs();
      }
    } catch (error) {
      if (refreshRequestId.current === requestId) {
        setLoadError(error instanceof Error ? error.message : String(error));
      }
    } finally {
      if (refreshRequestId.current === requestId) {
        setIsLoading(false);
      }
    }
  }, [isLocal, range, store.refreshJobs]);

  useEffect(() => {
    void refreshOverview();
  }, [refreshOverview]);

  const completionRate = overview
    ? ratioPercent(overview.metrics.completedJobs, overview.metrics.totalJobs)
    : null;
  const summaryCoverage = overview
    ? ratioPercent(overview.metrics.summaryReadyJobs, overview.metrics.completedJobs)
    : null;
  const activityJobs = useMemo(() => {
    const jobs = [...(overview?.attentionJobs ?? []), ...(overview?.recentResults ?? [])];
    return [...new Map(jobs.map((job) => [job.id, job])).values()]
      .sort((left, right) => right.createdAt.localeCompare(left.createdAt))
      .slice(0, 8);
  }, [overview]);

  return (
    <section className="view-stack native-page work-dashboard-page">
      <header className="work-dashboard-header">
        <div className="work-dashboard-title-row">
          <h2>{copy.title}</h2>
          {!isLocal && <span className="work-dashboard-source-badge">{copy.remoteSnapshot}</span>}
        </div>
      </header>

      {loadError && !overview ? (
        <section className="surface work-dashboard-state" role="alert">
          <strong>{copy.loadFailed}</strong>
          <span>{loadError}</span>
          <Button onClick={() => void refreshOverview()} variant="text">{copy.retry}</Button>
        </section>
      ) : overview ? (
        <>
          {loadError && <div className="work-dashboard-inline-error" role="status">{copy.staleData}</div>}

          <section className="work-dashboard-kpis" aria-label={copy.coreMetrics} aria-busy={isLoading}>
            <MetricCard
              label={copy.totalJobs}
              value={formatNumber(overview.metrics.totalJobs, store.settings.locale)}
              meta={copy.activeMeta(overview.metrics.activeJobs)}
            />
            <MetricCard
              label={copy.mediaDuration}
              value={formatMinutes(overview.metrics.mediaDurationMinutes, isEnglish)}
              meta={copy.processingMeta(formatDuration(overview.metrics.processingDurationSeconds, isEnglish))}
            />
            <MetricCard
              label={copy.completionRate}
              value={formatPercent(completionRate)}
              meta={copy.completionMeta(overview.metrics.completedJobs, overview.metrics.failedJobs)}
              tone={overview.metrics.failedJobs > 0 ? "warning" : "default"}
            />
            <MetricCard
              label={copy.summaryCoverage}
              value={formatPercent(summaryCoverage)}
              meta={copy.coverageMeta(overview.metrics.summaryReadyJobs, overview.metrics.completedJobs)}
            />
          </section>

          <section className="work-dashboard-workspace" aria-label={copy.workspaceLabel}>
            <Tabs
              activeKey={section}
              appearance="button"
              ariaLabel={copy.workspaceLabel}
              className="work-dashboard-section-tabs"
              items={copy.sections}
              onChange={(activeKey) => setSection(activeKey === "insights" ? "insights" : "activity")}
            />

            {section === "activity" ? (
              <>
                <div className="work-dashboard-summary-grid">
                  <SummaryBlock
                    action={<Link to="/jobs">{copy.allJobs}</Link>}
                    stats={[
                      { label: copy.activeJobs, value: overview.metrics.activeJobs },
                      { label: copy.exceptions, value: overview.metrics.failedJobs + overview.metrics.warningJobs },
                    ]}
                    title={copy.attention}
                  />
                  <SummaryBlock
                    action={<Link to="/jobs?status=completed">{copy.allResults}</Link>}
                    stats={[
                      { label: copy.completedJobs, value: overview.metrics.completedJobs },
                      { label: copy.summarizedJobs, value: overview.metrics.summaryReadyJobs },
                    ]}
                    title={copy.recentResults}
                  />
                </div>

                <section className="work-dashboard-table-section">
                  <header className="work-dashboard-table-toolbar">
                    <h3>{copy.recentJobs}</h3>
                    <div className="work-dashboard-table-actions">
                      <div className="work-dashboard-segmented" aria-label={copy.rangeLabel}>
                        {copy.ranges.map((item) => (
                          <button
                            key={item.value}
                            type="button"
                            className={range === item.value ? "active" : ""}
                            aria-pressed={range === item.value}
                            onClick={() => setRange(item.value)}
                          >
                            {item.label}
                          </button>
                        ))}
                      </div>
                      <Button
                        aria-busy={isLoading}
                        className="work-dashboard-refresh"
                        disabled={isLoading}
                        onClick={() => void refreshOverview()}
                        variant="text"
                      >
                        {copy.refresh}
                      </Button>
                      {isLocal ? (
                        <Link className="primary-button work-dashboard-primary-action" to="/jobs/new">{copy.newJob}</Link>
                      ) : (
                        <Button disabled title={operationUnavailable} variant="primary">{copy.newJob}</Button>
                      )}
                    </div>
                  </header>
                  <JobTable
                    copy={copy}
                    emptyLabel={copy.emptyJobs}
                    jobs={activityJobs}
                    locale={store.settings.locale}
                    source={store.settings.processingMode}
                    canOpenResults={canOpenResults}
                    operationUnavailable={store.remoteError ?? operationUnavailable}
                  />
                </section>
              </>
            ) : (
              <>
                <div className="work-dashboard-primary-grid">
                  <section className="work-dashboard-panel work-dashboard-trend-panel">
                    <PanelHeader title={copy.trend}>
                      <div className="work-dashboard-metric-tabs" aria-label={copy.trendMetricLabel}>
                        {copy.trendMetrics.map((item) => (
                          <button
                            key={item.value}
                            type="button"
                            className={trendMetric === item.value ? "active" : ""}
                            aria-pressed={trendMetric === item.value}
                            onClick={() => setTrendMetric(item.value)}
                          >
                            {item.label}
                          </button>
                        ))}
                      </div>
                    </PanelHeader>
                    <TrendChart
                      points={overview.trend}
                      metric={trendMetric}
                      locale={store.settings.locale}
                      emptyLabel={copy.emptyTrend}
                      unitLabel={copy.trendUnit(trendMetric)}
                    />
                  </section>

                  <section className="work-dashboard-panel">
                    <PanelHeader title={copy.completeness} />
                    <div className="work-dashboard-coverage-list">
                      <CoverageRow label={copy.transcript} value={overview.metrics.transcriptReadyJobs} total={overview.metrics.completedJobs} />
                      <CoverageRow label={copy.speakers} value={overview.metrics.speakerReadyJobs} total={overview.metrics.speakerEligibleJobs} />
                      <CoverageRow label={copy.summary} value={overview.metrics.summaryReadyJobs} total={overview.metrics.completedJobs} />
                      <CoverageRow label={copy.exported} value={overview.metrics.exportedJobs} total={overview.metrics.completedJobs} />
                    </div>
                  </section>
                </div>

                {isLocal && (
                  <div className="work-dashboard-secondary-grid">
                    <section className="work-dashboard-panel">
                      <PanelHeader title={copy.resources} />
                      <div className="work-dashboard-resource-grid">
                        <ResourceItem label={copy.runtime} value={isManagedRuntimeReady(store.runtimeStatus) ? copy.ready : copy.needsSetup} tone={isManagedRuntimeReady(store.runtimeStatus) ? "success" : "warning"} to="/settings" />
                        <ResourceItem label={copy.models} value={copy.modelValue(overview.resources.enabledAiModels, overview.resources.aiModels)} to="/models" />
                        <ResourceItem label={copy.templates} value={String(overview.resources.templates)} to="/templates" />
                        <ResourceItem label={copy.members} value={String(overview.resources.members)} to="/members" />
                      </div>
                    </section>

                    <section className="work-dashboard-panel">
                      <PanelHeader title={copy.companion} action={<Link className="text-button small-button" to="/pet">{copy.openCompanion}</Link>} />
                      {overview.companion ? (
                        <div className="work-dashboard-companion">
                          <div className="work-dashboard-companion-main">
                            <div><strong>{overview.companion.name}</strong><span>Lv.{overview.companion.level}</span></div>
                            <div className="work-dashboard-progress" aria-label={copy.levelProgress}><span style={{ width: `${overview.companion.levelProgressPercent}%` }} /></div>
                            <small>{overview.companion.nextLevelExperience > 0 ? `${overview.companion.currentLevelExperience} / ${overview.companion.nextLevelExperience} EXP` : copy.maxLevel}</small>
                          </div>
                          <div className="work-dashboard-companion-stats">
                            <Link to="/pet-store"><span>LP</span><strong>{overview.companion.lpBalance}</strong></Link>
                            <Link to="/daily-check-in"><span>{copy.checkIn}</span><strong>{overview.companion.checkedInToday ? copy.done : copy.pending}</strong></Link>
                            <Link to="/work-market"><span>{copy.claimable}</span><strong>{overview.companion.claimableActivities}</strong></Link>
                          </div>
                        </div>
                      ) : <div className="work-dashboard-empty">{copy.companionUnavailable}</div>}
                    </section>
                  </div>
                )}
              </>
            )}
          </section>
        </>
      ) : (
        <section className="surface work-dashboard-state" aria-busy="true">
          <strong>{copy.loading}</strong>
        </section>
      )}
    </section>
  );
}

function MetricCard({
  label,
  value,
  meta,
  tone = "default",
}: {
  label: string;
  value: string;
  meta: string;
  tone?: "default" | "warning";
}) {
  return (
    <div className="work-dashboard-kpi" data-tone={tone}>
      <span>{label}</span>
      <strong>{value}</strong>
      <small>{meta}</small>
    </div>
  );
}

function PanelHeader({ title, action, children }: { title: string; action?: ReactNode; children?: ReactNode }) {
  return (
    <header className="work-dashboard-panel-head">
      <h3>{title}</h3>
      {action ?? children}
    </header>
  );
}

function TrendChart({
  points,
  metric,
  locale,
  emptyLabel,
  unitLabel,
}: {
  points: DashboardTrendPoint[];
  metric: TrendMetric;
  locale: string;
  emptyLabel: string;
  unitLabel: string;
}) {
  const values = points.map((point) => trendValue(point, metric));
  const maximum = Math.max(1, ...values);
  if (points.length === 0) {
    return <div className="work-dashboard-chart-empty">{emptyLabel}</div>;
  }
  return (
    <div
      className="work-dashboard-chart"
      role="img"
      aria-label={points
        .map((point, index) => `${point.period}: ${formatNumber(values[index] ?? 0, locale)} ${unitLabel}`)
        .join("; ")}
    >
      <div className="work-dashboard-chart-scale"><span>{formatNumber(maximum, locale)}</span><span>0</span></div>
      <div className="work-dashboard-bars" style={{ gridTemplateColumns: `repeat(${points.length}, minmax(8px, 1fr))` }}>
        {points.map((point) => {
          const value = trendValue(point, metric);
          const style: BarStyle = { "--bar-height": `${Math.max(value > 0 ? 5 : 0, (value / maximum) * 100)}%` };
          return (
            <span
              className="work-dashboard-bar-slot"
              key={point.period}
              title={`${point.period} · ${formatNumber(value, locale)} ${unitLabel}`}
            >
              <span className="work-dashboard-bar" style={style} />
            </span>
          );
        })}
      </div>
      <div className="work-dashboard-chart-labels"><span>{points[0]?.period}</span><span>{points.at(-1)?.period}</span></div>
    </div>
  );
}

function CoverageRow({ label, value, total }: { label: string; value: number; total: number }) {
  const percent = ratioPercent(value, total);
  return (
    <div className="work-dashboard-coverage-row">
      <div><span>{label}</span><strong>{formatPercent(percent)}</strong></div>
      <div className="work-dashboard-progress" aria-label={`${label} ${formatPercent(percent)}`}>
        <span style={{ width: `${percent ?? 0}%` }} />
      </div>
      <small>{total > 0 ? `${value} / ${total}` : "—"}</small>
    </div>
  );
}

function SummaryBlock({
  action,
  stats,
  title,
}: {
  action: ReactNode;
  stats: Array<{ label: string; value: number }>;
  title: string;
}) {
  return (
    <section className="work-dashboard-summary-block">
      <header>
        <span className="work-dashboard-summary-mark" aria-hidden="true" />
        <h3>{title}</h3>
        {action}
      </header>
      <div className="work-dashboard-summary-stats">
        {stats.map((stat) => (
          <div key={stat.label}>
            <span>{stat.label}</span>
            <strong>{stat.value}</strong>
          </div>
        ))}
      </div>
    </section>
  );
}

function JobTable({
  canOpenResults,
  copy,
  emptyLabel,
  jobs,
  locale,
  operationUnavailable,
  source,
}: {
  canOpenResults: boolean;
  copy: ReturnType<typeof createCopy>;
  emptyLabel: string;
  jobs: DashboardJobSummary[];
  locale: string;
  operationUnavailable: string;
  source: ProcessingMode;
}) {
  if (jobs.length === 0) {
    return <div className="work-dashboard-empty">{emptyLabel}</div>;
  }
  return (
    <div className="work-dashboard-table" role="table" aria-label={copy.recentJobs}>
      <div className="work-dashboard-table-head" role="row">
        <span role="columnheader">{copy.jobTitle}</span>
        <span role="columnheader">{copy.duration}</span>
        <span role="columnheader">{copy.status}</span>
        <span role="columnheader">{copy.createdAt}</span>
        <span role="columnheader">{copy.action}</span>
      </div>
      {jobs.map((job) => (
        <div className="work-dashboard-table-row" key={job.id} role="row">
          <div className="work-dashboard-table-title" role="cell">
            <span className="work-dashboard-file-mark" aria-hidden="true" />
            <strong title={job.title}>{job.title}</strong>
          </div>
          <span className="work-dashboard-table-duration" role="cell">{formatMinutes(job.durationMinutes, locale === "en-US")}</span>
          <div className="work-dashboard-table-status" role="cell">
            <StatusBadge status={job.overallStatus} />
            {job.warningCount > 0 && <span className="work-dashboard-flag warning">{copy.warningCount(job.warningCount)}</span>}
            {job.overallStatus === "completed" && !job.hasSummary && <span className="work-dashboard-flag">{copy.noSummary}</span>}
          </div>
          <span className="work-dashboard-table-date" role="cell">{formatCreatedAt(job.createdAt, locale)}</span>
          <div className="work-dashboard-table-action" role="cell">
            {job.overallStatus === "completed" ? (
              <button
                className="work-dashboard-table-link"
                disabled={!canOpenResults}
                title={!canOpenResults ? operationUnavailable : undefined}
                type="button"
                onClick={() => void openJobWorkbenchWindow(job.id, job.title, source)}
              >
                {copy.openResult}
              </button>
            ) : (
              <Link
                className="work-dashboard-table-link"
                to={jobDetailPath({ jobId: job.id, source })}
              >
                {copy.openDetails}
              </Link>
            )}
          </div>
        </div>
      ))}
    </div>
  );
}

function ResourceItem({
  label,
  value,
  to,
  tone = "default",
}: {
  label: string;
  value: string;
  to: string;
  tone?: "default" | "success" | "warning";
}) {
  return (
    <Link className="work-dashboard-resource" data-tone={tone} to={to}>
      <span>{label}</span>
      <strong>{value}</strong>
    </Link>
  );
}

function trendValue(point: DashboardTrendPoint, metric: TrendMetric) {
  if (metric === "media") {
    return point.mediaDurationMinutes;
  }
  if (metric === "processing") {
    return Math.round(point.processingDurationSeconds / 60);
  }
  return point.totalJobs;
}

function formatNumber(value: number, locale: string) {
  return new Intl.NumberFormat(locale).format(value);
}

function formatPercent(value: number | null) {
  return value === null ? "—" : `${value}%`;
}

function formatMinutes(value: number, isEnglish: boolean) {
  const minutes = Math.max(0, Math.round(value));
  if (minutes < 60) {
    return isEnglish ? `${minutes} min` : `${minutes} 分钟`;
  }
  const hours = Math.floor(minutes / 60);
  const remainder = minutes % 60;
  if (isEnglish) {
    return remainder ? `${hours}h ${remainder}m` : `${hours}h`;
  }
  return remainder ? `${hours} 小时 ${remainder} 分钟` : `${hours} 小时`;
}

function formatDuration(seconds: number, isEnglish: boolean) {
  return formatMinutes(Math.round(Math.max(0, seconds) / 60), isEnglish);
}

function formatCreatedAt(value: string, locale: string) {
  return new Date(value).toLocaleString(locale, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function createCopy(isEnglish: boolean) {
  return isEnglish ? {
    title: "👋 · Welcome back! It's been a while.",
    remoteSnapshot: "Remote snapshot",
    rangeLabel: "Statistics range",
    ranges: [
      { value: "today" as const, label: "Today" },
      { value: "7d" as const, label: "7 days" },
      { value: "30d" as const, label: "30 days" },
      { value: "all" as const, label: "All" },
    ],
    refresh: "Refresh",
    newJob: "New Job",
    loadFailed: "Could not load dashboard data",
    retry: "Retry",
    staleData: "Refresh failed. Showing the last available snapshot.",
    loading: "Aggregating dashboard data…",
    coreMetrics: "Core meeting metrics",
    workspaceLabel: "Dashboard workspace",
    sections: [
      { key: "activity", label: "Task activity" },
      { key: "insights", label: "Analysis overview" },
    ],
    totalJobs: "Meeting jobs",
    mediaDuration: "Media duration",
    completionRate: "Completion rate",
    summaryCoverage: "AI summary coverage",
    activeMeta: (value: number) => `${value} currently processing`,
    processingMeta: (value: string) => `${value} processing time`,
    completionMeta: (completed: number, failed: number) => `${completed} completed · ${failed} failed`,
    coverageMeta: (ready: number, completed: number) => `${ready} of ${completed} completed jobs`,
    trend: "Workload trend",
    trendMetricLabel: "Trend metric",
    trendMetrics: [
      { value: "jobs" as const, label: "Jobs" },
      { value: "media" as const, label: "Media" },
      { value: "processing" as const, label: "Processing" },
    ],
    trendUnit: (metric: TrendMetric) => metric === "jobs" ? "jobs" : "min",
    emptyTrend: "No trend data in this range.",
    completeness: "Result completeness",
    transcript: "Transcript",
    speakers: "Speaker separation",
    summary: "AI summary",
    exported: "Exported",
    activeJobs: "Active",
    exceptions: "Exceptions",
    completedJobs: "Completed",
    summarizedJobs: "Summarized",
    failedJobs: "Failed",
    warningJobs: "Warnings",
    attention: "Needs attention",
    allJobs: "All jobs",
    emptyAttention: "Nothing needs attention in this range.",
    recentResults: "Recent results",
    recentJobs: "Recent jobs",
    emptyJobs: "No jobs in this range.",
    jobTitle: "Title",
    duration: "Duration",
    status: "Status",
    createdAt: "Created",
    action: "Action",
    allResults: "All results",
    emptyResults: "No completed results in this range.",
    openDetails: "Details",
    openResult: "View result",
    warningCount: (value: number) => `${value} warnings`,
    noSummary: "No summary",
    notExported: "Not exported",
    resources: "Processing resources",
    runtime: "Local runtime",
    ready: "Ready",
    needsSetup: "Needs setup",
    models: "AI models",
    modelValue: (enabled: number, total: number) => `${enabled} / ${total} enabled`,
    templates: "Templates",
    members: "Members",
    companion: "Companion & benefits",
    openCompanion: "Open companion",
    levelProgress: "Level progress",
    maxLevel: "Maximum level",
    checkIn: "Check-in",
    done: "Done",
    pending: "Pending",
    claimable: "Needs action",
    companionUnavailable: "Companion data has not been initialized.",
  } : {
    title: "👋 · 好久不见，欢迎回来！",
    remoteSnapshot: "远端任务快照",
    rangeLabel: "统计时间范围",
    ranges: [
      { value: "today" as const, label: "今日" },
      { value: "7d" as const, label: "近 7 天" },
      { value: "30d" as const, label: "近 30 天" },
      { value: "all" as const, label: "全部" },
    ],
    refresh: "刷新",
    newJob: "新建任务",
    loadFailed: "无法加载工作台数据",
    retry: "重试",
    staleData: "刷新失败，当前显示上一次可用数据。",
    loading: "正在聚合工作台数据…",
    coreMetrics: "核心会议指标",
    workspaceLabel: "工作台主内容",
    sections: [
      { key: "activity", label: "任务动态" },
      { key: "insights", label: "分析概览" },
    ],
    totalJobs: "会议任务",
    mediaDuration: "音视频时长",
    completionRate: "任务完成率",
    summaryCoverage: "AI 总结覆盖率",
    activeMeta: (value: number) => `${value} 个正在处理`,
    processingMeta: (value: string) => `累计处理 ${value}`,
    completionMeta: (completed: number, failed: number) => `${completed} 个完成 · ${failed} 个失败`,
    coverageMeta: (ready: number, completed: number) => `已完成任务中 ${ready} / ${completed}`,
    trend: "工作量趋势",
    trendMetricLabel: "趋势指标",
    trendMetrics: [
      { value: "jobs" as const, label: "任务" },
      { value: "media" as const, label: "音视频" },
      { value: "processing" as const, label: "处理耗时" },
    ],
    trendUnit: (metric: TrendMetric) => metric === "jobs" ? "个" : "分钟",
    emptyTrend: "当前时间范围内还没有趋势数据。",
    completeness: "结果完善度",
    transcript: "逐字稿",
    speakers: "讲话人分离",
    summary: "AI 总结",
    exported: "已导出",
    activeJobs: "处理中",
    exceptions: "异常与告警",
    completedJobs: "已完成",
    summarizedJobs: "已总结",
    failedJobs: "失败",
    warningJobs: "有告警",
    attention: "待关注",
    allJobs: "全部任务",
    emptyAttention: "当前时间范围内没有需要关注的任务。",
    recentResults: "最近结果",
    recentJobs: "最近任务",
    emptyJobs: "当前时间范围内还没有任务。",
    jobTitle: "任务名称",
    duration: "时长",
    status: "状态",
    createdAt: "创建时间",
    action: "操作",
    allResults: "全部结果",
    emptyResults: "当前时间范围内还没有完成的结果。",
    openDetails: "详情",
    openResult: "查看结果",
    warningCount: (value: number) => `${value} 条告警`,
    noSummary: "待总结",
    notExported: "待导出",
    resources: "处理资源",
    runtime: "本地运行时",
    ready: "已就绪",
    needsSetup: "待配置",
    models: "AI 模型",
    modelValue: (enabled: number, total: number) => `${enabled} / ${total} 已启用`,
    templates: "总结模板",
    members: "人员库",
    companion: "伙伴与福利",
    openCompanion: "查看伙伴",
    levelProgress: "等级进度",
    maxLevel: "已满级",
    checkIn: "今日签到",
    done: "已完成",
    pending: "待签到",
    claimable: "待处理",
    companionUnavailable: "伙伴数据尚未初始化。",
  };
}
