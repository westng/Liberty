import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { emit, emitTo, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { formatMessage, getCurrentMessages } from "@/shared/i18n";
import { localizeAppError } from "@/shared/services/errors/appError";
import { publishAppStatus } from "@/shared/services/ui/statusNotifications";
import type { ProcessingMode } from "@/shared/types/meeting";

const PET_STORE_ITEM_WINDOW_LABEL = "pet-store-item-detail";
const JOB_WORKBENCH_WINDOW_LABEL = "job-workbench";
const JOB_WINDOW_OPEN_REQUEST_EVENT = "liberty:job-window-open-request";
let petStoreItemWindowQueue: Promise<unknown> = Promise.resolve();
let jobWorkbenchWindowQueue: Promise<unknown> = Promise.resolve();
const ENTITY_CHANGED_EVENT = "liberty:entity-changed";

type AuxiliaryJobWindowLabel = "ai-summary" | "meeting-notes";

export type JobWindowOpenRequest = {
  windowLabel: AuxiliaryJobWindowLabel;
  jobId: string;
  source: ProcessingMode;
  title: string;
  scopeToken: string;
};

export type ChangedEntity = "job" | "summary" | "model" | "template" | "member";
export type EntityChangedPayload = {
  entity: ChangedEntity;
  id: string;
  action: "saved" | "deleted";
  revision: string;
};

export function publishEntityChanged(payload: Omit<EntityChangedPayload, "revision"> & { revision?: string }) {
  return emit(ENTITY_CHANGED_EVENT, {
    ...payload,
    revision: payload.revision ?? new Date().toISOString(),
  } satisfies EntityChangedPayload);
}

export function listenForEntityChanges(handler: (payload: EntityChangedPayload) => void): Promise<UnlistenFn> {
  return listen<EntityChangedPayload>(ENTITY_CHANGED_EVENT, (event) => handler(event.payload));
}

export function listenForJobWindowOpenRequests(
  handler: (payload: JobWindowOpenRequest) => void,
): Promise<UnlistenFn> {
  return listen<JobWindowOpenRequest>(JOB_WINDOW_OPEN_REQUEST_EVENT, (event) => handler(event.payload));
}

async function keepEditorOpenWhenCloseWasCancelled(label: string) {
  const existing = await WebviewWindow.getByLabel(label);
  if (!existing) {
    return null;
  }

  await existing.close();
  const remaining = await WebviewWindow.getByLabel(label);
  if (remaining) {
    await remaining.setFocus().catch(() => undefined);
  }
  return remaining;
}

export async function openAiSummaryWindow(jobId: string, title: string, source: ProcessingMode) {
  const windowTitle = formatMessage(getCurrentMessages().windows.aiSummaryTitle, { title });
  return openAuxiliaryJobWindow("ai-summary", jobId, windowTitle, source);
}

export async function openMeetingNotesWindow(
  jobId: string,
  title: string,
  source: ProcessingMode,
) {
  const windowTitle = formatMessage(getCurrentMessages().windows.meetingNotesTitle, { title });
  return openAuxiliaryJobWindow("meeting-notes", jobId, windowTitle, source);
}

export async function openJobWorkbenchWindow(
  jobId: string,
  title: string,
  source: ProcessingMode,
) {
  if (getCurrentWindow().label !== "main") {
    throw new Error("只有主窗口可以打开结果工作台。");
  }

  const request = jobWorkbenchWindowQueue.then(() => openSingleJobWorkbenchWindow(
    jobId,
    title,
    source,
  ));
  jobWorkbenchWindowQueue = request.catch(() => undefined);
  try {
    return await request;
  } catch (error) {
    const messages = getCurrentMessages();
    publishAppStatus(
      formatMessage(messages.shell.statusFailed, {
        action: messages.jobs.viewResult,
        message: localizeAppError(error),
      }),
      { tone: "error", durationMs: 7000 },
    );
    return null;
  }
}

async function openSingleJobWorkbenchWindow(
  jobId: string,
  title: string,
  source: ProcessingMode,
) {
  const existing = await WebviewWindow.getByLabel(JOB_WORKBENCH_WINDOW_LABEL);
  const identity = `${source}:${jobId}`;
  if (existing && activeJobWorkbenchIdentity === identity) {
    await existing.setFocus();
    return existing;
  }
  if (existing) {
    await existing.close();
  }

  const scopeToken = await issueJobWindowScope(JOB_WORKBENCH_WINDOW_LABEL, jobId, source);
  const window = new WebviewWindow(JOB_WORKBENCH_WINDOW_LABEL, {
    title: formatMessage(getCurrentMessages().windows.workbenchTitle, { title }),
    url: buildJobWindowUrl("/job-workbench", source, jobId, scopeToken),
    width: 1280,
    height: 900,
    minWidth: 960,
    minHeight: 720,
    resizable: true,
    center: true,
  });
  await waitForWindowCreation(window);
  activeJobWorkbenchIdentity = identity;
  return window;
}

export async function fulfillJobWindowOpenRequest(request: JobWindowOpenRequest) {
  if (!isJobWindowOpenRequest(request) || getCurrentWindow().label !== "main") {
    throw new Error("结果窗口请求无效。");
  }
  return openScopedAuxiliaryJobWindow(request);
}

let activeJobWorkbenchIdentity: string | null = null;

async function openAuxiliaryJobWindow(
  windowLabel: AuxiliaryJobWindowLabel,
  jobId: string,
  title: string,
  source: ProcessingMode,
) {
  if (getCurrentWindow().label === "main") {
    const scopeToken = await issueJobWindowScope(windowLabel, jobId, source);
    return openScopedAuxiliaryJobWindow({
      windowLabel,
      jobId,
      source,
      title,
      scopeToken,
    });
  }

  const parentScopeToken = readCurrentJobWindowScope(jobId, source);
  const scopeToken = await issueJobWindowScope(
    windowLabel,
    jobId,
    source,
    parentScopeToken,
  );
  await emitTo("main", JOB_WINDOW_OPEN_REQUEST_EVENT, {
    windowLabel,
    jobId,
    source,
    title,
    scopeToken,
  } satisfies JobWindowOpenRequest);
  return null;
}

async function openScopedAuxiliaryJobWindow(request: JobWindowOpenRequest) {
  const existing = await WebviewWindow.getByLabel(request.windowLabel);
  if (existing) {
    await existing.close();
  }

  const pathname = request.windowLabel === "ai-summary" ? "/ai-summary" : "/meeting-notes";
  return new WebviewWindow(request.windowLabel, {
    title: request.title,
    url: buildJobWindowUrl(pathname, request.source, request.jobId, request.scopeToken),
    width: 1120,
    height: 860,
    minWidth: request.windowLabel === "ai-summary" ? 960 : 920,
    minHeight: 720,
    resizable: true,
    center: true,
  });
}

function waitForWindowCreation(window: WebviewWindow) {
  return new Promise<void>((resolve, reject) => {
    void window.once("tauri://created", () => resolve());
    void window.once<unknown>("tauri://error", (event) => {
      reject(event.payload instanceof Error ? event.payload : new Error(String(event.payload)));
    });
  });
}

function issueJobWindowScope(
  windowLabel: string,
  jobId: string,
  source: ProcessingMode,
  parentScopeToken?: string,
) {
  return invoke<string>("issue_job_window_scope", {
    windowLabel,
    jobId,
    source,
    parentScopeToken,
  });
}

function readCurrentJobWindowScope(jobId: string, source: ProcessingMode) {
  if (getCurrentWindow().label !== JOB_WORKBENCH_WINDOW_LABEL) {
    throw new Error("当前窗口无权请求任务子窗口。");
  }
  const params = new URLSearchParams(window.location.search);
  const scopeToken = params.get("scopeToken")?.trim() ?? "";
  if (
    params.get("jobId") !== jobId
    || params.get("source") !== source
    || !scopeToken
  ) {
    throw new Error("结果工作台的任务作用域无效。");
  }
  return scopeToken;
}

function isJobWindowOpenRequest(value: JobWindowOpenRequest) {
  return Boolean(
    value
    && (value.windowLabel === "ai-summary" || value.windowLabel === "meeting-notes")
    && value.jobId?.trim()
    && (value.source === "local" || value.source === "remote")
    && value.title?.trim()
    && value.scopeToken?.trim(),
  );
}

function buildJobWindowUrl(
  pathname: string,
  source: ProcessingMode,
  jobId: string,
  scopeToken: string,
) {
  const params = new URLSearchParams({ source, jobId, scopeToken });
  return `${pathname}?${params.toString()}`;
}

export async function openModelEditorWindow(modelId?: string) {
  const label = "model-editor";
  const messages = getCurrentMessages().windows;

  const remaining = await keepEditorOpenWhenCloseWasCancelled(label);
  if (remaining) {
    return remaining;
  }

  const query = modelId ? `?id=${encodeURIComponent(modelId)}` : "";
  const window = new WebviewWindow(label, {
    title: modelId ? messages.editModel : messages.newModel,
    url: `/model-editor${query}`,
    width: 880,
    height: 760,
    minWidth: 760,
    minHeight: 680,
    resizable: true,
    center: true,
  });

  return window;
}

export async function openTemplateEditorWindow(templateId?: string) {
  const label = "template-editor";
  const messages = getCurrentMessages().windows;

  const remaining = await keepEditorOpenWhenCloseWasCancelled(label);
  if (remaining) {
    return remaining;
  }

  const query = templateId ? `?id=${encodeURIComponent(templateId)}` : "";
  const window = new WebviewWindow(label, {
    title: templateId ? messages.editTemplate : messages.newTemplate,
    url: `/template-editor${query}`,
    width: 960,
    height: 820,
    minWidth: 820,
    minHeight: 720,
    resizable: true,
    center: true,
  });

  return window;
}

export async function openMemberEditorWindow(memberId?: string) {
  const label = "member-editor";
  const messages = getCurrentMessages().windows;

  const remaining = await keepEditorOpenWhenCloseWasCancelled(label);
  if (remaining) {
    return remaining;
  }

  const query = memberId ? `?id=${encodeURIComponent(memberId)}` : "";
  const window = new WebviewWindow(label, {
    title: memberId ? messages.editMember : messages.newMember,
    url: `/member-editor${query}`,
    width: 760,
    height: 680,
    minWidth: 640,
    minHeight: 560,
    resizable: true,
    center: true,
  });

  return window;
}

export async function openPetStoreItemWindow(itemKey: string, title: string) {
  const request = petStoreItemWindowQueue.then(() => openSinglePetStoreItemWindow(itemKey, title));
  petStoreItemWindowQueue = request.catch(() => undefined);
  return request;
}

async function openSinglePetStoreItemWindow(itemKey: string, title: string) {
  const messages = getCurrentMessages().windows;
  const windowTitle = formatMessage(messages.petStoreItemTitle, { title });
  await closeLegacyPetStoreItemWindows();

  const existing = await WebviewWindow.getByLabel(PET_STORE_ITEM_WINDOW_LABEL);

  if (existing) {
    await existing.close().catch(() => undefined);
  }

  const window = new WebviewWindow(PET_STORE_ITEM_WINDOW_LABEL, {
    title: windowTitle,
    url: `/pet-store-item?itemKey=${encodeURIComponent(itemKey)}`,
    width: 920,
    height: 640,
    minWidth: 760,
    minHeight: 520,
    resizable: true,
    center: true,
  });

  return window;
}

async function closeLegacyPetStoreItemWindows() {
  const windows = await WebviewWindow.getAll();
  await Promise.all(
    windows
      .filter((window) => window.label.startsWith("pet-store-item-") && window.label !== PET_STORE_ITEM_WINDOW_LABEL)
      .map((window) => window.close().catch(() => undefined)),
  );
}
