<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { RouterLink, RouterView, useRoute, useRouter } from "vue-router";
import { getCurrentWindow } from "@tauri-apps/api/window";
import sidebarMascotUrl from "@/assets/sidebar-mascot.webp";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { usePetStore } from "@/features/pet/stores/usePetStore";
import { formatMessage, getMessages } from "@/shared/i18n";
import { applyDesktopPetState } from "@/shared/services/tauri/pet";
import { getProcessMetrics, openExternalUrl } from "@/shared/services/tauri/system";
import type { ProcessMetrics } from "@/shared/types/meeting";

const route = useRoute();
const router = useRouter();
const store = useMeetingStore();
const petStore = usePetStore();
const routeHistory = ref<string[]>([]);
const routeHistoryIndex = ref(-1);
const knownJobStatuses = new Map<string, string>();
let didHydrateJobStatuses = false;
let metricsPollingId: number | null = null;
let didRecordPetDailyOpen = false;
let lastAppliedDesktopPetEnabled: boolean | null = null;
const processMetrics = ref<ProcessMetrics>({
  cpuPercent: 0,
  memoryMb: 0,
});
const graphicsMemoryMb = ref(0);
const isWindowsTitlebar = ref(false);

const messages = computed(() => getMessages(store.settings.value.locale));
const navItems = computed(() => [
  { label: messages.value.nav.newJob, to: "/" },
  { label: messages.value.nav.jobs, to: "/jobs" },
  { label: messages.value.nav.models, to: "/models" },
  { label: messages.value.nav.templates, to: "/templates" },
  { label: messages.value.nav.members, to: "/members" },
  { label: messages.value.nav.pet, to: "/pet" },
  { label: messages.value.nav.settings, to: "/settings" },
]);
const isStandaloneRoute = computed(() => Boolean(route.meta.standalone));
const toolbarTitle = computed(() => {
  const titleKey = route.meta.titleKey;
  if (typeof titleKey === "string") {
    return messages.value.routeTitles[titleKey] ?? "Liberty";
  }

  return typeof route.meta.title === "string" ? route.meta.title : "Liberty";
});
const currentModeLabel = computed(() => {
  if (store.localMode.value) {
    return messages.value.shell.localMode;
  }

  if (store.settings.value.backendUrl) {
    return messages.value.shell.remoteMode;
  }

  return messages.value.shell.mockModeShort;
});
const desktopPetVisible = computed(() => Boolean(petStore.settings.value?.desktopEnabled));
const toolbarStatus = computed(() => {
  if (store.localMode.value) {
    return messages.value.shell.localReady;
  }

  if (store.settings.value.backendUrl) {
    return messages.value.shell.remoteReady;
  }

  return messages.value.shell.mockMode;
});
const toolbarMetrics = computed(() => [
  {
    key: "cpu",
    label: messages.value.shell.cpu,
    value: `${Math.max(0, processMetrics.value.cpuPercent).toFixed(1)}%`,
  },
  {
    key: "memory",
    label: messages.value.shell.memory,
    value: `${Math.max(0, processMetrics.value.memoryMb)}M`,
  },
  {
    key: "graphics",
    label: messages.value.shell.graphics,
    value: `${Math.max(0, graphicsMemoryMb.value)}M`,
  },
]);

const activeJobCount = computed(() =>
  store.jobs.value.filter((job) =>
    ["queued", "transcribing", "speaker_processing", "summarizing"].includes(
      job.overallStatus,
    ),
  ).length,
);

const canGoBack = computed(() => routeHistoryIndex.value > 0);
const canGoForward = computed(
  () => routeHistoryIndex.value >= 0 && routeHistoryIndex.value < routeHistory.value.length - 1,
);

watch(
  () => route.fullPath,
  (path) => {
    syncRouteHistory(path);
  },
  { immediate: true },
);

watch(
  () => store.jobs.value.map((job) => ({ id: job.id, title: job.title, status: job.overallStatus })),
  (jobs) => {
    if (!didHydrateJobStatuses) {
      knownJobStatuses.clear();
      for (const job of jobs) {
        knownJobStatuses.set(job.id, job.status);
      }
      didHydrateJobStatuses = true;
      return;
    }

    const nextStatuses = new Map<string, string>();

    for (const job of jobs) {
      const previousStatus = knownJobStatuses.get(job.id);
      nextStatuses.set(job.id, job.status);

      if (
        previousStatus
        && previousStatus !== "transcribing"
        && job.status === "transcribing"
      ) {
        void petStore.applyWorkflowEvent({
          eventType: "transcription_started",
          metadata: job.title,
        }).catch(() => undefined);
      }

      if (previousStatus && previousStatus !== "completed" && job.status === "completed") {
        void notifyJobCompleted(job.title);
      }
    }

    knownJobStatuses.clear();
    for (const [jobId, status] of nextStatuses) {
      knownJobStatuses.set(jobId, status);
    }
  },
  { deep: false },
);

onMounted(() => {
  void store.ensureSettingsLoaded();
  void initializeWindowTitlebar();
  void initializePetState();
  updateGraphicsMemoryEstimate();
  syncToolbarMetricsPolling();
  window.addEventListener("focus", syncToolbarMetricsPolling);
  window.addEventListener("blur", syncToolbarMetricsPolling);
  document.addEventListener("visibilitychange", syncToolbarMetricsPolling);
  window.addEventListener("resize", updateGraphicsMemoryEstimate);
  void refreshToolbarMetrics();
});

onBeforeUnmount(() => {
  window.removeEventListener("focus", syncToolbarMetricsPolling);
  window.removeEventListener("blur", syncToolbarMetricsPolling);
  document.removeEventListener("visibilitychange", syncToolbarMetricsPolling);
  window.removeEventListener("resize", updateGraphicsMemoryEstimate);
  if (metricsPollingId !== null) {
    window.clearInterval(metricsPollingId);
    metricsPollingId = null;
  }
});

function isActive(itemTo: string) {
  if (itemTo === "/jobs") {
    return route.path.startsWith("/jobs");
  }

  return route.path === itemTo;
}

function syncRouteHistory(path: string) {
  if (!path) {
    return;
  }

  if (routeHistoryIndex.value === -1) {
    routeHistory.value = [path];
    routeHistoryIndex.value = 0;
    return;
  }

  if (routeHistory.value[routeHistoryIndex.value] === path) {
    return;
  }

  const previousPath = routeHistory.value[routeHistoryIndex.value - 1];
  if (previousPath === path) {
    routeHistoryIndex.value -= 1;
    return;
  }

  const nextPath = routeHistory.value[routeHistoryIndex.value + 1];
  if (nextPath === path) {
    routeHistoryIndex.value += 1;
    return;
  }

  routeHistory.value = routeHistory.value.slice(0, routeHistoryIndex.value + 1);
  routeHistory.value.push(path);
  routeHistoryIndex.value = routeHistory.value.length - 1;
}

async function goBack() {
  if (!canGoBack.value) {
    return;
  }

  const target = routeHistory.value[routeHistoryIndex.value - 1];
  if (!target) {
    return;
  }

  await router.push(target);
}

async function goForward() {
  if (!canGoForward.value) {
    return;
  }

  const target = routeHistory.value[routeHistoryIndex.value + 1];
  if (!target) {
    return;
  }

  await router.push(target);
}

async function openProjectGithub() {
  await openExternalUrl("https://github.com/westng/Liberty");
}

async function initializeWindowTitlebar() {
  if (!navigator.userAgent.includes("Windows")) {
    return;
  }

  isWindowsTitlebar.value = true;
  await getCurrentWindow().setDecorations(false);
}

async function minimizeWindow() {
  await getCurrentWindow().minimize();
}

async function toggleMaximizeWindow() {
  await getCurrentWindow().toggleMaximize();
}

async function closeWindow() {
  await getCurrentWindow().close();
}

async function initializePetState() {
  try {
    await petStore.loadPetState();
    if (!didRecordPetDailyOpen) {
      didRecordPetDailyOpen = true;
      void petStore.applyWorkflowEvent({
        eventType: "daily_open",
        metadata: "Liberty app opened",
      }).catch(() => undefined);
    }
    const settings = petStore.settings.value;
    if (settings) {
      await syncDesktopPetState(settings, "startup").catch((error) => {
        console.error("[pet-window] failed to sync native pet window", error);
      });
    }
  } catch {
    // Keep the main app usable even if the pet state fails to load.
  }
}

async function syncDesktopPetState(settings: NonNullable<typeof petStore.settings.value>, source: string) {
  if (lastAppliedDesktopPetEnabled === settings.desktopEnabled) {
    return;
  }

  lastAppliedDesktopPetEnabled = settings.desktopEnabled;
  await applyDesktopPetState(settings, source);
}

async function toggleToolbarPetDesktop() {
  await petStore.loadPetState();
  const current = petStore.settings.value;
  if (!current) {
    return;
  }

  const savedSettings = await petStore.saveSettings({
    ...current,
    desktopEnabled: !current.desktopEnabled,
  });
  lastAppliedDesktopPetEnabled = null;
  await syncDesktopPetState(savedSettings, "toolbar");
}

async function notifyJobCompleted(jobTitle: string) {
  if (typeof window === "undefined" || typeof Notification === "undefined") {
    return;
  }

  let permission = Notification.permission;

  if (permission === "default") {
    permission = await Notification.requestPermission();
  }

  if (permission !== "granted") {
    return;
  }

  new Notification(messages.value.shell.jobCompletedTitle, {
    body: formatMessage(messages.value.shell.jobCompletedBody, { title: jobTitle }),
  });

  try {
    await petStore.applyWorkflowEvent({
      eventType: "transcription_completed",
      metadata: jobTitle,
    });
  } catch {
    // Pet updates are best-effort only.
  }
}

async function refreshToolbarMetrics() {
  try {
    processMetrics.value = await getProcessMetrics();
  } catch {
    // Keep the last known metrics when polling fails.
  }
  updateGraphicsMemoryEstimate();
}

function syncToolbarMetricsPolling() {
  if (typeof window === "undefined") {
    return;
  }

  const shouldPoll = document.visibilityState !== "hidden" && document.hasFocus();

  if (shouldPoll && metricsPollingId === null) {
    metricsPollingId = window.setInterval(() => {
      void refreshToolbarMetrics();
    }, 10000);
    return;
  }

  if (!shouldPoll && metricsPollingId !== null) {
    window.clearInterval(metricsPollingId);
    metricsPollingId = null;
  }
}

function updateGraphicsMemoryEstimate() {
  const width = window.innerWidth || 0;
  const height = window.innerHeight || 0;
  const pixelRatio = Math.max(window.devicePixelRatio || 1, 1);
  const bytes = width * height * 4 * pixelRatio * pixelRatio;
  graphicsMemoryMb.value = Math.max(1, Math.round(bytes / (1024 * 1024)));
}
</script>

<template>
  <RouterView v-if="isStandaloneRoute" />

  <div v-else class="app-shell">
    <div class="window-titlebar" :class="{ 'windows-titlebar': isWindowsTitlebar }">
      <div class="window-titlebar-metrics">
        <div v-for="metric in toolbarMetrics" :key="metric.key" class="toolbar-metric titlebar-metric">
          <span class="toolbar-metric-icon" aria-hidden="true">
            <svg v-if="metric.key === 'cpu'" viewBox="0 0 24 24">
              <path fill="currentColor" d="M9 2h6v2h2.5A2.5 2.5 0 0 1 20 6.5V9h2v6h-2v2.5A2.5 2.5 0 0 1 17.5 20H15v2H9v-2H6.5A2.5 2.5 0 0 1 4 17.5V15H2V9h2V6.5A2.5 2.5 0 0 1 6.5 4H9V2Zm-2.5 4a.5.5 0 0 0-.5.5v11a.5.5 0 0 0 .5.5h11a.5.5 0 0 0 .5-.5v-11a.5.5 0 0 0-.5-.5h-11ZM8 8h8v8H8V8Zm2 2v4h4v-4h-4Z" />
            </svg>
            <svg v-else-if="metric.key === 'memory'" viewBox="0 0 24 24">
              <path fill="currentColor" d="M4 7a3 3 0 0 1 3-3h10a3 3 0 0 1 3 3v10a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3V7Zm3-1a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h1v-2h2v2h4v-2h2v2h1a1 1 0 0 0 1-1V7a1 1 0 0 0-1-1h-1v2h-2V6h-4v2H8V6H7Zm1 4h8v4H8v-4Z" />
            </svg>
            <svg v-else viewBox="0 0 24 24">
              <path fill="currentColor" d="M3 5h18a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1h-7v2h3v2H7v-2h3v-2H3a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1Zm1 2v7h16V7H4Z" />
            </svg>
          </span>
          <span class="toolbar-metric-label">{{ metric.label }}</span>
          <strong class="toolbar-metric-value">{{ metric.value }}</strong>
        </div>
      </div>
      <div class="window-titlebar-spacer" data-tauri-drag-region></div>
      <div class="window-titlebar-actions">
        <button
          class="toolbar-pet-toggle titlebar-pet-toggle"
          type="button"
          :aria-pressed="desktopPetVisible"
          :title="desktopPetVisible ? '关闭桌面宠物' : '打开桌面宠物'"
          @click="toggleToolbarPetDesktop"
        >
          <span class="toolbar-pet-dot" :data-active="desktopPetVisible"></span>
          {{ desktopPetVisible ? "桌宠已开" : "桌宠已关" }}
        </button>
        <div class="toolbar-pill titlebar-pill">
          <span class="toolbar-pill-dot"></span>
          {{ toolbarStatus }}
        </div>
        <button
          class="toolbar-icon-btn titlebar-icon-btn"
          type="button"
          :aria-label="messages.shell.github"
          :title="messages.shell.github"
          @click="openProjectGithub"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path
              fill="currentColor"
              d="M12 2C6.48 2 2 6.59 2 12.25c0 4.53 2.87 8.37 6.84 9.72.5.1.68-.22.68-.49 0-.24-.01-1.04-.01-1.88-2.78.62-3.37-1.22-3.37-1.22-.45-1.19-1.11-1.5-1.11-1.5-.91-.64.07-.63.07-.63 1 .07 1.53 1.06 1.53 1.06.9 1.57 2.35 1.12 2.92.86.09-.67.35-1.12.63-1.38-2.22-.26-4.55-1.15-4.55-5.14 0-1.14.39-2.08 1.03-2.82-.1-.26-.45-1.3.1-2.72 0 0 .84-.28 2.75 1.08A9.3 9.3 0 0 1 12 6.84c.85 0 1.71.12 2.51.37 1.91-1.36 2.75-1.08 2.75-1.08.55 1.42.2 2.46.1 2.72.64.74 1.03 1.68 1.03 2.82 0 4-2.33 4.87-4.56 5.13.36.32.68.95.68 1.92 0 1.39-.01 2.5-.01 2.84 0 .27.18.6.69.49A10.25 10.25 0 0 0 22 12.25C22 6.59 17.52 2 12 2Z"
            />
          </svg>
        </button>
      </div>
      <div v-if="isWindowsTitlebar" class="window-controls">
        <button class="window-control" type="button" aria-label="Minimize" @click="minimizeWindow">
          −
        </button>
        <button class="window-control" type="button" aria-label="Maximize" @click="toggleMaximizeWindow">
          □
        </button>
        <button class="window-control window-control-close" type="button" aria-label="Close" @click="closeWindow">
          ×
        </button>
      </div>
    </div>

    <aside class="sidebar">
      <div class="nav-wrap">
        <header class="nav-header">
          <div class="nav-brand">
            <img class="nav-brand-image" :src="sidebarMascotUrl" alt="Liberty mascot" />
            <h1>Liberty</h1>
          </div>
          <p class="nav-slogan">{{ messages.shell.slogan }}</p>
        </header>

        <nav class="nav-list">
          <RouterLink
            v-for="item in navItems"
            :key="item.to"
            :to="item.to"
            class="nav-link"
            :class="{ active: isActive(item.to) }"
          >
            {{ item.label }}
          </RouterLink>
        </nav>

        <div class="nav-footer">
          <div class="nav-footer-item">
            <span>{{ messages.shell.modeLabel }}</span>
            <strong>{{ currentModeLabel }}</strong>
          </div>
          <div class="nav-footer-item">
            <span>{{ messages.shell.processingLabel }}</span>
            <strong>{{ activeJobCount }}</strong>
          </div>
        </div>
      </div>
    </aside>

    <main class="content">
      <section class="content-page">
        <div class="content-shell">
          <header class="content-toolbar">
            <div class="toolbar-nav">
              <button
                class="toolbar-nav-btn"
                type="button"
                :disabled="!canGoBack"
                :aria-label="messages.shell.back"
                @click="goBack"
              >
                ‹
              </button>
              <button
                class="toolbar-nav-btn"
                type="button"
                :disabled="!canGoForward"
                :aria-label="messages.shell.forward"
                @click="goForward"
              >
                ›
              </button>
            </div>

            <div class="toolbar-copy">
              <h2 class="toolbar-title">{{ toolbarTitle }}</h2>
            </div>
          </header>

          <section class="content-body">
            <RouterView />
          </section>
        </div>
      </section>
    </main>
  </div>
</template>
