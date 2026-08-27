import { describe, expect, it } from "vitest";
import {
  getDisplayTranscriptSegments,
  getPrimaryTranscriptSegments,
  hasDisplayableLegacySpeakerSegments,
  hasVerifiedSpeakerSegments,
} from "./transcript";
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
    speakerSegments: [{ id: "s1", startMs: 0, endMs: 1, text: "transcript", speaker: "speaker-0" }],
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
    expect(getPrimaryTranscriptSegments(job("completed"))[0]?.speaker).toBe("speaker-0");
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

describe("display transcript projection", () => {
  it("shows complete legacy speaker data without promoting it to verified data", () => {
    const legacyJob = job("legacy_unverified");

    expect(getDisplayTranscriptSegments(legacyJob)[0]?.speaker).toBe("speaker-0");
    expect(hasDisplayableLegacySpeakerSegments(legacyJob)).toBe(true);
    expect(getPrimaryTranscriptSegments(legacyJob)[0]?.text).toBe("transcript");
    expect(hasVerifiedSpeakerSegments(legacyJob)).toBe(false);
  });

  it("falls back to transcript when a legacy speaker label is missing", () => {
    const legacyJob = job("legacy_unverified");
    legacyJob.speakerSegments[0] = { ...legacyJob.speakerSegments[0], speaker: " " };

    expect(getDisplayTranscriptSegments(legacyJob)[0]?.text).toBe("transcript");
    expect(hasDisplayableLegacySpeakerSegments(legacyJob)).toBe(false);
  });

  it("falls back to the complete transcript when legacy speaker data is partial", () => {
    const legacyJob = job("legacy_unverified");
    legacyJob.transcriptSegments.push({
      id: "t2",
      startMs: 1,
      endMs: 2,
      text: "second transcript segment",
    });

    expect(getDisplayTranscriptSegments(legacyJob)).toBe(legacyJob.transcriptSegments);
    expect(hasDisplayableLegacySpeakerSegments(legacyJob)).toBe(false);
  });

  it("falls back when legacy speaker timing does not match the transcript", () => {
    const legacyJob = job("legacy_unverified");
    legacyJob.speakerSegments[0] = { ...legacyJob.speakerSegments[0], endMs: 2 };

    expect(getDisplayTranscriptSegments(legacyJob)).toBe(legacyJob.transcriptSegments);
    expect(hasDisplayableLegacySpeakerSegments(legacyJob)).toBe(false);
  });

  it("does not display speaker data from failed diarization", () => {
    expect(getDisplayTranscriptSegments(job("failed"))[0]?.text).toBe("transcript");
  });
});
