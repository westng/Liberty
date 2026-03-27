<script setup lang="ts">
import { confirm } from "@tauri-apps/plugin-dialog";
import { computed, onBeforeUnmount, onMounted } from "vue";
import { useAiStore } from "@/composables/useAiStore";
import { useMeetingStore } from "@/composables/useMeetingStore";
import { formatMessage, getMessages } from "@/services/i18n";
import { openTemplateEditorWindow } from "@/services/window";
import type { AiSummaryTemplate } from "@/types/meeting";

const aiStore = useAiStore();
const meetingStore = useMeetingStore();
const messages = computed(() => getMessages(meetingStore.settings.value.locale).templates);
const commonMessages = computed(() => getMessages(meetingStore.settings.value.locale).common);

const templates = computed(() =>
  [...aiStore.templates.value].sort((left, right) => {
    if (left.builtin !== right.builtin) {
      return left.builtin ? -1 : 1;
    }

    return right.updatedAt.localeCompare(left.updatedAt);
  }),
);
const builtinCount = computed(() => templates.value.filter((item) => item.builtin).length);
const customCount = computed(() => templates.value.filter((item) => !item.builtin).length);

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
  await openTemplateEditorWindow();
}

async function editTemplate(template: AiSummaryTemplate) {
  await openTemplateEditorWindow(template.id);
}

async function removeTemplate(template: AiSummaryTemplate) {
  if (template.builtin) {
    return;
  }

  const confirmed = await confirm(formatMessage(messages.value.deleteConfirm, { name: template.name }), {
    title: messages.value.deleteTitle,
    kind: "warning",
    okLabel: commonMessages.value.delete,
    cancelLabel: commonMessages.value.cancel,
  });

  if (!confirmed) {
    return;
  }

  await aiStore.deleteTemplate(template.id);
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
        <span>{{ messages.total }} {{ templates.length }}</span>
        <span>{{ messages.builtin }} {{ builtinCount }}</span>
        <span>{{ messages.custom }} {{ customCount }}</span>
      </div>
    </article>

    <div class="section-heading model-management-header">
      <h3>{{ messages.listTitle }}</h3>
    </div>

    <article class="surface model-list-card">
      <div v-if="templates.length" class="model-list-rows">
        <article
          v-for="template in templates"
          :key="template.id"
          class="model-list-row"
          @click="editTemplate(template)"
        >
          <div class="model-row-main">
            <strong>{{ template.name }}</strong>
            <span>{{ template.description || messages.emptyDescription }}</span>
          </div>

          <div class="model-row-side">
            <span class="record-meta">{{ template.builtin ? messages.builtin : messages.custom }}</span>
            <span class="record-meta">{{ formatUpdatedAt(template.updatedAt) }}</span>
            <button class="text-button" type="button" @click.stop="editTemplate(template)">{{ commonMessages.edit }}</button>
            <button
              v-if="!template.builtin"
              class="text-button danger-text"
              type="button"
              @click.stop="removeTemplate(template)"
            >
              {{ commonMessages.delete }}
            </button>
          </div>
        </article>
      </div>

      <div v-else class="empty-state">
        {{ messages.empty }}
      </div>
    </article>
  </section>
</template>
