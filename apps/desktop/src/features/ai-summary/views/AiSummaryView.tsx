import { confirm } from "@tauri-apps/plugin-dialog";
import { useEffect, useMemo, useRef, useState } from "react";
import MeetingNotesPanel from "@/shared/components/MeetingNotesPanel";
import StatusBadge from "@/shared/components/StatusBadge";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { formatMessage, getMessages } from "@/shared/i18n";
import {
  createEmptyMeetingSummary,
  summaryResultToMeetingSummary,
} from "@/shared/services/ai/storage";
import { startOrResumeAiSummaryRun } from "@/shared/services/ai/summary";
import {
  getPrimaryTranscriptSegments,
  hasVerifiedSpeakerSegments,
} from "@/shared/services/meeting/transcript";
import { createLocalAiService, type AiSummaryOptions } from "@/shared/services/tauri/ai";
import { createLocalMeetingService } from "@/shared/services/tauri/meeting";
import { closeCurrentWindow } from "@/shared/services/tauri/window";
import { publishEntityChanged } from "@/shared/services/ui/windows";
import type { AiSummaryRun, JobStage, MeetingJob, ProcessingMode } from "@/shared/types/meeting";

const localAiService = createLocalAiService();
const localMeetingService = createLocalMeetingService();

function parseJobSource(value: string | null): ProcessingMode | null {
  return value === "local" || value === "remote" ? value : null;
}

export default function AiSummaryView() {
  const meetingStore = useMeetingStore();
  const messages = getMessages(meetingStore.settings.locale).aiSummary;
  const commonMessages = getMessages(meetingStore.settings.locale).common;
  const query = new URLSearchParams(window.location.search);
  const jobId = query.get("jobId")?.trim() ?? "";
  const windowScopeToken = query.get("scopeToken")?.trim() ?? "";
  const jobSource = parseJobSource(query.get("source"));
  const [job, setJob] = useState<MeetingJob | null>(null);
  const [options, setOptions] = useState<AiSummaryOptions>({ models: [], templates: [] });
  const enabledModels = options.models.filter((model) => model.enabled);
  const templates = options.templates;
  const latestRuns = useMemo(
    () => [...(job?.summaryRuns ?? [])].sort((left, right) => right.updatedAt.localeCompare(left.updatedAt)),
    [job?.summaryRuns],
  );
  const latestRun = latestRuns[0] ?? null;
  const [selectedModelId, setSelectedModelId] = useState("");
  const [selectedTemplateId, setSelectedTemplateId] = useState("");
  const [selectedRunId, setSelectedRunId] = useState("");
  const [includeSpeaker, setIncludeSpeaker] = useState(true);
  const [includeTimestamp, setIncludeTimestamp] = useState(true);
  const [useMemberMapping, setUseMemberMapping] = useState(true);
  const [extraInstructions, setExtraInstructions] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [errorMessage, setErrorMessage] = useState("");
  const resumedRunIds = useRef(new Set<string>());
  const selectedModel = options.models.find((model) => model.id === selectedModelId);
  const selectedTemplate = options.templates.find((template) => template.id === selectedTemplateId);
  const selectedRun = latestRuns.find((run) => run.id === selectedRunId) ?? latestRuns[0] ?? null;
  const runningRun = latestRuns.find((run) => run.status === "running") ?? null;
  const resumableRun = runningRun ?? (latestRun?.status === "failed" ? latestRun : null);
  const remoteUnavailable = jobSource === "remote";
  const invalidJobRef = !jobSource || !jobId || !windowScopeToken;
  const transcriptCount = job ? getPrimaryTranscriptSegments(job).length : 0;
  const speakerSummaryAvailable = job ? hasVerifiedSpeakerSegments(job) : false;
  const previewSummary = selectedRun?.result
    ? summaryResultToMeetingSummary(selectedRun.result)
    : job?.summary || createEmptyMeetingSummary(job?.title);
  const hasSummaryContent = Boolean(
    job && (
      job.summary.overview.trim()
      || job.summary.topics.length
      || job.summary.decisions.length
      || job.summary.actionItems.length
      || job.summary.risks?.length
      || job.summary.followUps?.length
    ),
  );
  const summaryDisplayStatus: JobStage = submitting || latestRun?.status === "running"
    ? "summarizing"
    : latestRun?.status === "failed" && !hasSummaryContent
      ? "failed"
      : hasSummaryContent || latestRun?.status === "completed"
        ? "completed"
        : "idle";
  const previewStatus: JobStage = selectedRun
    ? selectedRun.status === "running"
      ? "summarizing"
      : selectedRun.status === "failed"
        ? "failed"
        : selectedRun.result
          ? "completed"
          : "idle"
    : summaryDisplayStatus;
  const selectedRunIsActive = Boolean(job?.activeSummaryRunId && job.activeSummaryRunId === selectedRun?.id);
  const canApplySelectedRun = Boolean(
    jobSource === "local" && selectedRun?.result && !selectedRunIsActive,
  );
  const activeSummaryLabel = !latestRun
    ? messages.activeLabelEmpty
    : summaryDisplayStatus === "failed"
      ? messages.activeLabelFailed
      : summaryDisplayStatus === "summarizing"
        ? messages.activeLabelRunning
        : messages.activeLabelSaved;

  useEffect(() => {
    if (!latestRuns.length) {
      setSelectedRunId("");
      return;
    }

    setSelectedRunId((current) => {
      if (latestRuns.some((run) => run.id === current)) {
        return current;
      }
      const preferredRun = latestRuns.find((run) => run.id === job?.activeSummaryRunId) ?? latestRuns[0];
      return preferredRun.id;
    });
  }, [latestRuns, job?.activeSummaryRunId]);

  useEffect(() => {
    if (!selectedTemplateId && templates[0]) {
      setSelectedTemplateId(templates[0].id);
    }
  }, [templates, selectedTemplateId]);

  useEffect(() => {
    if (!selectedModelId) {
      setSelectedModelId((enabledModels.find((model) => model.isDefault) ?? enabledModels[0])?.id ?? "");
    }
  }, [enabledModels, selectedModelId]);

  useEffect(() => {
    if (!selectedTemplate) {
      return;
    }
    setIncludeSpeaker(selectedTemplate.includeSpeakerByDefault && speakerSummaryAvailable);
    setIncludeTimestamp(selectedTemplate.includeTimestampByDefault);
  }, [selectedTemplate?.id, speakerSummaryAvailable]);

  useEffect(() => {
    void (async () => {
      if (jobSource !== "local" || !jobId || !windowScopeToken) {
        return;
      }
      try {
        const [refreshed, summaryOptions] = await Promise.all([
          localMeetingService.getJobResult(jobId, windowScopeToken),
          localAiService.getSummaryOptions("local"),
        ]);
        if (refreshed.source !== "local") {
          throw new Error(messages.jobSourceMismatch);
        }
        setJob(refreshed);
        setOptions(summaryOptions);
      } catch (error) {
        setErrorMessage(error instanceof Error ? error.message : messages.jobNotFound);
      }
    })();
  }, [jobId, jobSource, windowScopeToken]);

  useEffect(() => {
    if (!job || jobSource !== "local" || !runningRun) {
      return;
    }
    if (!resumedRunIds.current.has(runningRun.id)) {
      resumedRunIds.current.add(runningRun.id);
      void startOrResumeAiSummaryRun({
        source: "local",
        jobId: job.id,
        windowScopeToken,
        runId: runningRun.id,
      }).catch((error) => {
        setErrorMessage(error instanceof Error ? error.message : messages.requestFailed);
      });
    }

    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const refresh = async () => {
      try {
        const refreshed = await localMeetingService.getJobResult(job.id, windowScopeToken);
        if (refreshed.source !== "local") {
          throw new Error(messages.jobSourceMismatch);
        }
        if (!cancelled) {
          setJob(refreshed);
          setErrorMessage("");
        }
      } catch (error) {
        if (!cancelled) {
          setErrorMessage(error instanceof Error ? error.message : messages.requestFailed);
        }
      } finally {
        if (!cancelled) {
          timer = setTimeout(() => void refresh(), 1_000);
        }
      }
    };
    timer = setTimeout(() => void refresh(), 500);
    return () => {
      cancelled = true;
      if (timer) {
        clearTimeout(timer);
      }
    };
  }, [job?.id, jobSource, runningRun?.id, windowScopeToken]);

  async function submit() {
    if (!job) {
      setErrorMessage(messages.jobNotFound);
      return;
    }
    if (jobSource !== "local") {
      setErrorMessage(messages.remoteUnavailable);
      return;
    }
    if (resumableRun) {
      setErrorMessage("");
      setSubmitting(true);
      try {
        const run = await startOrResumeAiSummaryRun({
          source: "local",
          jobId: job.id,
          windowScopeToken,
          runId: resumableRun.id,
        });
        resumedRunIds.current.add(run.id);
        setSelectedRunId(run.id);
        setJob(await localMeetingService.getJobResult(job.id, windowScopeToken));
      } catch (error) {
        setErrorMessage(error instanceof Error ? error.message : messages.requestFailed);
      } finally {
        setSubmitting(false);
      }
      return;
    }
    if (!selectedModel) {
      setErrorMessage(messages.modelMissing);
      return;
    }
    if (!selectedTemplate) {
      setErrorMessage(messages.templateMissing);
      return;
    }

    const transcriptSegments = getPrimaryTranscriptSegments(job);
    if (!transcriptSegments.length) {
      setErrorMessage(messages.transcriptMissing);
      return;
    }

    setErrorMessage("");
    setSubmitting(true);
    try {
      const run = await startOrResumeAiSummaryRun({
        source: "local",
        jobId: job.id,
        windowScopeToken,
        modelConfigId: selectedModel.id,
        templateId: selectedTemplate.id,
        includeSpeaker: includeSpeaker && speakerSummaryAvailable,
        includeTimestamp,
        useMemberMapping,
        extraInstructions: extraInstructions.trim(),
      });
      resumedRunIds.current.add(run.id);
      setSelectedRunId(run.id);
      setJob(await localMeetingService.getJobResult(job.id, windowScopeToken));
    } catch (error) {
      const message = error instanceof Error ? error.message : messages.requestFailed;
      setErrorMessage(message);
    } finally {
      setSubmitting(false);
    }
  }

  async function closeWindow() {
    await closeCurrentWindow();
  }

  async function applySelectedRun() {
    if (!job || jobSource !== "local" || !selectedRun?.result) {
      return;
    }

    await localAiService.setActiveSummaryRun(
      "local",
      job.id,
      selectedRun.id,
      windowScopeToken,
    );
    setJob(await localMeetingService.getJobResult(job.id, windowScopeToken));
    await publishEntityChanged({ entity: "summary", id: job.id, action: "saved" }).catch(() => undefined);
  }

  async function removeRun(run: AiSummaryRun) {
    if (!job || jobSource !== "local") {
      return;
    }

    const confirmed = await confirm(messages.deleteConfirm, {
      title: messages.deleteTitle,
      kind: "warning",
      okLabel: commonMessages.delete,
      cancelLabel: commonMessages.cancel,
    });

    if (!confirmed) {
      return;
    }

    await localAiService.deleteSummaryRun(
      "local",
      job.id,
      run.id,
      windowScopeToken,
    );
    setJob(await localMeetingService.getJobResult(job.id, windowScopeToken));
    await publishEntityChanged({ entity: "summary", id: job.id, action: "deleted" }).catch(() => undefined);
    if (selectedRunId === run.id) {
      setSelectedRunId("");
    }
  }

  function formatCreatedAt(value: string) {
    return new Date(value).toLocaleString(meetingStore.settings.locale, {
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  return (
    <section className="summary-window-shell native-summary-window ai-summary-window">
      <article className="surface native-window-hero summary-window-hero">
        <div className="job-title-line">
          <div>
            <h3>{job?.title || messages.heroTitle}</h3>
            <p className="section-copy">{formatMessage(messages.heroCopy, { count: transcriptCount })}</p>
          </div>
          <div className="button-row">
            <StatusBadge status={summaryDisplayStatus} />
            <button className="secondary-button" type="button" onClick={closeWindow}>
              {commonMessages.closeWindow}
            </button>
          </div>
        </div>

        <div className="summary-inline">
          <span>{formatMessage(messages.transcriptCount, { count: transcriptCount })}</span>
          <span>{formatMessage(messages.currentStatus, { status: activeSummaryLabel })}</span>
          <span>{formatMessage(messages.inputFiles, { count: job?.sourceFiles.length || 0 })}</span>
          <span>{job?.sourceFiles.map((file) => file.name).join(" · ") || messages.jobMissing}</span>
        </div>
      </article>

      <div className="summary-window-layout ai-summary-layout">
        <aside className="summary-window-side">
          <article className="surface">
            <div className="section-heading summary-centered-heading">
              <h3>{messages.currentConfig}</h3>
              <StatusBadge status={summaryDisplayStatus} />
            </div>

            <div className="field-grid">
              <div className="field-grid two-col">
                <div className="field">
                  <label htmlFor="summary-model">{messages.model}</label>
                  <select disabled={remoteUnavailable || Boolean(runningRun)} id="summary-model" value={selectedModelId} onChange={(event) => setSelectedModelId(event.target.value)}>
                    <option disabled value="">{messages.chooseModel}</option>
                    {enabledModels.map((model) => (
                      <option key={model.id} value={model.id}>{model.name} · {model.model}</option>
                    ))}
                  </select>
                </div>

                <div className="field">
                  <label htmlFor="summary-template">{messages.template}</label>
                  <select disabled={remoteUnavailable || Boolean(runningRun)} id="summary-template" value={selectedTemplateId} onChange={(event) => setSelectedTemplateId(event.target.value)}>
                    <option disabled value="">{messages.chooseTemplate}</option>
                    {templates.map((template) => (
                      <option key={template.id} value={template.id}>{template.name}</option>
                    ))}
                  </select>
                </div>
              </div>

              <div className="field-grid two-col">
                <label className="toggle-field">
                  <input checked={includeSpeaker} disabled={remoteUnavailable || Boolean(runningRun) || !speakerSummaryAvailable} onChange={(event) => setIncludeSpeaker(event.target.checked)} type="checkbox" />
                  <span>{messages.includeSpeaker}</span>
                </label>
                <label className="toggle-field">
                  <input checked={includeTimestamp} disabled={remoteUnavailable || Boolean(runningRun)} onChange={(event) => setIncludeTimestamp(event.target.checked)} type="checkbox" />
                  <span>{messages.includeTimestamp}</span>
                </label>
              </div>

              {!speakerSummaryAvailable && job?.enableSpeaker && (
                <div className="note-block">{messages.speakerUnavailable}</div>
              )}

              <div className="field-grid two-col">
                <label className="toggle-field">
                  <input checked={useMemberMapping && speakerSummaryAvailable} disabled={remoteUnavailable || Boolean(runningRun) || !speakerSummaryAvailable} onChange={(event) => setUseMemberMapping(event.target.checked)} type="checkbox" />
                  <span>{messages.useMemberMapping}</span>
                </label>
              </div>

              <div className="field">
                <label htmlFor="summary-extra">{messages.extraInstructions}</label>
                <textarea disabled={remoteUnavailable || Boolean(runningRun)} id="summary-extra" value={extraInstructions} onChange={(event) => setExtraInstructions(event.target.value)} placeholder={messages.extraInstructionsPlaceholder} />
              </div>
            </div>

            {remoteUnavailable && <div className="note-block error-block">capability_unavailable: {messages.remoteUnavailable}</div>}
            {invalidJobRef && <div className="note-block error-block">{messages.jobRefMissing}</div>}
            {!remoteUnavailable && !invalidJobRef && errorMessage && <div className="note-block error-block">{errorMessage}</div>}

            <div className="button-row summary-submit-row">
              <button className="primary-button" type="button" disabled={remoteUnavailable || invalidJobRef || submitting || Boolean(runningRun)} onClick={submit}>
                {submitting ? messages.submitting : messages.submit}
              </button>
            </div>
          </article>

          <article className="surface summary-history-panel">
            <div className="section-heading">
              <h3>{messages.history}</h3>
            </div>

            {latestRuns.length ? (
              <div className="record-list">
                {latestRuns.map((run) => (
                  <button
                    key={run.id}
                    className={`record-item ${selectedRun?.id === run.id ? "active" : ""}`}
                    type="button"
                    onClick={() => setSelectedRunId(run.id)}
                  >
                    <div className="record-item-head">
                      <div className="record-title-stack">
                        <strong>{options.templates.find((template) => template.id === run.templateId)?.name || messages.unknownTemplate}</strong>
                        <div className="record-tags">
                          {job?.activeSummaryRunId === run.id ? (
                            <span className="record-tag active">{messages.currentResult}</span>
                          ) : latestRun?.id === run.id ? (
                            <span className="record-tag">{messages.latest}</span>
                          ) : null}
                        </div>
                      </div>
                      <StatusBadge status={run.status === "running" ? "summarizing" : run.status === "completed" ? "completed" : "failed"} />
                    </div>
                    <div className="record-item-body">
                      <span>{options.models.find((model) => model.id === run.modelConfigId)?.name || messages.unknownModel}</span>
                      <span>{formatCreatedAt(run.createdAt)}</span>
                    </div>
                    {run.result?.overview && <div className="record-item-copy">{run.result.overview}</div>}
                    {run.errorMessage && <div className="record-item-copy danger-text">{run.errorMessage}</div>}
                  </button>
                ))}
              </div>
            ) : (
              <div className="empty-state">{messages.emptyRuns}</div>
            )}
          </article>
        </aside>

        <div className="summary-window-main">
          <article className="surface summary-window-result ai-summary-result">
            <div className="section-heading summary-centered-heading">
              <h3>{messages.preview}</h3>
              <StatusBadge status={previewStatus} />
            </div>

            {selectedRun && (
              <div className="summary-preview-toolbar">
                <div className="record-meta summary-preview-meta">
                  <span>{options.templates.find((template) => template.id === selectedRun.templateId)?.name || messages.unknownTemplate}</span>
                  <span>{options.models.find((model) => model.id === selectedRun.modelConfigId)?.name || messages.unknownModel}</span>
                  <span>{formatCreatedAt(selectedRun.createdAt)}</span>
                  {selectedRunIsActive && <span>{messages.currentResult}</span>}
                </div>
                <div className="button-row">
                  <button className="secondary-button" type="button" disabled={!canApplySelectedRun} onClick={applySelectedRun}>
                    {selectedRunIsActive ? messages.usingCurrent : messages.setCurrent}
                  </button>
                  <button className="secondary-button jobs-delete-button" type="button" disabled={remoteUnavailable} onClick={() => removeRun(selectedRun)}>
                    {messages.deleteCurrent}
                  </button>
                </div>
              </div>
            )}

            <MeetingNotesPanel summary={previewSummary} />
          </article>
        </div>
      </div>
    </section>
  );
}
