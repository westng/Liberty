<script setup lang="ts">
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { computed, onMounted } from "vue";
import { useRoute } from "vue-router";
import MeetingNotesPanel from "@/components/MeetingNotesPanel.vue";
import StatusBadge from "@/components/StatusBadge.vue";
import { useMeetingStore } from "@/composables/useMeetingStore";
import { getMessages } from "@/services/i18n";
import { createEmptyMeetingSummary } from "@/services/aiStorage";

const route = useRoute();
const meetingStore = useMeetingStore();
const messages = computed(() => getMessages(meetingStore.settings.value.locale).notes);
const commonMessages = computed(() => getMessages(meetingStore.settings.value.locale).common);

const jobId = computed(() => String(route.query.jobId ?? ""));
const job = computed(() => meetingStore.getJobById(jobId.value));

onMounted(() => {
  void meetingStore.refreshJobs();
});

async function closeWindow() {
  await getCurrentWebviewWindow().close();
}
</script>

<template>
  <section class="summary-window-shell meeting-notes-window">
    <article class="surface summary-window-hero">
      <div class="job-title-line">
        <div>
          <h3>{{ job?.title || messages.windowTitle }}</h3>
          <p class="section-copy">
            {{ messages.windowCopy }}
          </p>
        </div>
        <div class="button-row">
          <StatusBadge :status="job?.summaryStatus || 'idle'" />
          <button class="secondary-button" type="button" @click="closeWindow">
            {{ commonMessages.closeWindow }}
          </button>
        </div>
      </div>
    </article>

    <article class="surface summary-window-result meeting-notes-result">
      <div class="section-heading summary-centered-heading">
        <h3>{{ messages.sectionTitle }}</h3>
        <StatusBadge :status="job?.summaryStatus || 'idle'" />
      </div>

      <MeetingNotesPanel :summary="job?.summary || createEmptyMeetingSummary(job?.title)" />
    </article>
  </section>
</template>
