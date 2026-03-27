<script setup lang="ts">
import { confirm } from "@tauri-apps/plugin-dialog";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { computed, onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import MeetingNotesPanel from "@/components/MeetingNotesPanel.vue";
import StatusBadge from "@/components/StatusBadge.vue";
import { useAiStore } from "@/composables/useAiStore";
import { useMeetingStore } from "@/composables/useMeetingStore";
import { formatMessage, getMessages } from "@/services/i18n";
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
const messages = computed(() => getMessages(meetingStore.settings.value.locale).aiSummary);
const commonMessages = computed(() => getMessages(meetingStore.settings.value.locale).common);

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
    return messages.value.activeLabelEmpty;
  }

  if (summaryDisplayStatus.value === "failed") {
    return messages.value.activeLabelFailed;
  }

  if (summaryDisplayStatus.value === "summarizing") {
    return messages.value.activeLabelRunning;
  }

  return messages.value.activeLabelSaved;
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
      errorMessage: messages.value.staleRunError,
      updatedAt: new Date().toISOString(),
    });
  }
}

async function submit() {
  if (!job.value) {
    errorMessage.value = messages.value.jobNotFound;
    return;
  }

  if (!selectedModel.value) {
    errorMessage.value = messages.value.modelMissing;
    return;
  }

  if (!selectedTemplate.value) {
    errorMessage.value = messages.value.templateMissing;
    return;
  }

  const transcriptSegments = getPrimaryTranscriptSegments(job.value);

  if (!transcriptSegments.length) {
    errorMessage.value = messages.value.transcriptMissing;
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
    const message = error instanceof Error ? error.message : messages.value.requestFailed;
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

  const confirmed = await confirm(messages.value.deleteConfirm, {
    title: messages.value.deleteTitle,
    kind: "warning",
    okLabel: commonMessages.value.delete,
    cancelLabel: commonMessages.value.cancel,
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
  return new Date(value).toLocaleString(meetingStore.settings.value.locale, {
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
          <h3>{{ job?.title || messages.heroTitle }}</h3>
          <p class="section-copy">
            {{ formatMessage(messages.heroCopy, { count: transcriptCount }) }}
          </p>
        </div>
        <div class="button-row">
          <StatusBadge :status="summaryDisplayStatus" />
          <button class="secondary-button" type="button" @click="closeWindow">
            {{ commonMessages.closeWindow }}
          </button>
        </div>
      </div>

      <div class="summary-inline">
        <span>{{ formatMessage(messages.transcriptCount, { count: transcriptCount }) }}</span>
        <span>{{ formatMessage(messages.currentStatus, { status: activeSummaryLabel }) }}</span>
        <span>{{ formatMessage(messages.inputFiles, { count: job?.sourceFiles.length || 0 }) }}</span>
        <span>{{ job?.sourceFiles.map((file) => file.name).join(" · ") || messages.jobMissing }}</span>
      </div>
    </article>

    <div class="summary-window-layout ai-summary-layout">
      <aside class="summary-window-side">
        <article class="surface">
          <div class="section-heading summary-centered-heading">
            <h3>{{ messages.currentConfig }}</h3>
            <StatusBadge :status="summaryDisplayStatus" />
          </div>

          <div class="field-grid">
            <div class="field-grid two-col">
              <div class="field">
                <label for="summary-model">{{ messages.model }}</label>
                <select id="summary-model" v-model="selectedModelId">
                  <option disabled value="">
                    {{ messages.chooseModel }}
                  </option>
                  <option v-for="model in enabledModels" :key="model.id" :value="model.id">
                    {{ model.name }} · {{ model.model }}
                  </option>
                </select>
              </div>

              <div class="field">
                <label for="summary-template">{{ messages.template }}</label>
                <select id="summary-template" v-model="selectedTemplateId">
                  <option disabled value="">
                    {{ messages.chooseTemplate }}
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
                <span>{{ messages.includeSpeaker }}</span>
              </label>

              <label class="toggle-field">
                <input v-model="includeTimestamp" type="checkbox" />
                <span>{{ messages.includeTimestamp }}</span>
              </label>
            </div>

            <div class="field">
              <label for="summary-extra">{{ messages.extraInstructions }}</label>
              <textarea
                id="summary-extra"
                v-model="extraInstructions"
                :placeholder="messages.extraInstructionsPlaceholder"
              />
            </div>
          </div>

          <div v-if="errorMessage" class="note-block error-block">
            {{ errorMessage }}
          </div>

          <div class="button-row summary-submit-row">
            <button class="primary-button" type="button" :disabled="submitting" @click="submit">
              {{ submitting ? messages.submitting : messages.submit }}
            </button>
          </div>
        </article>

        <article class="surface summary-history-panel">
          <div class="section-heading">
            <h3>{{ messages.history }}</h3>
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
                  <strong>{{ aiStore.getTemplateById(run.templateId)?.name || messages.unknownTemplate }}</strong>
                  <div class="record-tags">
                    <span v-if="job?.activeSummaryRunId === run.id" class="record-tag active">{{ messages.currentResult }}</span>
                    <span v-else-if="latestRun?.id === run.id" class="record-tag">{{ messages.latest }}</span>
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
                <span>{{ aiStore.getModelById(run.modelConfigId)?.name || messages.unknownModel }}</span>
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
            {{ messages.emptyRuns }}
          </div>
        </article>
      </aside>

      <div class="summary-window-main">

        <article class="surface summary-window-result ai-summary-result">
          <div class="section-heading summary-centered-heading">
            <h3>{{ messages.preview }}</h3>
            <StatusBadge :status="previewStatus" />
          </div>

          <div v-if="selectedRun" class="summary-preview-toolbar">
            <div class="record-meta summary-preview-meta">
              <span>{{ aiStore.getTemplateById(selectedRun.templateId)?.name || messages.unknownTemplate }}</span>
              <span>{{ aiStore.getModelById(selectedRun.modelConfigId)?.name || messages.unknownModel }}</span>
              <span>{{ formatCreatedAt(selectedRun.createdAt) }}</span>
              <span v-if="selectedRunIsActive">{{ messages.currentResult }}</span>
            </div>
            <div class="button-row">
              <button
                class="secondary-button"
                type="button"
                :disabled="!canApplySelectedRun"
                @click="applySelectedRun"
              >
                {{ selectedRunIsActive ? messages.usingCurrent : messages.setCurrent }}
              </button>
              <button
                class="secondary-button jobs-delete-button"
                type="button"
                @click="removeRun(selectedRun)"
              >
                {{ messages.deleteCurrent }}
              </button>
            </div>
          </div>

          <MeetingNotesPanel :summary="previewSummary" />
        </article>
      </div>
    </div>
  </section>
</template>
