import { useEffect, useMemo, useState } from "react";
import { applyAppearance, resolveTheme } from "@/shared/services/ui/appearance";
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

type SettingsFormState = {
  backendUrl: string;
  apiToken: string;
  defaultHotwords: string;
  summaryTemplate: string;
  concurrency: number;
  localAsrDevice: LocalAsrDevice;
  localAsrThreads: number;
  localAsrBatchSizeSeconds: number;
};

function formFromSettings(settings: SettingsState): SettingsFormState {
  return {
    backendUrl: settings.backendUrl,
    apiToken: settings.apiToken,
    defaultHotwords: settings.defaultHotwords,
    summaryTemplate: settings.summaryTemplate,
    concurrency: settings.concurrency,
    localAsrDevice: settings.localAsrDevice,
    localAsrThreads: settings.localAsrThreads,
    localAsrBatchSizeSeconds: settings.localAsrBatchSizeSeconds,
  };
}

export function useSettingsForm(store: MeetingStore) {
  const [saveError, setSaveError] = useState("");
  const [form, setForm] = useState<SettingsFormState>(() => formFromSettings(store.settings));
  const effectiveTheme = resolveTheme(store.settings.themeMode);
  const glassPreviewThemeClass = useMemo(
    () => (effectiveTheme === "light" ? "preview-glass-light" : "preview-glass-dark"),
    [effectiveTheme],
  );

  useEffect(() => {
    setForm(formFromSettings(store.settings));
  }, [store.settings]);

  function patchForm(patch: Partial<SettingsFormState>) {
    setForm((current) => ({ ...current, ...patch }));
  }

  function createNextSettings(patch: Partial<SettingsState> = {}): SettingsState {
    return {
      ...store.settings,
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
    setSaveError("");
    const nextSettings = createNextSettings(patch);
    applyAppearance(nextSettings);

    try {
      await store.saveSettings(nextSettings);
    } catch (error) {
      applyAppearance(store.settings);
      setSaveError(error instanceof Error ? error.message : String(error));
    }
  }

  async function setThemeMode(mode: ThemeMode) {
    if (store.settings.themeMode === mode) {
      return;
    }

    await saveAppearance({ themeMode: mode });
  }

  async function setGlassStyle(style: LiquidGlassStyle) {
    if (store.settings.liquidGlassStyle === style) {
      return;
    }

    await saveAppearance({ liquidGlassStyle: style });
  }

  async function setLocale(locale: LocaleCode) {
    if (store.settings.locale === locale) {
      return;
    }

    await saveAppearance({ locale });
  }

  async function setAccentColor(color: string) {
    if (store.settings.accentColor.toLowerCase() === color) {
      return;
    }

    await saveAppearance({ accentColor: color });
  }

  async function save() {
    setSaveError("");

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
      setSaveError(error instanceof Error ? error.message : String(error));
    }
  }

  return {
    form,
    patchForm,
    saveError,
    setSaveError,
    effectiveTheme,
    glassPreviewThemeClass,
    setThemeMode,
    setGlassStyle,
    setLocale,
    setAccentColor,
    save,
  };
}
