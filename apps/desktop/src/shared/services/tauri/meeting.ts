import { invoke } from "@tauri-apps/api/core";
import type { MeetingJobV1 } from "@liberty-contracts/meeting-job-v1";
import type {
  DashboardOverview,
  DashboardRange,
  MeetingJob,
  NewMeetingJobInput,
} from "@/shared/types/meeting";

interface LocalCreateJobInput extends NewMeetingJobInput {
  createdAt: string;
}

type MeetingJobTransport = Omit<
  MeetingJob,
  "runnerProtocolVersion" | "failureReason" | "transcriptSegments" | "speakerSegments"
> & MeetingJobV1;

function asLocalJob(job: MeetingJobTransport): MeetingJob {
  return {
    ...job,
    source: "local",
    runnerProtocolVersion: job.runnerProtocolVersion ?? undefined,
    failureReason: job.failureReason ?? undefined,
    transcriptSegments: job.transcriptSegments.map((segment) => ({
      ...segment,
      speaker: segment.speaker ?? undefined,
    })),
    speakerSegments: job.speakerSegments.map((segment) => ({
      ...segment,
      speaker: segment.speaker ?? undefined,
    })),
  };
}

export function createLocalMeetingService() {
  return {
    createJob: (payload: NewMeetingJobInput) =>
      invoke<MeetingJobTransport>("create_job", {
        input: {
          ...payload,
          createdAt: new Date().toISOString(),
        } satisfies LocalCreateJobInput,
      }).then(asLocalJob),
    listJobs: () => invoke<MeetingJobTransport[]>("list_jobs").then((jobs) => jobs.map(asLocalJob)),
    getDashboardOverview: (range: DashboardRange) =>
      invoke<DashboardOverview>("get_dashboard_overview", { range }),
    deleteJob: (id: string) => invoke<void>("delete_job", { id }),
    getJob: (id: string) => invoke<MeetingJobTransport>("get_job", { id }).then(asLocalJob),
    getJobResult: (id: string, windowScopeToken?: string) =>
      invoke<MeetingJobTransport>("get_job_result", { id, windowScopeToken }).then(asLocalJob),
    renameSpeaker: (
      id: string,
      fromSpeaker: string,
      toSpeaker: string,
      windowScopeToken?: string,
    ) =>
      invoke<MeetingJobTransport>("rename_job_speaker", {
        id,
        fromSpeaker,
        toSpeaker,
        windowScopeToken,
      }).then(asLocalJob),
    retryJob: (id: string) => invoke<MeetingJobTransport>("retry_job", { id }).then(asLocalJob),
  };
}
