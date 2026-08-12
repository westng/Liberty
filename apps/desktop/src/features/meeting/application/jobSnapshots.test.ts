import { describe, expect, it } from "vitest";
import type { MeetingJob } from "@/shared/types/meeting";
import { hasActiveJobs, mergeJobSnapshot } from "./jobSnapshots";

function makeJob(overrides: Partial<MeetingJob> = {}): MeetingJob {
  return {
    id: "job-1",
    source: "local",
    title: "会议",
    sourceFiles: [],
    durationMinutes: 1,
    createdAt: "2026-08-12T00:00:00Z",
    hotwords: [],
    lang: "zh-CN",
    enableSpeaker: false,
    asrBackend: "unknown",
    diarizationStatus: "disabled",
    warnings: [],
    summaryTemplate: "表格版会议纪要",
    uploadStatus: "completed",
    asrStatus: "completed",
    summaryStatus: "idle",
    overallStatus: "completed",
    transcriptSegments: [],
    speakerSegments: [],
    summary: { overview: "", topics: [], decisions: [], actionItems: [] },
    summaryRuns: [],
    exportFormats: [],
    ...overrides,
  };
}

describe("job snapshot policy", () => {
  it("detects active jobs", () => {
    expect(hasActiveJobs([makeJob()])).toBe(false);
    expect(hasActiveJobs([makeJob({ overallStatus: "transcribing" })])).toBe(true);
  });

  it("preserves hydrated detail when a list snapshot omits it", () => {
    const existing = makeJob({
      transcriptSegments: [{ id: "segment-1", startMs: 0, endMs: 10, text: "完整逐字稿" }],
      processLog: "diagnostic",
    });
    const incoming = makeJob({ title: "已更新标题" });

    const merged = mergeJobSnapshot(existing, incoming, new Set([existing.id]));

    expect(merged.title).toBe("已更新标题");
    expect(merged.transcriptSegments).toEqual(existing.transcriptSegments);
    expect(merged.processLog).toBe("diagnostic");
  });

  it("uses incoming data before a job is hydrated", () => {
    const existing = makeJob({ title: "旧标题" });
    const incoming = makeJob({ title: "新标题" });

    expect(mergeJobSnapshot(existing, incoming, new Set())).toBe(incoming);
  });
});
