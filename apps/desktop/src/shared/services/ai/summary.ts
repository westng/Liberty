import { getCurrentMessages } from "@/shared/i18n";
import {
  createLocalAiService,
  type StartAiSummaryRunInput,
} from "@/shared/services/tauri/ai";

const localAiService = createLocalAiService();

export async function startOrResumeAiSummaryRun(input: StartAiSummaryRunInput) {
  const messages = getCurrentMessages();

  try {
    return await localAiService.startOrResumeSummaryRun(input);
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
