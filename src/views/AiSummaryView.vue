<script setup lang="ts">
import { confirm } from "@tauri-apps/plugin-dialog";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { computed, onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import MeetingNotesPanel from "@/components/MeetingNotesPanel.vue";
import StatusBadge from "@/components/StatusBadge.vue";
import { useAiStore } from "@/composables/useAiStore";
import { useMeetingStore } from "@/composables/useMeetingStore";
import {
  buildSummaryRun,
  createEmptyMeetingSummary,
  summaryResultToMeetingSummary,
} from "@/services/aiStorage";
import { buildSummaryPromptPreview, generateAiSummary } from "@/services/aiSummary";
import { getPrimaryTranscriptSegments } from "@/services/transcript";
import type { AiSummaryRun, JobStage } from "@/types/meeting";

const route = useRoute();
const aiStore = useAiStore();
const meetingStore = useMeetingStore();

const jobId = computed(() => String(route.query.jobId ?? ""));
const job = computed(() => meetingStore.getJobById(jobId.value));
const enabledModels = computed(() => aiStore.models.value.filter((model) => model.enabled));
const templates = computed(() => aiStore.templates.value);
const latestRuns = computed(() =>
  [...(job.value?.summaryRuns ?? [])].sort((left, right) => right.updatedAt.localeCompare(left.updatedAt)),
);
const latestRun = computed(() => latestRuns.value[0] ?? null);

const selectedModelId = ref("");
const selectedTemplateId = ref("");
const selectedRunId = ref("");
const includeSpeaker = ref(true);
const includeTimestamp = ref(true);
const extraInstructions = ref("");
const submitting = ref(false);
const errorMessage = ref("");

const selectedModel = computed(() => aiStore.getModelById(selectedModelId.value));
const selectedTemplate = computed(() => aiStore.getTemplateById(selectedTemplateId.value));
const selectedRun = computed(() =>
  latestRuns.value.find((run) => run.id === selectedRunId.value) ?? latestRuns.value[0] ?? null,
);
const transcriptCount = computed(() => (job.value ? getPrimaryTranscriptSegments(job.value).length : 0));
const previewSummary = computed(() => {
  if (selectedRun.value?.result) {
    return summaryResultToMeetingSummary(selectedRun.value.result);
  }

  return job.value?.summary || createEmptyMeetingSummary(job.value?.title);
});
const previewStatus = computed<JobStage>(() => {
  if (!selectedRun.value) {
    return summaryDisplayStatus.value;
  }

  if (selectedRun.value.status === "running") {
    return "summarizing";
  }

  if (selectedRun.value.status === "failed") {
    return "failed";
  }

  if (selectedRun.value.result) {
    return "completed";
  }

  return "idle";
});
const selectedRunIsActive = computed(
  () => Boolean(job.value?.activeSummaryRunId && job.value.activeSummaryRunId === selectedRun.value?.id),
);
const canApplySelectedRun = computed(
  () => Boolean(job.value && selectedRun.value?.result && !selectedRunIsActive.value),
);
const hasSummaryContent = computed(() => {
  if (!job.value) {
    return false;
  }

  return Boolean(
    job.value.summary.overview.trim()
      || job.value.summary.topics.length
      || job.value.summary.decisions.length
      || job.value.summary.actionItems.length
      || job.value.summary.risks?.length
      || job.value.summary.followUps?.length,
  );
});
const summaryDisplayStatus = computed<JobStage>(() => {
  if (submitting.value || latestRun.value?.status === "running") {
    return "summarizing";
  }

  if (latestRun.value?.status === "failed" && !hasSummaryContent.value) {
    return "failed";
  }

  if (hasSummaryContent.value || latestRun.value?.status === "completed") {
    return "completed";
  }

  return "idle";
});
const activeSummaryLabel = computed(() => {
  if (!latestRun.value) {
    return "尚未生成";
  }

  if (summaryDisplayStatus.value === "failed") {
    return "最近一次失败";
  }

  if (summaryDisplayStatus.value === "summarizing") {
    return "生成中";
  }

  return "已保存结果";
});

watch(
  [latestRuns, () => job.value?.activeSummaryRunId],
  ([runs, activeRunId]) => {
    if (!runs.length) {
      selectedRunId.value = "";
      return;
    }

    const currentStillExists = runs.some((run) => run.id === selectedRunId.value);

    if (currentStillExists) {
      return;
    }

    const preferredRun = runs.find((run) => run.id === activeRunId) ?? runs[0];
    selectedRunId.value = preferredRun.id;
  },
  { immediate: true },
);

watch(
  templates,
  (nextTemplates) => {
    if (!selectedTemplateId.value) {
      selectedTemplateId.value = nextTemplates[0]?.id ?? "";
    }
  },
  { immediate: true },
);

watch(
  enabledModels,
  (nextModels) => {
    if (!selectedModelId.value) {
      selectedModelId.value = (aiStore.getDefaultModel() ?? nextModels[0])?.id ?? "";
    }
  },
  { immediate: true },
);

watch(
  selectedTemplate,
  (template) => {
    if (!template) {
      return;
    }

    includeSpeaker.value = template.includeSpeakerByDefault;
    includeTimestamp.value = template.includeTimestampByDefault;
  },
  { immediate: true },
);

onMounted(() => {
  void (async () => {
    await aiStore.ensureLoaded();
    await meetingStore.refreshJobs();
    await reconcileStaleRuns();
  })();
});

async function reconcileStaleRuns() {
  const now = Date.now();
  const staleRuns = latestRuns.value.filter((run) => {
    if (run.status !== "running") {
      return false;
    }

    return now - new Date(run.updatedAt).getTime() > 60_000;
  });

  for (const run of staleRuns) {
    await meetingStore.saveSummaryRun({
      ...run,
      status: "failed",
      errorMessage: "上一次 AI 总结未完成，可能因网络、代理或窗口中断导致失败。",
      updatedAt: new Date().toISOString(),
    });
  }
}

async function submit() {
  if (!job.value) {
    errorMessage.value = "没有找到当前任务。";
    return;
  }

  if (!selectedModel.value) {
    errorMessage.value = "请先在主窗口中配置可用模型。";
    return;
  }

  if (!selectedTemplate.value) {
    errorMessage.value = "请先在主窗口中配置可用模板。";
    return;
  }

  const transcriptSegments = getPrimaryTranscriptSegments(job.value);

  if (!transcriptSegments.length) {
    errorMessage.value = "当前任务还没有可用于总结的逐字稿内容。";
    return;
  }

  errorMessage.value = "";
  submitting.value = true;
  const promptPreview = buildSummaryPromptPreview({
    job: job.value,
    template: selectedTemplate.value,
    includeSpeaker: includeSpeaker.value,
    includeTimestamp: includeTimestamp.value,
    extraInstructions: extraInstructions.value.trim(),
  });

  const pendingRun = buildSummaryRun({
    jobId: job.value.id,
    modelConfigId: selectedModel.value.id,
    templateId: selectedTemplate.value.id,
    includeSpeaker: includeSpeaker.value,
    includeTimestamp: includeTimestamp.value,
    extraInstructions: extraInstructions.value.trim(),
    status: "running",
    promptPreview: `${promptPreview.system}\n\n---\n\n${promptPreview.user}`,
    result: undefined,
  });

  await meetingStore.saveSummaryRun(pendingRun);

  try {
    const response = await generateAiSummary({
      job: job.value,
      model: selectedModel.value,
      template: selectedTemplate.value,
      includeSpeaker: includeSpeaker.value,
      includeTimestamp: includeTimestamp.value,
      extraInstructions: extraInstructions.value.trim(),
    });

    await meetingStore.saveSummaryRun({
      ...pendingRun,
      status: "completed",
      promptPreview: response.promptPreview,
      rawResponse: response.rawResponse,
      result: response.result,
    });
    await meetingStore.setActiveSummaryRun(pendingRun.jobId, pendingRun.id);
    selectedRunId.value = pendingRun.id;
  } catch (error) {
    const message = error instanceof Error ? error.message : "AI 总结失败。";
    errorMessage.value = message;
    await meetingStore.saveSummaryRun({
      ...pendingRun,
      status: "failed",
      errorMessage: message,
    });
  } finally {
    submitting.value = false;
  }
}

async function closeWindow() {
  await getCurrentWebviewWindow().close();
}

async function applySelectedRun() {
  if (!job.value || !selectedRun.value?.result) {
    return;
  }

  await meetingStore.setActiveSummaryRun(job.value.id, selectedRun.value.id);
}

async function removeRun(run: AiSummaryRun) {
  if (!job.value) {
    return;
  }

  const confirmed = await confirm("删除后无法恢复，确认删除这条 AI 总结记录吗？", {
    title: "删除总结记录",
    kind: "warning",
    okLabel: "删除",
    cancelLabel: "取消",
  });

  if (!confirmed) {
    return;
  }

  await meetingStore.deleteSummaryRun(job.value.id, run.id);

  if (selectedRunId.value === run.id) {
    selectedRunId.value = "";
  }
}

function formatCreatedAt(value: string) {
  return new Date(value).toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}
</script>

<template>
  <section class="summary-window-shell ai-summary-window">
    <article class="surface summary-window-hero">
      <div class="job-title-line">
        <div>
          <h3>{{ job?.title || "AI 总结" }}</h3>
          <p class="section-copy">
            当前逐字稿段落 {{ transcriptCount }} 条，模型与模板来自主窗口中的资源管理页。
          </p>
        </div>
        <div class="button-row">
          <StatusBadge :status="summaryDisplayStatus" />
          <button class="secondary-button" type="button" @click="closeWindow">
            关闭窗口
          </button>
        </div>
      </div>

      <div class="summary-inline">
        <span>逐字稿 {{ transcriptCount }} 条</span>
        <span>当前状态 {{ activeSummaryLabel }}</span>
        <span>输入文件 {{ job?.sourceFiles.length || 0 }} 个</span>
        <span>{{ job?.sourceFiles.map((file) => file.name).join(" · ") || "未找到任务" }}</span>
      </div>
    </article>

    <div class="summary-window-layout ai-summary-layout">
      <aside class="summary-window-side">
        <article class="surface">
          <div class="section-heading summary-centered-heading">
            <h3>本次配置</h3>
            <StatusBadge :status="summaryDisplayStatus" />
          </div>

          <div class="field-grid">
            <div class="field-grid two-col">
              <div class="field">
                <label for="summary-model">模型</label>
                <select id="summary-model" v-model="selectedModelId">
                  <option disabled value="">
                    请选择模型
                  </option>
                  <option v-for="model in enabledModels" :key="model.id" :value="model.id">
                    {{ model.name }} · {{ model.model }}
                  </option>
                </select>
              </div>

              <div class="field">
                <label for="summary-template">模板</label>
                <select id="summary-template" v-model="selectedTemplateId">
                  <option disabled value="">
                    请选择模板
                  </option>
                  <option v-for="template in templates" :key="template.id" :value="template.id">
                    {{ template.name }}
                  </option>
                </select>
              </div>
            </div>

            <div class="field-grid two-col">
              <label class="toggle-field">
                <input v-model="includeSpeaker" type="checkbox" />
                <span>包含说话人</span>
              </label>

              <label class="toggle-field">
                <input v-model="includeTimestamp" type="checkbox" />
                <span>包含时间戳</span>
              </label>
            </div>

            <div class="field">
              <label for="summary-extra">补充要求</label>
              <textarea
                id="summary-extra"
                v-model="extraInstructions"
                placeholder="例如：重点关注风险项和负责人，输出适合直接发飞书。"
              />
            </div>
          </div>

          <div v-if="errorMessage" class="note-block error-block">
            {{ errorMessage }}
          </div>

          <div class="button-row summary-submit-row">
            <button class="primary-button" type="button" :disabled="submitting" @click="submit">
              {{ submitting ? "生成中..." : "生成总结" }}
            </button>
          </div>
        </article>

        <article class="surface summary-history-panel">
          <div class="section-heading">
            <h3>生成记录</h3>
          </div>

          <div v-if="latestRuns.length" class="record-list">
            <button
              v-for="run in latestRuns"
              :key="run.id"
              class="record-item"
              :class="{ active: selectedRun?.id === run.id }"
              type="button"
              @click="selectedRunId = run.id"
            >
              <div class="record-item-head">
                <div class="record-title-stack">
                  <strong>{{ aiStore.getTemplateById(run.templateId)?.name || "导入/未知模板" }}</strong>
                  <div class="record-tags">
                    <span v-if="job?.activeSummaryRunId === run.id" class="record-tag active">当前结果</span>
                    <span v-else-if="latestRun?.id === run.id" class="record-tag">最新</span>
                  </div>
                </div>
                <StatusBadge
                  :status="
                    run.status === 'running'
                      ? 'summarizing'
                      : run.status === 'completed'
                        ? 'completed'
                        : 'failed'
                  "
                />
              </div>
              <div class="record-item-body">
                <span>{{ aiStore.getModelById(run.modelConfigId)?.name || "未知模型" }}</span>
                <span>{{ formatCreatedAt(run.createdAt) }}</span>
              </div>
              <div v-if="run.result?.overview" class="record-item-copy">
                {{ run.result.overview }}
              </div>
              <div v-if="run.errorMessage" class="record-item-copy danger-text">
                {{ run.errorMessage }}
              </div>
            </button>
          </div>

          <div v-else class="empty-state">
            还没有生成过 AI 总结。
          </div>
        </article>
      </aside>

      <div class="summary-window-main">

        <article class="surface summary-window-result ai-summary-result">
          <div class="section-heading summary-centered-heading">
            <h3>结果预览</h3>
            <StatusBadge :status="previewStatus" />
          </div>

          <div v-if="selectedRun" class="summary-preview-toolbar">
            <div class="record-meta summary-preview-meta">
              <span>{{ aiStore.getTemplateById(selectedRun.templateId)?.name || "导入/未知模板" }}</span>
              <span>{{ aiStore.getModelById(selectedRun.modelConfigId)?.name || "未知模型" }}</span>
              <span>{{ formatCreatedAt(selectedRun.createdAt) }}</span>
              <span v-if="selectedRunIsActive">当前结果</span>
            </div>
            <div class="button-row">
              <button
                class="secondary-button"
                type="button"
                :disabled="!canApplySelectedRun"
                @click="applySelectedRun"
              >
                {{ selectedRunIsActive ? "当前采用中" : "设为当前结果" }}
              </button>
              <button
                class="secondary-button jobs-delete-button"
                type="button"
                @click="removeRun(selectedRun)"
              >
                删除本次
              </button>
            </div>
          </div>

          <MeetingNotesPanel :summary="previewSummary" />
        </article>
      </div>
    </div>
  </section>
</template>
