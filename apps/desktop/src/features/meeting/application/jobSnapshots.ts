import type { MeetingJob } from "@/shared/types/meeting";

const activeStatuses = new Set([
  "queued",
  "transcribing",
  "speaker_processing",
  "summarizing",
]);

export function hasActiveJobs(jobs: MeetingJob[]) {
  return jobs.some((job) => activeStatuses.has(job.overallStatus));
}

function hasSummaryContent(job: MeetingJob) {
  return Boolean(
    job.summary.overview
    || job.summary.topics.length
    || job.summary.decisions.length
    || job.summary.actionItems.length
    || job.summary.risks?.length
    || job.summary.followUps?.length,
  );
}

export function mergeJobSnapshot(
  existing: MeetingJob | undefined,
  incoming: MeetingJob,
  hydratedJobIds: Set<string>,
) {
  if (!existing || !hydratedJobIds.has(incoming.id)) {
    return incoming;
  }

  const hasIncomingSummary = incoming.summaryRuns.length > 0 || hasSummaryContent(incoming);
  const shouldPreserveSummary = !hasIncomingSummary
    && incoming.activeSummaryRunId === existing.activeSummaryRunId;

  return {
    ...existing,
    ...incoming,
    transcriptSegments: incoming.transcriptSegments.length
      ? incoming.transcriptSegments
      : existing.transcriptSegments,
    speakerSegments: incoming.speakerSegments.length
      ? incoming.speakerSegments
      : existing.speakerSegments,
    summaryRuns: shouldPreserveSummary ? existing.summaryRuns : incoming.summaryRuns,
    processLog: incoming.processLog ?? existing.processLog,
    summary: shouldPreserveSummary ? existing.summary : incoming.summary,
  } satisfies MeetingJob;
}
