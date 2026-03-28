<script setup lang="ts">
import { computed } from "vue";
import { RouterLink, useRoute } from "vue-router";
import StatusBadge from "@/components/StatusBadge.vue";
import { useMeetingStore } from "@/composables/useMeetingStore";
import { getMessages } from "@/services/i18n";
import progressBarUrl from "@/assets/progress-bar.webp";

const route = useRoute();
const store = useMeetingStore();
const messages = computed(() => getMessages(store.settings.value.locale).jobDetail);
const commonMessages = computed(() => getMessages(store.settings.value.locale).common);
const statusMessages = computed(() => getMessages(store.settings.value.locale).status);

const job = computed(() => store.getJobById(route.params.id as string));

const ansiPattern = /\u001b\[[0-9;]*m/g;

const stages = computed(() => {
  if (!job.value) {
    return [];
  }

  return [
    { label: messages.value.stageUploaded, status: job.value.uploadStatus },
    { label: messages.value.stageAsr, status: job.value.asrStatus },
    { label: messages.value.stageSummary, status: job.value.summaryStatus },
    { label: messages.value.stageOverall, status: job.value.overallStatus },
  ];
});

const progressPercent = computed(() => {
  if (!job.value) {
    return 0;
  }

  if (job.value.overallStatus === "completed" || job.value.asrStatus === "completed") {
    return 100;
  }

  if (typeof job.value.progressPercent === "number") {
    return Math.max(0, Math.min(100, Math.round(job.value.progressPercent)));
  }

  if (job.value.overallStatus === "speaker_processing" || job.value.asrStatus === "speaker_processing") {
    return 94;
  }

  if (job.value.overallStatus === "transcribing" || job.value.asrStatus === "transcribing") {
    return 32;
  }

  if (job.value.overallStatus === "queued" || job.value.asrStatus === "queued") {
    return 0;
  }

  return 0;
});

const progressMessage = computed(() => {
  if (!job.value) {
    return "";
  }

  const explicit = job.value.progressMessage?.trim();
  if (explicit) {
    return explicit;
  }

  return (
    statusMessages.value[job.value.asrStatus as keyof typeof statusMessages.value] ?? job.value.asrStatus
  );
});

const logEntries = computed(() => {
  const raw = job.value?.processLog ?? "";
  return raw
    .split(/[\r\n]+/)
    .map((line) => line.replace(ansiPattern, "").trim())
    .filter((line) => line.length > 0)
    .reverse()
    .map((line, index) => ({
      id: `${index}-${line.slice(0, 24)}`,
      text: line,
      tone: classifyLogLine(line),
    }));
});

function classifyLogLine(line: string) {
  const normalized = line.toLowerCase();
  if (
    normalized.includes("traceback")
    || normalized.includes("permissionerror")
    || normalized.includes("runtimeerror")
    || normalized.includes("error")
    || normalized.includes("failed")
    || normalized.includes("失败")
  ) {
    return "error";
  }

  if (normalized.includes("warning") || normalized.includes("warn")) {
    return "warning";
  }

  if (
    normalized.includes("completed")
    || normalized.includes("success")
    || normalized.includes("已完成")
    || normalized.includes("完成")
  ) {
    return "success";
  }

  return "info";
}
</script>

<template>
  <section class="view-stack">
    <div v-if="job" class="detail-grid">
      <article class="surface detail-hero full-span">
        <div class="job-title-line detail-hero-head">
          <div>
            <h3>{{ job.title }}</h3>
            <p class="section-copy">
              {{ job.sourceFiles.map((file) => file.name).join(" · ") }}
            </p>
          </div>
          <StatusBadge :status="job.overallStatus" />
        </div>

        <div class="detail-stage-grid">
          <div v-for="stage in stages" :key="stage.label" class="detail-stage-card">
            <span class="detail-stage-label">{{ stage.label }}</span>
            <StatusBadge :status="stage.status" />
          </div>
        </div>

        <div class="detail-hero-footer">
          <div class="summary-inline">
            <span>{{ messages.inputFiles }} {{ job.sourceFiles.length }}</span>
            <span>{{ messages.hotwords }} {{ job.hotwords.length }}</span>
            <span>{{ messages.speaker }} {{ job.enableSpeaker ? commonMessages.enabled : commonMessages.disabled }}</span>
          </div>

          <div class="button-row">
            <RouterLink
              v-if="job.overallStatus === 'completed'"
              class="primary-button"
              :to="`/jobs/${job.id}/workbench`"
            >
              {{ messages.viewWorkbench }}
            </RouterLink>
            <button
              v-if="job.overallStatus === 'failed'"
              class="secondary-button"
              type="button"
              @click="store.retryJob(job.id)"
            >
              {{ messages.retryJob }}
            </button>
          </div>
        </div>
      </article>

      <article class="surface detail-main-column">
        <div class="section-heading">
          <h3>{{ messages.filesSection }}</h3>
        </div>
        <div class="file-list">
          <div v-for="file in job.sourceFiles" :key="file.id" class="file-pill">
            <div>
              <strong>{{ file.name }}</strong>
              <div class="job-meta-line">
                {{ file.kind === "audio" ? commonMessages.audio : commonMessages.video }} · {{ file.sizeLabel }}
              </div>
            </div>
          </div>
        </div>
      </article>

      <article class="surface detail-side-column">
        <div class="section-heading">
          <h3>{{ messages.settingsSection }}</h3>
        </div>
        <div class="metric-strip metric-strip-tight">
          <div class="metric-pill">
            <span class="muted">{{ messages.language }}</span>
            <strong>{{ job.lang }}</strong>
          </div>
          <div class="metric-pill">
            <span class="muted">{{ messages.speakerDiarization }}</span>
            <strong>{{ job.enableSpeaker ? commonMessages.enabled : commonMessages.disabled }}</strong>
          </div>
          <div class="metric-pill">
            <span class="muted">{{ messages.hotwords }}</span>
            <strong>{{ job.hotwords.length }}</strong>
          </div>
        </div>
      </article>

      <article class="surface detail-progress-card full-span">
        <div class="section-heading">
          <h3>{{ messages.progressSection }}</h3>
          <strong class="detail-progress-percent">{{ progressPercent }}%</strong>
        </div>

        <div class="detail-progress-panel">
          <div class="detail-progress-meta">
            <span class="detail-progress-label">{{ messages.progressEngine }}</span>
            <StatusBadge :status="job.asrStatus" />
          </div>
          <div class="detail-progress-track">
            <div class="detail-progress-fill" :style="{ width: `${progressPercent}%` }">
              <img class="detail-progress-media" :src="progressBarUrl" alt="" aria-hidden="true" />
            </div>
          </div>
          <p class="section-copy detail-progress-copy">
            {{ progressMessage }}
          </p>
        </div>
      </article>

      <article class="surface detail-log-card full-span">
        <div class="section-heading">
          <h3>{{ messages.logSection }}</h3>
        </div>
        <div v-if="job.failureReason" class="note-block error-block">
          {{ job.failureReason }}
        </div>
        <div v-if="logEntries.length" class="job-log-list">
          <div
            v-for="entry in logEntries"
            :key="entry.id"
            class="job-log-entry"
            :class="`job-log-entry-${entry.tone}`"
          >
            {{ entry.text }}
          </div>
        </div>
        <div v-else class="empty-state">
          {{ messages.noLog }}
        </div>
      </article>
    </div>

    <div v-else class="empty-state">
      {{ messages.notFound }}
    </div>
  </section>
</template>
