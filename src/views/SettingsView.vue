<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { useMeetingStore } from "@/composables/useMeetingStore";
import { resolveTheme } from "@/services/appearance";
import { getMessages } from "@/services/i18n";
import type {
  LiquidGlassStyle,
  LocaleCode,
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
      <article class="surface settings-block">
        <div class="setting-row">
          <div class="settings-meta">
            <span class="settings-label">{{ messages.pythonPath }}</span>
            <p class="settings-hint">{{ messages.pythonPathHint }}</p>
          </div>
          <div class="setting-control">
            <input
              id="python-path"
              v-model="form.pythonPath"
              placeholder="例如：/opt/homebrew/bin/python3 或 C:\\Python311\\python.exe"
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
              placeholder="使用英文逗号分隔，例如：SeACo-Paraformer, FunASR, 会议纪要"
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
              placeholder="例如：默认会议纪要模板"
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
              placeholder="例如：http://127.0.0.1:8000"
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
              placeholder="可选"
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
