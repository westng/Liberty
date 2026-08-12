export function createSettingsSaveCoordinator<State, Intent>(options: {
  current: () => State;
  project: (intent: Intent, projection: State) => State;
  execute: (intent: Intent) => Promise<State>;
}) {
  let queue: Promise<unknown> = Promise.resolve();
  let projection = options.current();
  let pending = 0;

  return {
    enqueue(intent: Intent) {
      if (pending === 0) {
        projection = options.current();
      }
      projection = options.project(intent, projection);
      pending += 1;
      const queued = queue.then(() => options.execute(intent));
      const request = queued.finally(() => {
        pending -= 1;
        if (pending === 0) {
          projection = options.current();
        }
      });
      queue = request.catch(() => undefined);
      return request;
    },
    projected() {
      return projection;
    },
    pendingCount() {
      return pending;
    },
  };
}
