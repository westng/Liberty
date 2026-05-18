import { computed, reactive, ref, watch } from "vue";
import { resolveTheme } from "@/shared/services/ui/appearance";
import type {
  LiquidGlassStyle,
  LocaleCode,
  LocalAsrDevice,
  SettingsState,
  ThemeMode,
} from "@/shared/types/meeting";
import type { MeetingStore } from "@/features/meeting/stores/useMeetingStore";

export const accentColors = [
  "#8f96a3",
  "#2f6dff",
  "#a65dd9",
  "#f062a8",
  "#ff6a57",
  "#ffb020",
  "#f5dd00",
  "#33c96f",
] as const;

export function useSettingsForm(store: MeetingStore) {
  const saveError = ref("");
  const form = reactive({
    backendUrl: "",
    apiToken: "",
    defaultHotwords: "",
    summaryTemplate: "",
    concurrency: 2,
    localAsrDevice: "auto" as LocalAsrDevice,
    localAsrThreads: 0,
    localAsrBatchSizeSeconds: 300,
  });

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
      form.localAsrDevice = settings.localAsrDevice;
      form.localAsrThreads = settings.localAsrThreads;
      form.localAsrBatchSizeSeconds = settings.localAsrBatchSizeSeconds;
    },
    { immediate: true, deep: true },
  );

  function createNextSettings(patch: Partial<SettingsState> = {}): SettingsState {
    return {
      ...store.settings.value,
      backendUrl: form.backendUrl,
      apiToken: form.apiToken,
      defaultHotwords: form.defaultHotwords,
      summaryTemplate: form.summaryTemplate,
      concurrency: form.concurrency,
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
          localAsrDevice: form.localAsrDevice,
          localAsrThreads: Math.max(0, Number(form.localAsrThreads) || 0),
          localAsrBatchSizeSeconds: Math.max(30, Number(form.localAsrBatchSizeSeconds) || 300),
        }),
      );
    } catch (error) {
      saveError.value = error instanceof Error ? error.message : String(error);
    }
  }

  return {
    form,
    saveError,
    glassPreviewThemeClass,
    setThemeMode,
    setGlassStyle,
    setLocale,
    setAccentColor,
    save,
  };
}
