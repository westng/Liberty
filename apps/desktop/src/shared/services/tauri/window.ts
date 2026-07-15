import { invoke } from "@tauri-apps/api/core";

export function closeCurrentWindow() {
  return invoke<void>("close_current_window");
}

export function destroyCurrentWindow() {
  return invoke<void>("destroy_current_window");
}

export function setCurrentWindowTitle(title: string) {
  return invoke<void>("set_current_window_title", { title });
}

export function setCurrentWindowTheme(theme: "light" | "dark" | null) {
  return invoke<void>("set_current_window_theme", { theme });
}
