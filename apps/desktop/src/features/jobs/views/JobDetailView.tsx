import { message } from "@tauri-apps/plugin-dialog";
import { useEffect, useMemo, useState } from "react";
import { Link, useRouter } from "@/app/router/RouterContext";
import StatusBadge from "@/shared/components/StatusBadge";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { getMessages } from "@/shared/i18n";
import progressBarUrl from "@/assets/progress-bar.webp";
import type { JobStage } from "@/shared/types/meeting";

const ansiPattern = /\u001b\[[0-9;]*m/g;
const stageProgressBands: Record<JobStage, { min: number; max: number }> = {
  idle: { min: 0, max: 0 },
  uploaded: { min: 4, max: 8 },
  queued: { min: 8, max: 14 },
  transcribing: { min: 18, max: 72 },
  speaker_processing: { min: 72, max: 96 },
  summarizing: { min: 96, max: 99 },
  completed: { min: 100, max: 100 },
  failed: { min: 0, max: 99 },
};

export default function JobDetailView() {
  const router = useRouter();
  const store = useMeetingStore();
  const jobId = router.params.id ?? "";
  const job = store.getJobById(jobId);
  const messages = getMessages(store.settings.locale).jobDetail;
  const commonMessages = getMessages(store.settings.locale).common;
  const statusMessages = getMessages(store.settings.locale).status;
  const shouldWarnModelDownloadRequired = !store.settings.backendUrl.trim() && store.runtimeStatus.status !== "ready";
  const [displayProgressPercent, setDisplayProgressPercent] = useState(0);
  const [progressRunKey, setProgressRunKey] = useState("");
  const stages = job
    ? [
        { label: messages.stageUploaded, status: job.uploadStatus },
        { label: messages.stageAsr, status: job.asrStatus },
        { label: messages.stageSummary, status: job.summaryStatus },
        { label: messages.stageOverall, status: job.overallStatus },
      ]
    : [];

  useEffect(() => {
    if (jobId) {
      void store.refreshJob(jobId);
    }
  }, [jobId]);

  function clampPercent(value: number, min: number, max: number) {
    return Math.max(min, Math.min(max, Math.round(value)));
  }

  function getProgressStage() {
    if (!job) {
      return "idle" as JobStage;
    }

    if (job.overallStatus !== "idle") {
      return job.overallStatus;
    }

    if (job.summaryStatus !== "idle") {
      return job.summaryStatus;
    }

    if (job.asrStatus !== "idle") {
      return job.asrStatus;
    }

    return job.uploadStatus;
  }

  function resolveStageProgressPercent() {
    const stage = getProgressStage();
    const rawPercent = job?.progressPercent;

    if (stage === "completed") {
      return 100;
    }

    if (typeof rawPercent !== "number" || Number.isNaN(rawPercent)) {
      return stageProgressBands[stage].min;
    }

    if (stage === "failed") {
      return clampPercent(rawPercent, 0, 99);
    }

    const { min, max } = stageProgressBands[stage];
    return clampPercent(rawPercent, min, max);
  }

  useEffect(() => {
    if (!job) {
      setDisplayProgressPercent(0);
      setProgressRunKey("");
      return;
    }

    const runKey = `${job.id}:${job.processingStartedAtMs ?? 0}`;
    const nextPercent = resolveStageProgressPercent();

    if (progressRunKey !== runKey) {
      setProgressRunKey(runKey);
      setDisplayProgressPercent(nextPercent);
      return;
    }

    setDisplayProgressPercent((current) => Math.max(current, nextPercent));
  }, [job?.id, job?.processingStartedAtMs, job?.overallStatus, job?.summaryStatus, job?.asrStatus, job?.uploadStatus, job?.progressPercent]);

  const progressMessage = useMemo(() => {
    if (!job) {
      return "";
    }

    const explicit = job.progressMessage?.trim();
    if (explicit) {
      return explicit;
    }

    return statusMessages[job.overallStatus as keyof typeof statusMessages] ?? job.overallStatus;
  }, [job, statusMessages]);

  const logEntries = useMemo(() => {
    const raw = job?.processLog ?? "";
    return raw
      .split(/[\r\n]+/)
      .map((line) => line.replace(ansiPattern, "").trim())
      .filter((line) => line.length > 0)
      .reverse()
      .map((line, index) => ({
        id: `${index}-${line.slice(0, 24)}`,
        text: line,
        tone: classifyLogLine(line),
      }));
  }, [job?.processLog]);

  function classifyLogLine(line: string) {
    const normalized = line.toLowerCase();
    if (
      normalized.includes("traceback")
      || normalized.includes("permissionerror")
      || normalized.includes("runtimeerror")
      || normalized.includes("error")
      || normalized.includes("failed")
      || normalized.includes("失败")
    ) {
      return "error";
    }

    if (normalized.includes("warning") || normalized.includes("warn")) {
      return "warning";
    }

    if (
      normalized.includes("completed")
      || normalized.includes("success")
      || normalized.includes("已完成")
      || normalized.includes("完成")
    ) {
      return "success";
    }

    return "info";
  }

  async function retryJob(targetJobId: string) {
    if (shouldWarnModelDownloadRequired) {
      await message(commonMessages.modelUnavailableMessage, {
        title: commonMessages.modelUnavailableTitle,
        kind: "warning",
      });
      return;
    }

    await store.retryJob(targetJobId);
  }

  return (
    <section className="view-stack native-page detail-native-page">
      {job ? (
        <div className="detail-grid">
          <article className="surface native-page-hero detail-hero full-span">
            <div className="job-title-line detail-hero-head">
              <div className="native-title-stack">
                <Link className="text-button small-button native-back-link" to="/jobs">
                  {messages.backToJobs}
                </Link>
                <div>
                  <h3>{job.title}</h3>
                  <p className="section-copy">{job.sourceFiles.map((file) => file.name).join(" · ")}</p>
                </div>
              </div>
              <StatusBadge status={job.overallStatus} />
            </div>

            <div className="detail-stage-grid">
              {stages.map((stage) => (
                <div key={stage.label} className="detail-stage-card">
                  <span className="detail-stage-label">{stage.label}</span>
                  <StatusBadge status={stage.status} />
                </div>
              ))}
            </div>

            <div className="detail-hero-footer">
              <div className="summary-inline">
                <span>{messages.inputFiles} {job.sourceFiles.length}</span>
                <span>{messages.hotwords} {job.hotwords.length}</span>
                <span>{messages.speaker} {job.enableSpeaker ? commonMessages.enabled : commonMessages.disabled}</span>
              </div>

              <div className="button-row">
                {job.overallStatus === "completed" && (
                  <Link className="primary-button" to={`/jobs/${job.id}/workbench`}>
                    {messages.viewWorkbench}
                  </Link>
                )}
                {job.overallStatus === "failed" && (
                  <button className="secondary-button" type="button" onClick={() => retryJob(job.id)}>
                    {messages.retryJob}
                  </button>
                )}
              </div>
            </div>
          </article>

          <article className="surface native-info-panel detail-main-column">
            <div className="section-heading">
              <h3>{messages.filesSection}</h3>
            </div>
            <div className="file-list">
              {job.sourceFiles.map((file) => (
                <div key={file.id} className="file-pill">
                  <div>
                    <strong>{file.name}</strong>
                    <div className="job-meta-line">
                      {file.kind === "audio" ? commonMessages.audio : commonMessages.video} · {file.sizeLabel}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </article>

          <article className="surface native-info-panel detail-side-column">
            <div className="section-heading">
              <h3>{messages.settingsSection}</h3>
            </div>
            <div className="metric-strip metric-strip-tight">
              <div className="metric-pill">
                <span className="muted">{messages.language}</span>
                <strong>{job.lang}</strong>
              </div>
              <div className="metric-pill">
                <span className="muted">{messages.speakerDiarization}</span>
                <strong>{job.enableSpeaker ? commonMessages.enabled : commonMessages.disabled}</strong>
              </div>
              <div className="metric-pill">
                <span className="muted">{messages.hotwords}</span>
                <strong>{job.hotwords.length}</strong>
              </div>
            </div>
          </article>

          <article className="surface native-info-panel detail-progress-card full-span">
            <div className="section-heading">
              <h3>{messages.progressSection}</h3>
              <strong className="detail-progress-percent">{displayProgressPercent}%</strong>
            </div>

            <div className="detail-progress-panel">
              <div className="detail-progress-meta">
                <span className="detail-progress-label">{messages.stageOverall}</span>
                <StatusBadge status={job.overallStatus} />
              </div>
              <div className="detail-progress-track">
                <div className="detail-progress-fill" style={{ width: `${displayProgressPercent}%` }}>
                  <img className="detail-progress-media" src={progressBarUrl} alt="" aria-hidden="true" />
                </div>
              </div>
              <p className="section-copy detail-progress-copy">{progressMessage}</p>
            </div>
          </article>

          <article className="surface native-info-panel detail-log-card full-span">
            <div className="section-heading">
              <h3>{messages.logSection}</h3>
            </div>
            {job.failureReason && <div className="note-block error-block">{job.failureReason}</div>}
            {logEntries.length ? (
              <div className="job-log-list">
                {logEntries.map((entry) => (
                  <div key={entry.id} className={`job-log-entry job-log-entry-${entry.tone}`}>
                    {entry.text}
                  </div>
                ))}
              </div>
            ) : (
              <div className="empty-state">{messages.noLog}</div>
            )}
          </article>
        </div>
      ) : (
        <div className="empty-state">{messages.notFound}</div>
      )}
    </section>
  );
}
