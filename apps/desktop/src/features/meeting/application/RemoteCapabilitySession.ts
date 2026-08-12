export type CapabilitySessionScheduler = {
  setTimeout: (callback: () => void, delayMs: number) => unknown;
  clearTimeout: (timer: unknown) => void;
};

const defaultScheduler: CapabilitySessionScheduler = {
  setTimeout: (callback, delayMs) => globalThis.setTimeout(callback, delayMs),
  clearTimeout: (timer) => globalThis.clearTimeout(timer as ReturnType<typeof globalThis.setTimeout>),
};

export function createRemoteCapabilitySession<Capabilities>(options: {
  retryDelaysMs: readonly number[];
  scheduler?: CapabilitySessionScheduler;
  connect: () => Promise<Capabilities>;
  canRetry: () => boolean;
  cached: () => Capabilities | null;
  onChecking: () => void;
  onReady: (capabilities: Capabilities) => void;
  onUnavailable: (error: unknown) => void;
  onInvalidate: () => void;
  onRetryReady: () => void | Promise<void>;
}) {
  const scheduler = options.scheduler ?? defaultScheduler;
  let generation = 0;
  let request: Promise<Capabilities> | null = null;
  let retryTimer: unknown | null = null;
  let retryAttempt = 0;
  let wasReady = false;
  let enabled = false;

  function cancelRetry(resetAttempt = false) {
    if (retryTimer !== null) {
      scheduler.clearTimeout(retryTimer);
      retryTimer = null;
    }
    if (resetAttempt) {
      retryAttempt = 0;
    }
  }

  function scheduleRetry() {
    if (
      !enabled
      || wasReady
      || retryTimer !== null
      || retryAttempt >= options.retryDelaysMs.length
      || !options.canRetry()
    ) {
      return;
    }

    const delayMs = options.retryDelaysMs[retryAttempt];
    retryAttempt += 1;
    retryTimer = scheduler.setTimeout(() => {
      retryTimer = null;
      if (!enabled || wasReady || !options.canRetry()) {
        return;
      }
      void requestCapabilities(true)
        .then(() => options.onRetryReady())
        .catch(() => undefined);
    }, delayMs);
  }

  function requestCapabilities(force = false, resetRetryBudget = false) {
    if (resetRetryBudget) {
      cancelRetry(true);
    }
    const cached = options.cached();
    if (!force && cached) {
      return Promise.resolve(cached);
    }
    if (!force && request) {
      return request;
    }
    if (force) {
      options.onInvalidate();
    }

    const requestGeneration = ++generation;
    options.onChecking();
    const nextRequest = options.connect()
      .then((capabilities) => {
        if (requestGeneration === generation) {
          wasReady = true;
          cancelRetry(true);
          options.onReady(capabilities);
        }
        return capabilities;
      })
      .catch((error) => {
        if (requestGeneration === generation) {
          options.onUnavailable(error);
          scheduleRetry();
        }
        throw error;
      })
      .finally(() => {
        if (request === nextRequest) {
          request = null;
        }
      });
    request = nextRequest;
    return nextRequest;
  }

  return {
    request: requestCapabilities,
    generation: () => generation,
    degrade(error: unknown, requestGeneration: number) {
      if (requestGeneration !== generation) {
        return false;
      }
      generation += 1;
      request = null;
      options.onInvalidate();
      options.onUnavailable(error);
      return true;
    },
    reset() {
      cancelRetry(true);
      wasReady = false;
      generation += 1;
      request = null;
    },
    setEnabled(nextEnabled: boolean) {
      enabled = nextEnabled;
      if (!enabled) {
        cancelRetry();
      }
    },
    dispose() {
      enabled = false;
      cancelRetry(true);
      generation += 1;
      request = null;
    },
    hasRetryTimer: () => retryTimer !== null,
  };
}
