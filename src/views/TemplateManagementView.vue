<script setup lang="ts">
import { confirm } from "@tauri-apps/plugin-dialog";
import { computed, onBeforeUnmount, onMounted } from "vue";
import { useAiStore } from "@/composables/useAiStore";
import { openTemplateEditorWindow } from "@/services/window";
import type { AiSummaryTemplate } from "@/types/meeting";

const aiStore = useAiStore();

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

  const confirmed = await confirm(`确认删除模板“${template.name}”吗？`, {
    title: "删除模板",
    kind: "warning",
    okLabel: "删除",
    cancelLabel: "取消",
  });

  if (!confirmed) {
    return;
  }

  await aiStore.deleteTemplate(template.id);
}

function formatUpdatedAt(value: string) {
  return new Date(value).toLocaleString("zh-CN", {
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
          <h3>总结模板</h3>
          <p class="section-copy">内置 Prompt 负责结构稳定，自定义模板统一在独立窗口中编辑。</p>
        </div>
        <button class="primary-button" type="button" @click="startCreate">
          新增模板
        </button>
      </div>
      <div class="summary-inline">
        <span>全部模板 {{ templates.length }}</span>
        <span>内置 {{ builtinCount }}</span>
        <span>自定义 {{ customCount }}</span>
      </div>
    </article>

    <div class="section-heading model-management-header">
      <h3>模板列表</h3>
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
            <span>{{ template.description || "未填写说明" }}</span>
          </div>

          <div class="model-row-side">
            <span class="record-meta">{{ template.builtin ? "内置" : "自定义" }}</span>
            <span class="record-meta">{{ formatUpdatedAt(template.updatedAt) }}</span>
            <button class="text-button" type="button" @click.stop="editTemplate(template)">编辑</button>
            <button
              v-if="!template.builtin"
              class="text-button danger-text"
              type="button"
              @click.stop="removeTemplate(template)"
            >
              删除
            </button>
          </div>
        </article>
      </div>

      <div v-else class="empty-state">
        还没有可用模板。
      </div>
    </article>
  </section>
</template>
