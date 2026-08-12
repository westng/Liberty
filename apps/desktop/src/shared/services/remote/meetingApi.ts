import { invoke } from "@tauri-apps/api/core";
import type { MeetingJob } from "@/shared/types/meeting";
import { appError, normalizeAppError } from "@/shared/services/errors/appError";

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
    super(message);
    this.name = "RemoteCapabilityError";
  }
}

async function invokeRemote<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw normalizeAppError(error, {
      code: "remote_service_unavailable",
      category: "network",
      retryable: true,
      params: {},
    });
  }
}

function requireOperation(
  capabilities: RemoteMeetingCapabilities,
  operation: RemoteMeetingOperation,
) {
  if (!capabilities.operations.includes(operation)) {
    throw appError("remote_service_unavailable", "protocol", false, { operation });
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
    throw appError("remote_service_unavailable", "protocol", false);
  }
  return value;
}

export function createMeetingApi() {
  return {
    getCapabilities: async () => normalizeCapabilities(
      await invokeRemote<RemoteMeetingCapabilities>("get_remote_capabilities"),
    ),
    listJobs: async (capabilities: RemoteMeetingCapabilities) => {
      requireOperation(capabilities, "jobs.list");
      return invokeRemote<MeetingJob[]>("remote_list_jobs");
    },
    getJob: async (capabilities: RemoteMeetingCapabilities, id: string) => {
      requireOperation(capabilities, "jobs.read");
      return invokeRemote<MeetingJob>("remote_get_job", { id });
    },
    getResult: async (capabilities: RemoteMeetingCapabilities, id: string) => {
      requireOperation(capabilities, "jobs.result.read");
      return invokeRemote<MeetingJob>("remote_get_job_result", { id });
    },
    retryJob: async (capabilities: RemoteMeetingCapabilities, id: string) => {
      requireOperation(capabilities, "jobs.retry");
      return invokeRemote<MeetingJob>("remote_retry_job", { id });
    },
    deleteJob: async (capabilities: RemoteMeetingCapabilities, id: string) => {
      requireOperation(capabilities, "jobs.delete");
      await invokeRemote<void>("remote_delete_job", { id });
    },
    renameSpeaker: async (
      capabilities: RemoteMeetingCapabilities,
      id: string,
      fromSpeaker: string,
      toSpeaker: string,
    ) => {
      requireOperation(capabilities, "transcript.speakers.rename");
      return invokeRemote<MeetingJob>("remote_rename_job_speaker", {
        id,
        fromSpeaker,
        toSpeaker,
      });
    },
  };
}
