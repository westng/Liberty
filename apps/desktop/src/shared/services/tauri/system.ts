import { invoke } from "@tauri-apps/api/core";
import type { DiagnosticsReport, ProcessMetrics } from "@/shared/types/meeting";

export async function openExternalUrl(url: string) {
  await invoke<void>("open_external_url", { url });
}

export async function getProcessMetrics() {
  return invoke<ProcessMetrics>("get_process_metrics");
}

export async function getDiagnostics() {
  return invoke<DiagnosticsReport>("get_diagnostics");
}

export async function promptPetName(input: {
  title: string;
  message: string;
  defaultValue: string;
}) {
  return invoke<string | null>("prompt_pet_name", input);
}
