<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { useMeetingStore } from "@/composables/useMeetingStore";
import { resolveTheme } from "@/services/appearance";
import { getMessages } from "@/services/i18n";
import type {
  LiquidGlassStyle,
  LocaleCode,
  ManagedRuntimeStatus,
  LocalAsrDevice,
  SettingsState,
  ThemeMode,
} from "@/types/meeting";

const accentColors = [
  "#8f96a3",
  "#2f6dff",
  "#a65dd9",
  "#f062a8",
  "#ff6a57",
  "#ffb020",
  "#f5dd00",
  "#33c96f",
] as const;

const store = useMeetingStore();
const saveError = ref("");
let runtimePollingId: number | null = null;

const form = reactive({
  backendUrl: "",
  apiToken: "",
  defaultHotwords: "",
  summaryTemplate: "",
  concurrency: 2,
  pythonPath: "",
  localAsrDevice: "auto" as LocalAsrDevice,
  localAsrThreads: 0,
  localAsrBatchSizeSeconds: 300,
});

const messages = computed(() => getMessages(store.settings.value.locale).settings);
const shellMessages = computed(() => getMessages(store.settings.value.locale).shell);
const commonMessages = computed(() => getMessages(store.settings.value.locale).common);
const glassPreviewThemeClass = computed(() =>
  resolveTheme(store.settings.value.themeMode) === "light"
    ? "preview-glass-light"
    : "preview-glass-dark",
);

watch(
  () => store.settings.value,
  (settings) => {
    form.backendUrl = settings.backendUrl;
    form.apiToken = settings.apiToken;
    form.defaultHotwords = settings.defaultHotwords;
    form.summaryTemplate = settings.summaryTemplate;
    form.concurrency = settings.concurrency;
    form.pythonPath = settings.pythonPath;
    form.localAsrDevice = settings.localAsrDevice;
    form.localAsrThreads = settings.localAsrThreads;
    form.localAsrBatchSizeSeconds = settings.localAsrBatchSizeSeconds;
  },
  { immediate: true, deep: true },
);

const runtimeModeLabel = computed(() => {
  if (store.localMode.value) {
    return shellMessages.value.localMode;
  }

  if (store.settings.value.backendUrl) {
    return shellMessages.value.remoteMode;
  }

  return shellMessages.value.mockModeShort;
});

const runtimeStatus = computed(() => store.runtimeStatus.value);
const runtimeInstallLog = computed(() => store.runtimeInstallLog.value);
const runtimeActionLabel = computed(() =>
  runtimeStatus.value.status === "ready"
  || runtimeStatus.value.status === "failed"
  || runtimeStatus.value.status === "repair_required"
    ? messages.value.runtimeReinstallAction
    : messages.value.runtimeInstallAction,
);
const runtimeStatusLabel = computed(() => labelForRuntimeStatus(runtimeStatus.value));
const runtimeStatusDescription = computed(() => {
  if (runtimeStatus.value.lastError?.trim()) {
    return runtimeStatus.value.lastError.trim();
  }

  switch (runtimeStatus.value.status) {
    case "ready":
      return messages.value.runtimeDescriptionReady;
    case "installing":
      return messages.value.runtimeDescriptionInstalling;
    case "failed":
    case "repair_required":
      return messages.value.runtimeDescriptionFailed;
    default:
      return messages.value.runtimeDescriptionMissing;
  }
});
const runtimeBusy = computed(() => runtimeStatus.value.status === "installing");
const runtimeInstalledAtLabel = computed(() => formatRuntimeDate(runtimeStatus.value.installedAt));

function createNextSettings(patch: Partial<SettingsState> = {}): SettingsState {
  return {
    ...store.settings.value,
    backendUrl: form.backendUrl,
    apiToken: form.apiToken,
    defaultHotwords: form.defaultHotwords,
    summaryTemplate: form.summaryTemplate,
    concurrency: form.concurrency,
    pythonPath: form.pythonPath,
    localAsrDevice: form.localAsrDevice,
    localAsrThreads: form.localAsrThreads,
    localAsrBatchSizeSeconds: form.localAsrBatchSizeSeconds,
    ...patch,
  };
}

async function saveAppearance(patch: Partial<SettingsState>) {
  saveError.value = "";

  try {
    await store.saveSettings(createNextSettings(patch));
  } catch (error) {
    saveError.value = error instanceof Error ? error.message : String(error);
  }
}

async function setThemeMode(mode: ThemeMode) {
  if (store.settings.value.themeMode === mode) {
    return;
  }

  await saveAppearance({ themeMode: mode });
}

async function setGlassStyle(style: LiquidGlassStyle) {
  if (store.settings.value.liquidGlassStyle === style) {
    return;
  }

  await saveAppearance({ liquidGlassStyle: style });
}

async function setLocale(locale: LocaleCode) {
  if (store.settings.value.locale === locale) {
    return;
  }

  await saveAppearance({ locale });
}

async function setAccentColor(color: string) {
  if (store.settings.value.accentColor.toLowerCase() === color) {
    return;
  }

  await saveAppearance({ accentColor: color });
}

async function save() {
  saveError.value = "";

  try {
    await store.saveSettings(
      createNextSettings({
        backendUrl: form.backendUrl.trim(),
        apiToken: form.apiToken.trim(),
        defaultHotwords: form.defaultHotwords.trim(),
        summaryTemplate: form.summaryTemplate.trim(),
        concurrency: Number(form.concurrency) || 1,
        pythonPath: form.pythonPath.trim(),
        localAsrDevice: form.localAsrDevice,
        localAsrThreads: Math.max(0, Number(form.localAsrThreads) || 0),
        localAsrBatchSizeSeconds: Math.max(30, Number(form.localAsrBatchSizeSeconds) || 300),
      }),
    );
  } catch (error) {
    saveError.value = error instanceof Error ? error.message : String(error);
  }
}

async function refreshRuntimePanel() {
  await store.refreshRuntimeStatus();
  await store.refreshRuntimeInstallLog();
}

async function installManagedRuntime() {
  saveError.value = "";

  try {
    await store.installManagedRuntime();
    await refreshRuntimePanel();
  } catch (error) {
    saveError.value = error instanceof Error ? error.message : String(error);
  }
}

function labelForRuntimeStatus(status: ManagedRuntimeStatus) {
  switch (status.status) {
    case "installing":
      return messages.value.runtimeStatusInstalling;
    case "ready":
      return messages.value.runtimeStatusReady;
    case "failed":
      return messages.value.runtimeStatusFailed;
    case "repair_required":
      return messages.value.runtimeStatusRepair;
    default:
      return messages.value.runtimeStatusMissing;
  }
}

function formatRuntimeDate(value?: string) {
  const normalized = value?.trim();
  if (!normalized) {
    return commonMessages.value.dash;
  }

  const fromMillis = Number(normalized);
  const date = Number.isFinite(fromMillis) && fromMillis > 0 ? new Date(fromMillis) : new Date(normalized);
  return Number.isNaN(date.getTime()) ? normalized : date.toLocaleString(store.settings.value.locale);
}

onMounted(() => {
  void refreshRuntimePanel();

  runtimePollingId = window.setInterval(() => {
    void refreshRuntimePanel();
  }, 1500);
});

onBeforeUnmount(() => {
  if (runtimePollingId !== null) {
    window.clearInterval(runtimePollingId);
    runtimePollingId = null;
  }
});
</script>

<template>
  <section class="settings-page">
    <div class="settings-group">
      <h3 class="settings-group-title">{{ messages.appearance }}</h3>
      <article class="surface settings-block">
        <div class="setting-row">
          <div class="settings-meta">
            <span class="settings-label">{{ messages.themeMode }}</span>
          </div>
          <div class="setting-control">
            <div class="preview-grid preview-grid-3">
              <button
                class="preview-card"
                :class="{ active: store.settings.value.themeMode === 'auto' }"
                type="button"
                @click="setThemeMode('auto')"
              >
                <span class="preview-art preview-theme preview-theme-auto"></span>
                <span class="preview-label">{{ messages.auto }}</span>
              </button>
              <button
                class="preview-card"
                :class="{ active: store.settings.value.themeMode === 'light' }"
                type="button"
                @click="setThemeMode('light')"
              >
                <span class="preview-art preview-theme preview-theme-light"></span>
                <span class="preview-label">{{ messages.light }}</span>
              </button>
              <button
                class="preview-card"
                :class="{ active: store.settings.value.themeMode === 'dark' }"
                type="button"
                @click="setThemeMode('dark')"
              >
                <span class="preview-art preview-theme preview-theme-dark"></span>
                <span class="preview-label">{{ messages.dark }}</span>
              </button>
            </div>
          </div>
        </div>

        <div class="setting-row">
          <div class="settings-meta">
            <span class="settings-label">{{ messages.glassStyle }}</span>
            <p class="settings-hint">{{ messages.glassStyleHint }}</p>
          </div>
          <div class="setting-control">
            <div class="preview-grid preview-grid-2">
              <button
                class="preview-card"
                :class="{ active: store.settings.value.liquidGlassStyle === 'transparent' }"
                type="button"
                @click="setGlassStyle('transparent')"
              >
                <span
                  class="preview-art preview-glass preview-glass-transparent"
                  :class="glassPreviewThemeClass"
                ></span>
                <span class="preview-label">{{ messages.transparent }}</span>
              </button>
              <button
                class="preview-card"
                :class="{ active: store.settings.value.liquidGlassStyle === 'tinted' }"
                type="button"
                @click="setGlassStyle('tinted')"
              >
                <span
                  class="preview-art preview-glass preview-glass-tinted"
                  :class="glassPreviewThemeClass"
                ></span>
                <span class="preview-label">{{ messages.tinted }}</span>
              </button>
            </div>
          </div>
        </div>

        <div class="setting-row">
          <div class="settings-meta">
            <span class="settings-label">{{ messages.locale }}</span>
          </div>
          <div class="setting-control setting-control-inline">
            <select
              :value="store.settings.value.locale"
              @change="setLocale(($event.target as HTMLSelectElement).value as LocaleCode)"
            >
              <option value="zh-CN">{{ messages.localeZh }}</option>
              <option value="en-US">{{ messages.localeEn }}</option>
            </select>
          </div>
        </div>
      </article>
    </div>

    <div class="settings-group settings-group-accent">
      <h3 class="settings-group-title">{{ messages.themeSection }}</h3>
      <article class="surface settings-block">
        <div class="setting-row setting-row-color">
          <div class="settings-meta">
            <span class="settings-label">{{ messages.accentColor }}</span>
          </div>
          <div class="setting-control">
            <div class="color-row">
              <div
                v-for="color in accentColors"
                :key="color"
                class="color-option"
              >
                <button
                  class="color-dot"
                  :class="{ active: store.settings.value.accentColor.toLowerCase() === color }"
                  :style="{ background: color }"
                  type="button"
                  :title="messages.colorLabels[color]"
                  @click="setAccentColor(color)"
                ></button>
                <span
                  v-if="store.settings.value.accentColor.toLowerCase() === color"
                  class="color-option-label"
                >
                  {{ messages.colorLabels[color] }}
                </span>
              </div>
            </div>
          </div>
        </div>
      </article>
    </div>

    <div class="settings-group">
      <h3 class="settings-group-title">{{ messages.runtimeOverview }}</h3>
      <article class="surface settings-block">
        <div class="setting-row">
          <div class="settings-meta">
            <span class="settings-label">{{ messages.runtimeMode }}</span>
            <p class="settings-hint">{{ messages.runtimeModeHint }}</p>
          </div>
          <div class="setting-control">
            <div class="summary-inline">
              <span>{{ runtimeModeLabel }}</span>
              <span>{{ store.localMode.value ? messages.localDatabaseReady : messages.waitingLocalConfig }}</span>
            </div>
          </div>
        </div>
      </article>
    </div>

    <div class="settings-group">
      <h3 class="settings-group-title">{{ messages.localRuntime }}</h3>
      <article class="surface settings-block runtime-card">
        <div class="runtime-card-head">
          <div class="runtime-card-title-wrap">
            <span class="runtime-card-title">{{ messages.managedRuntime }}</span>
            <p class="runtime-card-hint">{{ messages.managedRuntimeHint }}</p>
          </div>
          <div class="runtime-card-status">
            <span class="runtime-status-label">{{ messages.runtimeStatus }}</span>
            <span
              class="runtime-status-badge"
              :class="`runtime-status-${runtimeStatus.status}`"
            >
              {{ runtimeStatusLabel }}
            </span>
          </div>
        </div>

        <div class="runtime-panel">
          <div class="runtime-hero">
            <p class="runtime-status-text">{{ runtimeStatusDescription }}</p>
            <button
              class="text-button runtime-primary-action"
              type="button"
              :disabled="runtimeBusy"
              @click="installManagedRuntime"
            >
              {{ runtimeBusy ? messages.runtimeStatusInstalling : runtimeActionLabel }}
            </button>
          </div>

          <div class="runtime-meta-grid">
            <div class="runtime-meta-item">
              <span>{{ messages.runtimeVersion }}</span>
              <strong>{{ runtimeStatus.runtimeVersion || "—" }}</strong>
            </div>
            <div class="runtime-meta-item">
              <span>{{ messages.runtimePythonVersion }}</span>
              <strong>{{ runtimeStatus.pythonVersion || "—" }}</strong>
            </div>
            <div class="runtime-meta-item">
              <span>{{ messages.runtimeInstalledAt }}</span>
              <strong>{{ runtimeInstalledAtLabel }}</strong>
            </div>
          </div>

          <div class="runtime-log">
            <span class="runtime-log-title">{{ messages.runtimeInstallLog }}</span>
            <pre>{{ runtimeInstallLog || messages.runtimeInstallLogEmpty }}</pre>
          </div>
        </div>
      </article>

      <article class="surface settings-block">
        <div class="setting-row">
          <div class="settings-meta">
            <span class="settings-label">{{ messages.manualPythonOverride }}</span>
            <p class="settings-hint">{{ messages.manualPythonOverrideHint }}</p>
          </div>
          <div class="setting-control">
            <input
              id="python-path"
              v-model="form.pythonPath"
              :placeholder="messages.pythonPathPlaceholder"
              @blur="save"
            />
          </div>
        </div>

        <div class="setting-row">
          <div class="settings-meta">
            <span class="settings-label">{{ messages.localAsrDevice }}</span>
            <p class="settings-hint">{{ messages.localAsrDeviceHint }}</p>
          </div>
          <div class="setting-control setting-control-inline">
            <select v-model="form.localAsrDevice" @change="save">
              <option value="auto">{{ messages.localAsrDeviceAuto }}</option>
              <option value="cpu">{{ messages.localAsrDeviceCpu }}</option>
              <option value="mps">{{ messages.localAsrDeviceMps }}</option>
              <option value="cuda">{{ messages.localAsrDeviceCuda }}</option>
            </select>
          </div>
        </div>
      </article>
    </div>

    <div class="settings-group">
      <h3 class="settings-group-title">{{ messages.processingDefaults }}</h3>
      <article class="surface settings-block">
        <div class="setting-row">
          <div class="settings-meta">
            <span class="settings-label">{{ messages.defaultHotwords }}</span>
            <p class="settings-hint">{{ messages.defaultHotwordsHint }}</p>
          </div>
          <div class="setting-control">
            <textarea
              id="default-hotwords"
              v-model="form.defaultHotwords"
              :placeholder="messages.defaultHotwordsPlaceholder"
              @blur="save"
            />
          </div>
        </div>

        <div class="setting-row">
          <div class="settings-meta">
            <span class="settings-label">{{ messages.defaultSummaryTemplate }}</span>
            <p class="settings-hint">{{ messages.defaultSummaryTemplateHint }}</p>
          </div>
          <div class="setting-control">
            <input
              id="summary-template"
              v-model="form.summaryTemplate"
              :placeholder="messages.defaultSummaryTemplatePlaceholder"
              @blur="save"
            />
          </div>
        </div>

        <div class="setting-row">
          <div class="settings-meta">
            <span class="settings-label">{{ messages.concurrency }}</span>
            <p class="settings-hint">{{ messages.concurrencyHint }}</p>
          </div>
          <div class="setting-control setting-control-inline">
            <input
              id="concurrency"
              v-model.number="form.concurrency"
              type="number"
              min="1"
              max="8"
              @blur="save"
            />
          </div>
        </div>

        <div class="setting-row">
          <div class="settings-meta">
            <span class="settings-label">{{ messages.localAsrThreads }}</span>
            <p class="settings-hint">{{ messages.localAsrThreadsHint }}</p>
          </div>
          <div class="setting-control setting-control-inline">
            <input
              id="local-asr-threads"
              v-model.number="form.localAsrThreads"
              type="number"
              min="0"
              max="32"
              @blur="save"
            />
          </div>
        </div>

        <div class="setting-row">
          <div class="settings-meta">
            <span class="settings-label">{{ messages.localAsrBatchSizeSeconds }}</span>
            <p class="settings-hint">{{ messages.localAsrBatchSizeSecondsHint }}</p>
          </div>
          <div class="setting-control setting-control-inline">
            <input
              id="local-asr-batch-size-seconds"
              v-model.number="form.localAsrBatchSizeSeconds"
              type="number"
              min="30"
              max="1200"
              step="30"
              @blur="save"
            />
          </div>
        </div>
      </article>
    </div>

    <div class="settings-group">
      <h3 class="settings-group-title">{{ messages.remoteCompatibility }}</h3>
      <article class="surface settings-block">
        <div class="setting-row">
          <div class="settings-meta">
            <span class="settings-label">{{ messages.backendUrl }}</span>
            <p class="settings-hint">{{ messages.backendUrlHint }}</p>
          </div>
          <div class="setting-control">
            <input
              id="backend-url"
              v-model="form.backendUrl"
              :placeholder="messages.backendUrlPlaceholder"
              @blur="save"
            />
          </div>
        </div>

        <div class="setting-row">
          <div class="settings-meta">
            <span class="settings-label">{{ messages.apiToken }}</span>
            <p class="settings-hint">{{ messages.apiTokenHint }}</p>
          </div>
          <div class="setting-control">
            <input
              id="api-token"
              v-model="form.apiToken"
              type="password"
              :placeholder="messages.apiTokenPlaceholder"
              autocomplete="off"
              @blur="save"
            />
          </div>
        </div>
      </article>
    </div>

    <p v-if="saveError" class="settings-error">{{ saveError }}</p>

    <footer class="settings-footer">
      <p>{{ messages.copyright }}</p>
      <p>
        {{ messages.authorGithub }}
        <a href="https://github.com/westng/Liberty" target="_blank" rel="noreferrer">github.com/westng/Liberty</a>
      </p>
    </footer>
  </section>
</template>

<style scoped>
.runtime-panel {
  display: grid;
  gap: 18px;
}

.runtime-card {
  display: grid;
  gap: 22px;
  padding: 18px 18px 16px;
}

.runtime-card::after {
  display: none;
}

.runtime-card-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 24px;
  padding-bottom: 16px;
  border-bottom: 1px solid var(--divider-soft);
}

.runtime-card-title-wrap {
  display: grid;
  gap: 6px;
  max-width: 760px;
}

.runtime-card-title {
  font-size: 1.02rem;
  font-weight: 700;
  color: var(--text-main);
  line-height: 1.2;
}

.runtime-card-hint {
  margin: 0;
  color: var(--text-muted);
  line-height: 1.55;
}

.runtime-card-status {
  display: grid;
  justify-items: end;
  gap: 8px;
  flex-shrink: 0;
}

.runtime-status-label {
  color: var(--text-secondary);
  font-size: 13px;
}

.runtime-status-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 84px;
  padding: 6px 12px;
  border-radius: 999px;
  font-size: 12px;
  font-weight: 600;
  background: rgba(255, 255, 255, 0.08);
}

.runtime-hero {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
}

.runtime-status-ready {
  color: rgb(31, 140, 84);
  background: rgba(41, 171, 107, 0.14);
}

.runtime-status-installing {
  color: rgb(191, 119, 9);
  background: rgba(255, 176, 32, 0.16);
}

.runtime-status-failed,
.runtime-status-repair_required {
  color: rgb(191, 60, 60);
  background: rgba(255, 106, 87, 0.16);
}

.runtime-status-text {
  margin: 0;
  color: var(--text-secondary);
  line-height: 1.6;
  max-width: 760px;
  font-size: 14px;
}

.runtime-meta-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
}

.runtime-meta-item {
  display: grid;
  gap: 8px;
  padding: 15px 16px;
  border-radius: 18px;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.56), rgba(255, 255, 255, 0.28)),
    rgba(15, 23, 42, 0.03);
  border: 1px solid rgba(15, 23, 42, 0.05);
}

.runtime-meta-item span {
  color: var(--text-secondary);
  font-size: 12px;
  font-weight: 600;
}

.runtime-meta-item strong {
  font-size: 15px;
  font-weight: 600;
  line-height: 1.35;
}

.runtime-primary-action {
  flex-shrink: 0;
  white-space: nowrap;
}

.runtime-primary-action.text-button[disabled] {
  opacity: 0.5;
  cursor: not-allowed;
}

.runtime-log {
  display: grid;
  gap: 10px;
}

.runtime-log-title {
  font-size: 0.95rem;
  font-weight: 700;
  color: var(--text-main);
}

.runtime-log pre {
  margin: 0;
  max-height: 220px;
  overflow: auto;
  padding: 14px 16px;
  border-radius: 16px;
  background: rgba(15, 23, 42, 0.05);
  border: 1px solid rgba(15, 23, 42, 0.06);
  color: var(--text-secondary);
  font-size: 12px;
  line-height: 1.55;
  white-space: pre-wrap;
  word-break: break-word;
}

@media (max-width: 980px) {
  .runtime-card-head {
    flex-direction: column;
    align-items: stretch;
  }

  .runtime-card-status {
    justify-content: space-between;
    justify-items: stretch;
  }

  .runtime-hero {
    flex-direction: column;
    align-items: stretch;
  }

  .runtime-meta-grid {
    grid-template-columns: 1fr;
  }
}
</style>
