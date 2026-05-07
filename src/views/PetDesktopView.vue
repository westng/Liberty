<script setup lang="ts">
import { getCurrentWindow } from "@tauri-apps/api/window";
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useMeetingStore } from "@/composables/useMeetingStore";
import { usePetStore } from "@/composables/usePetStore";
import {
  getPetSpriteFrameCountForEnvironment,
  getPetSpriteScale,
  getPetSpriteUrlForEnvironment,
} from "@/services/petSprites";
import { buildInteractionBubbles } from "@/services/petDialogues";
import type { PetEventLedgerEntry, PetInteractionAction } from "@/types/meeting";

const meetingStore = useMeetingStore();
const petStore = usePetStore();
const hasTauriWindowContext = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
const appWindow = hasTauriWindowContext ? getCurrentWindow() : null;
let unlistenMoved: (() => void) | null = null;
let savePositionTimeout: number | null = null;
let refreshTimer: number | null = null;
let bubbleHideTimer: number | null = null;
let proactiveSpeechTimer: number | null = null;
let animationTimer: number | null = null;
let pendingDrag = false;
let dragStarted = false;
let pointerStartX = 0;
let pointerStartY = 0;
let lastPetInteractAt = 0;
let petInteractStreak = 0;
let lastBubbleShownAt = 0;
let lastLocalInteractionAction: PetInteractionAction | null = null;
let lastLocalInteractionAt = 0;
const activeEventId = ref("");
const activeBubbleText = ref("");
const activeBubbleKey = ref("");
const spriteFrameIndex = ref(0);
const visualStateNow = ref(Date.now());
const BUBBLE_VISIBLE_MS = 9000;
const DRAG_THRESHOLD_PX = 8;
const PROACTIVE_IDLE_GUARD_MS = 6000;
const LOCAL_INTERACTION_DEDUP_MS = 1500;
const RECENT_MOOD_HOLD_MS = 18_000;
const NEEDY_AFTER_MS = 30_000;
const SLEEPY_AFTER_MS = 90_000;
const BORED_AFTER_MS = 8 * 60_000;
const ANIMATION_FRAME_INTERVAL_MS = 1500;

const EVENT_BUBBLES = {
  daily_open: [
    "我来了，今天也一起慢慢推进，我会一直陪着你。",
    "已经陪你上线啦，你先忙手头的，我在旁边看着就好。",
    "今天也见到你了，我会乖乖待在这儿陪你做完这一轮。",
  ],
  transcription_started: [
    "已经开始转写了，你不用一直盯着，我会陪你一起等它跑完。",
    "它已经在处理啦，你先歇一口气，我帮你守着这段进度。",
    "转写已经动起来了，我们不着急，等它慢慢把内容吐出来。",
  ],
  transcription_completed: [
    "已经处理好了，我们一会儿一起看看里面有没有值得记下来的重点。",
    "这一份已经跑完了，先放这儿，我陪你慢慢过一遍结果。",
    "转写结束啦，接下来就只剩把重点捋顺，我可以继续陪着你。",
  ],
  ai_summary_completed: [
    "总结已经出来了，你可以先看个大概，我陪你一起补细节。",
    "这份总结整理好了，我们慢慢看，不用一下子全消化完。",
    "AI 已经帮你收了一轮重点，接下来就交给我们一起判断哪些最重要。",
  ],
  export_completed: [
    "导出已经准备好了，收尾这一步也做完了，辛苦啦。",
    "文件已经顺利导出来了，这一轮算是稳稳落地了。",
    "结果已经帮你送到门口了，剩下就是安心带走它。",
  ],
  job_created: [
    "新的任务已经记下来了，我们就按这个节奏一点点往前推。",
    "我看到你又开了个新任务，没关系，我们慢慢做也来得及。",
    "新的内容已经排上队了，我会陪你把这一项也照顾好。",
  ],
} satisfies Record<string, string[]>;

const interactionBubbles = computed(() => buildInteractionBubbles(petStore.profile.value?.name ?? "Libby"));

function getStableBubble(options: string[], seed: string) {
  if (options.length === 0) {
    return "";
  }

  let hash = 0;
  for (let index = 0; index < seed.length; index += 1) {
    hash = (hash * 31 + seed.charCodeAt(index)) >>> 0;
  }

  return options[hash % options.length];
}

function formatMetadata(metadata?: string) {
  const value = metadata?.trim();
  if (!value) {
    return "";
  }

  return value.length > 18 ? `《${value.slice(0, 18)}...》` : `《${value}》`;
}

function getEventBubble(event?: PetEventLedgerEntry) {
  if (!event) {
    return null;
  }

  const key = event.eventType === "interaction" ? event.eventSource : event.eventType;
  const candidates = (interactionBubbles.value as Record<string, string[]>)[key] ?? EVENT_BUBBLES[key];
  if (!candidates?.length) {
    return null;
  }

  let bubble = getStableBubble(candidates, event.id);
  const subject = formatMetadata(event.metadata);

  if (subject) {
    if (event.eventType === "transcription_started") {
      bubble = `${subject}已经开始转写了，我会陪你一起等它跑完。`;
    } else if (event.eventType === "transcription_completed") {
      bubble = `${subject}已经处理好了，我们一会儿一起看看重点。`;
    } else if (event.eventType === "ai_summary_completed") {
      bubble = `${subject}的总结已经出来了，我们慢慢看，不着急。`;
    } else if (event.eventType === "job_created") {
      bubble = `${subject}已经排上了，我会陪你把它一点点往前推。`;
    } else if (event.eventType === "export_completed") {
      bubble = `${subject}已经导出来了，这一轮收尾得很稳。`;
    }
  }

  return bubble;
}

function getBubbleByKey(key: string, seed: string) {
  const candidates = (interactionBubbles.value as Record<string, string[]>)[key] ?? EVENT_BUBBLES[key];
  if (!candidates?.length) {
    return "";
  }

  return getStableBubble(candidates, seed);
}

function hideBubble() {
  activeBubbleText.value = "";
  activeBubbleKey.value = "";
  if (bubbleHideTimer !== null) {
    window.clearTimeout(bubbleHideTimer);
    bubbleHideTimer = null;
  }
}

function clearProactiveSpeechTimer() {
  if (proactiveSpeechTimer !== null) {
    window.clearTimeout(proactiveSpeechTimer);
    proactiveSpeechTimer = null;
  }
}

function getProactiveIntervalMs(level: number) {
  switch (level) {
    case 3:
      return 12000 + Math.floor(Math.random() * 4000);
    case 2:
      return 20000 + Math.floor(Math.random() * 6000);
    case 1:
      return 30000 + Math.floor(Math.random() * 8000);
    default:
      return 0;
  }
}

function scheduleProactiveSpeech() {
  clearProactiveSpeechTimer();

  const settings = petStore.settings.value;
  if (!settings || settings.muted || !settings.desktopEnabled) {
    return;
  }

  const intervalMs = getProactiveIntervalMs(settings.proactiveLevel);
  if (intervalMs <= 0) {
    return;
  }

  proactiveSpeechTimer = window.setTimeout(() => {
    proactiveSpeechTimer = null;
    void maybeSpeakProactively();
  }, intervalMs);
}

async function maybeSpeakProactively() {
  const settings = petStore.settings.value;
  if (!settings || settings.muted || !settings.desktopEnabled) {
    return;
  }

  if (activeBubbleText.value.trim() || Date.now() - lastBubbleShownAt < PROACTIVE_IDLE_GUARD_MS) {
    scheduleProactiveSpeech();
    return;
  }

  const bubbleText = getBubbleByKey("proactive", `proactive-${Date.now()}`);
  if (bubbleText) {
    showBubbleText(bubbleText, "proactive");
  }

  scheduleProactiveSpeech();
}

function showBubbleText(text: string, key = text) {
  if (petStore.settings.value?.muted || !text.trim()) {
    hideBubble();
    return;
  }

  lastBubbleShownAt = Date.now();
  activeBubbleKey.value = key;
  activeBubbleText.value = text;
  if (bubbleHideTimer !== null) {
    window.clearTimeout(bubbleHideTimer);
  }
  bubbleHideTimer = window.setTimeout(() => {
    activeBubbleText.value = "";
    bubbleHideTimer = null;
  }, BUBBLE_VISIBLE_MS);
}

function showEventBubble(event?: PetEventLedgerEntry | null) {
  if (!event || petStore.settings.value?.muted) {
    hideBubble();
    return;
  }

  const bubble = getEventBubble(event);
  if (!bubble) {
    hideBubble();
    return;
  }

  showBubbleText(bubble);
}

function shouldSuppressInteractionEvent(event?: PetEventLedgerEntry | null) {
  if (!event || event.eventType !== "interaction") {
    return false;
  }

  if (!lastLocalInteractionAction) {
    return false;
  }

  if (event.eventSource !== lastLocalInteractionAction) {
    return false;
  }

  return Date.now() - lastLocalInteractionAt <= LOCAL_INTERACTION_DEDUP_MS;
}

const bubbleText = computed(() => activeBubbleText.value);
const showBubble = computed(() => activeBubbleText.value.trim().length > 0);
const latestEventAtMs = computed(() => {
  const value = petStore.events.value[0]?.eventTime;
  if (!value) {
    return 0;
  }

  const parsed = Date.parse(value);
  return Number.isFinite(parsed) ? parsed : 0;
});
const visualMood = computed(() => {
  const persistedMood = petStore.profile.value?.currentMood ?? "idle";
  const fallbackMood = persistedMood === "excited" ? "idle" : persistedMood;

  if (environmentState.value) {
    return persistedMood;
  }

  const latestEventAt = latestEventAtMs.value;
  if (!latestEventAt) {
    return "idle";
  }

  const idleMs = Math.max(0, visualStateNow.value - latestEventAt);
  if (idleMs <= RECENT_MOOD_HOLD_MS) {
    return fallbackMood;
  }
  if (idleMs >= SLEEPY_AFTER_MS) {
    return "sleepy";
  }
  if (idleMs >= BORED_AFTER_MS) {
    return "bored";
  }
  if (idleMs >= NEEDY_AFTER_MS) {
    return "needy";
  }

  return "idle";
});
const environmentState = computed(() => {
  const jobs = meetingStore.jobs.value;

  if (jobs.some((job) => job.overallStatus === "transcribing")) {
    return "transcribing";
  }
  if (jobs.some((job) => job.overallStatus === "speaker_processing")) {
    return "speaker_processing";
  }
  if (jobs.some((job) => job.overallStatus === "summarizing")) {
    return "summarizing";
  }
  if (jobs.some((job) => job.overallStatus === "queued")) {
    return "queued";
  }

  return null;
});

const petSpriteUrl = computed(() => {
  const mood = visualMood.value;
  return getPetSpriteUrlForEnvironment(environmentState.value, mood, spriteFrameIndex.value);
});

const petSpriteStyle = computed(() => {
  const stage = petStore.profile.value?.stage ?? "baby";

  return {
    transform: `scale(${getPetSpriteScale(stage)})`,
  };
});

onMounted(async () => {
  console.info("[pet-desktop] mounted", { hasTauriWindowContext });

  try {
    await meetingStore.ensureSettingsLoaded();
    await meetingStore.refreshJobs();
    await petStore.loadPetState();
    await applyWindowPreferences();
  } catch (error) {
    console.error("[pet-desktop] mount failed", error);
    throw error;
  }

  if (appWindow) {
    unlistenMoved = await appWindow.onMoved(() => {
      if (savePositionTimeout !== null) {
        window.clearTimeout(savePositionTimeout);
      }

      savePositionTimeout = window.setTimeout(() => {
        void persistWindowPosition();
      }, 180);
    });
  }

  refreshTimer = window.setInterval(() => {
    visualStateNow.value = Date.now();
    void refreshPetWindowState();
  }, 1000);

  animationTimer = window.setInterval(() => {
    const mood = visualMood.value;
    const frameCount = getPetSpriteFrameCountForEnvironment(environmentState.value, mood);
    spriteFrameIndex.value = (spriteFrameIndex.value + 1) % Math.max(frameCount, 1);
  }, ANIMATION_FRAME_INTERVAL_MS);

  scheduleProactiveSpeech();
});

onBeforeUnmount(() => {
  if (unlistenMoved) {
    unlistenMoved();
    unlistenMoved = null;
  }

  if (savePositionTimeout !== null) {
    window.clearTimeout(savePositionTimeout);
    savePositionTimeout = null;
  }

  if (refreshTimer !== null) {
    window.clearInterval(refreshTimer);
    refreshTimer = null;
  }

  if (bubbleHideTimer !== null) {
    window.clearTimeout(bubbleHideTimer);
    bubbleHideTimer = null;
  }

  if (animationTimer !== null) {
    window.clearInterval(animationTimer);
    animationTimer = null;
  }

  clearProactiveSpeechTimer();
});

watch(
  () => visualMood.value,
  () => {
    spriteFrameIndex.value = 0;
  },
);

watch(environmentState, () => {
  spriteFrameIndex.value = 0;
});

watch(
  () => petStore.events.value[0]?.id ?? "",
  (nextEventId) => {
    if (!nextEventId || nextEventId === activeEventId.value) {
      return;
    }

    activeEventId.value = nextEventId;
    const latestEvent =
      petStore.events.value.find((event) => event.id === nextEventId) ?? petStore.events.value[0];
    if (shouldSuppressInteractionEvent(latestEvent)) {
      return;
    }
    showEventBubble(latestEvent);
  },
  { immediate: false },
);

watch(
  () => petStore.settings.value?.muted,
  (muted) => {
    if (muted) {
      hideBubble();
    }
    scheduleProactiveSpeech();
  },
);

watch(
  () => petStore.settings.value?.proactiveLevel,
  () => {
    scheduleProactiveSpeech();
  },
);

watch(
  () => petStore.settings.value?.desktopEnabled,
  (enabled) => {
    if (!enabled) {
      hideBubble();
    }
    scheduleProactiveSpeech();
  },
);

async function startWindowDrag() {
  if (!appWindow) {
    return;
  }

  try {
    await appWindow.setFocus();
    await appWindow.startDragging();
  } catch (error) {
    console.error("Failed to drag pet window", error);
  }
}

function handlePetPointerDown(event: MouseEvent) {
  pendingDrag = true;
  dragStarted = false;
  pointerStartX = event.clientX;
  pointerStartY = event.clientY;
}

async function handlePetPointerMove(event: MouseEvent) {
  if (!pendingDrag || dragStarted) {
    return;
  }

  const deltaX = event.clientX - pointerStartX;
  const deltaY = event.clientY - pointerStartY;
  if (Math.hypot(deltaX, deltaY) < DRAG_THRESHOLD_PX) {
    return;
  }

  dragStarted = true;
  await startWindowDrag();
}

function handlePetPointerCancel() {
  pendingDrag = false;
  dragStarted = false;
}

async function handlePetPointerUp() {
  const shouldInteract = pendingDrag && !dragStarted;
  pendingDrag = false;
  dragStarted = false;

  if (shouldInteract) {
    await interact("pet");
  }
}

async function persistWindowPosition() {
  if (!appWindow) {
    return;
  }

  const position = await appWindow.outerPosition();
  const current = petStore.settings.value;
  if (!current) {
    return;
  }

  await petStore.saveSettings({
    desktopEnabled: current.desktopEnabled,
    alwaysOnTop: current.alwaysOnTop,
    muted: current.muted,
    focusModeEnabled: current.focusModeEnabled,
    proactiveLevel: current.proactiveLevel,
    lastWindowX: position.x,
    lastWindowY: position.y,
  });
}

async function interact(action: PetInteractionAction) {
  const now = Date.now();
  if (action === "pet" && now - lastPetInteractAt <= 900) {
    petInteractStreak += 1;
  } else {
    petInteractStreak = 1;
  }
  if (action === "pet") {
    lastPetInteractAt = now;
  } else {
    lastPetInteractAt = 0;
    petInteractStreak = 0;
  }

  const dialogueKey = action === "pet" && petInteractStreak >= 3 ? "rapidTap" : action;
  const bubbleKey = `interaction-${dialogueKey}`;
  const bubbleText = getBubbleByKey(dialogueKey, `${bubbleKey}-${now}-${petInteractStreak}`);
  showBubbleText(bubbleText, bubbleKey);
  lastLocalInteractionAction = action;
  lastLocalInteractionAt = now;
  try {
    await petStore.applyInteraction(action);
  } catch (error) {
    console.error("[pet-desktop] interaction failed", error);
  }
}

async function applyWindowPreferences() {
  const settings = petStore.settings.value;
  if (!settings || !appWindow) {
    return;
  }

  await appWindow.setAlwaysOnTop(settings.alwaysOnTop);
}

async function refreshPetWindowState() {
  try {
    await petStore.refresh();
    await applyWindowPreferences();
  } catch (error) {
    console.error("[pet-desktop] refresh failed", error);
  }
}
</script>

<template>
  <main class="pet-desktop-root">
    <div v-if="showBubble" class="pet-bubble">{{ bubbleText }}</div>

    <button
      class="pet-body-button"
      type="button"
      @mousedown.left.prevent="handlePetPointerDown"
      @mousemove="handlePetPointerMove"
      @mouseup.left.prevent="handlePetPointerUp"
      @mouseleave="handlePetPointerCancel"
    >
      <div class="pet-body">
        <img class="pet-sprite" :src="petSpriteUrl" :style="petSpriteStyle" alt="" draggable="false" />
      </div>
    </button>
  </main>
</template>

<style scoped>
:global(html),
:global(body),
:global(#app) {
  background: transparent !important;
}

:global(body::before) {
  display: none;
}

.pet-desktop-root {
  position: relative;
  width: 100%;
  height: 100%;
  padding: 10px 4px 6px;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  justify-items: center;
  align-items: center;
  justify-content: flex-end;
  background: transparent;
  user-select: none;
}

.pet-bubble {
  position: absolute;
  top: 8px;
  left: 50%;
  transform: translateX(-50%);
  width: fit-content;
  max-width: min(220px, calc(100% - 20px));
  box-sizing: border-box;
  padding: 8px 10px;
  border-radius: 14px;
  background: color-mix(in srgb, var(--bg-panel) 84%, white);
  color: var(--text-main);
  text-align: center;
  font-size: 12px;
  line-height: 1.35;
  white-space: normal;
  overflow-wrap: break-word;
  box-shadow: 0 8px 20px rgba(20, 21, 26, 0.08);
  pointer-events: none;
  z-index: 2;
}

.pet-body-button {
  display: grid;
  place-items: center;
  width: 148px;
  height: 146px;
  padding: 0;
  border: none;
  background: transparent;
  appearance: none;
  -webkit-appearance: none;
  outline: none;
  box-shadow: none;
  -webkit-tap-highlight-color: transparent;
  margin-top: auto;
  margin-left: auto;
  margin-right: auto;
  cursor: grab;
}

.pet-body-button:focus,
.pet-body-button:focus-visible,
.pet-body-button:active {
  outline: none;
  box-shadow: none;
  background: transparent;
}

.pet-body-button:active {
  cursor: grabbing;
}

.pet-body {
  width: 148px;
  height: 146px;
  display: grid;
  place-items: center;
  background: transparent;
}

.pet-sprite {
  width: 148px;
  height: 146px;
  object-fit: contain;
  object-position: center bottom;
  pointer-events: none;
}
</style>
