<script setup lang="ts">
import { confirm } from "@tauri-apps/plugin-dialog";
import { computed, onBeforeUnmount, onMounted } from "vue";
import { useAiStore } from "@/composables/useAiStore";
import { useMeetingStore } from "@/composables/useMeetingStore";
import { formatMessage, getMessages } from "@/services/i18n";
import { openModelEditorWindow } from "@/services/window";
import type { AiModelConfig } from "@/types/meeting";

const aiStore = useAiStore();
const meetingStore = useMeetingStore();
const messages = computed(() => getMessages(meetingStore.settings.value.locale).models);
const commonMessages = computed(() => getMessages(meetingStore.settings.value.locale).common);

const models = computed(() =>
  [...aiStore.models.value].sort((left, right) => right.updatedAt.localeCompare(left.updatedAt)),
);
const defaultModel = computed(
  () => models.value.find((model) => model.isDefault && model.enabled) ?? models.value.find((model) => model.isDefault) ?? null,
);
const enabledCount = computed(() => models.value.filter((model) => model.enabled).length);

onMounted(async () => {
  await aiStore.ensureLoaded();
  window.addEventListener("focus", handleWindowFocus);
});

onBeforeUnmount(() => {
  window.removeEventListener("focus", handleWindowFocus);
});

function handleWindowFocus() {
  void aiStore.reloadState();
}

async function startCreate() {
  await openModelEditorWindow();
}

async function editModel(model: AiModelConfig) {
  await openModelEditorWindow(model.id);
}

async function removeModel(model: AiModelConfig) {
  const confirmed = await confirm(formatMessage(messages.value.deleteConfirm, { name: model.name }), {
    title: messages.value.deleteTitle,
    kind: "warning",
    okLabel: commonMessages.value.delete,
    cancelLabel: commonMessages.value.cancel,
  });
  if (!confirmed) {
    return;
  }

  await aiStore.deleteModel(model.id);
}

function formatUpdatedAt(value: string) {
  return new Date(value).toLocaleString(meetingStore.settings.value.locale, {
    year: "2-digit",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}
</script>

<template>
  <section class="view-stack model-page-stack" style="margin-top: 20px;">
    <article class="surface">
      <div class="section-heading">
        <div>
          <h3>{{ messages.title }}</h3>
          <p class="section-copy">{{ messages.copy }}</p>
        </div>
        <button class="primary-button" type="button" @click="startCreate">
          {{ messages.add }}
        </button>
      </div>
      <div class="summary-inline">
        <span>{{ messages.total }} {{ models.length }}</span>
        <span>{{ messages.enabled }} {{ enabledCount }}</span>
        <span>{{ messages.defaultLabel }} {{ defaultModel?.name ?? commonMessages.notSet }}</span>
      </div>
    </article>

    <div class="section-heading model-management-header">
      <h3>{{ messages.listTitle }}</h3>
    </div>

    <article class="surface model-list-card">
      <div v-if="models.length" class="model-list-rows">
        <article
          v-for="model in models"
          :key="model.id"
          class="model-list-row"
          @click="editModel(model)"
        >
          <div class="model-row-main">
            <strong>{{ model.name }}</strong>
            <span>{{ model.model }}</span>
          </div>

          <div class="model-row-side">
            <span class="record-meta">{{ formatUpdatedAt(model.updatedAt) }}</span>
            <span class="record-meta">{{ model.isDefault ? messages.defaultTag : model.enabled ? messages.enabledTag : messages.disabledTag }}</span>
            <button class="text-button" type="button" @click.stop="editModel(model)">{{ commonMessages.edit }}</button>
            <button class="text-button danger-text" type="button" @click.stop="removeModel(model)">{{ commonMessages.delete }}</button>
          </div>
        </article>
      </div>

      <div v-else class="empty-state">
        {{ messages.empty }}
      </div>
    </article>
  </section>
</template>
