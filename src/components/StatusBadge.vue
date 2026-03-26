<script setup lang="ts">
import { computed } from "vue";
import type { JobStage } from "@/types/meeting";

const props = defineProps<{
  status: JobStage;
  text?: string;
}>();

const labels: Record<JobStage, string> = {
  idle: "未生成",
  uploaded: "已上传",
  queued: "排队中",
  transcribing: "转写中",
  speaker_processing: "说话人处理中",
  summarizing: "纪要生成中",
  completed: "已完成",
  failed: "失败",
};

const label = computed(() => props.text ?? labels[props.status]);
</script>

<template>
  <span class="status-badge" :class="`status-${status}`">
    {{ label }}
  </span>
</template>
