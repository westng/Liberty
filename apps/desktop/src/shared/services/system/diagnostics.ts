import { useMemo, useState } from "react";
import { getDiagnostics } from "@/shared/services/tauri/system";
import type { DiagnosticsReport } from "@/shared/types/meeting";

export function useDiagnosticsPanel() {
  const [diagnostics, setDiagnostics] = useState<DiagnosticsReport | null>(null);
  const [diagnosticsError, setDiagnosticsError] = useState("");
  const diagnosticsRows = useMemo(() => {
    if (!diagnostics) {
      return [];
    }

    return [
      ["应用版本", diagnostics.appVersion],
      ["当前平台", diagnostics.currentPlatform ? `${diagnostics.currentPlatform.label} (${diagnostics.currentPlatform.id})` : "不支持"],
      ["数据库版本", String(diagnostics.schemaVersion)],
      ["运行时状态", diagnostics.runtimeStatus],
      ["桌宠诊断日志", diagnostics.desktopPetDiagnosticLogPath ?? "未生成"],
      ["CSP", diagnostics.securityBaseline.cspEnabled ? "已启用" : "未启用"],
      ["权限范围", diagnostics.securityBaseline.scopedCapabilities ? "已收敛" : "待收敛"],
      ["凭据存储", diagnostics.securityBaseline.credentialStoreRequired ? "需要系统钥匙串" : "未要求"],
    ] as const;
  }, [diagnostics]);
  const supportedPlatformTags = useMemo(
    () => diagnostics?.supportedPlatforms.map((platform) => platform.id) ?? [],
    [diagnostics],
  );

  async function refreshDiagnostics() {
    setDiagnosticsError("");

    try {
      setDiagnostics(await getDiagnostics());
    } catch (error) {
      setDiagnosticsError(error instanceof Error ? error.message : String(error));
    }
  }

  return {
    diagnostics,
    diagnosticsError,
    diagnosticsRows,
    supportedPlatformTags,
    refreshDiagnostics,
  };
}
