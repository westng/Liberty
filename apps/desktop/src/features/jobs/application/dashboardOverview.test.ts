import { describe, expect, it } from "vitest";
import type { MeetingJob } from "@/shared/types/meeting";
import {
  buildRemoteDashboardOverview,
  ratioPercent,
  selectVisibleDashboardOverview,
} from "./dashboardOverview";

function job(overrides: Partial<MeetingJob> = {}): MeetingJob {
  return {
    id: "job-1",
    source: "remote",
    title: "Weekly sync",
    sourceFiles: [],
    durationMinutes: 30,
    createdAt: "2026-08-13T08:00:00.000Z",
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

describe("dashboard overview projection", () => {
  it("builds a remote snapshot without local resource data", () => {
    const overview = buildRemoteDashboardOverview([
      job({ id: "complete", lastExportedAt: "2026-08-13T09:00:00.000Z" }),
      job({ id: "failed", overallStatus: "failed", asrStatus: "failed", durationMinutes: 10 }),
    ], "7d", new Date("2026-08-13T12:00:00.000Z"));

    expect(overview.metrics.totalJobs).toBe(2);
    expect(overview.metrics.mediaDurationMinutes).toBe(40);
    expect(overview.metrics.summaryReadyJobs).toBe(1);
    expect(overview.metrics.exportedJobs).toBe(1);
    expect(overview.attentionJobs.map((item) => item.id)).toContain("failed");
    expect(overview.resources.aiModels).toBe(0);
    expect(overview.companion).toBeUndefined();
  });

  it("does not represent a missing denominator as zero percent", () => {
    expect(ratioPercent(0, 0)).toBeNull();
    expect(ratioPercent(3, 4)).toBe(75);
  });

  it("keeps the previous local snapshot visible while another range loads", () => {
    const previousSnapshot = buildRemoteDashboardOverview([], "7d");
    const remoteSnapshot = buildRemoteDashboardOverview([], "30d");

    expect(selectVisibleDashboardOverview("local", previousSnapshot, remoteSnapshot)).toBe(previousSnapshot);
    expect(selectVisibleDashboardOverview("remote", previousSnapshot, remoteSnapshot)).toBe(remoteSnapshot);
  });
});
