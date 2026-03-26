import type { MeetingJob, NewMeetingJobInput } from "@/types/meeting";

async function requestJson<T>(
  url: string,
  init?: RequestInit,
  apiToken?: string,
): Promise<T> {
  const response = await fetch(url, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...(apiToken ? { Authorization: `Bearer ${apiToken}` } : {}),
      ...(init?.headers ?? {}),
    },
  });

  if (!response.ok) {
    throw new Error(`HTTP ${response.status}`);
  }

  return (await response.json()) as T;
}

export function createMeetingApi(baseUrl: string, apiToken?: string) {
  const normalized = baseUrl.replace(/\/$/, "");

  return {
    listJobs: () => requestJson<MeetingJob[]>(`${normalized}/api/jobs`, undefined, apiToken),
    getJob: (id: string) =>
      requestJson<MeetingJob>(`${normalized}/api/jobs/${id}`, undefined, apiToken),
    getResult: (id: string) =>
      requestJson<MeetingJob>(`${normalized}/api/jobs/${id}/result`, undefined, apiToken),
    createJob: (payload: NewMeetingJobInput) =>
      requestJson<MeetingJob>(
        `${normalized}/api/jobs`,
        {
          method: "POST",
          body: JSON.stringify(payload),
        },
        apiToken,
      ),
    retryJob: (id: string) =>
      requestJson<MeetingJob>(
        `${normalized}/api/jobs/${id}/retry`,
        {
          method: "POST",
        },
        apiToken,
      ),
  };
}
