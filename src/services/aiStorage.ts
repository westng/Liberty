import type {
  AiModelConfig,
  AiSummaryResult,
  AiSummaryRun,
  AiSummaryTemplate,
  MeetingSummary,
} from "@/types/meeting";

function nowIso() {
  return new Date().toISOString();
}

export function createEmptyAiSummaryResult(title = ""): AiSummaryResult {
  return {
    title,
    overview: "",
    topics: [],
    decisions: [],
    actionItems: [],
    risks: [],
    followUps: [],
  };
}

export function createEmptyMeetingSummary(title = ""): MeetingSummary {
  const summary = createEmptyAiSummaryResult(title);

  return {
    overview: summary.overview,
    topics: summary.topics,
    decisions: summary.decisions,
    actionItems: [],
    risks: summary.risks,
    followUps: summary.followUps,
  };
}

export function summaryResultToMeetingSummary(result: AiSummaryResult): MeetingSummary {
  return {
    overview: result.overview,
    topics: result.topics,
    decisions: result.decisions,
    actionItems: result.actionItems.map((item) => {
      const suffix = [item.owner?.trim(), item.dueDate?.trim()].filter(Boolean).join(" / ");
      return suffix ? `${item.task}（${suffix}）` : item.task;
    }),
    risks: result.risks,
    followUps: result.followUps,
  };
}

function toTrimmedString(value: unknown) {
  if (typeof value === "string") {
    return value.trim();
  }

  if (value === null || value === undefined) {
    return "";
  }

  if (typeof value === "number" || typeof value === "boolean") {
    return String(value).trim();
  }

  if (Array.isArray(value)) {
    return value
      .map((item) => toTrimmedString(item))
      .filter(Boolean)
      .join(" ")
      .trim();
  }

  if (typeof value === "object") {
    return Object.values(value as Record<string, unknown>)
      .map((item) => toTrimmedString(item))
      .filter(Boolean)
      .join(" ")
      .trim();
  }

  return "";
}

function toStringArray(value: unknown) {
  if (!Array.isArray(value)) {
    const single = toTrimmedString(value);
    return single ? [single] : [];
  }

  return value.map((item) => toTrimmedString(item)).filter(Boolean);
}

export function normalizeSummaryResult(input: Partial<AiSummaryResult>, fallbackTitle: string) {
  return {
    title: toTrimmedString(input.title) || fallbackTitle,
    overview: toTrimmedString(input.overview),
    topics: toStringArray(input.topics),
    decisions: toStringArray(input.decisions),
    actionItems: Array.isArray(input.actionItems)
      ? input.actionItems
          .map((item) => ({
            task: toTrimmedString(item?.task),
            owner: toTrimmedString(item?.owner),
            dueDate: toTrimmedString(item?.dueDate),
          }))
          .filter((item) => item.task)
      : [],
    risks: toStringArray(input.risks),
    followUps: toStringArray(input.followUps),
  } satisfies AiSummaryResult;
}

export function createDraftModelConfig(): AiModelConfig {
  const time = nowIso();
  return {
    id: crypto.randomUUID(),
    name: "",
    baseUrl: "",
    apiKey: "",
    model: "",
    enabled: true,
    isDefault: false,
    createdAt: time,
    updatedAt: time,
  };
}

export function createDraftTemplate(): AiSummaryTemplate {
  const time = nowIso();
  return {
    id: crypto.randomUUID(),
    name: "",
    description: "",
    prompt: "",
    includeSpeakerByDefault: true,
    includeTimestampByDefault: true,
    builtin: false,
    createdAt: time,
    updatedAt: time,
  };
}

export function buildSummaryRun(
  input: Omit<AiSummaryRun, "id" | "createdAt" | "updatedAt">,
): AiSummaryRun {
  const time = nowIso();
  return {
    ...input,
    id: crypto.randomUUID(),
    createdAt: time,
    updatedAt: time,
  };
}
