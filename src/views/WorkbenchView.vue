<script setup lang="ts">
import { confirm } from "@tauri-apps/plugin-dialog";
import { computed, ref } from "vue";
import { useRoute } from "vue-router";
import StatusBadge from "@/components/StatusBadge.vue";
import { useAiStore } from "@/composables/useAiStore";
import TranscriptTimeline from "@/components/TranscriptTimeline.vue";
import { useMeetingStore } from "@/composables/useMeetingStore";
import { exportJob } from "@/services/export";
import { getPrimaryTranscriptSegments } from "@/services/transcript";
import { openAiSummaryWindow, openMeetingNotesWindow } from "@/services/window";

const route = useRoute();
const store = useMeetingStore();
const aiStore = useAiStore();
const ALL_SPEAKERS = "__all__";

const job = computed(() => store.getJobById(route.params.id as string));
const query = ref("");
const selectedSpeaker = ref(ALL_SPEAKERS);
const isExporting = ref(false);
const isRenamingSpeaker = ref(false);
const transcriptSegments = computed(() =>
  job.value ? getPrimaryTranscriptSegments(job.value) : [],
);

function normalizeSpeakerLabel(value?: string) {
  return value?.trim() || "未知说话人";
}

const speakerOptions = computed(() => {
  const counts = new Map<string, number>();

  for (const segment of transcriptSegments.value) {
    const label = normalizeSpeakerLabel(segment.speaker);
    counts.set(label, (counts.get(label) ?? 0) + 1);
  }

  return [
    {
      key: ALL_SPEAKERS,
      label: "全部",
      count: transcriptSegments.value.length,
    },
    ...Array.from(counts.entries()).map(([label, count]) => ({
      key: label,
      label,
      count,
    })),
  ];
});
const speakerFilteredSegments = computed(() => {
  if (selectedSpeaker.value === ALL_SPEAKERS) {
    return transcriptSegments.value;
  }

  return transcriptSegments.value.filter(
    (segment) => normalizeSpeakerLabel(segment.speaker) === selectedSpeaker.value,
  );
});
const activeSummaryRun = computed(() =>
  job.value?.summaryRuns.find((run) => run.id === job.value?.activeSummaryRunId),
);
const activeTemplateName = computed(() =>
  activeSummaryRun.value ? aiStore.getTemplateById(activeSummaryRun.value.templateId)?.name : "",
);

async function doExport(kind: "transcript" | "notes" | "bundle") {
  if (!job.value) {
    return;
  }

  isExporting.value = true;

  try {
    const done = await exportJob(job.value, kind);

    if (done) {
      job.value.lastExportedAt = new Date().toISOString();
    }
  } finally {
    isExporting.value = false;
  }
}

async function launchAiSummary() {
  if (!job.value) {
    return;
  }

  await openAiSummaryWindow(job.value.id, job.value.title);
}

async function openNotes() {
  if (!job.value) {
    return;
  }

  await openMeetingNotesWindow(job.value.id, job.value.title);
}

async function renameSpeaker(fromSpeaker: string, toSpeaker: string) {
  if (!job.value) {
    return;
  }

  const sourceLabel = fromSpeaker.trim() || "未知说话人";
  const targetLabel = toSpeaker.trim();

  if (!targetLabel) {
    return;
  }

  const confirmed = await confirm(
    `确认将当前任务中的“${sourceLabel}”统一替换为“${targetLabel}”吗？`,
    {
      title: "编辑讲话人",
      kind: "warning",
      okLabel: "替换",
      cancelLabel: "取消",
    },
  );

  if (!confirmed) {
    return;
  }

  isRenamingSpeaker.value = true;

  try {
    await store.renameSpeaker(job.value.id, fromSpeaker, targetLabel);
    if (selectedSpeaker.value === sourceLabel) {
      selectedSpeaker.value = targetLabel;
    }
  } finally {
    isRenamingSpeaker.value = false;
  }
}
</script>

<template>
  <section class="view-stack">
    <div v-if="job" class="workbench-grid">
      <article class="surface full-span workbench-hero">
        <div class="job-title-line workbench-hero-head">
          <div>
            <h3>{{ job.title }}</h3>
            <p class="section-copy">
              当前聚焦逐字稿与 AI 总结，导出和任务上下文收拢到右侧区域。
            </p>
          </div>
          <StatusBadge :status="job.overallStatus" />
        </div>

        <div class="workbench-hero-actions">
          <button
            class="primary-button"
            type="button"
            :disabled="!transcriptSegments.length"
            @click="launchAiSummary"
          >
            AI 总结
          </button>
          <button class="secondary-button" type="button" @click="openNotes">
            查看会议纪要
          </button>
          <button class="primary-button" type="button" @click="doExport('bundle')">
            {{ isExporting ? "导出中..." : "导出完整结果" }}
          </button>
          <button class="secondary-button" type="button" @click="doExport('transcript')">
            导出逐字稿
          </button>
          <button class="secondary-button" type="button" @click="doExport('notes')">
            导出纪要
          </button>
        </div>

        <div class="summary-inline">
          <span>逐字稿 {{ transcriptSegments.length }} 段</span>
          <span>AI 总结 {{ job.summaryRuns.length }} 次</span>
          <span>当前模板 {{ activeTemplateName || "尚未生成" }}</span>
          <span>文件 {{ job.sourceFiles.length }} 个</span>
          <span>时长 {{ job.durationMinutes }} 分钟</span>
          <span>热词 {{ job.hotwords.join("、") || "未配置" }}</span>
          <span>{{ job.summary.overview ? "会议纪要已生成" : "暂无会议纪要" }}</span>
          <span>{{ activeSummaryRun ? "当前已选结果可导出" : "还没有生成 AI 总结" }}</span>
        </div>
      </article>

      <article class="surface transcript-column full-span">
        <div class="section-heading workbench-transcript-heading">
          <h3>逐字稿</h3>
          <div class="field workbench-search-field">
            <input
              id="transcript-search"
              v-model="query"
              placeholder="搜索说话人或正文"
            />
          </div>
        </div>
        <div class="speaker-filter-row">
          <button
            v-for="speaker in speakerOptions"
            :key="speaker.key"
            class="speaker-filter-chip"
            :class="{ active: selectedSpeaker === speaker.key }"
            type="button"
            @click="selectedSpeaker = speaker.key"
          >
            <span>{{ speaker.label }}</span>
            <strong>{{ speaker.count }}</strong>
          </button>
        </div>
        <TranscriptTimeline
          :segments="speakerFilteredSegments"
          :query="query"
          :busy="isRenamingSpeaker"
          @rename-speaker="renameSpeaker"
        />
      </article>
    </div>

    <div v-else class="empty-state">
      没有找到这个任务结果。
    </div>
  </section>
</template>
