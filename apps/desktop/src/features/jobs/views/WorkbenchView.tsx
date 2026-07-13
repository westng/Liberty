import { confirm } from "@tauri-apps/plugin-dialog";
import { useEffect, useMemo, useState } from "react";
import { Link, useRouter } from "@/app/router/RouterContext";
import StatusBadge from "@/shared/components/StatusBadge";
import { useAiStore } from "@/features/ai/stores/useAiStore";
import TranscriptTimeline from "@/shared/components/TranscriptTimeline";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { exportJob } from "@/shared/services/export/jobExport";
import { formatMessage, getMessages } from "@/shared/i18n";
import { getPrimaryTranscriptSegments } from "@/shared/services/meeting/transcript";
import { openAiSummaryWindow, openMeetingNotesWindow } from "@/shared/services/ui/windows";

const ALL_SPEAKERS = "__all__";

export default function WorkbenchView() {
  const router = useRouter();
  const store = useMeetingStore();
  const aiStore = useAiStore();
  const jobId = router.params.id ?? "";
  const job = store.getJobById(jobId);
  const [query, setQuery] = useState("");
  const [selectedSpeaker, setSelectedSpeaker] = useState(ALL_SPEAKERS);
  const [isExporting, setIsExporting] = useState(false);
  const [isRenamingSpeaker, setIsRenamingSpeaker] = useState(false);
  const messages = getMessages(store.settings.locale).workbench;
  const commonMessages = getMessages(store.settings.locale).common;
  const transcriptSegments = useMemo(() => (job ? getPrimaryTranscriptSegments(job) : []), [job]);

  function normalizeSpeakerLabel(value?: string) {
    return value?.trim() || commonMessages.unknownSpeaker;
  }

  const speakerOptions = useMemo(() => {
    const counts = new Map<string, number>();

    for (const segment of transcriptSegments) {
      const label = normalizeSpeakerLabel(segment.speaker);
      counts.set(label, (counts.get(label) ?? 0) + 1);
    }

    return [
      {
        key: ALL_SPEAKERS,
        label: messages.allSpeakers,
        count: transcriptSegments.length,
      },
      ...Array.from(counts.entries()).map(([label, count]) => ({
        key: label,
        label,
        count,
      })),
    ];
  }, [commonMessages.unknownSpeaker, messages.allSpeakers, transcriptSegments]);
  const speakerFilteredSegments = selectedSpeaker === ALL_SPEAKERS
    ? transcriptSegments
    : transcriptSegments.filter((segment) => normalizeSpeakerLabel(segment.speaker) === selectedSpeaker);
  const activeSummaryRun = job?.summaryRuns.find((run) => run.id === job.activeSummaryRunId);
  const activeTemplateName = activeSummaryRun ? aiStore.getTemplateById(activeSummaryRun.templateId)?.name : "";

  useEffect(() => {
    if (jobId) {
      void store.refreshJob(jobId);
    }
  }, [jobId]);

  async function doExport(kind: "transcript" | "notes" | "bundle" | "word") {
    if (!job) {
      return;
    }

    setIsExporting(true);

    try {
      const exportSnapshot = kind === "word" ? await store.refreshJob(job.id) : job;
      if (exportSnapshot) {
        await exportJob(exportSnapshot, kind);
      }
    } finally {
      setIsExporting(false);
    }
  }

  async function launchAiSummary() {
    if (!job) {
      return;
    }

    await openAiSummaryWindow(job.id, job.title);
  }

  async function openNotes() {
    if (!job) {
      return;
    }

    await openMeetingNotesWindow(job.id, job.title);
  }

  async function renameSpeaker(fromSpeaker: string, toSpeaker: string) {
    if (!job) {
      return;
    }

    const sourceLabel = fromSpeaker.trim() || commonMessages.unknownSpeaker;
    const targetLabel = toSpeaker.trim();

    if (!targetLabel) {
      return;
    }

    const confirmed = await confirm(
      formatMessage(messages.renameSpeakerConfirm, {
        source: sourceLabel,
        target: targetLabel,
      }),
      {
        title: messages.renameSpeakerTitle,
        kind: "warning",
        okLabel: messages.replace,
        cancelLabel: commonMessages.cancel,
      },
    );

    if (!confirmed) {
      return;
    }

    setIsRenamingSpeaker(true);

    try {
      await store.renameSpeaker(job.id, fromSpeaker, targetLabel);
      if (selectedSpeaker === sourceLabel) {
        setSelectedSpeaker(targetLabel);
      }
    } finally {
      setIsRenamingSpeaker(false);
    }
  }

  return (
    <section className="view-stack native-page workbench-native-page">
      {job ? (
        <div className="workbench-grid">
          <article className="surface native-page-hero full-span workbench-hero">
            <div className="job-title-line workbench-hero-head">
              <div className="native-title-stack">
                <Link className="text-button small-button native-back-link" to={`/jobs/${job.id}`}>
                  {messages.backToDetail}
                </Link>
                <div>
                  <h3>{job.title}</h3>
                  <p className="section-copy">{messages.heroCopy}</p>
                </div>
              </div>
              <StatusBadge status={job.overallStatus} />
            </div>

            <div className="workbench-hero-actions">
              <button className="primary-button" type="button" disabled={!transcriptSegments.length} onClick={launchAiSummary}>
                {messages.aiSummary}
              </button>
              <button className="secondary-button" type="button" onClick={openNotes}>
                {messages.viewNotes}
              </button>
              <button className="primary-button" type="button" onClick={() => doExport("bundle")}>
                {isExporting ? messages.exporting : messages.exportBundle}
              </button>
              <button className="secondary-button" type="button" onClick={() => doExport("transcript")}>
                {messages.exportTranscript}
              </button>
              <button className="secondary-button" type="button" onClick={() => doExport("notes")}>
                {messages.exportNotes}
              </button>
              <button className="secondary-button" type="button" onClick={() => doExport("word")}>
                {messages.exportWord}
              </button>
            </div>

            <div className="summary-inline">
              <span>{formatMessage(messages.transcriptCount, { count: transcriptSegments.length })}</span>
              <span>{formatMessage(messages.summaryCount, { count: job.summaryRuns.length })}</span>
              <span>{formatMessage(messages.currentTemplate, { name: activeTemplateName || messages.notGenerated })}</span>
              <span>{formatMessage(messages.fileCount, { count: job.sourceFiles.length })}</span>
              <span>{formatMessage(messages.durationMinutes, { count: job.durationMinutes })}</span>
              <span>{formatMessage(messages.hotwords, { value: job.hotwords.join("、") || messages.notConfigured })}</span>
              <span>{job.summary.overview ? messages.notesReady : messages.notesEmpty}</span>
              <span>{activeSummaryRun ? messages.activeResultReady : messages.summaryEmpty}</span>
            </div>
          </article>

          <article className="surface native-list-panel transcript-column full-span">
            <div className="section-heading workbench-transcript-heading">
              <h3>{messages.transcript}</h3>
              <div className="field workbench-search-field">
                <input
                  id="transcript-search"
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder={messages.searchPlaceholder}
                />
              </div>
            </div>
            <div className="speaker-filter-row">
              {speakerOptions.map((speaker) => (
                <button
                  key={speaker.key}
                  className={`speaker-filter-chip ${selectedSpeaker === speaker.key ? "active" : ""}`}
                  type="button"
                  onClick={() => setSelectedSpeaker(speaker.key)}
                >
                  <span>{speaker.label}</span>
                  <strong>{speaker.count}</strong>
                </button>
              ))}
            </div>
            <TranscriptTimeline
              segments={speakerFilteredSegments}
              query={query}
              busy={isRenamingSpeaker}
              onRenameSpeaker={renameSpeaker}
            />
          </article>
        </div>
      ) : (
        <div className="empty-state">{messages.notFound}</div>
      )}
    </section>
  );
}
