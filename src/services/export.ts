import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import { getCurrentMessages } from "@/services/i18n";
import type { MeetingJob } from "@/types/meeting";
import { getPrimaryTranscriptSegments } from "@/services/transcript";

export type ExportKind = "transcript" | "notes" | "bundle";

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
  const payload = buildExportPayload(job, kind);

  try {
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

    await writeTextFile(filePath, payload.content);
    return true;
  } catch {
    const blob = new Blob([payload.content], { type: "text/plain;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = payload.fileName;
    anchor.click();
    URL.revokeObjectURL(url);
    return true;
  }
}
