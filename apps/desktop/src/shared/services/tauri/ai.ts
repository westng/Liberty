import { invoke } from "@tauri-apps/api/core";
import type {
  AiModelConfig,
  AiModelSaveInput,
  AiSummaryRun,
  AiSummaryTemplate,
} from "@/shared/types/meeting";

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

export function createLocalAiService() {
  return {
    listModels: () => invoke<AiModelConfig[]>("list_ai_models"),
    saveModel: (model: AiModelSaveInput) => invoke<void>("save_ai_model", { model }),
    deleteModel: (id: string) => invoke<void>("delete_ai_model", { id }),
    listTemplates: () => invoke<AiSummaryTemplate[]>("list_ai_templates"),
    saveTemplate: (template: AiSummaryTemplate) => invoke<void>("save_ai_template", { template }),
    deleteTemplate: (id: string) => invoke<void>("delete_ai_template", { id }),
    getSummaryOptions: (source: "local") => invoke<AiSummaryOptions>("get_ai_summary_options", { source }),
    listSummaryRuns: (source: "local", jobId: string, windowScopeToken?: string) =>
      invoke<AiSummaryRun[]>("list_ai_summary_runs", { source, jobId, windowScopeToken }),
    startOrResumeSummaryRun: (input: StartAiSummaryRunInput) =>
      invoke<AiSummaryRun>("start_or_resume_ai_summary_run", { input }),
    setActiveSummaryRun: (source: "local", jobId: string, runId: string, windowScopeToken?: string) =>
      invoke<void>("set_active_ai_summary_run", { source, jobId, runId, windowScopeToken }),
    deleteSummaryRun: (source: "local", jobId: string, runId: string, windowScopeToken?: string) =>
      invoke<void>("delete_ai_summary_run", { source, jobId, runId, windowScopeToken }),
  };
}
