<script setup lang="ts">
import { confirm } from "@tauri-apps/plugin-dialog";
import { computed, onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useAiStore } from "@/composables/useAiStore";
import type { AiModelConfig } from "@/types/meeting";

const route = useRoute();
const aiStore = useAiStore();

const selectedId = ref<string | null>(null);
const draft = ref<AiModelConfig>(createFreshDraft());
const errorMessage = ref("");

const selectedModel = computed(() =>
  selectedId.value ? aiStore.getModelById(selectedId.value) ?? null : null,
);

watch(selectedModel, (nextModel) => {
  if (!nextModel) {
    return;
  }

  draft.value = { ...nextModel };
});

onMounted(async () => {
  await aiStore.ensureLoaded();
  const modelId = typeof route.query.id === "string" ? route.query.id : null;

  if (modelId) {
    const model = aiStore.getModelById(modelId);
    if (model) {
      selectedId.value = model.id;
      draft.value = { ...model };
      await syncWindowTitle(true);
      return;
    }
  }

  selectedId.value = null;
  draft.value = createFreshDraft();
  await syncWindowTitle(false);
});

function createFreshDraft() {
  return aiStore.createModel();
}

function validateDraft() {
  if (!draft.value.name.trim()) {
    return "模型名称不能为空。";
  }

  if (!draft.value.baseUrl.trim()) {
    return "Base URL 不能为空。";
  }

  if (!draft.value.apiKey.trim()) {
    return "API Key 不能为空。";
  }

  if (!draft.value.model.trim()) {
    return "Model 不能为空。";
  }

  return "";
}

async function syncWindowTitle(isEdit: boolean) {
  try {
    await getCurrentWindow().setTitle(isEdit ? "编辑模型" : "新增模型");
  } catch {
    // ignore
  }
}

async function save() {
  const validation = validateDraft();

  if (validation) {
    errorMessage.value = validation;
    return;
  }

  await aiStore.saveModel({
    ...draft.value,
    name: draft.value.name.trim(),
    baseUrl: draft.value.baseUrl.trim(),
    apiKey: draft.value.apiKey.trim(),
    model: draft.value.model.trim(),
  });

  selectedId.value = draft.value.id;
  draft.value = { ...(aiStore.getModelById(draft.value.id) ?? draft.value) };
  errorMessage.value = "";
  await syncWindowTitle(true);
}

async function removeModel() {
  if (!selectedModel.value) {
    return;
  }

  const confirmed = await confirm(`确认删除模型“${selectedModel.value.name}”吗？`, {
    title: "删除模型",
    kind: "warning",
    okLabel: "删除",
    cancelLabel: "取消",
  });
  if (!confirmed) {
    return;
  }

  await aiStore.deleteModel(selectedModel.value.id);
  selectedId.value = null;
  draft.value = createFreshDraft();
  await syncWindowTitle(false);
}

function resetDraft() {
  selectedId.value = null;
  draft.value = createFreshDraft();
  errorMessage.value = "";
  void syncWindowTitle(false);
}
</script>

<template>
  <section class="editor-window-shell">
    <article class="surface editor-window-card">
      <div class="section-heading">
        <h3>{{ selectedId ? "编辑模型" : "新增模型" }}</h3>
      </div>

      <div class="field-grid">
        <div class="field">
          <label for="model-name">模型名称</label>
          <input id="model-name" v-model="draft.name" placeholder="例如：OpenAI GPT-4.1" />
        </div>

        <div class="field">
          <label for="model-base-url">Base URL</label>
          <input
            id="model-base-url"
            v-model="draft.baseUrl"
            placeholder="例如：https://api.openai.com/v1"
          />
        </div>

        <div class="field">
          <label for="model-api-key">API Key</label>
          <input
            id="model-api-key"
            v-model="draft.apiKey"
            type="password"
            placeholder="sk-..."
            autocomplete="off"
          />
        </div>

        <div class="field">
          <label for="model-id">Model</label>
          <input id="model-id" v-model="draft.model" placeholder="例如：gpt-4.1" />
        </div>

        <div class="field-grid two-col">
          <label class="toggle-field">
            <input v-model="draft.enabled" type="checkbox" />
            <span>启用该模型</span>
          </label>

          <label class="toggle-field">
            <input v-model="draft.isDefault" type="checkbox" />
            <span>设为默认模型</span>
          </label>
        </div>
      </div>

      <div v-if="errorMessage" class="note-block error-block">
        {{ errorMessage }}
      </div>

      <div class="button-row">
        <button class="primary-button" type="button" @click="save">
          保存模型
        </button>
        <button class="secondary-button" type="button" @click="resetDraft">
          清空表单
        </button>
        <button
          v-if="selectedId"
          class="text-button danger-text"
          type="button"
          @click="removeModel"
        >
          删除模型
        </button>
      </div>
    </article>
  </section>
</template>
