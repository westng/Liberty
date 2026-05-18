<script setup lang="ts">
import "./PetManagementView.css";
import { message } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { usePetStore } from "@/features/pet/stores/usePetStore";
import {
  getPetEnvironmentState,
  getPetSpriteFrameCountForGroup,
  getPetSpriteScale,
  getPetSpriteUrlForGroup,
  resolvePetVisualAction,
} from "@/features/pet/services/petSprites";
import { applyDesktopPetState, openExtraDesktopPet } from "@/shared/services/tauri/pet";
import type { PetInteractionAction } from "@/shared/types/meeting";

const meetingStore = useMeetingStore();
const petStore = usePetStore();
const currentWindow = getCurrentWindow();
let refreshTimer: number | null = null;
let coverAnimationTimer: number | null = null;
const petNameDraft = ref("");
const petNameSaving = ref(false);
const extraPetOpening = ref(false);
const petCoverFrameIndex = ref(0);
const petVisualStateNow = ref(Date.now());
const ANIMATION_FRAME_INTERVAL_MS = 1000;

const settingsForm = reactive({
  desktopEnabled: true,
  alwaysOnTop: true,
  muted: false,
  focusModeEnabled: false,
  proactiveLevel: 2,
  lastWindowX: undefined as number | undefined,
  lastWindowY: undefined as number | undefined,
});

const stageLabel = computed(() => {
  const value = petStore.profile.value?.stage;
  if (value === "mature") return "成熟期";
  if (value === "growing") return "成长期";
  return "幼年期";
});

const moodLabel = computed(() => {
  const value = petStore.profile.value?.currentMood;
  switch (value) {
    case "cheerful":
      return "开心";
    case "excited":
      return "兴奋";
    case "proud":
      return "得意";
    case "needy":
      return "想互动";
    case "sleepy":
      return "犯困";
    case "bored":
      return "无聊";
    default:
      return "待机";
  }
});

function formatEventSource(source: string) {
  if (locale.value === "en-US") {
    switch (source) {
      case "interaction":
        return "Interaction";
      case "workflow":
        return "Workflow";
      case "pet":
        return "Pet";
      case "tap":
        return "Tap";
      case "feed":
        return "Feed";
      case "encourage":
        return "Encourage";
      case "job_created":
        return "Job Created";
      case "daily_open":
        return "Daily Open";
      case "transcription_started":
        return "Transcription Started";
      case "transcription_completed":
        return "Transcription Completed";
      case "ai_summary_completed":
        return "AI Summary Completed";
      case "export_completed":
        return "Export Completed";
      default:
        return source;
    }
  }

  switch (source) {
    case "interaction":
      return "互动";
    case "workflow":
      return "流程";
    case "pet":
      return "宠物";
    case "tap":
      return "点击";
    case "feed":
      return "投喂";
    case "encourage":
      return "鼓励";
    case "job_created":
      return "创建任务";
    case "daily_open":
      return "每日上线";
    case "transcription_started":
      return "开始转写";
    case "transcription_completed":
      return "转写完成";
    case "ai_summary_completed":
      return "总结完成";
    case "export_completed":
      return "导出完成";
    default:
      return source;
  }
}

function formatEventMetadata(source: string, metadata?: string | null) {
  const value = metadata?.trim();
  if (!value) {
    return locale.value === "en-US" ? "No extra context" : "无额外上下文";
  }

  if (locale.value !== "en-US" && source === "daily_open" && value === "Liberty app opened") {
    return "Liberty 已启动";
  }

  return value;
}

const progressRatio = computed(() => ((petStore.stageProgress.value ?? 0) / 20) * 100);
const petCoverMood = computed(() => petStore.profile.value?.currentMood ?? "idle");
const petCoverEnvironmentState = computed(() => getPetEnvironmentState(meetingStore.jobs.value));
const petCoverAction = computed(() =>
  resolvePetVisualAction({
    environmentState: petCoverEnvironmentState.value,
    latestEvent: petStore.events.value[0],
    mood: petCoverMood.value,
    nowMs: petVisualStateNow.value,
  }),
);
const petCoverUrl = computed(() => getPetSpriteUrlForGroup(petCoverAction.value, petCoverFrameIndex.value));
const petCoverStyle = computed(() => ({
  transform: `scale(${getPetSpriteScale(petStore.profile.value?.stage ?? "baby")})`,
}));
const unlockedCosmeticsLabel = computed(() =>
  petStore.cosmetics.value.map((item) => {
    if (item.cosmeticKey === "sprout-ribbon") {
      return locale.value === "en-US" ? "Sprout Ribbon" : "新芽丝带";
    }

    if (item.cosmeticKey === "golden-bell") {
      return locale.value === "en-US" ? "Golden Bell" : "金色铃铛";
    }

    return item.cosmeticKey;
  }),
);

onMounted(async () => {
  await petStore.loadPetState();
  syncSettingsForm();
  window.addEventListener("focus", handleWindowFocus);
  refreshTimer = window.setInterval(() => {
    petVisualStateNow.value = Date.now();
    void petStore.refresh();
  }, 4000);
  coverAnimationTimer = window.setInterval(() => {
    const frameCount = getPetSpriteFrameCountForGroup(petCoverAction.value);
    petCoverFrameIndex.value = (petCoverFrameIndex.value + 1) % Math.max(frameCount, 1);
  }, ANIMATION_FRAME_INTERVAL_MS);
});

onBeforeUnmount(() => {
  window.removeEventListener("focus", handleWindowFocus);
  if (refreshTimer !== null) {
    window.clearInterval(refreshTimer);
    refreshTimer = null;
  }
  if (coverAnimationTimer !== null) {
    window.clearInterval(coverAnimationTimer);
    coverAnimationTimer = null;
  }
});

watch(petCoverAction, () => {
  petCoverFrameIndex.value = 0;
});

async function handleWindowFocus() {
  await petStore.refresh();
  syncSettingsForm();
}

function syncSettingsForm() {
  const settings = petStore.settings.value;
  if (!settings) {
    return;
  }

  settingsForm.desktopEnabled = settings.desktopEnabled;
  settingsForm.alwaysOnTop = settings.alwaysOnTop;
  settingsForm.muted = settings.muted;
  settingsForm.focusModeEnabled = settings.focusModeEnabled;
  settingsForm.proactiveLevel = settings.proactiveLevel;
  settingsForm.lastWindowX = settings.lastWindowX;
  settingsForm.lastWindowY = settings.lastWindowY;
  petNameDraft.value = petStore.profile.value?.name ?? "Libby";
}

async function saveSettings() {
  const savedSettings = await petStore.saveSettings({
    desktopEnabled: settingsForm.desktopEnabled,
    alwaysOnTop: settingsForm.alwaysOnTop,
    muted: settingsForm.muted,
    focusModeEnabled: settingsForm.focusModeEnabled,
    proactiveLevel: Number(settingsForm.proactiveLevel),
    lastWindowX: settingsForm.lastWindowX,
    lastWindowY: settingsForm.lastWindowY,
  });

  settingsForm.desktopEnabled = savedSettings.desktopEnabled;
  settingsForm.alwaysOnTop = savedSettings.alwaysOnTop;
  settingsForm.muted = savedSettings.muted;
  settingsForm.focusModeEnabled = savedSettings.focusModeEnabled;
  settingsForm.proactiveLevel = savedSettings.proactiveLevel;
  settingsForm.lastWindowX = savedSettings.lastWindowX;
  settingsForm.lastWindowY = savedSettings.lastWindowY;

  try {
    await applyDesktopPetState(savedSettings, "pet-settings");
  } catch (error) {
    console.error("[pet-settings] settings saved but native pet sync failed", error);
  }

  settingsForm.desktopEnabled = savedSettings.desktopEnabled;
  await currentWindow.setFocus().catch(() => undefined);
  await message(
    locale.value === "en-US" ? "Pet desktop settings have been saved." : "桌面行为设置已保存。",
    {
      title: locale.value === "en-US" ? "Pet Center" : "宠物中心",
      kind: "info",
    },
  );
}

async function openAnotherDesktopPet() {
  extraPetOpening.value = true;
  try {
    const status = await openExtraDesktopPet();
    if (!settingsForm.desktopEnabled) {
      settingsForm.desktopEnabled = true;
      await petStore.refresh();
      syncSettingsForm();
    }
    await currentWindow.setFocus().catch(() => undefined);
    await message(
      locale.value === "en-US"
        ? `Desktop pet opened. Active pets: ${status.instanceCount}.`
        : `已多开一个桌面宠物，当前共 ${status.instanceCount} 个。`,
      {
        title: locale.value === "en-US" ? "Pet Center" : "宠物中心",
        kind: "info",
      },
    );
  } catch (error) {
    const content = error instanceof Error ? error.message : String(error);
    await currentWindow.setFocus().catch(() => undefined);
    await message(content || (locale.value === "en-US" ? "Failed to open another desktop pet." : "多开桌面宠物失败。"), {
      title: locale.value === "en-US" ? "Pet Center" : "宠物中心",
      kind: "error",
    });
  } finally {
    extraPetOpening.value = false;
  }
}

async function triggerInteraction(action: PetInteractionAction) {
  try {
    await petStore.applyInteraction(action);
  } catch (error) {
    const content = error instanceof Error ? error.message : String(error);
    await currentWindow.setFocus().catch(() => undefined);
    await message(content || "今天的互动已经达到上限。", {
      title: locale.value === "en-US" ? "Pet Center" : "宠物中心",
      kind: "warning",
    });
  }
}

async function savePetName() {
  const nextName = petNameDraft.value.trim();
  if (!nextName) {
    petNameDraft.value = petStore.profile.value?.name ?? "Libby";
    return;
  }

  if (nextName === (petStore.profile.value?.name ?? "")) {
    return;
  }

  petNameSaving.value = true;
  try {
    await petStore.saveProfile({ name: nextName });
    petNameDraft.value = petStore.profile.value?.name ?? nextName;
    await currentWindow.setFocus().catch(() => undefined);
    await message(
      locale.value === "en-US" ? "Pet name has been saved." : "宠物名字已保存。",
      {
        title: locale.value === "en-US" ? "Pet Center" : "宠物中心",
        kind: "info",
      },
    );
  } finally {
    petNameSaving.value = false;
  }
}

const locale = computed(() => meetingStore.settings.value.locale);
</script>

<template>
  <section class="view-stack pet-page-stack" style="margin-top: 20px;">
    <div class="pet-primary-grid">
      <article class="surface pet-companion-card">
        <div class="pet-companion-visual">
          <div class="pet-avatar-shell pet-avatar-shell-large" :data-stage="petStore.profile.value?.stage ?? 'baby'">
            <img class="pet-cover-image" :src="petCoverUrl" :style="petCoverStyle" alt="" draggable="false" />
          </div>
          <div class="pet-presence-badge">
            <strong>{{ stageLabel }}</strong>
            <span>{{ moodLabel }}</span>
          </div>
        </div>

        <div class="pet-companion-copy">
          <div class="pet-avatar-meta">
            <strong>{{ petStore.profile.value?.name ?? "Libby" }}</strong>
            <span>
              {{ locale === "en-US" ? "Your desktop companion stays nearby and follows your pace." : "它会留在桌面上，按照你的节奏陪着你。" }}
            </span>
          </div>

          <div class="pet-name-editor">
            <label for="pet-name-input">{{ locale === "en-US" ? "Pet Name" : "宠物名字" }}</label>
            <div class="pet-name-row">
              <input
                id="pet-name-input"
                v-model="petNameDraft"
                type="text"
                maxlength="24"
                :placeholder="locale === 'en-US' ? 'Enter pet name' : '输入宠物名字'"
                @keydown.enter.prevent="savePetName"
              />
              <button class="secondary-button" type="button" :disabled="petNameSaving" @click="savePetName">
                {{ locale === "en-US" ? "Save" : "保存" }}
              </button>
            </div>
          </div>

          <div class="pet-status-grid">
            <div class="pet-status-tile">
              <span>{{ locale === "en-US" ? "Level" : "等级" }}</span>
              <strong>{{ petStore.profile.value?.level ?? 1 }}</strong>
            </div>
            <div class="pet-status-tile">
              <span>{{ locale === "en-US" ? "Experience" : "经验" }}</span>
              <strong>{{ petStore.profile.value?.experience ?? 0 }}</strong>
            </div>
            <div class="pet-status-tile">
              <span>{{ locale === "en-US" ? "Rewards" : "奖励" }}</span>
              <strong>{{ petStore.cosmetics.value.length }}</strong>
            </div>
          </div>

          <div class="pet-progress-card">
            <div class="pet-stat-row">
              <span>{{ locale === "en-US" ? "Stage progress" : "阶段进度" }}</span>
              <strong>{{ petStore.stageProgress.value }}/20</strong>
            </div>
            <div class="pet-progress">
              <div class="pet-progress-bar" :style="{ width: `${progressRatio}%` }"></div>
            </div>
          </div>
        </div>
      </article>

      <article class="surface pet-interaction-panel">
        <div class="section-heading">
          <div>
            <h3>{{ locale === "en-US" ? "Care & Interaction" : "互动照顾" }}</h3>
            <p class="section-copy">
              {{ locale === "en-US"
                ? "Small actions shift mood, strengthen presence, and keep the companion feeling alive."
                : "轻量互动会影响情绪，也会让这只桌宠更有陪伴感。" }}
            </p>
          </div>
        </div>

        <div class="pet-action-grid pet-action-grid-rich">
          <button class="secondary-button pet-action-button" type="button" @click="triggerInteraction('tap')">
            <span class="pet-action-icon" aria-hidden="true">·</span>
            <strong>{{ locale === "en-US" ? "Tap" : "点击" }}</strong>
            <span>{{ locale === "en-US" ? "Quick check-in" : "轻轻叫它一下" }}</span>
          </button>
          <button class="secondary-button pet-action-button" type="button" @click="triggerInteraction('pet')">
            <span class="pet-action-icon" aria-hidden="true">·</span>
            <strong>{{ locale === "en-US" ? "Pet" : "抚摸" }}</strong>
            <span>{{ locale === "en-US" ? "Gentle comfort" : "给它一点安抚" }}</span>
          </button>
          <button class="secondary-button pet-action-button" type="button" @click="triggerInteraction('feed')">
            <span class="pet-action-icon" aria-hidden="true">·</span>
            <strong>{{ locale === "en-US" ? "Feed" : "投喂" }}</strong>
            <span>{{ locale === "en-US" ? "Restore energy" : "补充一点活力" }}</span>
          </button>
          <button class="secondary-button pet-action-button" type="button" @click="triggerInteraction('encourage')">
            <span class="pet-action-icon" aria-hidden="true">·</span>
            <strong>{{ locale === "en-US" ? "Encourage" : "鼓励" }}</strong>
            <span>{{ locale === "en-US" ? "Boost confidence" : "让它更有劲" }}</span>
          </button>
        </div>
      </article>
    </div>

    <div class="pet-grid">
      <article class="surface pet-settings-panel">
        <div class="section-heading">
          <div>
            <h3>{{ locale === "en-US" ? "Desktop Behavior" : "桌面行为" }}</h3>
            <p class="section-copy">
              {{ locale === "en-US"
                ? "These settings control desktop visibility, interruption level, and companion behavior."
                : "这些设置决定宠物是否常驻桌面、是否打扰以及陪伴方式。" }}
            </p>
          </div>
        </div>

        <div class="pet-settings-form">
          <label class="toggle-row">
            <span>{{ locale === "en-US" ? "Enable desktop pet" : "启用桌面常驻" }}</span>
            <input v-model="settingsForm.desktopEnabled" type="checkbox" />
          </label>
          <label class="toggle-row">
            <span>{{ locale === "en-US" ? "Always on top" : "始终置顶" }}</span>
            <input v-model="settingsForm.alwaysOnTop" type="checkbox" />
          </label>
          <label class="toggle-row">
            <span>{{ locale === "en-US" ? "Mute proactive prompts" : "静音主动提示" }}</span>
            <input v-model="settingsForm.muted" type="checkbox" />
          </label>
          <label class="toggle-row">
            <span>{{ locale === "en-US" ? "Focus mode" : "专注模式" }}</span>
            <input v-model="settingsForm.focusModeEnabled" type="checkbox" />
          </label>
          <label class="range-row">
            <span>{{ locale === "en-US" ? "Proactive level" : "主动程度" }}</span>
            <input v-model.number="settingsForm.proactiveLevel" type="range" min="0" max="3" step="1" />
            <strong>{{ settingsForm.proactiveLevel }}</strong>
          </label>
          <div class="desktop-pet-actions">
            <button class="secondary-button desktop-pet-open-button" type="button" :disabled="extraPetOpening" @click="openAnotherDesktopPet">
              <span aria-hidden="true">+</span>
              {{ extraPetOpening ? (locale === "en-US" ? "Opening" : "正在打开") : (locale === "en-US" ? "Open Another Desktop Pet" : "多开一个桌面宠物") }}
            </button>
            <p>
              {{
                locale === "en-US"
                  ? "The toolbar switch opens one primary pet and closes all desktop pets."
                  : "顶部开关只开启一个主桌宠，关闭时会关闭全部桌宠。"
              }}
            </p>
          </div>
          <div class="button-row">
            <button class="primary-button" type="button" @click="saveSettings">
              {{ locale === "en-US" ? "Save Pet Settings" : "保存宠物设置" }}
            </button>
          </div>
        </div>
      </article>

      <article class="surface pet-cosmetics-panel">
        <div class="section-heading">
          <div>
            <h3>{{ locale === "en-US" ? "Unlocked Cosmetics" : "已解锁装扮" }}</h3>
            <p class="section-copy">
              {{ locale === "en-US"
                ? "Growth unlocks cosmetic rewards so the companion keeps feeling more personal over time."
                : "随着成长推进，宠物会逐步解锁更多可见奖励和个性元素。" }}
            </p>
          </div>
        </div>

        <div v-if="petStore.cosmetics.value.length" class="pet-cosmetic-grid">
          <article
            v-for="(label, index) in unlockedCosmeticsLabel"
            :key="petStore.cosmetics.value[index]?.id"
            class="pet-cosmetic-card"
          >
            <div class="pet-cosmetic-swatch"></div>
            <div class="pet-cosmetic-copy">
              <strong>{{ label }}</strong>
              <span>
                {{ new Date(petStore.cosmetics.value[index].unlockedAt).toLocaleDateString(locale) }}
              </span>
            </div>
          </article>
        </div>

        <div v-else class="empty-state">
          {{ locale === "en-US" ? "No cosmetics unlocked yet." : "暂时还没有解锁装扮。" }}
        </div>
      </article>
    </div>

    <article class="surface pet-ledger-panel">
      <div class="section-heading">
        <div>
          <h3>{{ locale === "en-US" ? "Recent Pet Events" : "最近宠物事件" }}</h3>
          <p class="section-copy">
            {{ locale === "en-US"
              ? "A running ledger of the moments that changed the pet's mood, growth, and presence."
              : "这里会记录最近影响宠物状态、成长和陪伴感的关键事件。" }}
          </p>
        </div>
      </div>

      <div v-if="petStore.events.value.length" class="pet-ledger-list">
        <article v-for="entry in petStore.events.value" :key="entry.id" class="pet-ledger-item">
          <div>
            <strong>{{ formatEventSource(entry.eventSource) }}</strong>
            <p>{{ formatEventMetadata(entry.eventSource, entry.metadata) }}</p>
          </div>
          <div class="pet-ledger-side">
            <span>+{{ entry.eventValue }} XP</span>
            <span>{{ new Date(entry.eventTime).toLocaleString(locale) }}</span>
          </div>
        </article>
      </div>

      <div v-else class="empty-state">
        {{ locale === "en-US" ? "No pet events yet." : "还没有宠物事件。" }}
      </div>
    </article>
  </section>
</template>

