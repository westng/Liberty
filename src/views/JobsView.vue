<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { RouterLink } from "vue-router";
import StatusBadge from "@/components/StatusBadge.vue";
import { useMeetingStore } from "@/composables/useMeetingStore";
import { formatMessage, getMessages } from "@/services/i18n";

const store = useMeetingStore();
const deletingJobId = ref<string | null>(null);
const messages = computed(() => getMessages(store.settings.value.locale).jobs);
const commonMessages = computed(() => getMessages(store.settings.value.locale).common);

const sortedJobs = computed(() =>
  [...store.jobs.value].sort((left, right) => right.createdAt.localeCompare(left.createdAt)),
);

const completedJobs = computed(
  () => store.jobs.value.filter((job) => job.overallStatus === "completed").length,
);

const processingJobs = computed(
  () =>
    store.jobs.value.filter((job) =>
      ["queued", "transcribing", "speaker_processing", "summarizing"].includes(
        job.overallStatus,
      ),
    ).length,
);

onMounted(() => {
  void store.refreshJobs();
});

function isDeleting(jobId: string) {
  return deletingJobId.value === jobId;
}

function isDeleteDisabled(status: string) {
  return ["queued", "transcribing", "speaker_processing", "summarizing"].includes(status);
}

async function deleteJob(jobId: string) {
  const job = store.getJobById(jobId);

  if (!job || isDeleteDisabled(job.overallStatus)) {
    return;
  }

  const confirmed = window.confirm(formatMessage(messages.value.deleteConfirm, { title: job.title }));
  if (!confirmed) {
    return;
  }

  deletingJobId.value = jobId;

  try {
    await store.deleteJob(jobId);
  } finally {
    deletingJobId.value = null;
  }
}

function formatCreatedAt(value: string) {
  return new Date(value).toLocaleString(store.settings.value.locale, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatFileDuration(minutes: number) {
  if (!minutes || minutes <= 0) {
    return messages.value.pending;
  }

  const totalMinutes = Math.max(1, Math.round(minutes));
  const hours = Math.floor(totalMinutes / 60);
  const remainingMinutes = totalMinutes % 60;

  if (hours <= 0) {
    return formatMessage(messages.value.minutes, { count: remainingMinutes });
  }

  if (remainingMinutes === 0) {
    return formatMessage(messages.value.hours, { count: hours });
  }

  return formatMessage(messages.value.hoursMinutes, { hours, minutes: remainingMinutes });
}

function formatProcessingDuration(seconds?: number) {
  if (typeof seconds !== "number" || seconds < 0) {
    return messages.value.notCompleted;
  }

  const totalSeconds = Math.max(0, Math.round(seconds));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const remainingSeconds = totalSeconds % 60;

  if (hours > 0) {
    return formatMessage(messages.value.processingWithHours, {
      hours,
      minutes: String(minutes).padStart(2, "0"),
    });
  }

  return formatMessage(messages.value.processingWithMinutes, {
    minutes,
    seconds: String(remainingSeconds).padStart(2, "0"),
  });
}
</script>

<template>
  <section class="view-stack">
    <article class="surface">
      <div class="section-heading">
        <div>
          <h3>{{ messages.pageTitle }}</h3>
          <p class="section-copy">
            {{ messages.pageCopy }}
          </p>
        </div>
      </div>
      <div class="summary-inline">
        <span>{{ messages.total }} {{ sortedJobs.length }}</span>
        <span>{{ messages.processing }} {{ processingJobs }}</span>
        <span>{{ messages.completed }} {{ completedJobs }}</span>
      </div>
    </article>

    <article class="surface">
      <div class="section-heading">
        <div>
          <h3>{{ messages.queueTitle }}</h3>
          <p class="section-copy">{{ messages.queueCopy }}</p>
        </div>
      </div>

      <div class="jobs-table">
        <div class="jobs-table-head">
          <span>{{ messages.colTask }}</span>
          <span>{{ messages.colFileInfo }}</span>
          <span>{{ messages.colProcessingTime }}</span>
          <span>{{ messages.colCreatedAt }}</span>
          <span>{{ messages.colStatus }}</span>
          <span>{{ messages.colActions }}</span>
        </div>

        <div v-for="job in sortedJobs" :key="job.id" class="jobs-row">
          <div class="jobs-primary">
            <strong>{{ job.title }}</strong>
            <div class="job-meta-line">
              {{ job.sourceFiles.map((file) => file.name).join(" · ") }}
            </div>
          </div>

          <div class="jobs-cell">
            <strong>{{ formatMessage(messages.filesCount, { count: job.sourceFiles.length }) }}</strong>
            <div class="job-meta-line">
              {{ formatMessage(messages.fileDuration, { duration: formatFileDuration(job.durationMinutes) }) }}
            </div>
            <div class="job-meta-line">
              {{ job.enableSpeaker ? messages.diarizationEnabled : messages.transcriptOnly }}
            </div>
          </div>

          <div class="jobs-cell">
            <strong>{{ formatProcessingDuration(job.processingDurationSeconds) }}</strong>
            <div class="job-meta-line">
              {{ job.overallStatus === "completed" ? messages.processCompleted : job.overallStatus === "failed" ? messages.processFailed : messages.processRunning }}
            </div>
          </div>

          <div class="jobs-cell">
            {{ formatCreatedAt(job.createdAt) }}
          </div>

          <div class="jobs-cell">
            <StatusBadge :status="job.overallStatus" />
          </div>

          <div class="jobs-actions">
            <RouterLink class="text-button" :to="`/jobs/${job.id}`">
              {{ messages.details }}
            </RouterLink>
            <RouterLink
              v-if="job.overallStatus === 'completed'"
              class="primary-button small-button"
              :to="`/jobs/${job.id}/workbench`"
            >
              {{ messages.workbench }}
            </RouterLink>
            <button
              v-if="job.overallStatus === 'failed'"
              class="secondary-button small-button"
              type="button"
              @click="store.retryJob(job.id)"
            >
              {{ commonMessages.retry }}
            </button>
            <button
              class="text-button small-button jobs-delete-button"
              type="button"
              :disabled="isDeleteDisabled(job.overallStatus) || isDeleting(job.id)"
              :title="isDeleteDisabled(job.overallStatus) ? messages.deleteDisabled : messages.deleteAction"
              @click="deleteJob(job.id)"
            >
              {{ isDeleting(job.id) ? messages.deleting : commonMessages.delete }}
            </button>
          </div>
        </div>
      </div>
    </article>
  </section>
</template>
