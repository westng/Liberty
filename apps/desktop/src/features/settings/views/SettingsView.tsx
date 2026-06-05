import { useEffect } from "react";
import progressBarUrl from "@/assets/progress-bar.webp";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { accentColors, useSettingsForm } from "@/features/settings/application/useSettingsForm";
import { useRuntimePanel } from "@/features/settings/application/useRuntimePanel";
import { useDiagnosticsPanel } from "@/shared/services/system/diagnostics";
import { getMessages } from "@/shared/i18n";
import type { LiquidGlassStyle, LocaleCode, LocalAsrDevice, ThemeMode } from "@/shared/types/meeting";
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
    supportedPlatformText,
    refreshDiagnostics,
  } = useDiagnosticsPanel();

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
              <div className="summary-inline">
                <span>{runtimeModeLabel}</span>
                <span>{store.localMode ? messages.localDatabaseReady : messages.waitingLocalConfig}</span>
              </div>
            </div>
          </div>
        </article>
      </div>

      <div className="settings-group">
        <h3 className="settings-group-title">工程诊断</h3>
        <article className="surface settings-block runtime-card diagnostics-card">
          <div className="runtime-card-head">
            <div className="runtime-card-title-wrap">
              <span className="runtime-card-title">企业级基线</span>
              <p className="runtime-card-hint">用于验收平台矩阵、数据库版本、运行时状态和安全基线。</p>
            </div>
            <button className="text-button runtime-primary-action" type="button" onClick={refreshDiagnostics}>
              刷新
            </button>
          </div>

          <div className="runtime-meta-grid diagnostics-grid">
            {diagnosticsRows.map(([label, value]) => (
              <div key={label} className="runtime-meta-item">
                <span>{label}</span>
                <strong>{value}</strong>
              </div>
            ))}
          </div>

          {supportedPlatformText && (
            <div className="runtime-log">
              <span className="runtime-log-title">发布平台矩阵</span>
              <pre>{supportedPlatformText}</pre>
            </div>
          )}

          {diagnostics?.desktopPetDiagnosticLogTail && (
            <div className="runtime-log">
              <span className="runtime-log-title">桌宠拖拽诊断日志</span>
              <pre>{diagnostics.desktopPetDiagnosticLogTail}</pre>
            </div>
          )}

          {diagnosticsError && <p className="settings-error">{diagnosticsError}</p>}
        </article>
      </div>

      <div className="settings-group">
        <h3 className="settings-group-title">{messages.localRuntime}</h3>
        <article className="surface settings-block runtime-card">
          <div className="runtime-card-head">
            <div className="runtime-card-title-wrap">
              <span className="runtime-card-title">{messages.managedRuntime}</span>
              <p className="runtime-card-hint">{messages.managedRuntimeHint}</p>
            </div>
            <div className="runtime-card-status">
              <span className="runtime-status-label">{messages.runtimeStatus}</span>
              <span className={`runtime-status-badge runtime-status-${runtimeStatus.status}`}>
                {runtimeStatusLabel}
              </span>
            </div>
          </div>

          <div className="runtime-panel">
            <div className="runtime-source-row">
              <label className="runtime-source-label" htmlFor="runtime-download-source">
                {messages.runtimeDownloadSource}
              </label>
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

            <div className="runtime-hero">
              <p className="runtime-status-text">{runtimeStatusDescription}</p>
              <button
                className="text-button runtime-primary-action"
                type="button"
                disabled={runtimeBusy || !runtimeSelectedSourceId}
                onClick={installManagedRuntime}
              >
                {runtimeActionLabel}
              </button>
            </div>

            <div className="runtime-resource-list">
              {runtimeResourceRows.map((resource) => (
                <div key={resource.id} className="runtime-resource-row">
                  <div className="runtime-resource-name">{resource.name}</div>
                  <div className="runtime-resource-progress">
                    <div className="runtime-progress-track">
                      <span className="runtime-progress-bar" style={{ width: `${resource.percent}%` }}>
                        {resource.percent > 0 && (
                          <img className="runtime-progress-media" src={progressBarUrl} alt="" aria-hidden="true" />
                        )}
                      </span>
                    </div>
                    <span className="runtime-resource-status">{resource.statusLabel}</span>
                  </div>
                  <button
                    className="text-button runtime-resource-action"
                    type="button"
                    disabled={resource.disabled}
                    onClick={installManagedRuntime}
                  >
                    {resource.actionLabel}
                  </button>
                </div>
              ))}
            </div>

            <div className="runtime-log">
              <span className="runtime-log-title">{messages.runtimeInstallLog}</span>
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
              <textarea id="default-hotwords" value={form.defaultHotwords} onChange={(event) => patchForm({ defaultHotwords: event.target.value })} placeholder={messages.defaultHotwordsPlaceholder} onBlur={save} />
            </div>
          </div>

          <div className="setting-row">
            <div className="settings-meta">
              <span className="settings-label">{messages.defaultSummaryTemplate}</span>
              <p className="settings-hint">{messages.defaultSummaryTemplateHint}</p>
            </div>
            <div className="setting-control">
              <input id="summary-template" value={form.summaryTemplate} onChange={(event) => patchForm({ summaryTemplate: event.target.value })} placeholder={messages.defaultSummaryTemplatePlaceholder} onBlur={save} />
            </div>
          </div>

          <div className="setting-row">
            <div className="settings-meta">
              <span className="settings-label">{messages.concurrency}</span>
              <p className="settings-hint">{messages.concurrencyHint}</p>
            </div>
            <div className="setting-control setting-control-inline">
              <input id="concurrency" value={form.concurrency} onChange={(event) => patchForm({ concurrency: Number(event.target.value) })} type="number" min="1" max="8" onBlur={save} />
            </div>
          </div>

          <div className="setting-row">
            <div className="settings-meta">
              <span className="settings-label">{messages.localAsrDevice}</span>
              <p className="settings-hint">{messages.localAsrDeviceHint}</p>
            </div>
            <div className="setting-control setting-control-inline">
              <select value={form.localAsrDevice} onChange={(event) => {
                patchForm({ localAsrDevice: event.target.value as LocalAsrDevice });
                void save();
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
              <input id="local-asr-threads" value={form.localAsrThreads} onChange={(event) => patchForm({ localAsrThreads: Number(event.target.value) })} type="number" min="0" max="32" onBlur={save} />
            </div>
          </div>

          <div className="setting-row">
            <div className="settings-meta">
              <span className="settings-label">{messages.localAsrBatchSizeSeconds}</span>
              <p className="settings-hint">{messages.localAsrBatchSizeSecondsHint}</p>
            </div>
            <div className="setting-control setting-control-inline">
              <input id="local-asr-batch-size-seconds" value={form.localAsrBatchSizeSeconds} onChange={(event) => patchForm({ localAsrBatchSizeSeconds: Number(event.target.value) })} type="number" min="30" max="1200" step="30" onBlur={save} />
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
              <input id="backend-url" value={form.backendUrl} onChange={(event) => patchForm({ backendUrl: event.target.value })} placeholder={messages.backendUrlPlaceholder} onBlur={save} />
            </div>
          </div>

          <div className="setting-row">
            <div className="settings-meta">
              <span className="settings-label">{messages.apiToken}</span>
              <p className="settings-hint">{messages.apiTokenHint}</p>
            </div>
            <div className="setting-control">
              <input id="api-token" value={form.apiToken} onChange={(event) => patchForm({ apiToken: event.target.value })} type="password" placeholder={messages.apiTokenPlaceholder} autoComplete="off" onBlur={save} />
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
