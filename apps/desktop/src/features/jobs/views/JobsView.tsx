import { message } from "@tauri-apps/plugin-dialog";
import { useEffect, useMemo, useState } from "react";
import { Link, useRouter } from "@/app/router/RouterContext";
import StatusBadge from "@/shared/components/StatusBadge";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { formatMessage, getMessages } from "@/shared/i18n";
import type { MeetingJob } from "@/shared/types/meeting";
import {
  jobDetailPath,
  jobRef,
  jobRefKey,
  jobWorkbenchPath,
} from "./jobRoutes";

function diarizationLabel(job: MeetingJob, messages: ReturnType<typeof getMessages>["jobs"]) {
  if (!job.enableSpeaker || job.diarizationStatus === "disabled") {
    return messages.transcriptOnly;
  }
  switch (job.diarizationStatus) {
    case "completed":
      return messages.diarizationCompleted;
    case "unavailable":
      return messages.diarizationUnavailable;
    case "failed":
      return messages.diarizationFailed;
    case "legacy_unverified":
      return messages.diarizationUnverified;
    case "pending":
    case "processing":
      return messages.diarizationEnabled;
  }
}

export default function JobsView() {
  const store = useMeetingStore();
  const router = useRouter();
  const [deletingJobId, setDeletingJobId] = useState<string | null>(null);
  const messages = getMessages(store.settings.locale).jobs;
  const commonMessages = getMessages(store.settings.locale).common;
  const operationUnavailable = getMessages(store.settings.locale).workbench.remoteOperationUnavailable;
  const sortedJobs = useMemo(
    () => store.jobs
      .filter((job) => job.source === store.settings.processingMode)
      .sort((left, right) => right.createdAt.localeCompare(left.createdAt)),
    [store.jobs, store.settings.processingMode],
  );
  const completedJobs = sortedJobs.filter((job) => job.overallStatus === "completed").length;
  const processingJobs = sortedJobs.filter((job) =>
    ["queued", "transcribing", "speaker_processing", "summarizing"].includes(job.overallStatus),
  ).length;
  const failedJobs = sortedJobs.filter((job) => job.overallStatus === "failed").length;

  useEffect(() => {
    void store.refreshJobs().catch(() => undefined);
  }, []);

  function isDeleting(job: MeetingJob) {
    return deletingJobId === jobRefKey(jobRef(job));
  }

  function supports(job: MeetingJob, operation: "jobs.read" | "jobs.result.read" | "jobs.retry" | "jobs.delete") {
    return job.source === "local" || store.canRemoteOperation(operation);
  }

  function supportsResult(job: MeetingJob) {
    return supports(job, "jobs.read") && supports(job, "jobs.result.read");
  }

  function isDeleteDisabled(job: MeetingJob) {
    return ["queued", "transcribing", "speaker_processing", "summarizing"].includes(job.overallStatus)
      || !supports(job, "jobs.delete");
  }

  async function openJobDetail(job: MeetingJob) {
    if (supports(job, "jobs.read")) {
      await router.push(jobDetailPath(jobRef(job)));
    }
  }

  async function deleteJob(job: MeetingJob) {
    if (isDeleteDisabled(job)) {
      return;
    }

    const confirmed = window.confirm(formatMessage(messages.deleteConfirm, { title: job.title }));
    if (!confirmed) {
      return;
    }

    setDeletingJobId(jobRefKey(jobRef(job)));

    try {
      await store.deleteJob(jobRef(job));
    } finally {
      setDeletingJobId(null);
    }
  }

  async function retryJob(job: MeetingJob) {
    if (!supports(job, "jobs.retry")) {
      return;
    }
    if (job.source === "local" && !store.runtimeStatus.shellReady) {
      await message(commonMessages.modelUnavailableMessage, {
        title: commonMessages.modelUnavailableTitle,
        kind: "warning",
      });
      return;
    }

    await store.retryJob(jobRef(job));
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
                  key={jobRefKey(jobRef(job))}
                  className="jobs-row"
                  onDoubleClick={() => void openJobDetail(job)}
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
                      {diarizationLabel(job, messages)}
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
                    {supports(job, "jobs.read") ? (
                      <Link className="text-button" to={jobDetailPath(jobRef(job))} onClick={(event) => event.stopPropagation()}>
                        {messages.details}
                      </Link>
                    ) : (
                      <button className="text-button" type="button" disabled title={store.remoteError ?? operationUnavailable}>
                        {messages.details}
                      </button>
                    )}
                    {job.overallStatus === "completed" && (
                      supportsResult(job) ? (
                        <Link className="primary-button small-button" to={jobWorkbenchPath(jobRef(job))} onClick={(event) => event.stopPropagation()}>
                          {messages.workbench}
                        </Link>
                      ) : (
                        <button className="primary-button small-button" type="button" disabled title={store.remoteError ?? operationUnavailable}>
                          {messages.workbench}
                        </button>
                      )
                    )}
                    {job.overallStatus === "failed" && (
                      <button
                        className="secondary-button small-button"
                        type="button"
                        disabled={!supports(job, "jobs.retry")}
                        title={!supports(job, "jobs.retry") ? store.remoteError ?? operationUnavailable : undefined}
                        onClick={(event) => {
                          event.stopPropagation();
                          void retryJob(job);
                        }}
                      >
                        {commonMessages.retry}
                      </button>
                    )}
                    <button
                      className="text-button small-button jobs-delete-button"
                      type="button"
                      disabled={isDeleteDisabled(job) || isDeleting(job)}
                      title={!supports(job, "jobs.delete")
                        ? store.remoteError ?? operationUnavailable
                        : isDeleteDisabled(job)
                          ? messages.deleteDisabled
                          : messages.deleteAction}
                      onClick={(event) => {
                        event.stopPropagation();
                        void deleteJob(job);
                      }}
                    >
                      {isDeleting(job) ? messages.deleting : commonMessages.delete}
                    </button>
                  </div>
                </div>
              ))}
            </div>
          </div>
      </article>
    </section>
  );
}
