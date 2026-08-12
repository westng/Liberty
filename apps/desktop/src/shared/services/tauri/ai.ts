import { invoke } from "@tauri-apps/api/core";
import type { AiModelMetadataV1, AiSummaryRunV1 } from "@liberty-contracts/ai-v1";
import type {
  AiModelSaveInput,
  AiSummaryRun,
  AiSummaryTemplate,
} from "@/shared/types/meeting";
import { normalizeAppError } from "@/shared/services/errors/appError";

async function invokeAiCredentialCommand(command: string, args: Record<string, unknown>) {
  try {
    return await invoke<void>(command, args);
  } catch (error) {
    throw normalizeAppError(error, {
      code: "ai_credential_operation_failed",
      category: "credentials",
      retryable: true,
      params: {},
    });
  }
}

export interface StartAiSummaryRunInput {
  source: "local";
  jobId: string;
  windowScopeToken?: string;
  runId?: string;
  modelConfigId?: string;
  templateId?: string;
  includeSpeaker?: boolean;
  includeTimestamp?: boolean;
  useMemberMapping?: boolean;
  extraInstructions?: string;
}

export interface AiSummaryModelOption {
  id: string;
  name: string;
  model: string;
  enabled: boolean;
  isDefault: boolean;
}

export interface AiSummaryOptions {
  models: AiSummaryModelOption[];
  templates: AiSummaryTemplate[];
}

type AiSummaryRunTransport = Omit<AiSummaryRun, "errorMessage"> & AiSummaryRunV1;

function normalizeSummaryRun(run: AiSummaryRunTransport): AiSummaryRun {
  return {
    ...run,
    errorMessage: run.errorMessage ?? undefined,
  };
}

export function createLocalAiService() {
  return {
    listModels: () => invoke<AiModelMetadataV1[]>("list_ai_models"),
    saveModel: (model: AiModelSaveInput) => invokeAiCredentialCommand("save_ai_model", { model }),
    deleteModel: (id: string) => invokeAiCredentialCommand("delete_ai_model", { id }),
    listTemplates: () => invoke<AiSummaryTemplate[]>("list_ai_templates"),
    saveTemplate: (template: AiSummaryTemplate) => invoke<void>("save_ai_template", { template }),
    deleteTemplate: (id: string) => invoke<void>("delete_ai_template", { id }),
    getSummaryOptions: (source: "local") => invoke<AiSummaryOptions>("get_ai_summary_options", { source }),
    listSummaryRuns: (source: "local", jobId: string, windowScopeToken?: string) =>
      invoke<AiSummaryRunTransport[]>("list_ai_summary_runs", { source, jobId, windowScopeToken })
        .then((runs) => runs.map(normalizeSummaryRun)),
    startOrResumeSummaryRun: (input: StartAiSummaryRunInput) =>
      invoke<AiSummaryRunTransport>("start_or_resume_ai_summary_run", { input }).then(normalizeSummaryRun),
    setActiveSummaryRun: (source: "local", jobId: string, runId: string, windowScopeToken?: string) =>
      invoke<void>("set_active_ai_summary_run", { source, jobId, runId, windowScopeToken }),
    deleteSummaryRun: (source: "local", jobId: string, runId: string, windowScopeToken?: string) =>
      invoke<void>("delete_ai_summary_run", { source, jobId, runId, windowScopeToken }),
  };
}
