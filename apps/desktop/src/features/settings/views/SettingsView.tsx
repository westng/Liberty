import { useEffect } from "react";
import { confirm, message, save as saveFile } from "@tauri-apps/plugin-dialog";
import progressBarUrl from "@/assets/progress-bar.webp";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { accentColors, useSettingsForm } from "@/features/settings/application/useSettingsForm";
import {
  useRuntimePanel,
  type RuntimeResourceId,
  type RuntimeResourceSource,
} from "@/features/settings/application/useRuntimePanel";
import { useDiagnosticsPanel } from "@/shared/services/system/diagnostics";
import { exportDesktopPetDiagnosticLog } from "@/shared/services/tauri/system";
import { formatMessage, getMessages } from "@/shared/i18n";
import { runAppStatusAction } from "@/shared/services/ui/statusNotifications";
import type { LiquidGlassStyle, LocaleCode, LocalAsrDevice, ProcessingMode, ThemeMode } from "@/shared/types/meeting";
import "./SettingsView.css";

export default function SettingsView() {
  const store = useMeetingStore();
  const messages = getMessages(store.settings.locale).settings;
  const shellMessages = getMessages(store.settings.locale).shell;
  const {
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
  } = useSettingsForm(store);
  const {
    runtimeModeLabel,
    runtimeStatus,
    runtimeInstallLogReversed,
    runtimeActionLabel,
    runtimeStatusLabel,
    runtimeStatusDescription,
    runtimeBusy,
    runtimeSelectedSourceId,
    runtimeDownloadSourceOptions,
    runtimeResourceRows,
    refreshRuntimePanel,
  } = useRuntimePanel(store, messages, shellMessages);
  const {
    diagnostics,
    diagnosticsError,
    diagnosticsRows,
    supportedPlatformTags,
    refreshDiagnostics,
  } = useDiagnosticsPanel();
  const remoteStatusLabel = store.remoteStatus === "checking"
    ? messages.remoteStatusChecking
    : store.remoteStatus === "ready"
      ? messages.remoteStatusReady
      : store.remoteStatus === "unavailable"
        ? messages.remoteStatusUnavailable
        : messages.remoteStatusIdle;

  useEffect(() => {
    void refreshRuntimePanel();
    void refreshDiagnostics();
  }, []);

  async function installManagedRuntime() {
    setSaveError("");

    try {
      await store.installManagedRuntime();
      await refreshRuntimePanel();
    } catch (error) {
      setSaveError(error instanceof Error ? error.message : String(error));
    }
  }

  async function updateRuntimeDownloadSource(sourceId: string) {
    await setRuntimeDownloadSource(sourceId);
  }

  async function updateRuntimeResourceSource(
    resourceId: RuntimeResourceId,
    source: RuntimeResourceSource,
  ) {
    if (resourceId === "model") {
      return;
    }
    setSaveError("");
    try {
      await store.setRuntimeComponentSource(resourceId, source);
      await refreshRuntimePanel();
    } catch (error) {
      setSaveError(error instanceof Error ? error.message : String(error));
    }
  }

  async function runRuntimeResourceAction(
    resourceId: RuntimeResourceId,
    actionKind: "detect" | "install",
  ) {
    setSaveError("");
    try {
      if (actionKind === "detect" && resourceId !== "model") {
        await store.detectRuntimeComponent(resourceId);
      } else {
        await store.installRuntimeComponent(resourceId);
      }
      await refreshRuntimePanel();
    } catch (error) {
      setSaveError(error instanceof Error ? error.message : String(error));
    }
  }

  async function exportPetDiagnosticLog() {
    setSaveError("");
    const filePath = await saveFile({
      defaultPath: "桌宠拖拽诊断日志.log",
      filters: [{ name: "Log", extensions: ["log", "txt"] }],
    });

    if (!filePath) {
      return;
    }

    try {
      await runAppStatusAction(
        "exportDiagnostics",
        () => exportDesktopPetDiagnosticLog(filePath),
      );
      await message(`诊断日志已导出：${filePath}`, {
        title: "工程诊断",
        kind: "info",
      });
    } catch (error) {
      setSaveError(error instanceof Error ? error.message : String(error));
    }
  }

  async function clearRemoteApiToken() {
    const confirmed = await confirm(messages.apiTokenClearConfirm, {
      title: messages.apiToken,
      kind: "warning",
    });
    if (confirmed) {
      await clearApiToken();
    }
  }

  async function checkRemoteService() {
    setSaveError("");
    try {
      await store.ensureRemoteCapabilities(true);
    } catch (error) {
      setSaveError(error instanceof Error ? error.message : String(error));
    }
  }

  return (
    <section className="settings-page">
      <div className="settings-group">
        <h3 className="settings-group-title">{messages.appearance}</h3>
        <article className="surface settings-block">
          <div className="setting-row">
            <div className="settings-meta">
              <span className="settings-label">{messages.themeMode}</span>
              <p className="settings-hint">
                {messages.effectiveTheme}: {effectiveTheme === "dark" ? messages.dark : messages.light}
              </p>
            </div>
            <div className="setting-control">
              <div className="preview-grid preview-grid-3">
                {(["auto", "light", "dark"] as ThemeMode[]).map((mode) => (
                  <button
                    key={mode}
                    className={`preview-card ${store.settings.themeMode === mode ? "active" : ""}`}
                    type="button"
                    onClick={() => setThemeMode(mode)}
                  >
                    <span className={`preview-art preview-theme preview-theme-${mode}`} />
                    <span className="preview-label">{mode === "auto" ? messages.auto : mode === "light" ? messages.light : messages.dark}</span>
                  </button>
                ))}
              </div>
            </div>
          </div>

          <div className="setting-row">
            <div className="settings-meta">
              <span className="settings-label">{messages.glassStyle}</span>
              <p className="settings-hint">{messages.glassStyleHint}</p>
            </div>
            <div className="setting-control">
              <div className="preview-grid preview-grid-2">
                {(["transparent", "tinted"] as LiquidGlassStyle[]).map((style) => (
                  <button
                    key={style}
                    className={`preview-card ${store.settings.liquidGlassStyle === style ? "active" : ""}`}
                    type="button"
                    onClick={() => setGlassStyle(style)}
                  >
                    <span className={`preview-art preview-glass preview-glass-${style} ${glassPreviewThemeClass}`} />
                    <span className="preview-label">{style === "transparent" ? messages.transparent : messages.tinted}</span>
                  </button>
                ))}
              </div>
            </div>
          </div>

          <div className="setting-row">
            <div className="settings-meta">
              <span className="settings-label">{messages.locale}</span>
            </div>
            <div className="setting-control setting-control-inline">
              <select value={store.settings.locale} onChange={(event) => setLocale(event.target.value as LocaleCode)}>
                <option value="zh-CN">{messages.localeZh}</option>
                <option value="en-US">{messages.localeEn}</option>
              </select>
            </div>
          </div>

        </article>
      </div>

      <div className="settings-group settings-group-accent">
        <h3 className="settings-group-title">{messages.themeSection}</h3>
        <article className="surface settings-block">
          <div className="setting-row setting-row-color">
            <div className="settings-meta">
              <span className="settings-label">{messages.accentColor}</span>
            </div>
            <div className="setting-control">
              <div className="color-row">
                {accentColors.map((color) => (
                  <div key={color} className="color-option">
                    <button
                      className={`color-dot ${store.settings.accentColor.toLowerCase() === color ? "active" : ""}`}
                      style={{ background: color }}
                      type="button"
                      title={messages.colorLabels[color]}
                      onClick={() => setAccentColor(color)}
                    />
                    {store.settings.accentColor.toLowerCase() === color && (
                      <span className="color-option-label">{messages.colorLabels[color]}</span>
                    )}
                  </div>
                ))}
              </div>
            </div>
          </div>
        </article>
      </div>

      <div className="settings-group">
        <h3 className="settings-group-title">{messages.runtimeOverview}</h3>
        <article className="surface settings-block">
          <div className="setting-row">
            <div className="settings-meta">
              <span className="settings-label">{messages.runtimeMode}</span>
              <p className="settings-hint">{messages.runtimeModeHint}</p>
            </div>
            <div className="setting-control">
              <select
                value={form.processingMode}
                onChange={(event) => {
                  const processingMode = event.target.value as ProcessingMode;
                  patchForm({ processingMode });
                  void save({ processingMode });
                }}
              >
                <option value="local">{shellMessages.localMode}</option>
                <option value="remote">{shellMessages.remoteMode}</option>
              </select>
              <div className="summary-inline">
                <span>{runtimeModeLabel}</span>
                <span>{runtimeStatus.shellReady ? messages.localDatabaseReady : messages.waitingLocalConfig}</span>
              </div>
            </div>
          </div>
        </article>
      </div>

      <div className="settings-group">
        <h3 className="settings-group-title">工程诊断</h3>
        <article className="surface settings-block diagnostics-card">
          <div className="setting-row diagnostics-row">
            <div className="settings-meta">
              <span className="settings-label">系统健康检查</span>
              <p className="settings-hint">平台矩阵、数据库版本、运行时与安全基线</p>
            </div>
            <div className="setting-control setting-control-inline diagnostics-actions">
              <span
                className={`diagnostics-state ${
                  diagnosticsError
                    ? "diagnostics-state-error"
                    : diagnostics
                      ? "diagnostics-state-ready"
                      : "diagnostics-state-pending"
                }`}
              >
                {diagnosticsError ? "异常" : diagnostics ? "正常" : "待刷新"}
              </span>
              <button className="text-button diagnostics-refresh" type="button" onClick={refreshDiagnostics}>
                刷新
              </button>
            </div>
          </div>

          {diagnosticsRows.map(([label, value]) => (
            <div key={label} className="setting-row diagnostics-row">
              <div className="settings-meta">
                <span className="settings-label">{label}</span>
              </div>
              <div className="setting-control diagnostics-value">
                <strong>{value}</strong>
              </div>
            </div>
          ))}

          {supportedPlatformTags.length > 0 && (
            <div className="setting-row diagnostics-row">
              <div className="settings-meta">
                <span className="settings-label">发布平台矩阵</span>
              </div>
              <div className="setting-control diagnostics-tags">
                {supportedPlatformTags.map((tag) => (
                  <span key={tag} className="diagnostics-tag">
                    {tag}
                  </span>
                ))}
              </div>
            </div>
          )}

          {diagnostics && (
            <div className="setting-row diagnostics-row">
              <div className="settings-meta">
                <span className="settings-label">桌宠拖拽诊断日志</span>
              </div>
              <div className="setting-control setting-control-inline">
                <button
                  className="text-button diagnostics-export"
                  type="button"
                  disabled={!diagnostics.desktopPetDiagnosticLogPath}
                  onClick={exportPetDiagnosticLog}
                >
                  导出诊断日志
                </button>
              </div>
            </div>
          )}

          {diagnosticsError && <p className="settings-error">{diagnosticsError}</p>}
        </article>
      </div>

      <div className="settings-group">
        <h3 className="settings-group-title">{messages.localRuntime}</h3>
        <article className="surface settings-block runtime-settings-block">
          <div className="setting-row runtime-setting-row">
            <div className="settings-meta">
              <span className="settings-label">{messages.managedRuntime}</span>
              <p className="settings-hint">{messages.managedRuntimeHint}</p>
            </div>
            <div className="setting-control setting-control-inline runtime-state-control">
              <span className={`runtime-status-badge runtime-status-${runtimeStatus.status}`}>
                {runtimeStatusLabel}
              </span>
            </div>
          </div>

          <div className="setting-row runtime-setting-row">
            <div className="settings-meta">
              <label className="settings-label" htmlFor="runtime-download-source">{messages.runtimeDownloadSource}</label>
              <p className="settings-hint">{runtimeStatusDescription}</p>
            </div>
            <div className="setting-control setting-control-inline runtime-source-control">
              <select
                id="runtime-download-source"
                value={runtimeSelectedSourceId}
                onChange={(event) => {
                  void updateRuntimeDownloadSource(event.target.value);
                }}
              >
                <option value="">{messages.runtimeDownloadSourcePlaceholder}</option>
                {runtimeDownloadSourceOptions.map((source) => (
                  <option key={source.id} value={source.id}>
                    {source.label}
                  </option>
                ))}
              </select>
            </div>
          </div>

          {runtimeResourceRows.map((resource) => (
            <div key={resource.id} className="setting-row runtime-setting-row runtime-resource-setting-row">
              <div className="settings-meta">
                <span className="settings-label">{resource.name}</span>
              </div>
              <div className="setting-control runtime-resource-control">
                {resource.sourceSelectable && (
                  <div className="runtime-resource-source">
                    <select
                      aria-label={`${resource.name} ${messages.runtimeResourceSource}`}
                      value={resource.source}
                      disabled={runtimeBusy || resource.busy}
                      onChange={(event) => {
                        void updateRuntimeResourceSource(
                          resource.id,
                          event.target.value as RuntimeResourceSource,
                        );
                      }}
                    >
                      <option value="managed">{messages.runtimeResourceManaged}</option>
                      <option value="system">{messages.runtimeResourceSystem}</option>
                    </select>
                  </div>
                )}
                <div className="runtime-resource-progress">
                  <div className="runtime-resource-progress-copy">
                    <div className="runtime-progress-track">
                      <span
                        className={`runtime-progress-bar${resource.indeterminate ? " runtime-progress-bar-indeterminate" : ""}`}
                        style={{ width: `${resource.percent}%` }}
                      >
                        {resource.percent > 0 && (
                          <img className="runtime-progress-media" src={progressBarUrl} alt="" aria-hidden="true" />
                        )}
                      </span>
                    </div>
                    <span className="runtime-resource-status">{resource.statusLabel}</span>
                  </div>
                </div>
                <button
                  className="text-button runtime-resource-action"
                  type="button"
                  disabled={resource.disabled}
                  onClick={() => {
                    void runRuntimeResourceAction(resource.id, resource.actionKind);
                  }}
                >
                  {resource.actionLabel}
                </button>
              </div>
            </div>
          ))}

          <div className="setting-row runtime-setting-row">
            <div className="settings-meta">
              <span className="settings-label">{messages.runtimeInstallAction}</span>
            </div>
            <div className="setting-control setting-control-inline">
              <button
                className="text-button runtime-primary-action"
                type="button"
                disabled={runtimeBusy || !runtimeSelectedSourceId}
                onClick={installManagedRuntime}
              >
                {runtimeActionLabel}
              </button>
            </div>
          </div>

          <div className="setting-row runtime-setting-row runtime-log-setting-row">
            <div className="setting-control runtime-log">
              <pre>{runtimeInstallLogReversed || messages.runtimeInstallLogEmpty}</pre>
            </div>
          </div>
        </article>
      </div>

      <div className="settings-group">
        <h3 className="settings-group-title">{messages.processingDefaults}</h3>
        <article className="surface settings-block">
          <div className="setting-row">
            <div className="settings-meta">
              <span className="settings-label">{messages.defaultHotwords}</span>
              <p className="settings-hint">{messages.defaultHotwordsHint}</p>
            </div>
            <div className="setting-control">
              <textarea id="default-hotwords" value={form.defaultHotwords} onChange={(event) => patchForm({ defaultHotwords: event.target.value })} placeholder={messages.defaultHotwordsPlaceholder} onBlur={() => void save()} />
            </div>
          </div>

          <div className="setting-row">
            <div className="settings-meta">
              <span className="settings-label">{messages.defaultSummaryTemplate}</span>
              <p className="settings-hint">{messages.defaultSummaryTemplateHint}</p>
            </div>
            <div className="setting-control">
              <input id="summary-template" value={form.summaryTemplate} onChange={(event) => patchForm({ summaryTemplate: event.target.value })} placeholder={messages.defaultSummaryTemplatePlaceholder} onBlur={() => void save()} />
            </div>
          </div>

          <div className="setting-row">
            <div className="settings-meta">
              <span className="settings-label">{messages.concurrency}</span>
              <p className="settings-hint">{messages.concurrencyHint}</p>
            </div>
            <div className="setting-control setting-control-inline">
              <input id="concurrency" value={form.concurrency} onChange={(event) => patchForm({ concurrency: Number(event.target.value) })} type="number" min="1" max="8" onBlur={() => void save()} />
            </div>
          </div>

          <div className="setting-row">
            <div className="settings-meta">
              <span className="settings-label">{messages.localAsrDevice}</span>
              <p className="settings-hint">{messages.localAsrDeviceHint}</p>
            </div>
            <div className="setting-control setting-control-inline">
              <select value={form.localAsrDevice} onChange={(event) => {
                const localAsrDevice = event.target.value as LocalAsrDevice;
                patchForm({ localAsrDevice });
                void save({ localAsrDevice });
              }}>
                <option value="auto">{messages.localAsrDeviceAuto}</option>
                <option value="cpu">{messages.localAsrDeviceCpu}</option>
                <option value="mps">{messages.localAsrDeviceMps}</option>
                <option value="cuda">{messages.localAsrDeviceCuda}</option>
              </select>
            </div>
          </div>

          <div className="setting-row">
            <div className="settings-meta">
              <span className="settings-label">{messages.localAsrThreads}</span>
              <p className="settings-hint">{messages.localAsrThreadsHint}</p>
            </div>
            <div className="setting-control setting-control-inline">
              <input id="local-asr-threads" value={form.localAsrThreads} onChange={(event) => patchForm({ localAsrThreads: Number(event.target.value) })} type="number" min="0" max="32" onBlur={() => void save()} />
            </div>
          </div>

          <div className="setting-row">
            <div className="settings-meta">
              <span className="settings-label">{messages.localAsrBatchSizeSeconds}</span>
              <p className="settings-hint">{messages.localAsrBatchSizeSecondsHint}</p>
            </div>
            <div className="setting-control setting-control-inline">
              <input id="local-asr-batch-size-seconds" value={form.localAsrBatchSizeSeconds} onChange={(event) => patchForm({ localAsrBatchSizeSeconds: Number(event.target.value) })} type="number" min="30" max="1200" step="30" onBlur={() => void save()} />
            </div>
          </div>
        </article>
      </div>

      <div className="settings-group">
        <h3 className="settings-group-title">{messages.remoteCompatibility}</h3>
        <article className="surface settings-block">
          <div className="setting-row">
            <div className="settings-meta">
              <span className="settings-label">{messages.backendUrl}</span>
              <p className="settings-hint">{messages.backendUrlHint}</p>
            </div>
            <div className="setting-control">
              <input id="backend-url" value={form.backendUrl} onChange={(event) => patchForm({ backendUrl: event.target.value })} placeholder={messages.backendUrlPlaceholder} onBlur={() => void save()} />
            </div>
          </div>

          <div className="setting-row">
            <div className="settings-meta">
              <span className="settings-label">{messages.apiToken}</span>
              <p className="settings-hint">{messages.apiTokenHint}</p>
            </div>
            <div className="setting-control">
              <div className="setting-control-inline">
                <input
                  id="api-token"
                  value={form.apiToken}
                  onChange={(event) => patchForm({ apiToken: event.target.value })}
                  type="password"
                  placeholder={store.settings.apiTokenConfigured ? messages.apiTokenConfigured : messages.apiTokenPlaceholder}
                  autoComplete="off"
                  onBlur={() => void save()}
                />
                {store.settings.apiTokenConfigured && (
                  <button className="text-button danger-text" type="button" onClick={() => void clearRemoteApiToken()}>
                    {messages.apiTokenClear}
                  </button>
                )}
              </div>
              <p className="settings-hint">
                {store.settings.apiTokenConfigured ? messages.apiTokenConfigured : messages.apiTokenNotConfigured}
              </p>
            </div>
          </div>

          <div className="setting-row">
            <div className="settings-meta">
              <span className="settings-label">{messages.remoteStatus}</span>
              <p className="settings-hint">
                {store.remoteCapabilities
                  ? formatMessage(messages.remoteServiceVersion, { version: store.remoteCapabilities.serviceVersion })
                  : store.remoteError || messages.backendUrlHint}
              </p>
            </div>
            <div className="setting-control setting-control-inline">
              <span className={`diagnostics-state diagnostics-state-${store.remoteStatus === "ready" ? "ready" : store.remoteStatus === "unavailable" ? "error" : "pending"}`}>
                {remoteStatusLabel}
              </span>
              <button
                className="text-button"
                type="button"
                disabled={store.remoteStatus === "checking" || !form.backendUrl.trim()}
                onClick={() => void checkRemoteService()}
              >
                {messages.remoteCheck}
              </button>
            </div>
          </div>
        </article>
      </div>

      {saveError && <p className="settings-error">{saveError}</p>}

      <footer className="settings-footer">
        <p>{messages.copyright}</p>
        <p>
          {messages.authorGithub}{" "}
          <a href="https://github.com/westng/Liberty" target="_blank" rel="noreferrer">github.com/westng/Liberty</a>
        </p>
      </footer>
    </section>
  );
}
