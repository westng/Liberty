<script setup lang="ts">
import { confirm } from "@tauri-apps/plugin-dialog";
import { computed, onBeforeUnmount, onMounted } from "vue";
import { useAiStore } from "@/composables/useAiStore";
import { openModelEditorWindow } from "@/services/window";
import type { AiModelConfig } from "@/types/meeting";

const aiStore = useAiStore();

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
  const confirmed = await confirm(`确认删除模型“${model.name}”吗？`, {
    title: "删除模型",
    kind: "warning",
    okLabel: "删除",
    cancelLabel: "取消",
  });
  if (!confirmed) {
    return;
  }

  await aiStore.deleteModel(model.id);
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
          <h3>模型管理</h3>
          <p class="section-copy">新增与编辑都在独立窗口中完成。</p>
        </div>
        <button class="primary-button" type="button" @click="startCreate">
          新增模型
        </button>
      </div>
      <div class="summary-inline">
        <span>全部模型 {{ models.length }}</span>
        <span>可用 {{ enabledCount }}</span>
        <span>默认 {{ defaultModel?.name ?? "未设置" }}</span>
      </div>
    </article>

    <div class="section-heading model-management-header">
      <h3>模型列表</h3>
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
            <span class="record-meta">{{ model.isDefault ? "默认" : model.enabled ? "启用" : "停用" }}</span>
            <button class="text-button" type="button" @click.stop="editModel(model)">编辑</button>
            <button class="text-button danger-text" type="button" @click.stop="removeModel(model)">删除</button>
          </div>
        </article>
      </div>

      <div v-else class="empty-state">
        还没有配置模型。
      </div>
    </article>
  </section>
</template>
