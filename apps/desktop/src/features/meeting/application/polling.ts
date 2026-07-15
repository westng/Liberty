export type PollingTask = () => void | Promise<void>;

export interface PollingScheduler {
  sync(enabled: boolean, intervalMs: number, task: PollingTask): void;
  stop(): void;
  isRunning(): boolean;
}

export function createPollingScheduler(): PollingScheduler {
  let timerId: number | null = null;
  let activeIntervalMs = 0;
  let schedulerEnabled = false;
  let running = false;
  let rerunAfterCurrent = false;
  let activeTask: PollingTask | null = null;
  let generation = 0;

  function clearTimer() {
    if (timerId !== null && typeof window !== "undefined") {
      window.clearTimeout(timerId);
    }
    timerId = null;
  }

  function schedule(delayMs: number, expectedGeneration: number) {
    if (!schedulerEnabled || typeof window === "undefined" || timerId !== null) {
      return;
    }

    timerId = window.setTimeout(() => {
      timerId = null;
      void run(expectedGeneration);
    }, delayMs);
  }

  async function run(expectedGeneration: number) {
    if (!schedulerEnabled || expectedGeneration !== generation || !activeTask) {
      return;
    }

    if (running) {
      rerunAfterCurrent = true;
      return;
    }

    running = true;
    const task = activeTask;

    try {
      await task();
    } finally {
      running = false;

      if (!schedulerEnabled || typeof window === "undefined") {
        return;
      }

      const delayMs = rerunAfterCurrent || expectedGeneration !== generation ? 0 : activeIntervalMs;
      rerunAfterCurrent = false;
      schedule(delayMs, generation);
    }
  }

  function stop() {
    schedulerEnabled = false;
    generation += 1;
    rerunAfterCurrent = false;
    activeTask = null;
    clearTimer();
    activeIntervalMs = 0;
  }

  return {
    sync(shouldRun, intervalMs, task) {
      if (typeof window === "undefined") {
        return;
      }

      if (!shouldRun) {
        stop();
        return;
      }

      activeTask = task;

      if (activeIntervalMs === intervalMs && (timerId !== null || running)) {
        return;
      }

      const wasRunning = running;
      clearTimer();
      generation += 1;
      schedulerEnabled = true;
      activeIntervalMs = intervalMs;

      if (wasRunning) {
        rerunAfterCurrent = true;
        return;
      }

      schedule(intervalMs, generation);
    },
    stop,
    isRunning() {
      return schedulerEnabled;
    },
  };
}
