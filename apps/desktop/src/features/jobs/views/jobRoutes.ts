import { useEffect, useState } from "react";
import type { MeetingJob, MeetingJobRef, ProcessingMode } from "@/shared/types/meeting";

export type JobRouteIdentity = Pick<MeetingJobRef, "jobId" | "source">;
export type JobRouteRef = MeetingJobRef;

function parseSource(value: string | null, fallback?: ProcessingMode): ProcessingMode | null {
  if (value === null) {
    return fallback ?? null;
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

export function useBoundJobRouteRef(
  decodedJobId: string,
  settingsLoaded: boolean,
  fallbackSource: ProcessingMode,
) {
  const [reference, setReference] = useState<JobRouteRef | null>(() => {
    const params = new URLSearchParams(window.location.search);
    const source = params.get("source");
    const windowScopeToken = params.get("scopeToken")?.trim() || undefined;
    if (!decodedJobId || (source !== "local" && source !== "remote")) {
      return null;
    }
    return { jobId: decodedJobId, source, windowScopeToken };
  });

  useEffect(() => {
    if (!decodedJobId) {
      setReference(null);
      return;
    }
    const params = new URLSearchParams(window.location.search);
    const source = params.get("source");
    const windowScopeToken = params.get("scopeToken")?.trim() || undefined;
    if (source === "local" || source === "remote") {
      setReference({ jobId: decodedJobId, source, windowScopeToken });
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
