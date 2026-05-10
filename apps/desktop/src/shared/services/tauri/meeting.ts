import { invoke } from "@tauri-apps/api/core";
import type { MeetingJob, NewMeetingJobInput } from "@/shared/types/meeting";

interface LocalCreateJobInput extends NewMeetingJobInput {
  createdAt: string;
}

export function createLocalMeetingService() {
  return {
    createJob: (payload: NewMeetingJobInput) =>
      invoke<MeetingJob>("create_job", {
        input: {
          ...payload,
          createdAt: new Date().toISOString(),
        } satisfies LocalCreateJobInput,
      }),
    listJobs: () => invoke<MeetingJob[]>("list_jobs"),
    deleteJob: (id: string) => invoke<void>("delete_job", { id }),
    getJob: (id: string) => invoke<MeetingJob>("get_job", { id }),
    getJobResult: (id: string) => invoke<MeetingJob>("get_job_result", { id }),
    renameSpeaker: (id: string, fromSpeaker: string, toSpeaker: string) =>
      invoke<MeetingJob>("rename_job_speaker", {
        id,
        fromSpeaker,
        toSpeaker,
      }),
    retryJob: (id: string) => invoke<MeetingJob>("retry_job", { id }),
  };
}
