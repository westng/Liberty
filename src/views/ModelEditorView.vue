<script setup lang="ts">
import { confirm } from "@tauri-apps/plugin-dialog";
import { computed, onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useAiStore } from "@/composables/useAiStore";
import { useMeetingStore } from "@/composables/useMeetingStore";
import { formatMessage, getMessages } from "@/services/i18n";
import type { AiModelConfig } from "@/types/meeting";

const route = useRoute();
const aiStore = useAiStore();
const meetingStore = useMeetingStore();

const selectedId = ref<string | null>(null);
const draft = ref<AiModelConfig>(createFreshDraft());
const errorMessage = ref("");
const messages = computed(() => getMessages(meetingStore.settings.value.locale).models);
const commonMessages = computed(() => getMessages(meetingStore.settings.value.locale).common);

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
    return messages.value.validationName;
  }

  if (!draft.value.baseUrl.trim()) {
    return messages.value.validationBaseUrl;
  }

  if (!draft.value.apiKey.trim()) {
    return messages.value.validationApiKey;
  }

  if (!draft.value.model.trim()) {
    return messages.value.validationModel;
  }

  return "";
}

async function syncWindowTitle(isEdit: boolean) {
  try {
    await getCurrentWindow().setTitle(isEdit ? messages.value.editorEditTitle : messages.value.editorNewTitle);
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

  const confirmed = await confirm(formatMessage(messages.value.deleteConfirm, { name: selectedModel.value.name }), {
    title: messages.value.deleteTitle,
    kind: "warning",
    okLabel: commonMessages.value.delete,
    cancelLabel: commonMessages.value.cancel,
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
        <h3>{{ selectedId ? messages.editorEditTitle : messages.editorNewTitle }}</h3>
      </div>

      <div class="field-grid">
        <div class="field">
          <label for="model-name">{{ messages.name }}</label>
          <input id="model-name" v-model="draft.name" :placeholder="messages.namePlaceholder" />
        </div>

        <div class="field">
          <label for="model-base-url">{{ messages.baseUrl }}</label>
          <input
            id="model-base-url"
            v-model="draft.baseUrl"
            :placeholder="messages.baseUrlPlaceholder"
          />
        </div>

        <div class="field">
          <label for="model-api-key">{{ messages.apiKey }}</label>
          <input
            id="model-api-key"
            v-model="draft.apiKey"
            type="password"
            :placeholder="messages.apiKeyPlaceholder"
            autocomplete="off"
          />
        </div>

        <div class="field">
          <label for="model-id">{{ messages.model }}</label>
          <input id="model-id" v-model="draft.model" :placeholder="messages.modelPlaceholder" />
        </div>

        <div class="field-grid two-col">
          <label class="toggle-field">
            <input v-model="draft.enabled" type="checkbox" />
            <span>{{ messages.enabledSwitch }}</span>
          </label>

          <label class="toggle-field">
            <input v-model="draft.isDefault" type="checkbox" />
            <span>{{ messages.defaultSwitch }}</span>
          </label>
        </div>
      </div>

      <div v-if="errorMessage" class="note-block error-block">
        {{ errorMessage }}
      </div>

      <div class="button-row">
        <button class="primary-button" type="button" @click="save">
          {{ messages.save }}
        </button>
        <button class="secondary-button" type="button" @click="resetDraft">
          {{ messages.reset }}
        </button>
        <button
          v-if="selectedId"
          class="text-button danger-text"
          type="button"
          @click="removeModel"
        >
          {{ commonMessages.delete }}
        </button>
      </div>
    </article>
  </section>
</template>
