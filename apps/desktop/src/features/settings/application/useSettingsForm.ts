import { useEffect, useMemo, useRef, useState } from "react";
import { applyAppearance, resolveTheme } from "@/shared/services/ui/appearance";
import type {
  LiquidGlassStyle,
  LocaleCode,
  LocalAsrDevice,
  ProcessingMode,
  SettingsCredentialUpdate,
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
  processingMode: ProcessingMode;
  backendUrl: string;
  apiToken: string;
  defaultHotwords: string;
  summaryTemplate: string;
  concurrency: number;
  localAsrDevice: LocalAsrDevice;
  localAsrThreads: number;
  localAsrBatchSizeSeconds: number;
};

type SettingsFormKey = keyof SettingsFormState;
const settingsFormKeys = [
  "processingMode",
  "backendUrl",
  "apiToken",
  "defaultHotwords",
  "summaryTemplate",
  "concurrency",
  "localAsrDevice",
  "localAsrThreads",
  "localAsrBatchSizeSeconds",
] as const satisfies readonly SettingsFormKey[];

function formFromSettings(settings: SettingsState): SettingsFormState {
  return {
    processingMode: settings.processingMode,
    backendUrl: settings.backendUrl,
    apiToken: "",
    defaultHotwords: settings.defaultHotwords,
    summaryTemplate: settings.summaryTemplate,
    concurrency: settings.concurrency,
    localAsrDevice: settings.localAsrDevice,
    localAsrThreads: settings.localAsrThreads,
    localAsrBatchSizeSeconds: settings.localAsrBatchSizeSeconds,
  };
}

function settingsFromForm(settings: SettingsState, form: SettingsFormState): SettingsState {
  return {
    ...settings,
    processingMode: form.processingMode,
    backendUrl: form.backendUrl.trim(),
    defaultHotwords: form.defaultHotwords.trim(),
    summaryTemplate: form.summaryTemplate.trim(),
    concurrency: Number(form.concurrency) || 1,
    localAsrDevice: form.localAsrDevice,
    localAsrThreads: Math.max(0, Number(form.localAsrThreads) || 0),
    localAsrBatchSizeSeconds: Math.max(30, Number(form.localAsrBatchSizeSeconds) || 300),
  };
}

export function useSettingsForm(store: MeetingStore) {
  const [saveError, setSaveError] = useState("");
  const [form, setForm] = useState<SettingsFormState>(() => formFromSettings(store.settings));
  const formRef = useRef(form);
  const persistedSettingsRef = useRef(store.settings);
  const desiredSettingsRef = useRef(store.settings);
  const dirtyFieldsRef = useRef(new Set<SettingsFormKey>());
  const saveSequenceRef = useRef(0);
  const pendingSavesRef = useRef(0);
  formRef.current = form;
  persistedSettingsRef.current = store.settings;
  if (pendingSavesRef.current === 0) {
    desiredSettingsRef.current = store.settings;
  }
  const effectiveTheme = resolveTheme(store.settings.themeMode);
  const glassPreviewThemeClass = useMemo(
    () => (effectiveTheme === "light" ? "preview-glass-light" : "preview-glass-dark"),
    [effectiveTheme],
  );

  function reconcileForm(settings: SettingsState) {
    const current = formRef.current;
    const next = formFromSettings(settings);
    for (const key of settingsFormKeys) {
      if (dirtyFieldsRef.current.has(key)) {
        Object.assign(next, { [key]: current[key] });
      }
    }
    formRef.current = next;
    setForm(next);
  }

  useEffect(() => {
    persistedSettingsRef.current = store.settings;
    if (pendingSavesRef.current === 0) {
      desiredSettingsRef.current = store.settings;
    }
    reconcileForm(store.settings);
  }, [store.settings]);

  function patchForm(patch: Partial<SettingsFormState>) {
    for (const key of Object.keys(patch) as SettingsFormKey[]) {
      dirtyFieldsRef.current.add(key);
    }
    const next = { ...formRef.current, ...patch };
    formRef.current = next;
    setForm(next);
  }

  function beginSave() {
    const sequence = saveSequenceRef.current + 1;
    saveSequenceRef.current = sequence;
    pendingSavesRef.current += 1;
    setSaveError("");
    return sequence;
  }

  function finishSave(saved?: SettingsState) {
    pendingSavesRef.current = Math.max(0, pendingSavesRef.current - 1);
    if (pendingSavesRef.current === 0) {
      desiredSettingsRef.current = saved ?? persistedSettingsRef.current;
    }
  }

  function reportSaveError(sequence: number, error: unknown) {
    if (sequence === saveSequenceRef.current) {
      setSaveError(error instanceof Error ? error.message : String(error));
    }
  }

  async function saveAppearance(patch: Partial<SettingsState>) {
    const sequence = beginSave();
    const nextSettings = { ...desiredSettingsRef.current, ...patch };
    desiredSettingsRef.current = nextSettings;
    let savedSettings: SettingsState | undefined;

    try {
      savedSettings = await store.saveSettings(nextSettings);
    } catch (error) {
      if (sequence === saveSequenceRef.current) {
        applyAppearance(persistedSettingsRef.current);
      }
      reportSaveError(sequence, error);
    } finally {
      finishSave(savedSettings);
    }
  }

  async function setThemeMode(mode: ThemeMode) {
    await saveAppearance({ themeMode: mode });
  }

  async function setGlassStyle(style: LiquidGlassStyle) {
    await saveAppearance({ liquidGlassStyle: style });
  }

  async function setLocale(locale: LocaleCode) {
    await saveAppearance({ locale });
  }

  async function setAccentColor(color: string) {
    await saveAppearance({ accentColor: color });
  }

  async function setRuntimeDownloadSource(sourceId: string) {
    const normalizedSourceId = sourceId.trim();
    const sequence = beginSave();
    const nextSettings = {
      ...desiredSettingsRef.current,
      runtimeDownloadSource: normalizedSourceId,
    };
    desiredSettingsRef.current = nextSettings;
    let savedSettings: SettingsState | undefined;

    try {
      savedSettings = await store.saveSettings(nextSettings);
    } catch (error) {
      reportSaveError(sequence, error);
    } finally {
      finishSave(savedSettings);
    }
  }

  async function save(
    patch: Partial<SettingsFormState> = {},
    credentialOverride?: SettingsCredentialUpdate,
  ) {
    const sequence = beginSave();
    const patchKeys = Object.keys(patch) as SettingsFormKey[];
    for (const key of patchKeys) {
      dirtyFieldsRef.current.add(key);
    }
    const nextForm = { ...formRef.current, ...patch };
    formRef.current = nextForm;
    setForm(nextForm);
    const savedFields = new Set(dirtyFieldsRef.current);
    const apiToken = nextForm.apiToken.trim();
    const credential = credentialOverride
      ?? (apiToken ? { action: "set", value: apiToken } satisfies SettingsCredentialUpdate : { action: "keep" });
    const nextSettings = settingsFromForm(desiredSettingsRef.current, nextForm);
    desiredSettingsRef.current = nextSettings;
    let savedSettings: SettingsState | undefined;

    try {
      savedSettings = await store.saveSettings(nextSettings, credential);
      for (const key of savedFields) {
        if (Object.is(formRef.current[key], nextForm[key])) {
          dirtyFieldsRef.current.delete(key);
        }
      }
      if (
        credential.action !== "keep"
        && formRef.current.apiToken === nextForm.apiToken
      ) {
        formRef.current = { ...formRef.current, apiToken: "" };
        dirtyFieldsRef.current.delete("apiToken");
      }
      reconcileForm(savedSettings);
    } catch (error) {
      if (sequence === saveSequenceRef.current) {
        applyAppearance(persistedSettingsRef.current);
      }
      reportSaveError(sequence, error);
    } finally {
      finishSave(savedSettings);
    }
  }

  async function clearApiToken() {
    await save({ apiToken: "" }, { action: "clear" });
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
    setRuntimeDownloadSource,
    clearApiToken,
    save,
  };
}
