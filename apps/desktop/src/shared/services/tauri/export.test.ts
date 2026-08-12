import { describe, expect, it } from "vitest";
import { getMessages } from "@/shared/i18n";
import type { MeetingJob } from "@/shared/types/meeting";
import { buildTextExportRequest } from "./export";

function job(source: MeetingJob["source"]): MeetingJob {
  return {
    id: "job-1",
    source,
    title: "Weekly Sync",
    sourceFiles: [{ id: "file-1", name: "private.wav", path: "/private/media.wav", sizeLabel: "1 MB", kind: "audio" }],
    durationMinutes: 1,
    createdAt: "2026-08-13T00:00:00Z",
    hotwords: [],
    lang: "zh",
    enableSpeaker: true,
    asrBackend: "funasr",
    diarizationStatus: "completed",
    warnings: [],
    summaryTemplate: "default",
    uploadStatus: "completed",
    asrStatus: "completed",
    summaryStatus: "completed",
    overallStatus: "completed",
    transcriptSegments: [{ id: "t1", startMs: 0, endMs: 1, text: "raw" }],
    speakerSegments: [{ id: "s1", startMs: 0, endMs: 1, text: "speaker", speaker: "Alice" }],
    summary: {
      overview: "Overview",
      topics: [],
      decisions: [],
      actionItems: [],
      risks: [],
      followUps: [],
    },
    summaryRuns: [],
    exportFormats: [],
    processLog: "private process output",
  };
}

describe("text export IPC projection", () => {
  it("sends only a local job reference", () => {
    const request = buildTextExportRequest(job("local"), "notes", getMessages("zh-CN"), "/tmp/notes.md");

    expect(request.remoteJob).toBeUndefined();
    expect(request).toMatchObject({ jobId: "job-1", source: "local", kind: "notes" });
  });

  it("limits remote snapshots to export content", () => {
    const request = buildTextExportRequest(job("remote"), "bundle", getMessages("en-US"), "/tmp/bundle.md");

    expect(request.remoteJob).toEqual({
      id: "job-1",
      source: "remote",
      title: "Weekly Sync",
      diarizationStatus: "completed",
      transcriptSegments: [{ id: "t1", startMs: 0, endMs: 1, text: "raw" }],
      speakerSegments: [{ id: "s1", startMs: 0, endMs: 1, text: "speaker", speaker: "Alice" }],
      summary: job("remote").summary,
    });
    expect(request.remoteJob).not.toHaveProperty("sourceFiles");
    expect(request.remoteJob).not.toHaveProperty("processLog");
  });
});
