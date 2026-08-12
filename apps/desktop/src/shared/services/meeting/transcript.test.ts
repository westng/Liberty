import { describe, expect, it } from "vitest";
import { getPrimaryTranscriptSegments, hasVerifiedSpeakerSegments } from "./transcript";
import type { MeetingJob } from "@/shared/types/meeting";

function job(status: MeetingJob["diarizationStatus"]): MeetingJob {
  return {
    id: "job-1",
    source: "local",
    title: "test",
    sourceFiles: [],
    durationMinutes: 1,
    createdAt: "2026-08-12T00:00:00Z",
    hotwords: [],
    lang: "zh",
    enableSpeaker: true,
    asrBackend: "funasr",
    diarizationStatus: status,
    warnings: [],
    summaryTemplate: "default",
    uploadStatus: "completed",
    asrStatus: "completed",
    summaryStatus: "idle",
    overallStatus: "completed",
    transcriptSegments: [{ id: "t1", startMs: 0, endMs: 1, text: "transcript" }],
    speakerSegments: [{ id: "s1", startMs: 0, endMs: 1, text: "speaker", speaker: "speaker-0" }],
    summary: {
      overview: "",
      topics: [],
      decisions: [],
      actionItems: [],
      risks: [],
      followUps: [],
    },
    summaryRuns: [],
    exportFormats: [],
  };
}

describe("primary transcript projection", () => {
  it("uses verified speaker segments only after diarization completed", () => {
    expect(getPrimaryTranscriptSegments(job("completed"))[0]?.text).toBe("speaker");
    expect(hasVerifiedSpeakerSegments(job("completed"))).toBe(true);
  });

  it.each(["unavailable", "failed", "legacy_unverified"] as const)(
    "uses transcript for %s speaker state",
    (status) => {
      expect(getPrimaryTranscriptSegments(job(status))[0]?.text).toBe("transcript");
      expect(hasVerifiedSpeakerSegments(job(status))).toBe(false);
    },
  );
});
