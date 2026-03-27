<script setup lang="ts">
import { computed } from "vue";
import { useMeetingStore } from "@/composables/useMeetingStore";
import { getMessages } from "@/services/i18n";
import type { MeetingSummary } from "@/types/meeting";

defineProps<{
  summary: MeetingSummary;
}>();

const store = useMeetingStore();
const messages = computed(() => getMessages(store.settings.value.locale).notes);
</script>

<template>
  <div class="notes-list">
    <article class="note-block">
      <div class="notes-head">
        <h4>{{ messages.summary }}</h4>
      </div>
      <div class="note-content">
        {{ summary.overview || messages.emptySummary }}
      </div>
    </article>

    <article v-if="summary.topics.length" class="note-block">
      <div class="notes-head">
        <h4>{{ messages.topics }}</h4>
      </div>
      <ul class="note-list">
        <li v-for="item in summary.topics" :key="item">{{ item }}</li>
      </ul>
    </article>

    <article v-if="summary.decisions.length" class="note-block">
      <div class="notes-head">
        <h4>{{ messages.decisions }}</h4>
      </div>
      <ul class="note-list">
        <li v-for="item in summary.decisions" :key="item">{{ item }}</li>
      </ul>
    </article>

    <article v-if="summary.actionItems.length" class="note-block">
      <div class="notes-head">
        <h4>{{ messages.actionItems }}</h4>
      </div>
      <ul class="note-list">
        <li v-for="item in summary.actionItems" :key="item">{{ item }}</li>
      </ul>
    </article>

    <article v-if="summary.risks?.length" class="note-block">
      <div class="notes-head">
        <h4>{{ messages.risks }}</h4>
      </div>
      <ul class="note-list">
        <li v-for="item in summary.risks" :key="item">{{ item }}</li>
      </ul>
    </article>

    <article v-if="summary.followUps?.length" class="note-block">
      <div class="notes-head">
        <h4>{{ messages.followUps }}</h4>
      </div>
      <ul class="note-list">
        <li v-for="item in summary.followUps" :key="item">{{ item }}</li>
      </ul>
    </article>
  </div>
</template>
