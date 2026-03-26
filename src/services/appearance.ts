import { getCurrentWindow, type Theme } from "@tauri-apps/api/window";
import type { SettingsState, ThemeMode } from "@/types/meeting";

export function resolveTheme(mode: ThemeMode): "light" | "dark" {
  if (mode === "light" || mode === "dark") {
    return mode;
  }

  if (typeof window !== "undefined" && window.matchMedia("(prefers-color-scheme: dark)").matches) {
    return "dark";
  }

  return "light";
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
  root.dataset.glassStyle = settings.liquidGlassStyle;
  root.style.setProperty("--accent", settings.accentColor);
  root.style.setProperty("--accent-contrast", resolveAccentContrast(settings.accentColor));
  root.lang = settings.locale;
  void syncWindowTheme(theme);
}

async function syncWindowTheme(theme: Theme): Promise<void> {
  try {
    await getCurrentWindow().setTheme(theme);
  } catch {
    // Ignore native theme sync failures and keep CSS theme as the source of truth.
  }
}
