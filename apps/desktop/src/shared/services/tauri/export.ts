import { invoke } from "@tauri-apps/api/core";
import type { MessageTree } from "@/shared/i18n";
import type { MeetingJob } from "@/shared/types/meeting";

export type TextExportKind = "transcript" | "notes" | "bundle";

export function buildTextExportRequest(
  job: MeetingJob,
  kind: TextExportKind,
  messages: MessageTree,
  filePath: string,
) {
  return {
    jobId: job.id,
    source: job.source,
    kind,
    filePath,
    labels: {
      unknownSpeaker: messages.common.unknownSpeaker,
      transcriptHeading: messages.export.transcriptHeading,
      summaryHeading: messages.export.summaryHeading,
      topicsHeading: messages.export.topicsHeading,
      decisionsHeading: messages.export.decisionsHeading,
      actionItemsHeading: messages.export.actionItemsHeading,
      risksHeading: messages.export.risksHeading,
      followUpsHeading: messages.export.followUpsHeading,
      emptySummary: messages.export.emptySummary,
    },
    remoteJob: job.source === "remote"
      ? {
          id: job.id,
          source: job.source,
          title: job.title,
          diarizationStatus: job.diarizationStatus,
          transcriptSegments: job.transcriptSegments,
          speakerSegments: job.speakerSegments,
          summary: job.summary,
        }
      : undefined,
  };
}

export function exportJobText(
  job: MeetingJob,
  kind: TextExportKind,
  messages: MessageTree,
  filePath: string,
) {
  return invoke<void>("export_job_text", {
    input: buildTextExportRequest(job, kind, messages, filePath),
  });
}

export function exportJobSummaryDocx(job: MeetingJob, filePath: string) {
  return invoke<void>("export_job_summary_docx", {
    jobId: job.id,
    summaryRunId: job.activeSummaryRunId ?? null,
    filePath,
  });
}
