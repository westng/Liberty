<script setup lang="ts">
import { confirm } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { computed, onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { useAiStore } from "@/composables/useAiStore";
import type { AiSummaryTemplate } from "@/types/meeting";

const route = useRoute();
const aiStore = useAiStore();

const selectedId = ref<string | null>(null);
const draft = ref<AiSummaryTemplate>(createFreshDraft());
const errorMessage = ref("");

const selectedTemplate = computed(() =>
  selectedId.value ? aiStore.getTemplateById(selectedId.value) ?? null : null,
);

watch(selectedTemplate, (nextTemplate) => {
  if (!nextTemplate) {
    return;
  }

  draft.value = { ...nextTemplate };
});

onMounted(async () => {
  await aiStore.ensureLoaded();
  const templateId = typeof route.query.id === "string" ? route.query.id : null;

  if (templateId) {
    const template = aiStore.getTemplateById(templateId);
    if (template) {
      selectedId.value = template.id;
      draft.value = { ...template };
      await syncWindowTitle(true);
      return;
    }
  }

  selectedId.value = null;
  draft.value = createFreshDraft();
  await syncWindowTitle(false);
});

function createFreshDraft() {
  return aiStore.createTemplate();
}

function validateDraft() {
  if (!draft.value.name.trim()) {
    return "模板名称不能为空。";
  }

  if (!draft.value.prompt.trim()) {
    return "Prompt 不能为空。";
  }

  return "";
}

async function syncWindowTitle(isEdit: boolean) {
  try {
    await getCurrentWindow().setTitle(isEdit ? "编辑模板" : "新增模板");
  } catch {
    // ignore
  }
}

async function save() {
  if (draft.value.builtin) {
    errorMessage.value = "内置模板不可直接修改，请先复制。";
    return;
  }

  const validation = validateDraft();

  if (validation) {
    errorMessage.value = validation;
    return;
  }

  await aiStore.saveTemplate({
    ...draft.value,
    name: draft.value.name.trim(),
    description: draft.value.description.trim(),
    prompt: draft.value.prompt.trim(),
  });

  selectedId.value = draft.value.id;
  draft.value = { ...(aiStore.getTemplateById(draft.value.id) ?? draft.value) };
  errorMessage.value = "";
  await syncWindowTitle(true);
}

async function duplicateTemplate() {
  if (!selectedTemplate.value) {
    return;
  }

  const duplicated = aiStore.duplicateTemplate(selectedTemplate.value.id);

  if (!duplicated) {
    return;
  }

  await aiStore.insertTemplate(duplicated);
  selectedId.value = duplicated.id;
  draft.value = { ...(aiStore.getTemplateById(duplicated.id) ?? duplicated) };
  errorMessage.value = "";
  await syncWindowTitle(true);
}

async function removeTemplate() {
  if (!selectedTemplate.value || selectedTemplate.value.builtin) {
    return;
  }

  const confirmed = await confirm(`确认删除模板“${selectedTemplate.value.name}”吗？`, {
    title: "删除模板",
    kind: "warning",
    okLabel: "删除",
    cancelLabel: "取消",
  });

  if (!confirmed) {
    return;
  }

  await aiStore.deleteTemplate(selectedTemplate.value.id);
  selectedId.value = null;
  draft.value = createFreshDraft();
  errorMessage.value = "";
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
        <h3>{{ selectedId ? "编辑模板" : "新增模板" }}</h3>
        <button
          v-if="selectedTemplate"
          class="secondary-button"
          type="button"
          @click="duplicateTemplate"
        >
          复制模板
        </button>
      </div>

      <div class="field-grid">
        <div class="field-grid two-col">
          <div class="field">
            <label for="template-name">模板名称</label>
            <input id="template-name" v-model="draft.name" :readonly="draft.builtin" />
          </div>

          <div class="field">
            <label for="template-description">模板说明</label>
            <input
              id="template-description"
              v-model="draft.description"
              :readonly="draft.builtin"
            />
          </div>
        </div>

        <div class="field-grid two-col">
          <label class="toggle-field">
            <input
              v-model="draft.includeSpeakerByDefault"
              type="checkbox"
              :disabled="draft.builtin"
            />
            <span>默认包含说话人</span>
          </label>

          <label class="toggle-field">
            <input
              v-model="draft.includeTimestampByDefault"
              type="checkbox"
              :disabled="draft.builtin"
            />
            <span>默认包含时间戳</span>
          </label>
        </div>

        <div class="field">
          <label for="template-prompt">Prompt</label>
          <textarea
            id="template-prompt"
            v-model="draft.prompt"
            :readonly="draft.builtin"
            placeholder="输入结构化 Prompt"
          />
        </div>
      </div>

      <div v-if="errorMessage" class="note-block error-block">
        {{ errorMessage }}
      </div>

      <div class="button-row">
        <button
          class="primary-button"
          type="button"
          :disabled="draft.builtin"
          @click="save"
        >
          保存模板
        </button>
        <button class="secondary-button" type="button" @click="resetDraft">
          新建空模板
        </button>
        <button
          v-if="selectedTemplate && !selectedTemplate.builtin"
          class="text-button danger-text"
          type="button"
          @click="removeTemplate"
        >
          删除模板
        </button>
      </div>
    </article>
  </section>
</template>
