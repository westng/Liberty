import { invoke } from "@tauri-apps/api/core";
import type { MeetingJob, NewMeetingJobInput, SettingsState } from "@/types/meeting";

interface LocalCreateJobInput extends NewMeetingJobInput {
  createdAt: string;
  pythonPath: string;
}

interface LocalRuntimeConfig {
  pythonPath: SettingsState["pythonPath"];
}

export function createLocalMeetingService() {
  return {
    createJob: (payload: NewMeetingJobInput, runtime: LocalRuntimeConfig) =>
      invoke<MeetingJob>("create_job", {
        input: {
          ...payload,
          createdAt: new Date().toISOString(),
          pythonPath: runtime.pythonPath,
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
    retryJob: (id: string, runtime: LocalRuntimeConfig) =>
      invoke<MeetingJob>("retry_job", {
        id,
        pythonPath: runtime.pythonPath,
      }),
  };
}
