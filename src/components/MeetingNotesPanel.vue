<script setup lang="ts">
import type { MeetingSummary } from "@/types/meeting";

defineProps<{
  summary: MeetingSummary;
}>();
</script>

<template>
  <div class="notes-list">
    <article class="note-block">
      <div class="notes-head">
        <h4>摘要</h4>
      </div>
      <div class="note-content">
        {{ summary.overview || "当前还没有可用的 AI 总结内容。" }}
      </div>
    </article>

    <article v-if="summary.topics.length" class="note-block">
      <div class="notes-head">
        <h4>议题</h4>
      </div>
      <ul class="note-list">
        <li v-for="item in summary.topics" :key="item">{{ item }}</li>
      </ul>
    </article>

    <article v-if="summary.decisions.length" class="note-block">
      <div class="notes-head">
        <h4>结论</h4>
      </div>
      <ul class="note-list">
        <li v-for="item in summary.decisions" :key="item">{{ item }}</li>
      </ul>
    </article>

    <article v-if="summary.actionItems.length" class="note-block">
      <div class="notes-head">
        <h4>行动项</h4>
      </div>
      <ul class="note-list">
        <li v-for="item in summary.actionItems" :key="item">{{ item }}</li>
      </ul>
    </article>

    <article v-if="summary.risks?.length" class="note-block">
      <div class="notes-head">
        <h4>风险</h4>
      </div>
      <ul class="note-list">
        <li v-for="item in summary.risks" :key="item">{{ item }}</li>
      </ul>
    </article>

    <article v-if="summary.followUps?.length" class="note-block">
      <div class="notes-head">
        <h4>跟进事项</h4>
      </div>
      <ul class="note-list">
        <li v-for="item in summary.followUps" :key="item">{{ item }}</li>
      </ul>
    </article>
  </div>
</template>
