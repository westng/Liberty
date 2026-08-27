import { describe, expect, it } from "vitest";
import type { JobStage, MeetingJob } from "@/shared/types/meeting";
import {
  filterJobQueue,
  isProcessingStage,
  paginateJobQueue,
} from "./jobQueue";

function job(overrides: Partial<MeetingJob> = {}): MeetingJob {
  return {
    id: "job-1",
    source: "local",
    title: "Weekly sync",
    sourceFiles: [{ id: "file-1", name: "meeting.mp3", path: "/meeting.mp3", sizeLabel: "100 B", kind: "audio" }],
    durationMinutes: 30,
    createdAt: "2026-08-20T08:00:00.000Z",
    hotwords: [],
    lang: "zh",
    enableSpeaker: true,
    asrBackend: "funasr",
    diarizationStatus: "completed",
    warnings: [],
    summaryTemplate: "",
    uploadStatus: "completed",
    asrStatus: "completed",
    summaryStatus: "completed",
    overallStatus: "completed",
    transcriptSegments: [],
    speakerSegments: [],
    summary: { overview: "ready", topics: [], decisions: [], actionItems: [] },
    summaryRuns: [],
    exportFormats: [],
    ...overrides,
  };
}

describe("job queue filtering", () => {
  it("filters by processing mode, state, title, and source filename", () => {
    const jobs = [
      job({ id: "completed", title: "Product review" }),
      job({ id: "processing", overallStatus: "transcribing", title: "Daily standup" }),
      job({
        id: "failed",
        overallStatus: "failed",
        sourceFiles: [{ id: "file-2", name: "incident.wav", path: "/incident.wav", sizeLabel: "200 B", kind: "audio" }],
      }),
      job({ id: "remote", source: "remote", title: "Remote meeting" }),
    ];

    expect(filterJobQueue(jobs, "local", "processing", "").map((item) => item.id)).toEqual(["processing"]);
    expect(filterJobQueue(jobs, "local", "all", "product").map((item) => item.id)).toEqual(["completed"]);
    expect(filterJobQueue(jobs, "local", "failed", "incident").map((item) => item.id)).toEqual(["failed"]);
  });

  it("recognizes every active processing stage", () => {
    const processingStages: JobStage[] = ["queued", "transcribing", "speaker_processing", "summarizing"];

    expect(processingStages.every(isProcessingStage)).toBe(true);
    expect(isProcessingStage("completed")).toBe(false);
  });

  it("paginates filtered jobs and clamps invalid pages", () => {
    const jobs = Array.from({ length: 23 }, (_, index) => job({ id: `job-${index + 1}` }));

    expect(paginateJobQueue(jobs, 2)).toMatchObject({
      page: 2,
      pageCount: 3,
      from: 11,
      to: 20,
      total: 23,
    });
    expect(paginateJobQueue(jobs, 2).items.map((item) => item.id)).toEqual([
      "job-11",
      "job-12",
      "job-13",
      "job-14",
      "job-15",
      "job-16",
      "job-17",
      "job-18",
      "job-19",
      "job-20",
    ]);
    expect(paginateJobQueue(jobs, 99)).toMatchObject({ page: 3, from: 21, to: 23 });
    expect(paginateJobQueue([], 3)).toMatchObject({ page: 1, pageCount: 1, from: 0, to: 0, total: 0 });
  });
});
