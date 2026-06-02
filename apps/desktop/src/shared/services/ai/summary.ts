import { invoke } from "@tauri-apps/api/core";
import { getCurrentMessages } from "@/shared/i18n";
import type {
  AiModelConfig,
  MeetingMinutesPayload,
  AiSummaryResult,
  AiSummaryTemplate,
  MeetingJob,
  MeetingMember,
} from "@/shared/types/meeting";

interface GenerateAiSummaryInput {
  job: MeetingJob;
  model: AiModelConfig;
  template: AiSummaryTemplate;
  includeSpeaker: boolean;
  includeTimestamp: boolean;
  useMemberMapping?: boolean;
  members?: MeetingMember[];
  extraInstructions: string;
}

export async function generateAiSummary(input: GenerateAiSummaryInput) {
  const messages = getCurrentMessages();

  try {
    return await invoke<{
      promptPreview: string;
      rawResponse: string;
      result: AiSummaryResult;
      minutesPayload: MeetingMinutesPayload;
    }>("generate_ai_summary", { input });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);

    if (message.includes("原始 JSON")) {
      throw new Error(messages.aiSummary.invalidApiJson);
    }

    if (message.includes("结构化 JSON")) {
      throw new Error(messages.aiSummary.invalidStructuredJson);
    }

    if (message.includes("响应内容为空")) {
      throw new Error(messages.aiSummary.emptyResponse);
    }

    throw error;
  }
}
