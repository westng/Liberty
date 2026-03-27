<script setup lang="ts">
import { confirm } from "@tauri-apps/plugin-dialog";
import { computed, ref } from "vue";
import { useRoute } from "vue-router";
import StatusBadge from "@/components/StatusBadge.vue";
import { useAiStore } from "@/composables/useAiStore";
import TranscriptTimeline from "@/components/TranscriptTimeline.vue";
import { useMeetingStore } from "@/composables/useMeetingStore";
import { exportJob } from "@/services/export";
import { formatMessage, getMessages } from "@/services/i18n";
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
const messages = computed(() => getMessages(store.settings.value.locale).workbench);
const commonMessages = computed(() => getMessages(store.settings.value.locale).common);
const transcriptSegments = computed(() =>
  job.value ? getPrimaryTranscriptSegments(job.value) : [],
);

function normalizeSpeakerLabel(value?: string) {
  return value?.trim() || commonMessages.value.unknownSpeaker;
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
      label: messages.value.allSpeakers,
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

  const sourceLabel = fromSpeaker.trim() || commonMessages.value.unknownSpeaker;
  const targetLabel = toSpeaker.trim();

  if (!targetLabel) {
    return;
  }

  const confirmed = await confirm(
    formatMessage(messages.value.renameSpeakerConfirm, {
      source: sourceLabel,
      target: targetLabel,
    }),
    {
      title: messages.value.renameSpeakerTitle,
      kind: "warning",
      okLabel: messages.value.replace,
      cancelLabel: commonMessages.value.cancel,
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
              {{ messages.heroCopy }}
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
            {{ messages.aiSummary }}
          </button>
          <button class="secondary-button" type="button" @click="openNotes">
            {{ messages.viewNotes }}
          </button>
          <button class="primary-button" type="button" @click="doExport('bundle')">
            {{ isExporting ? messages.exporting : messages.exportBundle }}
          </button>
          <button class="secondary-button" type="button" @click="doExport('transcript')">
            {{ messages.exportTranscript }}
          </button>
          <button class="secondary-button" type="button" @click="doExport('notes')">
            {{ messages.exportNotes }}
          </button>
        </div>

        <div class="summary-inline">
          <span>{{ formatMessage(messages.transcriptCount, { count: transcriptSegments.length }) }}</span>
          <span>{{ formatMessage(messages.summaryCount, { count: job.summaryRuns.length }) }}</span>
          <span>{{ formatMessage(messages.currentTemplate, { name: activeTemplateName || messages.notGenerated }) }}</span>
          <span>{{ formatMessage(messages.fileCount, { count: job.sourceFiles.length }) }}</span>
          <span>{{ formatMessage(messages.durationMinutes, { count: job.durationMinutes }) }}</span>
          <span>{{ formatMessage(messages.hotwords, { value: job.hotwords.join("、") || messages.notConfigured }) }}</span>
          <span>{{ job.summary.overview ? messages.notesReady : messages.notesEmpty }}</span>
          <span>{{ activeSummaryRun ? messages.activeResultReady : messages.summaryEmpty }}</span>
        </div>
      </article>

      <article class="surface transcript-column full-span">
        <div class="section-heading workbench-transcript-heading">
          <h3>{{ messages.transcript }}</h3>
          <div class="field workbench-search-field">
            <input
              id="transcript-search"
              v-model="query"
              :placeholder="messages.searchPlaceholder"
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
      {{ messages.notFound }}
    </div>
  </section>
</template>
