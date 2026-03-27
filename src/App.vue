<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { RouterLink, RouterView, useRoute, useRouter } from "vue-router";
import sidebarMascotUrl from "@/assets/sidebar-mascot.webp";
import { useMeetingStore } from "@/composables/useMeetingStore";
import { formatMessage, getMessages } from "@/services/i18n";
import { openExternalUrl } from "@/services/system";

const route = useRoute();
const router = useRouter();
const store = useMeetingStore();
const routeHistory = ref<string[]>([]);
const routeHistoryIndex = ref(-1);
const knownJobStatuses = new Map<string, string>();
let didHydrateJobStatuses = false;

const messages = computed(() => getMessages(store.settings.value.locale));
const navItems = computed(() => [
  { label: messages.value.nav.newJob, to: "/" },
  { label: messages.value.nav.jobs, to: "/jobs" },
  { label: messages.value.nav.models, to: "/models" },
  { label: messages.value.nav.templates, to: "/templates" },
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
const toolbarStatus = computed(() => {
  if (store.localMode.value) {
    return messages.value.shell.localReady;
  }

  if (store.settings.value.backendUrl) {
    return messages.value.shell.remoteReady;
  }

  return messages.value.shell.mockMode;
});

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
}
</script>

<template>
  <RouterView v-if="isStandaloneRoute" />

  <div v-else class="app-shell">
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

            <div class="toolbar-actions">
            <div class="toolbar-pill">
              <span class="toolbar-pill-dot"></span>
              {{ toolbarStatus }}
            </div>
            <button
              class="toolbar-icon-btn"
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
        </header>

          <section class="content-body">
            <RouterView />
          </section>
        </div>
      </section>
    </main>
  </div>
</template>
