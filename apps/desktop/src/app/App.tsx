import { Suspense, useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Link, RouterProvider, useRouter } from "@/app/router/RouterContext";
import sidebarMascotUrl from "@/assets/sidebar-mascot.webp";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { usePetStore } from "@/features/pet/stores/usePetStore";
import { formatMessage, getMessages } from "@/shared/i18n";
import { applyDesktopPetState } from "@/shared/services/tauri/pet";
import { getProcessMetrics, openExternalUrl } from "@/shared/services/tauri/system";
import { applyAppearance, watchSystemThemeChange } from "@/shared/services/ui/appearance";
import { navIconSvg, type NavIconKey } from "@/shared/services/ui/navIcons";
import type { PetSettings, ProcessMetrics } from "@/shared/types/meeting";

type NavItem = {
  label: string;
  to: string;
  icon: NavIconKey;
};

function AppContent() {
  const router = useRouter();
  const store = useMeetingStore();
  const petStore = usePetStore();
  const [processMetrics, setProcessMetrics] = useState<ProcessMetrics>({
    cpuPercent: 0,
    memoryMb: 0,
  });
  const [graphicsMemoryMb, setGraphicsMemoryMb] = useState(0);
  const [isWindowsTitlebar, setIsWindowsTitlebar] = useState(false);
  const knownJobStatuses = useRef(new Map<string, string>());
  const didHydrateJobStatuses = useRef(false);
  const metricsPollingId = useRef<number | null>(null);
  const didRecordPetDailyOpen = useRef(false);
  const lastAppliedDesktopPetEnabled = useRef<boolean | null>(null);
  const latestSettings = useRef(store.settings);
  const messages = getMessages(store.settings.locale);
  const CurrentView = router.route.component;
  const isStandaloneRoute = Boolean(router.route.standalone);
  const isSettingsRoute = router.path === "/settings";
  const navSections = useMemo(
    () => [
      {
        key: "work",
        title: store.settings.locale === "en-US" ? "Work" : "工作",
        items: [
          { label: messages.nav.newJob, to: "/", icon: "plus" },
          { label: messages.nav.jobs, to: "/jobs", icon: "tray" },
        ] satisfies NavItem[],
      },
      {
        key: "resources",
        title: store.settings.locale === "en-US" ? "Resources" : "资源",
        items: [
          { label: messages.nav.models, to: "/models", icon: "chip" },
          { label: messages.nav.templates, to: "/templates", icon: "doc" },
          { label: messages.nav.members, to: "/members", icon: "people" },
        ] satisfies NavItem[],
      },
      {
        key: "companion",
        title: store.settings.locale === "en-US" ? "Companion" : "伙伴",
        items: [
          { label: messages.nav.pet, to: "/pet", icon: "pet" },
          { label: messages.nav.petStore, to: "/pet-store", icon: "store" },
        ] satisfies NavItem[],
      },
      {
        key: "benefits",
        title: store.settings.locale === "en-US" ? "Benefits" : "福利",
        items: [
          { label: messages.nav.redeemKey, to: "/redeem-key", icon: "key" },
          { label: messages.nav.dailyCheckIn, to: "/daily-check-in", icon: "check" },
          { label: messages.nav.dailyBlindBox, to: "/daily-blind-box", icon: "gift" },
        ] satisfies NavItem[],
      },
    ],
    [messages, store.settings.locale],
  );
  const currentModeLabel = store.localMode
    ? messages.shell.localMode
    : store.settings.backendUrl
      ? messages.shell.remoteMode
      : messages.shell.mockModeShort;
  const desktopPetVisible = Boolean(petStore.settings?.desktopEnabled);
  const toolbarStatus = store.localMode
    ? messages.shell.localReady
    : store.settings.backendUrl
      ? messages.shell.remoteReady
      : messages.shell.mockMode;
  const toolbarMetrics = [
    {
      key: "cpu",
      label: messages.shell.cpu,
      value: `${Math.max(0, processMetrics.cpuPercent).toFixed(1)}%`,
    },
    {
      key: "memory",
      label: messages.shell.memory,
      value: `${Math.max(0, processMetrics.memoryMb)}M`,
    },
    {
      key: "graphics",
      label: messages.shell.graphics,
      value: `${Math.max(0, graphicsMemoryMb)}M`,
    },
  ];
  const activeJobCount = store.jobs.filter((job) =>
    ["queued", "transcribing", "speaker_processing", "summarizing"].includes(job.overallStatus),
  ).length;

  useEffect(() => {
    const jobs = store.jobs.map((job) => ({
      id: job.id,
      title: job.title,
      status: job.overallStatus,
    }));

    if (!didHydrateJobStatuses.current) {
      knownJobStatuses.current.clear();
      for (const job of jobs) {
        knownJobStatuses.current.set(job.id, job.status);
      }
      didHydrateJobStatuses.current = true;
      return;
    }

    const nextStatuses = new Map<string, string>();

    for (const job of jobs) {
      const previousStatus = knownJobStatuses.current.get(job.id);
      nextStatuses.set(job.id, job.status);

      if (previousStatus && previousStatus !== "transcribing" && job.status === "transcribing") {
        void petStore.applyWorkflowEvent({
          eventType: "transcription_started",
          metadata: job.id,
        }).catch(() => undefined);
      }

      if (previousStatus && previousStatus !== "completed" && job.status === "completed") {
        void notifyJobCompleted(job);
      }
    }

    knownJobStatuses.current = nextStatuses;
  }, [store.jobs]);

  useEffect(() => {
    void store.ensureSettingsLoaded();
    void initializeWindowTitlebar();
    void initializePetState();
    updateGraphicsMemoryEstimate();
    syncToolbarMetricsPolling();
    window.addEventListener("focus", syncToolbarMetricsPolling);
    window.addEventListener("blur", syncToolbarMetricsPolling);
    document.addEventListener("visibilitychange", syncToolbarMetricsPolling);
    window.addEventListener("resize", updateGraphicsMemoryEstimate);
    const stopWatchingSystemTheme = watchSystemThemeChange(() => {
      if (latestSettings.current.themeMode === "auto") {
        applyAppearance(latestSettings.current);
      }
    });
    void refreshToolbarMetrics();

    return () => {
      window.removeEventListener("focus", syncToolbarMetricsPolling);
      window.removeEventListener("blur", syncToolbarMetricsPolling);
      document.removeEventListener("visibilitychange", syncToolbarMetricsPolling);
      window.removeEventListener("resize", updateGraphicsMemoryEstimate);
      stopWatchingSystemTheme();
      if (metricsPollingId.current !== null) {
        window.clearInterval(metricsPollingId.current);
        metricsPollingId.current = null;
      }
    };
  }, []);

  useEffect(() => {
    latestSettings.current = store.settings;
    applyAppearance(store.settings);
  }, [store.settings]);

  function isActive(itemTo: string) {
    if (itemTo === "/jobs") {
      return router.path.startsWith("/jobs");
    }

    return router.path === itemTo;
  }

  async function openProjectGithub() {
    await openExternalUrl("https://github.com/westng/Liberty");
  }

  async function initializeWindowTitlebar() {
    if (!navigator.userAgent.includes("Windows")) {
      return;
    }

    setIsWindowsTitlebar(true);
    await getCurrentWindow().setDecorations(false);
  }

  async function minimizeWindow() {
    await getCurrentWindow().minimize();
  }

  async function toggleMaximizeWindow() {
    await getCurrentWindow().toggleMaximize();
  }

  async function closeWindow() {
    await getCurrentWindow().close();
  }

  async function initializePetState() {
    try {
      await petStore.loadPetState();
      if (!didRecordPetDailyOpen.current) {
        didRecordPetDailyOpen.current = true;
        void petStore.applyWorkflowEvent({
          eventType: "daily_open",
          metadata: "Liberty app opened",
        }).catch(() => undefined);
      }
      const settings = petStore.settings;
      if (settings) {
        await syncDesktopPetState(settings, "startup").catch((error) => {
          console.error("[pet-window] failed to sync native pet window", error);
        });
      }
    } catch {
      // Keep the main app usable even if the pet state fails to load.
    }
  }

  async function syncDesktopPetState(settings: PetSettings, source: string) {
    if (lastAppliedDesktopPetEnabled.current === settings.desktopEnabled) {
      return;
    }

    lastAppliedDesktopPetEnabled.current = settings.desktopEnabled;
    await applyDesktopPetState(settings, source);
  }

  async function toggleToolbarPetDesktop() {
    await petStore.loadPetState();
    const current = petStore.settings;
    if (!current) {
      return;
    }

    const savedSettings = await petStore.saveSettings({
      ...current,
      desktopEnabled: !current.desktopEnabled,
    });
    lastAppliedDesktopPetEnabled.current = null;
    await syncDesktopPetState(savedSettings, "toolbar");
  }

  async function notifyJobCompleted(job: { id: string; title: string }) {
    if (typeof window !== "undefined" && typeof Notification !== "undefined") {
      let permission = Notification.permission;

      if (permission === "default") {
        permission = await Notification.requestPermission();
      }

      if (permission === "granted") {
        new Notification(messages.shell.jobCompletedTitle, {
          body: formatMessage(messages.shell.jobCompletedBody, { title: job.title }),
        });
      }
    }

    try {
      await petStore.applyWorkflowEvent({
        eventType: "transcription_completed",
        metadata: job.id,
      });
    } catch {
      // Pet updates are best-effort only.
    }
  }

  async function refreshToolbarMetrics() {
    try {
      setProcessMetrics(await getProcessMetrics());
    } catch {
      // Keep the last known metrics when polling fails.
    }
    updateGraphicsMemoryEstimate();
  }

  function syncToolbarMetricsPolling() {
    if (typeof window === "undefined") {
      return;
    }

    const shouldPoll = document.visibilityState !== "hidden" && document.hasFocus();

    if (shouldPoll && metricsPollingId.current === null) {
      metricsPollingId.current = window.setInterval(() => {
        void refreshToolbarMetrics();
      }, 10000);
      return;
    }

    if (!shouldPoll && metricsPollingId.current !== null) {
      window.clearInterval(metricsPollingId.current);
      metricsPollingId.current = null;
    }
  }

  function updateGraphicsMemoryEstimate() {
    const width = window.innerWidth || 0;
    const height = window.innerHeight || 0;
    const pixelRatio = Math.max(window.devicePixelRatio || 1, 1);
    const bytes = width * height * 4 * pixelRatio * pixelRatio;
    setGraphicsMemoryMb(Math.max(1, Math.round(bytes / (1024 * 1024))));
  }

  const currentView = (
    <Suspense fallback={<div className="view-stack native-page" />}>
      <CurrentView />
    </Suspense>
  );

  if (isStandaloneRoute) {
    return currentView;
  }

  return (
    <div className="app-shell">
      <div className={`window-titlebar ${isWindowsTitlebar ? "windows-titlebar" : ""}`}>
        <div className="window-titlebar-metrics">
          {toolbarMetrics.map((metric) => (
            <div key={metric.key} className="toolbar-metric titlebar-metric">
              <span className="toolbar-metric-icon" aria-hidden="true">
                {metric.key === "cpu" ? (
                  <svg viewBox="0 0 24 24">
                    <path fill="currentColor" d="M9 2h6v2h2.5A2.5 2.5 0 0 1 20 6.5V9h2v6h-2v2.5A2.5 2.5 0 0 1 17.5 20H15v2H9v-2H6.5A2.5 2.5 0 0 1 4 17.5V15H2V9h2V6.5A2.5 2.5 0 0 1 6.5 4H9V2Zm-2.5 4a.5.5 0 0 0-.5.5v11a.5.5 0 0 0 .5.5h11a.5.5 0 0 0 .5-.5v-11a.5.5 0 0 0-.5-.5h-11ZM8 8h8v8H8V8Zm2 2v4h4v-4h-4Z" />
                  </svg>
                ) : metric.key === "memory" ? (
                  <svg viewBox="0 0 24 24">
                    <path fill="currentColor" d="M4 7a3 3 0 0 1 3-3h10a3 3 0 0 1 3 3v10a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3V7Zm3-1a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h1v-2h2v2h4v-2h2v2h1a1 1 0 0 0 1-1V7a1 1 0 0 0-1-1h-1v2h-2V6h-4v2H8V6H7Zm1 4h8v4H8v-4Z" />
                  </svg>
                ) : (
                  <svg viewBox="0 0 24 24">
                    <path fill="currentColor" d="M3 5h18a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1h-7v2h3v2H7v-2h3v-2H3a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1Zm1 2v7h16V7H4Z" />
                  </svg>
                )}
              </span>
              <span className="toolbar-metric-label">{metric.label}</span>
              <strong className="toolbar-metric-value">{metric.value}</strong>
            </div>
          ))}
        </div>
        <div className="window-titlebar-spacer" data-tauri-drag-region />
        <div className="window-titlebar-actions">
          <button
            className="toolbar-pet-toggle titlebar-pet-toggle"
            type="button"
            aria-pressed={desktopPetVisible}
            title={desktopPetVisible ? messages.shell.petDisableTitle : messages.shell.petEnableTitle}
            onClick={toggleToolbarPetDesktop}
          >
            <span className="toolbar-pet-dot" data-active={desktopPetVisible} />
            {desktopPetVisible ? messages.shell.petEnabled : messages.shell.petDisabled}
          </button>
          <div className="toolbar-pill titlebar-pill">
            <span className="toolbar-pill-dot" />
            {toolbarStatus}
          </div>
          <button
            className="toolbar-icon-btn titlebar-icon-btn"
            type="button"
            aria-label={messages.shell.github}
            title={messages.shell.github}
            onClick={openProjectGithub}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path
                fill="currentColor"
                d="M12 2C6.48 2 2 6.59 2 12.25c0 4.53 2.87 8.37 6.84 9.72.5.1.68-.22.68-.49 0-.24-.01-1.04-.01-1.88-2.78.62-3.37-1.22-3.37-1.22-.45-1.19-1.11-1.5-1.11-1.5-.91-.64.07-.63.07-.63 1 .07 1.53 1.06 1.53 1.06.9 1.57 2.35 1.12 2.92.86.09-.67.35-1.12.63-1.38-2.22-.26-4.55-1.15-4.55-5.14 0-1.14.39-2.08 1.03-2.82-.1-.26-.45-1.3.1-2.72 0 0 .84-.28 2.75 1.08A9.3 9.3 0 0 1 12 6.84c.85 0 1.71.12 2.51.37 1.91-1.36 2.75-1.08 2.75-1.08.55 1.42.2 2.46.1 2.72.64.74 1.03 1.68 1.03 2.82 0 4-2.33 4.87-4.56 5.13.36.32.68.95.68 1.92 0 1.39-.01 2.5-.01 2.84 0 .27.18.6.69.49A10.25 10.25 0 0 0 22 12.25C22 6.59 17.52 2 12 2Z"
              />
            </svg>
          </button>
        </div>
        {isWindowsTitlebar && (
          <div className="window-controls">
            <button className="window-control" type="button" aria-label="Minimize" onClick={minimizeWindow}>
              -
            </button>
            <button className="window-control" type="button" aria-label="Maximize" onClick={toggleMaximizeWindow}>
              □
            </button>
            <button className="window-control window-control-close" type="button" aria-label="Close" onClick={closeWindow}>
              ×
            </button>
          </div>
        )}
      </div>

      <aside className="sidebar">
        <div className="nav-wrap">
          <header className="nav-header">
            <div className="nav-brand">
              <img className="nav-brand-image" src={sidebarMascotUrl} alt="Liberty mascot" />
              <h1>Liberty</h1>
            </div>
            <p className="nav-slogan">{messages.shell.slogan}</p>
          </header>

          <nav className="nav-list" aria-label="Liberty">
            {navSections.map((section) => (
              <section key={section.key} className="nav-section">
                <p className="nav-section-title">{section.title}</p>
                {section.items.map((item) => (
                  <Link key={item.to} to={item.to} className={`nav-link ${isActive(item.to) ? "active" : ""}`}>
                    <span className="nav-link-icon" aria-hidden="true" dangerouslySetInnerHTML={{ __html: navIconSvg[item.icon] }} />
                    <span className="nav-link-label">{item.label}</span>
                  </Link>
                ))}
              </section>
            ))}
          </nav>

          <div className="nav-footer">
            <Link to="/settings" className={`nav-link nav-footer-link ${isActive("/settings") ? "active" : ""}`}>
              <span className="nav-link-icon" aria-hidden="true" dangerouslySetInnerHTML={{ __html: navIconSvg.gear }} />
              <span className="nav-link-label">{messages.nav.settings}</span>
            </Link>
            <div className="nav-footer-item">
              <span className="nav-footer-label">
                <span className="nav-link-icon" aria-hidden="true" dangerouslySetInnerHTML={{ __html: navIconSvg.mode }} />
                <span>{messages.shell.modeLabel}</span>
              </span>
              <strong>{currentModeLabel}</strong>
            </div>
            <div className="nav-footer-item">
              <span className="nav-footer-label">
                <span className="nav-link-icon" aria-hidden="true" dangerouslySetInnerHTML={{ __html: navIconSvg.processing }} />
                <span>{messages.shell.processingLabel}</span>
              </span>
              <strong>{activeJobCount}</strong>
            </div>
          </div>
        </div>
      </aside>

      <main className="content">
        <section className="content-page">
          <div className={`content-shell ${isSettingsRoute ? "settings-content-shell" : ""}`}>
            <section className="content-body">
              {currentView}
            </section>
          </div>
        </section>
      </main>
    </div>
  );
}

export default function App() {
  return (
    <RouterProvider>
      <AppContent />
    </RouterProvider>
  );
}
