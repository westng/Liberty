import { invoke } from "@tauri-apps/api/core";
import { getCurrentMessages } from "@/services/i18n";
import type {
  AiModelConfig,
  AiSummaryResult,
  AiSummaryTemplate,
  MeetingJob,
  TranscriptSegment,
} from "@/types/meeting";
import { normalizeSummaryResult } from "@/services/aiStorage";
import { getPrimaryTranscriptSegments } from "@/services/transcript";

interface GenerateAiSummaryInput {
  job: MeetingJob;
  model: AiModelConfig;
  template: AiSummaryTemplate;
  includeSpeaker: boolean;
  includeTimestamp: boolean;
  extraInstructions: string;
}

interface OpenAiChatCompletionResponse {
  choices?: Array<{
    message?: {
      content?:
        | string
        | Array<{
            type?: string;
            text?: string;
          }>;
    };
  }>;
}

function formatTimestamp(ms: number) {
  return new Date(ms).toISOString().slice(11, 19);
}

function formatSegment(
  segment: TranscriptSegment,
  includeSpeaker: boolean,
  includeTimestamp: boolean,
) {
  const messages = getCurrentMessages();
  const parts: string[] = [];

  if (includeTimestamp) {
    parts.push(`[${formatTimestamp(segment.startMs)} - ${formatTimestamp(segment.endMs)}]`);
  }

  if (includeSpeaker) {
    parts.push(`${segment.speaker ?? messages.common.unknownSpeaker}:`);
  }

  parts.push(segment.text);
  return parts.join(" ");
}

function extractResponseText(payload: OpenAiChatCompletionResponse) {
  const messages = getCurrentMessages();
  const content = payload.choices?.[0]?.message?.content;

  if (typeof content === "string") {
    return content.trim();
  }

  if (Array.isArray(content)) {
    return content
      .map((item) => item.text ?? "")
      .join("")
      .trim();
  }

  throw new Error(messages.aiSummary.emptyResponse);
}

export function buildSummaryPromptPreview({
  job,
  template,
  includeSpeaker,
  includeTimestamp,
  extraInstructions,
}: Omit<GenerateAiSummaryInput, "model">) {
  const { aiSummary, common } = getCurrentMessages();
  const transcript = getPrimaryTranscriptSegments(job)
    .map((segment) => formatSegment(segment, includeSpeaker, includeTimestamp))
    .join("\n");

  const userMessage = [
    `Meeting title: ${job.title}`,
    `Meeting language: ${job.lang}`,
    `Hotwords: ${job.hotwords.join(", ") || common.none}`,
    `Include speaker info: ${includeSpeaker ? "yes" : "no"}`,
    `Include timestamps: ${includeTimestamp ? "yes" : "no"}`,
    `Extra instructions: ${extraInstructions.trim() || common.none}`,
    "",
    "Please output JSON based on the following meeting content:",
    transcript || aiSummary.transcriptMissing,
  ].join("\n");

  return {
    system: template.prompt.trim(),
    user: userMessage,
  };
}

export async function generateAiSummary(input: GenerateAiSummaryInput) {
  const messages = getCurrentMessages();
  const { model, job } = input;
  const promptPreview = buildSummaryPromptPreview(input);
  const { rawResponse } = await invoke<{ rawResponse: string }>("request_ai_chat_completion", {
    input: {
      baseUrl: model.baseUrl,
      apiKey: model.apiKey,
      model: model.model,
      systemPrompt: promptPreview.system,
      userPrompt: promptPreview.user,
    },
  });

  let payload: OpenAiChatCompletionResponse;

  try {
    payload = JSON.parse(rawResponse) as OpenAiChatCompletionResponse;
  } catch {
    throw new Error(messages.aiSummary.invalidApiJson);
  }

  const content = extractResponseText(payload);

  let structured: Partial<AiSummaryResult>;

  try {
    structured = JSON.parse(content) as Partial<AiSummaryResult>;
  } catch {
    throw new Error(messages.aiSummary.invalidStructuredJson);
  }

  return {
    promptPreview: `${promptPreview.system}\n\n---\n\n${promptPreview.user}`,
    rawResponse,
    result: normalizeSummaryResult(structured, job.title),
  };
}
