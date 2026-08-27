import type { JobStage, MeetingJob, ProcessingMode } from "@/shared/types/meeting";

export type JobQueueFilter = "all" | "processing" | "completed" | "failed";

export const JOB_QUEUE_PAGE_SIZE = 10;

const PROCESSING_STAGES = new Set<JobStage>([
  "queued",
  "transcribing",
  "speaker_processing",
  "summarizing",
]);

export function isProcessingStage(stage: JobStage) {
  return PROCESSING_STAGES.has(stage);
}

export function filterJobQueue(
  jobs: MeetingJob[],
  processingMode: ProcessingMode,
  filter: JobQueueFilter,
  searchQuery: string,
) {
  const normalizedQuery = searchQuery.trim().toLocaleLowerCase();

  return jobs
    .filter((job) => job.source === processingMode)
    .filter((job) => matchesFilter(job, filter))
    .filter((job) => {
      if (!normalizedQuery) {
        return true;
      }
      return [job.title, ...job.sourceFiles.map((file) => file.name)]
        .some((value) => value.toLocaleLowerCase().includes(normalizedQuery));
    })
    .sort((left, right) => right.createdAt.localeCompare(left.createdAt));
}

export function paginateJobQueue(
  jobs: MeetingJob[],
  requestedPage: number,
  pageSize = JOB_QUEUE_PAGE_SIZE,
) {
  const safePageSize = Math.max(1, Math.floor(pageSize));
  const pageCount = Math.max(1, Math.ceil(jobs.length / safePageSize));
  const page = Math.min(Math.max(1, Math.floor(requestedPage)), pageCount);
  const startIndex = (page - 1) * safePageSize;

  return {
    items: jobs.slice(startIndex, startIndex + safePageSize),
    page,
    pageCount,
    from: jobs.length === 0 ? 0 : startIndex + 1,
    to: Math.min(startIndex + safePageSize, jobs.length),
    total: jobs.length,
  };
}

function matchesFilter(job: MeetingJob, filter: JobQueueFilter) {
  if (filter === "processing") {
    return isProcessingStage(job.overallStatus);
  }
  if (filter === "completed" || filter === "failed") {
    return job.overallStatus === filter;
  }
  return true;
}
