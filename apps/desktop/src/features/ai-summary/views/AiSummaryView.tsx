import { confirm } from "@tauri-apps/plugin-dialog";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useMemo, useState } from "react";
import MeetingNotesPanel from "@/shared/components/MeetingNotesPanel";
import StatusBadge from "@/shared/components/StatusBadge";
import { useAiStore } from "@/features/ai/stores/useAiStore";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { formatMessage, getMessages } from "@/shared/i18n";
import { createLocalMembersService } from "@/shared/services/tauri/members";
import {
  buildSummaryRun,
  createEmptyMeetingSummary,
  summaryResultToMeetingSummary,
} from "@/shared/services/ai/storage";
import { generateAiSummary } from "@/shared/services/ai/summary";
import { getPrimaryTranscriptSegments } from "@/shared/services/meeting/transcript";
import type { AiSummaryRun, JobStage, MeetingMember } from "@/shared/types/meeting";

const membersService = createLocalMembersService();

export default function AiSummaryView() {
  const aiStore = useAiStore();
  const meetingStore = useMeetingStore();
  const messages = getMessages(meetingStore.settings.locale).aiSummary;
  const commonMessages = getMessages(meetingStore.settings.locale).common;
  const jobId = new URLSearchParams(window.location.search).get("jobId") ?? "";
  const job = meetingStore.getJobById(jobId);
  const enabledModels = aiStore.models.filter((model) => model.enabled);
  const templates = aiStore.templates;
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
  const [members, setMembers] = useState<MeetingMember[]>([]);
  const selectedModel = aiStore.getModelById(selectedModelId);
  const selectedTemplate = aiStore.getTemplateById(selectedTemplateId);
  const selectedRun = latestRuns.find((run) => run.id === selectedRunId) ?? latestRuns[0] ?? null;
  const transcriptCount = job ? getPrimaryTranscriptSegments(job).length : 0;
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
  const canApplySelectedRun = Boolean(job && selectedRun?.result && !selectedRunIsActive);
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
      setSelectedModelId((aiStore.getDefaultModel() ?? enabledModels[0])?.id ?? "");
    }
  }, [enabledModels, selectedModelId]);

  useEffect(() => {
    if (!selectedTemplate) {
      return;
    }
    setIncludeSpeaker(selectedTemplate.includeSpeakerByDefault);
    setIncludeTimestamp(selectedTemplate.includeTimestampByDefault);
  }, [selectedTemplate?.id]);

  useEffect(() => {
    void (async () => {
      await aiStore.ensureLoaded();
      if (jobId) {
        await meetingStore.refreshJob(jobId);
      }
      setMembers(await membersService.listMembers());
      await reconcileStaleRuns();
    })();
  }, [jobId]);

  async function reconcileStaleRuns() {
    const now = Date.now();
    const staleRuns = latestRuns.filter((run) => {
      if (run.status !== "running") {
        return false;
      }
      return now - new Date(run.updatedAt).getTime() > 60_000;
    });

    for (const run of staleRuns) {
      await meetingStore.saveSummaryRun({
        ...run,
        status: "failed",
        errorMessage: messages.staleRunError,
        updatedAt: new Date().toISOString(),
      });
    }
  }

  async function submit() {
    if (!job) {
      setErrorMessage(messages.jobNotFound);
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
    const pendingRun = buildSummaryRun({
      jobId: job.id,
      modelConfigId: selectedModel.id,
      templateId: selectedTemplate.id,
      includeSpeaker,
      includeTimestamp,
      extraInstructions: extraInstructions.trim(),
      status: "running",
      promptPreview: undefined,
      result: undefined,
    });

    await meetingStore.saveSummaryRun(pendingRun);

    try {
      const response = await generateAiSummary({
        job,
        model: selectedModel,
        template: selectedTemplate,
        includeSpeaker,
        includeTimestamp,
        useMemberMapping,
        members,
        extraInstructions: extraInstructions.trim(),
      });

      await meetingStore.saveSummaryRun({
        ...pendingRun,
        status: "completed",
        promptPreview: response.promptPreview,
        rawResponse: response.rawResponse,
        result: response.result,
      });
      await meetingStore.setActiveSummaryRun(pendingRun.jobId, pendingRun.id);
      setSelectedRunId(pendingRun.id);
    } catch (error) {
      const message = error instanceof Error ? error.message : messages.requestFailed;
      setErrorMessage(message);
      await meetingStore.saveSummaryRun({
        ...pendingRun,
        status: "failed",
        errorMessage: message,
      });
    } finally {
      setSubmitting(false);
    }
  }

  async function closeWindow() {
    await getCurrentWebviewWindow().close();
  }

  async function applySelectedRun() {
    if (!job || !selectedRun?.result) {
      return;
    }

    await meetingStore.setActiveSummaryRun(job.id, selectedRun.id);
  }

  async function removeRun(run: AiSummaryRun) {
    if (!job) {
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

    await meetingStore.deleteSummaryRun(job.id, run.id);
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
                  <select id="summary-model" value={selectedModelId} onChange={(event) => setSelectedModelId(event.target.value)}>
                    <option disabled value="">{messages.chooseModel}</option>
                    {enabledModels.map((model) => (
                      <option key={model.id} value={model.id}>{model.name} · {model.model}</option>
                    ))}
                  </select>
                </div>

                <div className="field">
                  <label htmlFor="summary-template">{messages.template}</label>
                  <select id="summary-template" value={selectedTemplateId} onChange={(event) => setSelectedTemplateId(event.target.value)}>
                    <option disabled value="">{messages.chooseTemplate}</option>
                    {templates.map((template) => (
                      <option key={template.id} value={template.id}>{template.name}</option>
                    ))}
                  </select>
                </div>
              </div>

              <div className="field-grid two-col">
                <label className="toggle-field">
                  <input checked={includeSpeaker} onChange={(event) => setIncludeSpeaker(event.target.checked)} type="checkbox" />
                  <span>{messages.includeSpeaker}</span>
                </label>
                <label className="toggle-field">
                  <input checked={includeTimestamp} onChange={(event) => setIncludeTimestamp(event.target.checked)} type="checkbox" />
                  <span>{messages.includeTimestamp}</span>
                </label>
              </div>

              <div className="field-grid two-col">
                <label className="toggle-field">
                  <input checked={useMemberMapping} onChange={(event) => setUseMemberMapping(event.target.checked)} type="checkbox" />
                  <span>{messages.useMemberMapping}</span>
                </label>
              </div>

              <div className="field">
                <label htmlFor="summary-extra">{messages.extraInstructions}</label>
                <textarea id="summary-extra" value={extraInstructions} onChange={(event) => setExtraInstructions(event.target.value)} placeholder={messages.extraInstructionsPlaceholder} />
              </div>
            </div>

            {errorMessage && <div className="note-block error-block">{errorMessage}</div>}

            <div className="button-row summary-submit-row">
              <button className="primary-button" type="button" disabled={submitting} onClick={submit}>
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
                        <strong>{aiStore.getTemplateById(run.templateId)?.name || messages.unknownTemplate}</strong>
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
                      <span>{aiStore.getModelById(run.modelConfigId)?.name || messages.unknownModel}</span>
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
                  <span>{aiStore.getTemplateById(selectedRun.templateId)?.name || messages.unknownTemplate}</span>
                  <span>{aiStore.getModelById(selectedRun.modelConfigId)?.name || messages.unknownModel}</span>
                  <span>{formatCreatedAt(selectedRun.createdAt)}</span>
                  {selectedRunIsActive && <span>{messages.currentResult}</span>}
                </div>
                <div className="button-row">
                  <button className="secondary-button" type="button" disabled={!canApplySelectedRun} onClick={applySelectedRun}>
                    {selectedRunIsActive ? messages.usingCurrent : messages.setCurrent}
                  </button>
                  <button className="secondary-button jobs-delete-button" type="button" onClick={() => removeRun(selectedRun)}>
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
