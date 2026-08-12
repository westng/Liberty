import { save } from "@tauri-apps/plugin-dialog";
import { getCurrentMessages } from "@/shared/i18n";
import { applyLocalPetWorkflowEvent } from "@/shared/services/tauri/pet";
import {
  exportJobSummaryDocx,
  exportJobText,
  type TextExportKind,
} from "@/shared/services/tauri/export";
import { publishAppStatus, runAppStatusAction } from "@/shared/services/ui/statusNotifications";
import type { MeetingJob } from "@/shared/types/meeting";

export type ExportKind = "transcript" | "notes" | "bundle" | "word";

export function buildExportPayload(job: MeetingJob, kind: ExportKind) {
  if (kind === "word") {
    return {
      ext: "docx",
      fileName: `${job.title}-meeting-minutes.docx`,
    };
  }

  if (kind === "transcript") {
    return {
      ext: "txt",
      fileName: `${job.title}-transcript.txt`,
    };
  }

  if (kind === "notes") {
    return {
      ext: "md",
      fileName: `${job.title}-notes.md`,
    };
  }

  return {
    ext: "md",
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
      await exportJobSummaryDocx(job, filePath);
      void applyLocalPetWorkflowEvent({ eventType: "export_completed", metadata: job.id }).catch(() => undefined);
      return true;
    }

    await exportJobText(job, kind satisfies TextExportKind, getCurrentMessages(), filePath);

    void applyLocalPetWorkflowEvent({ eventType: "export_completed", metadata: job.id }).catch(() => undefined);
    return true;
  });
}
