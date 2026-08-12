import type { PollingScheduler } from "./polling";
import { createPollingScheduler } from "./polling";

export function createRuntimeInstallController<Status>(options: {
  pollingIntervalMs: number;
  scheduler?: PollingScheduler;
  isOperationActive: (status: Status) => boolean;
  install: () => Promise<Status>;
  refresh: () => Promise<void>;
}) {
  const scheduler = options.scheduler ?? createPollingScheduler();
  let installRequest: Promise<Status> | null = null;

  return {
    install() {
      if (installRequest) {
        return installRequest;
      }
      const request = options.install().finally(() => {
        if (installRequest === request) {
          installRequest = null;
        }
      });
      installRequest = request;
      return request;
    },
    sync(enabled: boolean, status: Status) {
      scheduler.sync(
        enabled && options.isOperationActive(status),
        options.pollingIntervalMs,
        options.refresh,
      );
    },
    dispose() {
      scheduler.stop();
      installRequest = null;
    },
    isPolling: () => scheduler.isRunning(),
    isInstalling: () => installRequest !== null,
  };
}
