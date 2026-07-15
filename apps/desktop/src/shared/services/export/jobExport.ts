import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentMessages } from "@/shared/i18n";
import { applyLocalPetWorkflowEvent } from "@/shared/services/tauri/pet";
import { publishAppStatus, runAppStatusAction } from "@/shared/services/ui/statusNotifications";
import type { MeetingJob } from "@/shared/types/meeting";
import { getPrimaryTranscriptSegments } from "@/shared/services/meeting/transcript";

export type ExportKind = "transcript" | "notes" | "bundle" | "word";

function timestamp(ms: number): string {
  const date = new Date(ms);
  return date.toISOString().slice(11, 19);
}

function makeTranscript(job: MeetingJob): string {
  const messages = getCurrentMessages();
  return getPrimaryTranscriptSegments(job)
    .map(
      (segment) =>
        `[${timestamp(segment.startMs)} - ${timestamp(segment.endMs)}] ${segment.speaker ?? messages.common.unknownSpeaker}: ${segment.text}`,
    )
    .join("\n");
}

function makeNotes(job: MeetingJob): string {
  const messages = getCurrentMessages();
  const { summary } = job;
  const riskLines = summary.risks?.length
    ? ["", messages.export.risksHeading, ...summary.risks.map((item) => `- ${item}`)]
    : [];
  const followUpLines = summary.followUps?.length
    ? ["", messages.export.followUpsHeading, ...summary.followUps.map((item) => `- ${item}`)]
    : [];

  return [
    `# ${job.title}`,
    "",
    messages.export.summaryHeading,
    summary.overview || messages.export.emptySummary,
    "",
    messages.export.topicsHeading,
    ...summary.topics.map((item) => `- ${item}`),
    "",
    messages.export.decisionsHeading,
    ...summary.decisions.map((item) => `- ${item}`),
    "",
    messages.export.actionItemsHeading,
    ...summary.actionItems.map((item) => `- ${item}`),
    ...riskLines,
    ...followUpLines,
  ].join("\n");
}

function makeBundle(job: MeetingJob): string {
  const messages = getCurrentMessages();
  return [makeNotes(job), "", messages.export.transcriptHeading, makeTranscript(job)].join("\n");
}

export function buildExportPayload(job: MeetingJob, kind: ExportKind) {
  if (kind === "word") {
    return {
      ext: "docx",
      content: "",
      fileName: `${job.title}-meeting-minutes.docx`,
    };
  }

  if (kind === "transcript") {
    return {
      ext: "txt",
      content: makeTranscript(job),
      fileName: `${job.title}-transcript.txt`,
    };
  }

  if (kind === "notes") {
    return {
      ext: "md",
      content: makeNotes(job),
      fileName: `${job.title}-notes.md`,
    };
  }

  return {
    ext: "md",
    content: makeBundle(job),
    fileName: `${job.title}-bundle.md`,
  };
}

export async function exportJob(job: MeetingJob, kind: ExportKind) {
  if (kind === "word" && job.source !== "local") {
    publishAppStatus(getCurrentMessages().export.remoteWordUnavailable, {
      tone: "error",
      durationMs: 7000,
    });
    return false;
  }

  const payload = buildExportPayload(job, kind);

  const filePath = await save({
    defaultPath: payload.fileName,
    filters: [
      {
        name: payload.ext.toUpperCase(),
        extensions: [payload.ext],
      },
    ],
  });

  if (!filePath) {
    return false;
  }

  return runAppStatusAction("exportFile", async () => {
    if (kind === "word") {
      await invoke("export_job_summary_docx", {
        jobId: job.id,
        summaryRunId: job.activeSummaryRunId ?? null,
        filePath,
      });
      void applyLocalPetWorkflowEvent({ eventType: "export_completed", metadata: job.id }).catch(() => undefined);
      return true;
    }

    try {
      await writeTextFile(filePath, payload.content);
    } catch {
      const blob = new Blob([payload.content], { type: "text/plain;charset=utf-8" });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = payload.fileName;
      anchor.click();
      URL.revokeObjectURL(url);
    }

    void applyLocalPetWorkflowEvent({ eventType: "export_completed", metadata: job.id }).catch(() => undefined);
    return true;
  });
}
