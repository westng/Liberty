import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { formatMessage, getCurrentMessages } from "@/shared/i18n";
import type { MessageTree } from "@/shared/i18n";

export type AppStatusTone = "idle" | "progress" | "success" | "error";
export type AppStatusAction = keyof MessageTree["shell"]["statusActions"];

export type AppStatusNotification = {
  id: number;
  message: string;
  tone: AppStatusTone;
};

type PublishOptions = {
  tone?: AppStatusTone;
  durationMs?: number | null;
};

type ActionOptions<T> = {
  completedAsStarted?: boolean;
  shouldNotifySuccess?: (result: T) => boolean;
};

let nextNotificationId = 1;
let currentNotification: AppStatusNotification | null = null;
const notifications = new Map<number, AppStatusNotification>();
const clearTimers = new Map<number, ReturnType<typeof globalThis.setTimeout>>();
const listeners = new Set<() => void>();
const STATUS_EVENT = "liberty:status-notification";

export type ForwardedAppStatusEvent = {
  kind: "publish" | "clear";
  source: string;
  notificationId: number;
  notification?: AppStatusNotification;
  durationMs?: number | null;
};

function forwardStatusEvent(event: Omit<ForwardedAppStatusEvent, "source">) {
  try {
    const source = getCurrentWindow().label;
    if (source !== "main") {
      void emit(STATUS_EVENT, { ...event, source } satisfies ForwardedAppStatusEvent).catch(() => undefined);
    }
  } catch {
    // Browser previews do not have a Tauri window.
  }
}

export function listenForForwardedAppStatus(
  handler: (event: ForwardedAppStatusEvent) => void,
): Promise<UnlistenFn> {
  return listen<ForwardedAppStatusEvent>(STATUS_EVENT, (event) => handler(event.payload));
}

function emitChange() {
  for (const listener of listeners) {
    listener();
  }
}

function updateCurrentNotification() {
  let latest: AppStatusNotification | null = null;

  for (const notification of notifications.values()) {
    latest = notification;
  }

  currentNotification = latest;
  emitChange();
}

export function subscribeAppStatus(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function getAppStatusNotification() {
  return currentNotification;
}

export function clearAppStatusNotification(id?: number) {
  if (id === undefined) {
    for (const timer of clearTimers.values()) {
      globalThis.clearTimeout(timer);
    }
    clearTimers.clear();
    notifications.clear();
    updateCurrentNotification();
    return;
  }

  const timer = clearTimers.get(id);
  if (timer) {
    globalThis.clearTimeout(timer);
    clearTimers.delete(id);
  }

  if (notifications.delete(id)) {
    updateCurrentNotification();
    forwardStatusEvent({ kind: "clear", notificationId: id });
  }
}

export function publishAppStatus(message: string, options: PublishOptions = {}) {
  const notification: AppStatusNotification = {
    id: nextNotificationId,
    message,
    tone: options.tone ?? "success",
  };
  nextNotificationId += 1;
  notifications.set(notification.id, notification);
  updateCurrentNotification();

  const durationMs = options.durationMs === undefined ? 4000 : options.durationMs;
  forwardStatusEvent({
    kind: "publish",
    notificationId: notification.id,
    notification,
    durationMs,
  });
  if (durationMs !== null && durationMs > 0) {
    const timer = globalThis.setTimeout(() => {
      clearTimers.delete(notification.id);
      clearAppStatusNotification(notification.id);
    }, durationMs);
    clearTimers.set(notification.id, timer);
  }

  return notification.id;
}

export async function runAppStatusAction<T>(
  action: AppStatusAction,
  operation: () => Promise<T>,
  options: ActionOptions<T> = {},
) {
  const shell = getCurrentMessages().shell;
  const actionLabel = shell.statusActions[action];
  const pendingId = publishAppStatus(
    formatMessage(shell.statusWorking, { action: actionLabel }),
    { tone: "progress", durationMs: null },
  );

  try {
    const result = await operation();
    const shouldNotifySuccess = options.shouldNotifySuccess?.(result) ?? true;
    clearAppStatusNotification(pendingId);

    if (shouldNotifySuccess) {
      const template = options.completedAsStarted ? shell.statusStarted : shell.statusSucceeded;
      publishAppStatus(formatMessage(template, { action: actionLabel }), { tone: "success" });
    }

    return result;
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    clearAppStatusNotification(pendingId);
    publishAppStatus(
      formatMessage(shell.statusFailed, { action: actionLabel, message: detail }),
      { tone: "error", durationMs: 7000 },
    );
    throw error;
  }
}
