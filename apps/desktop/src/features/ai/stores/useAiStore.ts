import { useSyncExternalStore } from "react";
import { createDraftModelConfig, createDraftTemplate } from "@/shared/services/ai/storage";
import { createLocalAiService } from "@/shared/services/tauri/ai";
import type { AiModelConfig, AiSummaryTemplate } from "@/shared/types/meeting";

type AiState = {
  models: AiModelConfig[];
  templates: AiSummaryTemplate[];
};

let state: AiState = {
  models: [],
  templates: [],
};

const aiService = createLocalAiService();
const listeners = new Set<() => void>();
let hasLoaded = false;

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function getSnapshot() {
  return state;
}

function setState(patch: Partial<AiState>) {
  state = { ...state, ...patch };
  for (const listener of listeners) {
    listener();
  }
}

function normalizeDefaultModel(models: AiModelConfig[]) {
  const enabledModels = models.filter((model) => model.enabled);
  const defaultModel = enabledModels.find((model) => model.isDefault) ?? enabledModels[0];

  return models.map((model) => ({
    ...model,
    isDefault: defaultModel ? model.id === defaultModel.id : false,
  }));
}

async function reloadState() {
  const [models, templates] = await Promise.all([
    aiService.listModels(),
    aiService.listTemplates(),
  ]);

  setState({ models, templates });
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
  return state.models.find((model) => model.isDefault && model.enabled)
    ?? state.models.find((model) => model.enabled);
}

function getTemplateById(id: string) {
  return state.templates.find((template) => template.id === id);
}

function getModelById(id: string) {
  return state.models.find((model) => model.id === id);
}

const actions = {
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

export function useAiStore() {
  const snapshot = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);

  return {
    ...snapshot,
    ...actions,
  };
}

export type AiStore = ReturnType<typeof useAiStore>;
