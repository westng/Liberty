<script setup lang="ts">
import { computed, ref } from "vue";
import type { TranscriptSegment } from "@/types/meeting";

const props = defineProps<{
  segments: TranscriptSegment[];
  query: string;
  busy?: boolean;
}>();
const emit = defineEmits<{
  renameSpeaker: [fromSpeaker: string, toSpeaker: string];
}>();

const editingSegmentId = ref<string | null>(null);
const originalSpeaker = ref("");
const draftSpeaker = ref("");

const filteredSegments = computed(() => {
  const keyword = props.query.trim().toLowerCase();

  return props.segments.filter((segment) => {
    const body = `${segment.speaker ?? ""} ${segment.text}`.toLowerCase();
    return !keyword || body.includes(keyword);
  });
});

function formatClock(ms: number) {
  const date = new Date(ms);
  return date.toISOString().slice(14, 19);
}

function getSpeakerLabel(segment: TranscriptSegment) {
  return segment.speaker?.trim() || "未知说话人";
}

function startEdit(segment: TranscriptSegment) {
  editingSegmentId.value = segment.id;
  originalSpeaker.value = segment.speaker?.trim() || "";
  draftSpeaker.value = segment.speaker?.trim() || "";
}

function cancelEdit() {
  editingSegmentId.value = null;
  originalSpeaker.value = "";
  draftSpeaker.value = "";
}

function submitEdit() {
  const nextSpeaker = draftSpeaker.value.trim();

  if (!editingSegmentId.value || !nextSpeaker || props.busy) {
    return;
  }

  emit("renameSpeaker", originalSpeaker.value, nextSpeaker);
  cancelEdit();
}
</script>

<template>
  <div class="timeline">
    <div
      v-for="segment in filteredSegments"
      :key="segment.id"
      class="timeline-item"
    >
      <div class="timeline-head">
        <div class="timeline-speaker-tools">
          <template v-if="editingSegmentId === segment.id">
            <input
              v-model="draftSpeaker"
              class="speaker-edit-input"
              type="text"
              placeholder="输入讲话人名称"
              :disabled="busy"
              @keydown.enter.prevent="submitEdit"
              @keydown.esc.prevent="cancelEdit"
            />
            <button class="text-button" type="button" :disabled="busy" @click="submitEdit">
              保存
            </button>
            <button class="text-button" type="button" :disabled="busy" @click="cancelEdit">
              取消
            </button>
          </template>
          <template v-else>
            <span class="speaker-tag">{{ getSpeakerLabel(segment) }}</span>
            <button
              class="text-button speaker-edit-button"
              type="button"
              :disabled="busy"
              @click="startEdit(segment)"
            >
              编辑
            </button>
          </template>
        </div>
        <span class="job-meta-line">
          {{ formatClock(segment.startMs) }} - {{ formatClock(segment.endMs) }}
        </span>
      </div>
      <div class="timeline-text">{{ segment.text }}</div>
    </div>

    <div
      v-if="!filteredSegments.length"
      class="empty-state"
    >
      没有匹配到逐字稿片段。
    </div>
  </div>
</template>
