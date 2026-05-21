import { message } from "@tauri-apps/plugin-dialog";
import { useEffect, useMemo, useState } from "react";
import { Link, useRouter } from "@/app/router/RouterContext";
import StatusBadge from "@/shared/components/StatusBadge";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { formatMessage, getMessages } from "@/shared/i18n";

export default function JobsView() {
  const store = useMeetingStore();
  const router = useRouter();
  const [deletingJobId, setDeletingJobId] = useState<string | null>(null);
  const [selectedJobId, setSelectedJobId] = useState<string | null>(null);
  const messages = getMessages(store.settings.locale).jobs;
  const commonMessages = getMessages(store.settings.locale).common;
  const shouldWarnModelDownloadRequired = !store.settings.backendUrl.trim() && store.runtimeStatus.status !== "ready";
  const sortedJobs = useMemo(
    () => [...store.jobs].sort((left, right) => right.createdAt.localeCompare(left.createdAt)),
    [store.jobs],
  );
  const completedJobs = store.jobs.filter((job) => job.overallStatus === "completed").length;
  const processingJobs = store.jobs.filter((job) =>
    ["queued", "transcribing", "speaker_processing", "summarizing"].includes(job.overallStatus),
  ).length;
  const failedJobs = store.jobs.filter((job) => job.overallStatus === "failed").length;
  const latestJob = sortedJobs[0] ?? null;
  const selectedJob = selectedJobId
    ? sortedJobs.find((job) => job.id === selectedJobId) ?? latestJob
    : latestJob;

  useEffect(() => {
    void store.refreshJobs();
  }, []);

  function isDeleting(jobId: string) {
    return deletingJobId === jobId;
  }

  function isDeleteDisabled(status: string) {
    return ["queued", "transcribing", "speaker_processing", "summarizing"].includes(status);
  }

  async function openJobDetail(jobId: string) {
    await router.push(`/jobs/${jobId}`);
  }

  async function deleteJob(jobId: string) {
    const job = store.getJobById(jobId);

    if (!job || isDeleteDisabled(job.overallStatus)) {
      return;
    }

    const confirmed = window.confirm(formatMessage(messages.deleteConfirm, { title: job.title }));
    if (!confirmed) {
      return;
    }

    setDeletingJobId(jobId);

    try {
      await store.deleteJob(jobId);
    } finally {
      setDeletingJobId(null);
    }
  }

  async function retryJob(jobId: string) {
    if (shouldWarnModelDownloadRequired) {
      await message(commonMessages.modelUnavailableMessage, {
        title: commonMessages.modelUnavailableTitle,
        kind: "warning",
      });
      return;
    }

    await store.retryJob(jobId);
  }

  function formatCreatedAt(value: string) {
    return new Date(value).toLocaleString(store.settings.locale, {
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  function formatFileDuration(minutes: number) {
    if (!minutes || minutes <= 0) {
      return messages.pending;
    }

    const totalMinutes = Math.max(1, Math.round(minutes));
    const hours = Math.floor(totalMinutes / 60);
    const remainingMinutes = totalMinutes % 60;

    if (hours <= 0) {
      return formatMessage(messages.minutes, { count: remainingMinutes });
    }

    if (remainingMinutes === 0) {
      return formatMessage(messages.hours, { count: hours });
    }

    return formatMessage(messages.hoursMinutes, { hours, minutes: remainingMinutes });
  }

  function formatProcessingDuration(seconds?: number) {
    if (typeof seconds !== "number" || seconds < 0) {
      return messages.notCompleted;
    }

    const totalSeconds = Math.max(0, Math.round(seconds));
    const hours = Math.floor(totalSeconds / 3600);
    const minutes = Math.floor((totalSeconds % 3600) / 60);
    const remainingSeconds = totalSeconds % 60;

    if (hours > 0) {
      return formatMessage(messages.processingWithHours, {
        hours,
        minutes: String(minutes).padStart(2, "0"),
      });
    }

    return formatMessage(messages.processingWithMinutes, {
      minutes,
      seconds: String(remainingSeconds).padStart(2, "0"),
    });
  }

  return (
    <section className="view-stack native-page native-split-page jobs-native-page">
      <article className="surface native-page-hero jobs-native-hero">
        <div className="section-heading">
          <div>
            <h3>{messages.pageTitle}</h3>
            <p className="section-copy">{messages.pageCopy}</p>
          </div>
        </div>
        <div className="summary-inline">
          <span>{messages.total} {sortedJobs.length}</span>
          <span>{messages.processing} {processingJobs}</span>
          <span>{messages.completed} {completedJobs}</span>
          <span>{messages.processFailed} {failedJobs}</span>
        </div>
      </article>

      <div className="native-split-layout">
        <article className="surface native-list-panel jobs-native-panel">
          <div className="section-heading">
            <div>
              <h3>{messages.queueTitle}</h3>
              <p className="section-copy">{messages.queueCopy}</p>
            </div>
          </div>

          <div className="jobs-table-scroll">
            <div className="jobs-table">
              <div className="jobs-table-head">
                <span>{messages.colTask}</span>
                <span>{messages.colFileInfo}</span>
                <span>{messages.colProcessingTime}</span>
                <span>{messages.colCreatedAt}</span>
                <span>{messages.colStatus}</span>
                <span>{messages.colActions}</span>
              </div>

              {sortedJobs.map((job) => (
                <div
                  key={job.id}
                  className={`jobs-row ${selectedJob?.id === job.id ? "selected" : ""}`}
                  onClick={() => setSelectedJobId(job.id)}
                  onDoubleClick={() => openJobDetail(job.id)}
                >
                  <div className="jobs-primary">
                    <strong>{job.title}</strong>
                    <div className="job-meta-line">{job.sourceFiles.map((file) => file.name).join(" · ")}</div>
                  </div>

                  <div className="jobs-cell">
                    <strong>{formatMessage(messages.filesCount, { count: job.sourceFiles.length })}</strong>
                    <div className="job-meta-line">
                      {formatMessage(messages.fileDuration, { duration: formatFileDuration(job.durationMinutes) })}
                    </div>
                    <div className="job-meta-line">
                      {job.enableSpeaker ? messages.diarizationEnabled : messages.transcriptOnly}
                    </div>
                  </div>

                  <div className="jobs-cell">
                    <strong>{formatProcessingDuration(job.processingDurationSeconds)}</strong>
                    <div className="job-meta-line">
                      {job.overallStatus === "completed"
                        ? messages.processCompleted
                        : job.overallStatus === "failed"
                          ? messages.processFailed
                          : messages.processRunning}
                    </div>
                  </div>

                  <div className="jobs-cell">{formatCreatedAt(job.createdAt)}</div>

                  <div className="jobs-cell">
                    <StatusBadge status={job.overallStatus} />
                  </div>

                  <div className="jobs-actions">
                    <Link className="text-button" to={`/jobs/${job.id}`} onClick={(event) => event.stopPropagation()}>
                      {messages.details}
                    </Link>
                    {job.overallStatus === "completed" && (
                      <Link className="primary-button small-button" to={`/jobs/${job.id}/workbench`} onClick={(event) => event.stopPropagation()}>
                        {messages.workbench}
                      </Link>
                    )}
                    {job.overallStatus === "failed" && (
                      <button
                        className="secondary-button small-button"
                        type="button"
                        onClick={(event) => {
                          event.stopPropagation();
                          void retryJob(job.id);
                        }}
                      >
                        {commonMessages.retry}
                      </button>
                    )}
                    <button
                      className="text-button small-button jobs-delete-button"
                      type="button"
                      disabled={isDeleteDisabled(job.overallStatus) || isDeleting(job.id)}
                      title={isDeleteDisabled(job.overallStatus) ? messages.deleteDisabled : messages.deleteAction}
                      onClick={(event) => {
                        event.stopPropagation();
                        void deleteJob(job.id);
                      }}
                    >
                      {isDeleting(job.id) ? messages.deleting : commonMessages.delete}
                    </button>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </article>

        <aside className="surface native-inspector-panel">
          <div className="section-heading">
            <h3>{messages.colStatus}</h3>
          </div>
          <div className="native-stat-list">
            <div>
              <span>{messages.total}</span>
              <strong>{sortedJobs.length}</strong>
            </div>
            <div>
              <span>{messages.processing}</span>
              <strong>{processingJobs}</strong>
            </div>
            <div>
              <span>{messages.completed}</span>
              <strong>{completedJobs}</strong>
            </div>
            <div>
              <span>{messages.processFailed}</span>
              <strong>{failedJobs}</strong>
            </div>
          </div>
          {selectedJob && (
            <div className="native-inspector-note jobs-selected-note">
              <span>{messages.selectedTask}</span>
              <strong>{selectedJob.title}</strong>
              <p>{formatCreatedAt(selectedJob.createdAt)}</p>
              <div className="job-meta-line">{selectedJob.sourceFiles.map((file) => file.name).join(" · ")}</div>
              <div className="button-row jobs-inspector-actions">
                <Link className="text-button small-button" to={`/jobs/${selectedJob.id}`}>
                  {messages.details}
                </Link>
                {selectedJob.overallStatus === "completed" && (
                  <Link className="primary-button small-button" to={`/jobs/${selectedJob.id}/workbench`}>
                    {messages.workbench}
                  </Link>
                )}
              </div>
              <p>{messages.doubleClickHint}</p>
            </div>
          )}
        </aside>
      </div>
    </section>
  );
}
