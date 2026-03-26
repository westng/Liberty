import { invoke } from "@tauri-apps/api/core";
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
  const parts: string[] = [];

  if (includeTimestamp) {
    parts.push(`[${formatTimestamp(segment.startMs)} - ${formatTimestamp(segment.endMs)}]`);
  }

  if (includeSpeaker) {
    parts.push(`${segment.speaker ?? "未知说话人"}:`);
  }

  parts.push(segment.text);
  return parts.join(" ");
}

function extractResponseText(payload: OpenAiChatCompletionResponse) {
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

  throw new Error("模型返回内容为空。");
}

export function buildSummaryPromptPreview({
  job,
  template,
  includeSpeaker,
  includeTimestamp,
  extraInstructions,
}: Omit<GenerateAiSummaryInput, "model">) {
  const transcript = getPrimaryTranscriptSegments(job)
    .map((segment) => formatSegment(segment, includeSpeaker, includeTimestamp))
    .join("\n");

  const userMessage = [
    `会议标题：${job.title}`,
    `会议语言：${job.lang}`,
    `热词：${job.hotwords.join("、") || "无"}`,
    `说话人信息：${includeSpeaker ? "包含" : "不包含"}`,
    `时间戳：${includeTimestamp ? "包含" : "不包含"}`,
    `补充要求：${extraInstructions.trim() || "无"}`,
    "",
    "请基于以下会议内容输出 JSON：",
    transcript || "当前没有可用的逐字稿内容。",
  ].join("\n");

  return {
    system: template.prompt.trim(),
    user: userMessage,
  };
}

export async function generateAiSummary(input: GenerateAiSummaryInput) {
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
    throw new Error("AI 接口返回的不是合法 JSON。");
  }

  const content = extractResponseText(payload);

  let structured: Partial<AiSummaryResult>;

  try {
    structured = JSON.parse(content) as Partial<AiSummaryResult>;
  } catch {
    throw new Error("模型返回内容无法解析为结构化 JSON。");
  }

  return {
    promptPreview: `${promptPreview.system}\n\n---\n\n${promptPreview.user}`,
    rawResponse,
    result: normalizeSummaryResult(structured, job.title),
  };
}
