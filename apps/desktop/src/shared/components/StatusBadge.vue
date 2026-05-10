<script setup lang="ts">
import { computed } from "vue";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { getMessages } from "@/shared/i18n";
import type { JobStage } from "@/shared/types/meeting";

const props = defineProps<{
  status: JobStage;
  text?: string;
}>();

const store = useMeetingStore();
const labels = computed(() => getMessages(store.settings.value.locale).status);

const label = computed(() => props.text ?? labels.value[props.status]);
</script>

<template>
  <span class="status-badge" :class="`status-${status}`">
    {{ label }}
  </span>
</template>
