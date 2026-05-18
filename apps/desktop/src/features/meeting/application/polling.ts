export type PollingTask = () => void;

export interface PollingScheduler {
  sync(enabled: boolean, intervalMs: number, task: PollingTask): void;
  stop(): void;
  isRunning(): boolean;
}

export function createPollingScheduler(): PollingScheduler {
  let timerId: number | null = null;
  let activeIntervalMs = 0;

  function stop() {
    if (timerId === null || typeof window === "undefined") {
      timerId = null;
      activeIntervalMs = 0;
      return;
    }

    window.clearInterval(timerId);
    timerId = null;
    activeIntervalMs = 0;
  }

  return {
    sync(enabled, intervalMs, task) {
      if (typeof window === "undefined") {
        return;
      }

      if (!enabled) {
        stop();
        return;
      }

      if (timerId !== null && activeIntervalMs === intervalMs) {
        return;
      }

      stop();
      timerId = window.setInterval(task, intervalMs);
      activeIntervalMs = intervalMs;
    },
    stop,
    isRunning() {
      return timerId !== null;
    },
  };
}
