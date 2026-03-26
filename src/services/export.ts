import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import type { MeetingJob } from "@/types/meeting";
import { getPrimaryTranscriptSegments } from "@/services/transcript";

export type ExportKind = "transcript" | "notes" | "bundle";

function timestamp(ms: number): string {
  const date = new Date(ms);
  return date.toISOString().slice(11, 19);
}

function makeTranscript(job: MeetingJob): string {
  return getPrimaryTranscriptSegments(job)
    .map(
      (segment) =>
        `[${timestamp(segment.startMs)} - ${timestamp(segment.endMs)}] ${segment.speaker ?? "未知说话人"}: ${segment.text}`,
    )
    .join("\n");
}

function makeNotes(job: MeetingJob): string {
  const { summary } = job;
  const riskLines = summary.risks?.length ? ["", "## 风险", ...summary.risks.map((item) => `- ${item}`)] : [];
  const followUpLines = summary.followUps?.length
    ? ["", "## 跟进事项", ...summary.followUps.map((item) => `- ${item}`)]
    : [];

  return [
    `# ${job.title}`,
    "",
    "## 摘要",
    summary.overview || "当前还没有可导出的 AI 总结内容。",
    "",
    "## 议题",
    ...summary.topics.map((item) => `- ${item}`),
    "",
    "## 结论",
    ...summary.decisions.map((item) => `- ${item}`),
    "",
    "## 行动项",
    ...summary.actionItems.map((item) => `- ${item}`),
    ...riskLines,
    ...followUpLines,
  ].join("\n");
}

function makeBundle(job: MeetingJob): string {
  return [makeNotes(job), "", "## 逐字稿", makeTranscript(job)].join("\n");
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
