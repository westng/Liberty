import type { MeetingJob } from "@/shared/types/meeting";

const activeLocalStatuses = new Set([
  "queued",
  "transcribing",
  "speaker_processing",
  "summarizing",
]);

export function hasActiveLocalJobs(jobs: MeetingJob[]) {
  return jobs.some((job) => activeLocalStatuses.has(job.overallStatus));
}

export function mergeJobSnapshot(
  existing: MeetingJob | undefined,
  incoming: MeetingJob,
  hydratedJobIds: Set<string>,
) {
  if (!existing || !hydratedJobIds.has(incoming.id)) {
    return incoming;
  }

  return {
    ...existing,
    ...incoming,
    transcriptSegments: existing.transcriptSegments,
    speakerSegments: existing.speakerSegments,
    summaryRuns: existing.summaryRuns,
    processLog: existing.processLog,
    summary: existing.summary,
    activeSummaryRunId: existing.activeSummaryRunId,
  } satisfies MeetingJob;
}
