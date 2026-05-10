import { invoke } from "@tauri-apps/api/core";
import type { ProcessMetrics } from "@/shared/types/meeting";

export async function openExternalUrl(url: string) {
  await invoke<void>("open_external_url", { url });
}

export async function getProcessMetrics() {
  return invoke<ProcessMetrics>("get_process_metrics");
}
