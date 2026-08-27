import type { Theme } from "@tauri-apps/api/window";
import { applyLocalPetWorkflowEvent } from "@/shared/services/tauri/pet";
import { setCurrentWindowTheme } from "@/shared/services/tauri/window";
import type { SettingsState, ThemeMode } from "@/shared/types/meeting";

const systemThemeQuery = "(prefers-color-scheme: dark)";
let lastRecordedDarkThemeDate = "";

export function resolveTheme(mode: ThemeMode): "light" | "dark" {
  if (mode === "light" || mode === "dark") {
    return mode;
  }

  if (typeof window !== "undefined" && window.matchMedia(systemThemeQuery).matches) {
    return "dark";
  }

  return "light";
}

export function watchSystemThemeChange(callback: () => void): () => void {
  if (typeof window === "undefined") {
    return () => undefined;
  }

  const mediaQuery = window.matchMedia(systemThemeQuery);
  mediaQuery.addEventListener("change", callback);

  return () => {
    mediaQuery.removeEventListener("change", callback);
  };
}

export function resolveAccentContrast(color: string): string {
  const hex = color.trim().replace("#", "");
  if (!/^[0-9a-fA-F]{6}$/.test(hex)) {
    return "#ffffff";
  }

  const red = Number.parseInt(hex.slice(0, 2), 16);
  const green = Number.parseInt(hex.slice(2, 4), 16);
  const blue = Number.parseInt(hex.slice(4, 6), 16);
  const luminance = (red * 299 + green * 587 + blue * 114) / 1000;

  return luminance >= 160 ? "#14151a" : "#ffffff";
}

export function applyAppearance(settings: SettingsState): void {
  if (typeof document === "undefined") {
    return;
  }

  const root = document.documentElement;
  const theme = resolveTheme(settings.themeMode);
  root.dataset.theme = theme;
  document.body.setAttribute("theme-mode", theme);
  root.dataset.glassStyle = settings.liquidGlassStyle;
  root.style.setProperty("--accent", settings.accentColor);
  root.style.setProperty("--accent-contrast", resolveAccentContrast(settings.accentColor));
  root.lang = settings.locale;
  void syncWindowTheme(settings.themeMode === "auto" ? null : theme);
  recordDarkThemeUsage(theme);
}

async function syncWindowTheme(theme: Theme | null): Promise<void> {
  try {
    await setCurrentWindowTheme(theme);
  } catch {
    // Ignore native theme sync failures and keep CSS theme as the source of truth.
  }
}

function recordDarkThemeUsage(theme: Theme): void {
  if (theme !== "dark") {
    return;
  }

  const today = new Date().toISOString().slice(0, 10);
  if (lastRecordedDarkThemeDate === today) {
    return;
  }

  lastRecordedDarkThemeDate = today;
  void applyLocalPetWorkflowEvent({
    eventType: "dark_theme_used",
    metadata: today,
  }).catch(() => undefined);
}
