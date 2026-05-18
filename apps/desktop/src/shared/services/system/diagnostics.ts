import { computed, ref } from "vue";
import { getDiagnostics } from "@/shared/services/tauri/system";
import type { DiagnosticsReport } from "@/shared/types/meeting";

export function useDiagnosticsPanel() {
  const diagnostics = ref<DiagnosticsReport | null>(null);
  const diagnosticsError = ref("");

  const diagnosticsRows = computed(() => {
    const report = diagnostics.value;
    if (!report) {
      return [];
    }

    return [
      ["应用版本", report.appVersion],
      ["当前平台", report.currentPlatform ? `${report.currentPlatform.label} (${report.currentPlatform.id})` : "不支持"],
      ["数据库版本", String(report.schemaVersion)],
      ["运行时状态", report.runtimeStatus],
      ["CSP", report.securityBaseline.cspEnabled ? "已启用" : "未启用"],
      ["权限范围", report.securityBaseline.scopedCapabilities ? "已收敛" : "待收敛"],
      ["凭据存储", report.securityBaseline.credentialStoreRequired ? "需要系统钥匙串" : "未要求"],
    ] as const;
  });

  const supportedPlatformText = computed(() =>
    diagnostics.value?.supportedPlatforms
      .map((platform) => `${platform.label}: ${platform.rustTarget}`)
      .join("\n") ?? "",
  );

  async function refreshDiagnostics() {
    diagnosticsError.value = "";

    try {
      diagnostics.value = await getDiagnostics();
    } catch (error) {
      diagnosticsError.value = error instanceof Error ? error.message : String(error);
    }
  }

  return {
    diagnostics,
    diagnosticsError,
    diagnosticsRows,
    supportedPlatformText,
    refreshDiagnostics,
  };
}
