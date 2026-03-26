import { invoke } from "@tauri-apps/api/core";
import type { AiModelConfig, AiSummaryRun, AiSummaryTemplate } from "@/types/meeting";

export function createLocalAiService() {
  return {
    listModels: () => invoke<AiModelConfig[]>("list_ai_models"),
    saveModel: (model: AiModelConfig) => invoke<void>("save_ai_model", { model }),
    deleteModel: (id: string) => invoke<void>("delete_ai_model", { id }),
    listTemplates: () => invoke<AiSummaryTemplate[]>("list_ai_templates"),
    saveTemplate: (template: AiSummaryTemplate) => invoke<void>("save_ai_template", { template }),
    deleteTemplate: (id: string) => invoke<void>("delete_ai_template", { id }),
    listSummaryRuns: (jobId: string) => invoke<AiSummaryRun[]>("list_ai_summary_runs", { jobId }),
    saveSummaryRun: (run: AiSummaryRun) => invoke<void>("save_ai_summary_run", { run }),
    setActiveSummaryRun: (jobId: string, runId: string) =>
      invoke<void>("set_active_ai_summary_run", { jobId, runId }),
    deleteSummaryRun: (jobId: string, runId: string) =>
      invoke<void>("delete_ai_summary_run", { jobId, runId }),
  };
}
