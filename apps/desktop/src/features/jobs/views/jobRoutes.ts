import { useEffect, useState } from "react";
import type { MeetingJob, MeetingJobRef, ProcessingMode } from "@/shared/types/meeting";

export type JobRouteIdentity = Pick<MeetingJobRef, "jobId" | "source">;
export type JobRouteRef = JobRouteIdentity;

function parseSource(value: string | null, fallback: ProcessingMode): ProcessingMode | null {
  if (value === null) {
    return fallback;
  }
  return value === "local" || value === "remote" ? value : null;
}

export function toJobRouteIdentity(job: Pick<MeetingJob, "id" | "source">): JobRouteIdentity {
  return { jobId: job.id, source: job.source };
}

export const jobRef = toJobRouteIdentity;

export function jobRefKey(reference: JobRouteIdentity) {
  return JSON.stringify([reference.source, reference.jobId]);
}

export function jobDetailRoute(reference: JobRouteIdentity) {
  const query = new URLSearchParams({ source: reference.source });
  return `/jobs/${encodeURIComponent(reference.jobId)}?${query.toString()}`;
}

export const jobDetailPath = jobDetailRoute;

export function jobWorkbenchRoute(reference: JobRouteIdentity) {
  const query = new URLSearchParams({ source: reference.source });
  return `/jobs/${encodeURIComponent(reference.jobId)}/workbench?${query.toString()}`;
}

export const jobWorkbenchPath = jobWorkbenchRoute;

export function resultsCenterRoute(reference?: JobRouteIdentity) {
  if (!reference) {
    return "/results";
  }
  const query = new URLSearchParams({
    job: reference.jobId,
    source: reference.source,
  });
  return `/results?${query.toString()}`;
}

export const resultsPath = resultsCenterRoute;

export function parseJobPathReference(
  decodedJobId: string | undefined,
  search: string,
  fallbackSource: ProcessingMode,
): JobRouteIdentity | null {
  if (!decodedJobId) {
    return null;
  }
  const params = new URLSearchParams(search);
  const source = parseSource(params.get("source"), fallbackSource);
  if (!source) {
    return null;
  }
  return {
    jobId: decodedJobId,
    source,
  };
}

export function parseResultsReference(
  search: string,
  fallbackSource: ProcessingMode,
): JobRouteIdentity | null {
  const params = new URLSearchParams(search);
  const jobId = params.get("job");
  if (!jobId) {
    return null;
  }
  const source = parseSource(params.get("source"), fallbackSource);
  if (!source) {
    return null;
  }
  return {
    jobId,
    source,
  };
}

export function readResultsJobRef(fallbackSource: ProcessingMode) {
  return parseResultsReference(window.location.search, fallbackSource);
}

export function useBoundJobRouteRef(
  decodedJobId: string,
  settingsLoaded: boolean,
  fallbackSource: ProcessingMode,
) {
  const [reference, setReference] = useState<JobRouteRef | null>(() => {
    const params = new URLSearchParams(window.location.search);
    const source = params.get("source");
    if (!decodedJobId || (source !== "local" && source !== "remote")) {
      return null;
    }
    return { jobId: decodedJobId, source };
  });

  useEffect(() => {
    if (!decodedJobId) {
      setReference(null);
      return;
    }
    const params = new URLSearchParams(window.location.search);
    const source = params.get("source");
    if (source === "local" || source === "remote") {
      setReference({ jobId: decodedJobId, source });
      return;
    }
    if (source !== null) {
      setReference(null);
      return;
    }
    if (settingsLoaded) {
      setReference((current) => current?.jobId === decodedJobId
        ? current
        : { jobId: decodedJobId, source: fallbackSource });
    }
  }, [decodedJobId, fallbackSource, settingsLoaded]);

  return reference;
}
