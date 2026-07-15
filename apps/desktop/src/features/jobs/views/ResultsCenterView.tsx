import { useEffect, useMemo, useState } from "react";
import { Link } from "@/app/router/RouterContext";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import MeetingNotesPanel from "@/shared/components/MeetingNotesPanel";
import StatusBadge from "@/shared/components/StatusBadge";
import { exportJob } from "@/shared/services/export/jobExport";
import { getPrimaryTranscriptSegments } from "@/shared/services/meeting/transcript";
import { openAiSummaryWindow, openMeetingNotesWindow } from "@/shared/services/ui/windows";
import { getMessages } from "@/shared/i18n";
import type { MeetingJob } from "@/shared/types/meeting";
import {
  jobDetailPath,
  jobRef,
  jobRefKey,
  jobWorkbenchPath,
  readResultsJobRef,
  resultsPath,
  type JobRouteRef,
} from "./jobRoutes";
import "./WorkHubViews.css";

export default function ResultsCenterView() {
  const store = useMeetingStore();
  const operationUnavailable = getMessages(store.settings.locale).workbench.remoteOperationUnavailable;
  const isEnglish = store.settings.locale === "en-US";
  const copy = isEnglish
    ? {
        title: "Results Center",
        description: "Choose a completed meeting and open every result from one place.",
        newJob: "New Job",
        completedJobs: "Completed Jobs",
        noCompleted: "No completed jobs yet.",
        transcript: "Transcript",
        aiSummary: "AI Summary",
        remoteSummaryUnavailable: "Remote summary editing is not available for this service.",
        notes: "Meeting Notes",
        exportWord: "Export Word",
        exporting: "Exporting...",
        jobDetails: "Job Details",
        resultOverview: "Result Overview",
        transcriptPreview: "Transcript Preview",
        notesPreview: "Notes Preview",
        segments: "Segments",
        summaries: "Summary Runs",
        files: "Files",
        minutes: "Minutes",
        emptyTranscript: "No transcript content.",
        moreSegments: "Open the transcript to view all segments.",
      }
    : {
        title: "结果中心",
        description: "选择已完成的会议，在一个页面直接进入逐字稿、AI 总结、会议纪要和导出。",
        newJob: "新建任务",
        completedJobs: "已完成任务",
        noCompleted: "还没有已完成的会议任务。",
        transcript: "查看逐字稿",
        aiSummary: "AI 总结",
        remoteSummaryUnavailable: "当前远端服务未提供总结版本编辑能力。",
        notes: "会议纪要",
        exportWord: "导出 Word",
        exporting: "正在导出...",
        jobDetails: "任务详情",
        resultOverview: "结果概览",
        transcriptPreview: "逐字稿预览",
        notesPreview: "会议纪要预览",
        segments: "逐字稿",
        summaries: "总结记录",
        files: "文件",
        minutes: "分钟",
        emptyTranscript: "当前没有逐字稿内容。",
        moreSegments: "打开逐字稿可查看全部内容。",
      };
  const hasRequestedJob = new URLSearchParams(window.location.search).has("job");
  const [selectedJobRef, setSelectedJobRef] = useState<JobRouteRef | null>(() => {
    const params = new URLSearchParams(window.location.search);
    return params.has("source")
      ? readResultsJobRef(store.settings.processingMode)
      : null;
  });
  const [isExporting, setIsExporting] = useState(false);

  useEffect(() => {
    void store.refreshJobs().catch(() => undefined);
  }, []);

  useEffect(() => {
    if (store.settingsLoaded && !selectedJobRef) {
      setSelectedJobRef(readResultsJobRef(store.settings.processingMode));
    }
  }, [store.settingsLoaded, selectedJobRef, store.settings.processingMode]);

  const completedJobs = useMemo(
    () => store.jobs
      .filter((job) => (
        job.source === store.settings.processingMode
        && job.overallStatus === "completed"
      ))
      .sort((left, right) => right.createdAt.localeCompare(left.createdAt)),
    [store.jobs, store.settings.processingMode],
  );
  const requestedJob = selectedJobRef
    ? store.getJobById(selectedJobRef.jobId, selectedJobRef.source)
    : undefined;
  const selectedJob = selectedJobRef
    ? (requestedJob?.overallStatus === "completed" ? requestedJob : null)
    : hasRequestedJob ? null : completedJobs[0] ?? null;
  const canReadSelectedJob = Boolean(
    selectedJob
    && (selectedJob.source === "local" || store.canRemoteOperation("jobs.read")),
  );
  const canReadSelectedResult = Boolean(
    canReadSelectedJob
    && selectedJob
    && (selectedJob.source === "local" || store.canRemoteOperation("jobs.result.read")),
  );
  const transcriptSegments = useMemo(
    () => selectedJob && canReadSelectedResult ? getPrimaryTranscriptSegments(selectedJob) : [],
    [selectedJob, canReadSelectedResult],
  );

  useEffect(() => {
    if (selectedJob) {
      const nextRef = jobRef(selectedJob);
      if (!selectedJobRef || jobRefKey(nextRef) !== jobRefKey(selectedJobRef)) {
        setSelectedJobRef(nextRef);
        window.history.replaceState({}, "", resultsPath(nextRef));
      }
    }
  }, [selectedJob?.id, selectedJob?.source, selectedJobRef]);

  useEffect(() => {
    if (selectedJobRef) {
      void store.refreshJobResult(selectedJobRef).catch(() => undefined);
    }
  }, [selectedJobRef?.jobId, selectedJobRef?.source]);

  function chooseJob(job: MeetingJob) {
    const nextRef = jobRef(job);
    setSelectedJobRef(nextRef);
    window.history.replaceState({}, "", resultsPath(nextRef));
  }

  async function exportWord() {
    if (!selectedJob || selectedJob.source !== "local") return;
    setIsExporting(true);
    try {
      const snapshot = await store.refreshJobResult(jobRef(selectedJob));
      if (snapshot) await exportJob(snapshot, "word");
    } finally {
      setIsExporting(false);
    }
  }

  return (
    <section className="view-stack native-page results-center-page">
      <header className="work-hub-page-head">
        <div><h2>{copy.title}</h2><p>{copy.description}</p></div>
        {store.settings.processingMode === "local" ? (
          <Link className="primary-button" to="/jobs/new">{copy.newJob}</Link>
        ) : (
          <button className="primary-button" type="button" disabled title={operationUnavailable}>
            {copy.newJob}
          </button>
        )}
      </header>

      <div className="results-center-layout">
        <aside className="surface results-job-panel">
          <div className="work-hub-panel-head"><div><h3>{copy.completedJobs}</h3><p>{completedJobs.length}</p></div></div>
          <div className="results-job-list">
            {completedJobs.map((job) => (
              <button
                className={`results-job-row ${selectedJob?.id === job.id ? "selected" : ""}`}
                type="button"
                key={jobRefKey(jobRef(job))}
                onClick={() => chooseJob(job)}
              >
                <span><strong>{job.title}</strong><small>{new Date(job.createdAt).toLocaleString(store.settings.locale)}</small></span>
                <StatusBadge status={job.overallStatus} />
              </button>
            ))}
            {!completedJobs.length && <div className="work-hub-empty">{copy.noCompleted}</div>}
          </div>
        </aside>

        <article className="surface results-detail-panel">
          {selectedJob && canReadSelectedResult ? (
            <>
              <div className="results-detail-head">
                <div><span>{copy.resultOverview}</span><h3>{selectedJob.title}</h3></div>
                <StatusBadge status={selectedJob.overallStatus} />
              </div>

              <div className="results-actions">
                <Link className="primary-button" to={jobWorkbenchPath(jobRef(selectedJob))}>{copy.transcript}</Link>
                <button
                  className="secondary-button"
                  type="button"
                  disabled={selectedJob.source !== "local"}
                  title={selectedJob.source === "local" ? undefined : copy.remoteSummaryUnavailable}
                  onClick={() => openAiSummaryWindow(selectedJob.id, selectedJob.title, selectedJob.source)}
                >
                  {copy.aiSummary}
                </button>
                <button
                  className="secondary-button"
                  type="button"
                  disabled={selectedJob.source !== "local"}
                  title={selectedJob.source !== "local" ? operationUnavailable : undefined}
                  onClick={() => openMeetingNotesWindow(selectedJob.id, selectedJob.title, selectedJob.source)}
                >{copy.notes}</button>
                <button
                  className="secondary-button"
                  type="button"
                  disabled={isExporting || selectedJob.source !== "local"}
                  title={selectedJob.source !== "local" ? operationUnavailable : undefined}
                  onClick={exportWord}
                >{isExporting ? copy.exporting : copy.exportWord}</button>
                <Link className="text-button" to={jobDetailPath(jobRef(selectedJob))}>{copy.jobDetails}</Link>
              </div>

              <div className="results-metrics">
                <div><span>{copy.segments}</span><strong>{transcriptSegments.length}</strong></div>
                <div><span>{copy.summaries}</span><strong>{selectedJob.summaryRuns.length}</strong></div>
                <div><span>{copy.files}</span><strong>{selectedJob.sourceFiles.length}</strong></div>
                <div><span>{copy.minutes}</span><strong>{Math.max(0, Math.round(selectedJob.durationMinutes))}</strong></div>
              </div>

              <section className="results-preview-section">
                <div className="results-preview-head"><h3>{copy.transcriptPreview}</h3><span>{copy.moreSegments}</span></div>
                <div className="results-transcript-preview">
                  {transcriptSegments.slice(0, 6).map((segment) => (
                    <div key={segment.id}><strong>{segment.speaker?.trim() || "-"}</strong><p>{segment.text}</p></div>
                  ))}
                  {!transcriptSegments.length && <div className="work-hub-empty">{copy.emptyTranscript}</div>}
                </div>
              </section>

              <section className="results-preview-section">
                <div className="results-preview-head"><h3>{copy.notesPreview}</h3></div>
                <div className="results-notes-preview">
                  <MeetingNotesPanel summary={selectedJob.summary} />
                </div>
              </section>
            </>
          ) : selectedJob ? (
            <div className="work-hub-empty results-empty-detail">
              {store.remoteError ?? operationUnavailable}
            </div>
          ) : (
            <div className="work-hub-empty results-empty-detail">{copy.noCompleted}</div>
          )}
        </article>
      </div>
    </section>
  );
}
