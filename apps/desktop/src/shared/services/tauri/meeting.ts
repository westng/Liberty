import { invoke } from "@tauri-apps/api/core";
import type { MeetingJob, NewMeetingJobInput } from "@/shared/types/meeting";

interface LocalCreateJobInput extends NewMeetingJobInput {
  createdAt: string;
}

function asLocalJob(job: MeetingJob): MeetingJob {
  return { ...job, source: "local" };
}

export function createLocalMeetingService() {
  return {
    createJob: (payload: NewMeetingJobInput) =>
      invoke<MeetingJob>("create_job", {
        input: {
          ...payload,
          createdAt: new Date().toISOString(),
        } satisfies LocalCreateJobInput,
      }).then(asLocalJob),
    listJobs: () => invoke<MeetingJob[]>("list_jobs").then((jobs) => jobs.map(asLocalJob)),
    deleteJob: (id: string) => invoke<void>("delete_job", { id }),
    getJob: (id: string) => invoke<MeetingJob>("get_job", { id }).then(asLocalJob),
    getJobResult: (id: string, windowScopeToken?: string) =>
      invoke<MeetingJob>("get_job_result", { id, windowScopeToken }).then(asLocalJob),
    renameSpeaker: (id: string, fromSpeaker: string, toSpeaker: string) =>
      invoke<MeetingJob>("rename_job_speaker", {
        id,
        fromSpeaker,
        toSpeaker,
      }).then(asLocalJob),
    retryJob: (id: string) => invoke<MeetingJob>("retry_job", { id }).then(asLocalJob),
  };
}
