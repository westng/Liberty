import { useEffect, useMemo } from "react";
import { Link } from "@/app/router/RouterContext";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import StatusBadge from "@/shared/components/StatusBadge";
import { getMessages } from "@/shared/i18n";
import type { MeetingJob } from "@/shared/types/meeting";
import {
  jobDetailPath,
  jobRef,
  jobRefKey,
  resultsPath,
} from "./jobRoutes";
import "./WorkHubViews.css";

const ACTIVE_STATUSES = ["queued", "transcribing", "speaker_processing", "summarizing"];

export default function WorkDashboardView() {
  const store = useMeetingStore();
  const isEnglish = store.settings.locale === "en-US";
  const operationUnavailable = getMessages(store.settings.locale).workbench.remoteOperationUnavailable;
  const copy = isEnglish
    ? {
        title: "Dashboard",
        description: "Track active meeting jobs and move directly into completed results.",
        newJob: "New Job",
        total: "All Jobs",
        processing: "Processing",
        completed: "Completed",
        failed: "Failed",
        queue: "Current Queue",
        queueCopy: "Jobs that still need attention or processing.",
        recent: "Recent Results",
        recentCopy: "Open transcripts, summaries, notes, and exports without passing through job details.",
        emptyQueue: "No jobs are currently processing or waiting for attention.",
        emptyResults: "No completed results yet.",
        viewQueue: "View Queue",
        openDetails: "Details",
        openResult: "Open Result",
      }
    : {
        title: "工作台",
        description: "集中查看任务进度，并直接进入最近完成的会议结果。",
        newJob: "新建任务",
        total: "全部任务",
        processing: "处理中",
        completed: "已完成",
        failed: "失败",
        queue: "当前队列",
        queueCopy: "仍在处理或需要关注的会议任务。",
        recent: "最近结果",
        recentCopy: "无需经过任务详情，直接查看逐字稿、总结、纪要和导出。",
        emptyQueue: "当前没有处理中或需要关注的任务。",
        emptyResults: "还没有已完成的会议结果。",
        viewQueue: "查看队列",
        openDetails: "任务详情",
        openResult: "打开结果",
      };

  useEffect(() => {
    void store.refreshJobs().catch(() => undefined);
  }, []);

  const sortedJobs = useMemo(
    () => store.jobs
      .filter((job) => job.source === store.settings.processingMode)
      .sort((left, right) => right.createdAt.localeCompare(left.createdAt)),
    [store.jobs, store.settings.processingMode],
  );
  const processingJobs = sortedJobs.filter((job) => ACTIVE_STATUSES.includes(job.overallStatus));
  const failedJobs = sortedJobs.filter((job) => job.overallStatus === "failed");
  const queueJobs = [...processingJobs, ...failedJobs].slice(0, 6);
  const completedJobs = sortedJobs.filter((job) => job.overallStatus === "completed");

  function formatCreatedAt(value: string) {
    return new Date(value).toLocaleString(store.settings.locale, {
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  function supports(job: MeetingJob, operation: "jobs.read" | "jobs.result.read") {
    return job.source === "local" || store.canRemoteOperation(operation);
  }

  function supportsResult(job: MeetingJob) {
    return supports(job, "jobs.read") && supports(job, "jobs.result.read");
  }

  return (
    <section className="view-stack native-page work-dashboard-page">
      <header className="work-hub-page-head">
        <div>
          <h2>{copy.title}</h2>
          <p>{copy.description}</p>
        </div>
        {store.settings.processingMode === "local" ? (
          <Link className="primary-button" to="/jobs/new">{copy.newJob}</Link>
        ) : (
          <button className="primary-button" type="button" disabled title={operationUnavailable}>
            {copy.newJob}
          </button>
        )}
      </header>

      <section className="surface work-dashboard-metrics" aria-label={copy.title}>
        <div><span>{copy.total}</span><strong>{sortedJobs.length}</strong></div>
        <div><span>{copy.processing}</span><strong>{processingJobs.length}</strong></div>
        <div><span>{copy.completed}</span><strong>{completedJobs.length}</strong></div>
        <div><span>{copy.failed}</span><strong>{failedJobs.length}</strong></div>
      </section>

      <div className="work-dashboard-grid">
        <section className="surface work-hub-panel">
          <div className="work-hub-panel-head">
            <div><h3>{copy.queue}</h3><p>{copy.queueCopy}</p></div>
            <Link className="text-button" to="/jobs">{copy.viewQueue}</Link>
          </div>
          <div className="work-hub-list">
            {queueJobs.length ? queueJobs.map((job) => (
              <div className="work-hub-row" key={jobRefKey(jobRef(job))}>
                <div className="work-hub-row-main">
                  <strong>{job.title}</strong>
                  <span>{formatCreatedAt(job.createdAt)} · {job.sourceFiles.map((file) => file.name).join(" · ")}</span>
                </div>
                <StatusBadge status={job.overallStatus} />
                {supports(job, "jobs.read") ? (
                  <Link className="text-button small-button" to={jobDetailPath(jobRef(job))}>{copy.openDetails}</Link>
                ) : (
                  <button className="text-button small-button" type="button" disabled title={store.remoteError ?? operationUnavailable}>
                    {copy.openDetails}
                  </button>
                )}
              </div>
            )) : <div className="work-hub-empty">{copy.emptyQueue}</div>}
          </div>
        </section>

        <section className="surface work-hub-panel">
          <div className="work-hub-panel-head">
            <div><h3>{copy.recent}</h3><p>{copy.recentCopy}</p></div>
            <Link className="text-button" to="/results">{copy.openResult}</Link>
          </div>
          <div className="work-hub-list">
            {completedJobs.slice(0, 6).map((job) => (
              <div className="work-hub-row" key={jobRefKey(jobRef(job))}>
                <div className="work-hub-row-main">
                  <strong>{job.title}</strong>
                  <span>
                    {formatCreatedAt(job.createdAt)}
                    {supportsResult(job)
                      ? ` · ${job.transcriptSegments.length} ${isEnglish ? "segments" : "条逐字稿"}`
                      : ""}
                  </span>
                </div>
                {supportsResult(job) ? (
                  <Link className="primary-button small-button" to={resultsPath(jobRef(job))}>{copy.openResult}</Link>
                ) : (
                  <button className="primary-button small-button" type="button" disabled title={store.remoteError ?? operationUnavailable}>
                    {copy.openResult}
                  </button>
                )}
              </div>
            ))}
            {!completedJobs.length && <div className="work-hub-empty">{copy.emptyResults}</div>}
          </div>
        </section>
      </div>
    </section>
  );
}
