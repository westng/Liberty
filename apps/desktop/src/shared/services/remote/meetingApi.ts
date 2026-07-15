import { invoke } from "@tauri-apps/api/core";
import type { MeetingJob } from "@/shared/types/meeting";

const PROTOCOL_NAME = "liberty-meeting";
const PROTOCOL_VERSION = 1;
const JOB_SCHEMA_VERSION = 1;

export type RemoteMeetingOperation =
  | "jobs.list"
  | "jobs.read"
  | "jobs.result.read"
  | "jobs.create"
  | "jobs.retry"
  | "jobs.delete"
  | "transcript.speakers.rename"
  | "summary.runs.read"
  | "summary.runs.write"
  | "exports.generate";

export interface RemoteMeetingCapabilities {
  protocol: typeof PROTOCOL_NAME;
  protocolVersion: typeof PROTOCOL_VERSION;
  jobSchemaVersion: typeof JOB_SCHEMA_VERSION;
  serviceVersion: string;
  operations: RemoteMeetingOperation[];
  jobCreate?: {
    uploadMode: "multipart" | "chunked";
    maxFiles: number;
    maxBytesPerFile: number;
    extensions: string[];
  };
  exports?: {
    formats: string[];
  };
}

export class RemoteCapabilityError extends Error {
  constructor(message: string) {
    super(`capability_unavailable: ${message}`);
    this.name = "RemoteCapabilityError";
  }
}

function requireOperation(
  capabilities: RemoteMeetingCapabilities,
  operation: RemoteMeetingOperation,
) {
  if (!capabilities.operations.includes(operation)) {
    throw new RemoteCapabilityError(`远端服务未声明 ${operation} 能力。`);
  }
}

function normalizeCapabilities(value: RemoteMeetingCapabilities): RemoteMeetingCapabilities {
  if (
    value.protocol !== PROTOCOL_NAME
    || value.protocolVersion !== PROTOCOL_VERSION
    || value.jobSchemaVersion !== JOB_SCHEMA_VERSION
    || !value.serviceVersion?.trim()
    || !Array.isArray(value.operations)
  ) {
    throw new RemoteCapabilityError("远端服务协议版本与当前客户端不兼容。");
  }
  return value;
}

export function createMeetingApi() {
  return {
    getCapabilities: async () => normalizeCapabilities(
      await invoke<RemoteMeetingCapabilities>("get_remote_capabilities"),
    ),
    listJobs: async (capabilities: RemoteMeetingCapabilities) => {
      requireOperation(capabilities, "jobs.list");
      return invoke<MeetingJob[]>("remote_list_jobs");
    },
    getJob: async (capabilities: RemoteMeetingCapabilities, id: string) => {
      requireOperation(capabilities, "jobs.read");
      return invoke<MeetingJob>("remote_get_job", { id });
    },
    getResult: async (capabilities: RemoteMeetingCapabilities, id: string) => {
      requireOperation(capabilities, "jobs.result.read");
      return invoke<MeetingJob>("remote_get_job_result", { id });
    },
    retryJob: async (capabilities: RemoteMeetingCapabilities, id: string) => {
      requireOperation(capabilities, "jobs.retry");
      return invoke<MeetingJob>("remote_retry_job", { id });
    },
    deleteJob: async (capabilities: RemoteMeetingCapabilities, id: string) => {
      requireOperation(capabilities, "jobs.delete");
      await invoke<void>("remote_delete_job", { id });
    },
    renameSpeaker: async (
      capabilities: RemoteMeetingCapabilities,
      id: string,
      fromSpeaker: string,
      toSpeaker: string,
    ) => {
      requireOperation(capabilities, "transcript.speakers.rename");
      return invoke<MeetingJob>("remote_rename_job_speaker", {
        id,
        fromSpeaker,
        toSpeaker,
      });
    },
  };
}
