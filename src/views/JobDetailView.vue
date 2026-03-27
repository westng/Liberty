<script setup lang="ts">
import { computed } from "vue";
import { RouterLink, useRoute } from "vue-router";
import StatusBadge from "@/components/StatusBadge.vue";
import { useMeetingStore } from "@/composables/useMeetingStore";
import { getMessages } from "@/services/i18n";

const route = useRoute();
const store = useMeetingStore();
const messages = computed(() => getMessages(store.settings.value.locale).jobDetail);
const commonMessages = computed(() => getMessages(store.settings.value.locale).common);

const job = computed(() => store.getJobById(route.params.id as string));

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

      <article class="surface detail-log-card full-span">
        <div class="section-heading">
          <h3>{{ messages.errorSection }}</h3>
        </div>
        <div v-if="job.failureReason" class="note-block error-block">
          {{ job.failureReason }}
        </div>
        <pre
          v-if="job.processLog"
          class="log-block"
        ><code>{{ job.processLog }}</code></pre>
        <div v-else class="empty-state">
          {{ messages.noError }}
        </div>
      </article>
    </div>

    <div v-else class="empty-state">
      {{ messages.notFound }}
    </div>
  </section>
</template>
