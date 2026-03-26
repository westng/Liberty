import { reactive, toRefs } from "vue";
import { createDraftModelConfig, createDraftTemplate } from "@/services/aiStorage";
import { createLocalAiService } from "@/services/localAi";
import type { AiModelConfig, AiSummaryTemplate } from "@/types/meeting";

const aiService = createLocalAiService();

const state = reactive({
  models: [] as AiModelConfig[],
  templates: [] as AiSummaryTemplate[],
});

let hasLoaded = false;

function normalizeDefaultModel(models: AiModelConfig[]) {
  const enabledModels = models.filter((model) => model.enabled);
  const defaultModel = enabledModels.find((model) => model.isDefault) ?? enabledModels[0];

  return models.map((model) => ({
    ...model,
    isDefault: defaultModel ? model.id === defaultModel.id : false,
  }));
}

export function useAiStore() {
  async function reloadState() {
    const [models, templates] = await Promise.all([
      aiService.listModels(),
      aiService.listTemplates(),
    ]);

    state.models = models;
    state.templates = templates;
    hasLoaded = true;
  }

  async function ensureLoaded() {
    if (hasLoaded) {
      return;
    }

    await reloadState();
  }

  function createModel() {
    return createDraftModelConfig();
  }

  async function saveModel(model: AiModelConfig) {
    const nextModel = {
      ...model,
      updatedAt: new Date().toISOString(),
    };
    const current = state.models.some((item) => item.id === model.id)
      ? state.models.map((item) => (item.id === model.id ? nextModel : item))
      : [nextModel, ...state.models];
    const normalized = normalizeDefaultModel(current);
    const target = normalized.find((item) => item.id === nextModel.id) ?? nextModel;

    await aiService.saveModel(target);
    await reloadState();
  }

  async function deleteModel(id: string) {
    await aiService.deleteModel(id);
    await reloadState();
  }

  function createTemplate() {
    return createDraftTemplate();
  }

  function duplicateTemplate(templateId: string) {
    const source = state.templates.find((item) => item.id === templateId);

    if (!source) {
      return null;
    }

    const time = new Date().toISOString();
    return {
      ...source,
      id: crypto.randomUUID(),
      name: `${source.name} - 副本`,
      builtin: false,
      createdAt: time,
      updatedAt: time,
    } satisfies AiSummaryTemplate;
  }

  async function saveTemplate(template: AiSummaryTemplate) {
    const nextTemplate = {
      ...template,
      builtin: false,
      updatedAt: new Date().toISOString(),
    };

    await aiService.saveTemplate(nextTemplate);
    await reloadState();
  }

  async function deleteTemplate(id: string) {
    await aiService.deleteTemplate(id);
    await reloadState();
  }

  async function insertTemplate(template: AiSummaryTemplate) {
    await aiService.saveTemplate(template);
    await reloadState();
  }

  function getDefaultModel() {
    return state.models.find((model) => model.isDefault && model.enabled) ?? state.models.find((model) => model.enabled);
  }

  function getTemplateById(id: string) {
    return state.templates.find((template) => template.id === id);
  }

  function getModelById(id: string) {
    return state.models.find((model) => model.id === id);
  }

  return {
    ...toRefs(state),
    ensureLoaded,
    createModel,
    saveModel,
    deleteModel,
    createTemplate,
    insertTemplate,
    duplicateTemplate,
    saveTemplate,
    deleteTemplate,
    getDefaultModel,
    getTemplateById,
    getModelById,
    reloadState,
  };
}
