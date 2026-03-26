<script setup lang="ts">
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { computed, onMounted } from "vue";
import { useRoute } from "vue-router";
import MeetingNotesPanel from "@/components/MeetingNotesPanel.vue";
import StatusBadge from "@/components/StatusBadge.vue";
import { useMeetingStore } from "@/composables/useMeetingStore";
import { createEmptyMeetingSummary } from "@/services/aiStorage";

const route = useRoute();
const meetingStore = useMeetingStore();

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
          <h3>{{ job?.title || "会议纪要" }}</h3>
          <p class="section-copy">
            当前窗口用于阅读完整会议纪要，不和结果工作台正文混排。
          </p>
        </div>
        <div class="button-row">
          <StatusBadge :status="job?.summaryStatus || 'idle'" />
          <button class="secondary-button" type="button" @click="closeWindow">
            关闭窗口
          </button>
        </div>
      </div>
    </article>

    <article class="surface summary-window-result meeting-notes-result">
      <div class="section-heading summary-centered-heading">
        <h3>会议纪要</h3>
        <StatusBadge :status="job?.summaryStatus || 'idle'" />
      </div>

      <MeetingNotesPanel :summary="job?.summary || createEmptyMeetingSummary(job?.title)" />
    </article>
  </section>
</template>
