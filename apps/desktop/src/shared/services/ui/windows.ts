import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { formatMessage, getCurrentMessages } from "@/shared/i18n";
import type { ProcessingMode } from "@/shared/types/meeting";

const PET_STORE_ITEM_WINDOW_LABEL = "pet-store-item-detail";
let petStoreItemWindowQueue: Promise<unknown> = Promise.resolve();
const ENTITY_CHANGED_EVENT = "liberty:entity-changed";

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
  const label = "ai-summary";
  const existing = await WebviewWindow.getByLabel(label);
  const messages = getCurrentMessages().windows;

  if (existing) {
    await existing.close();
  }

  const scopeToken = await issueJobWindowScope(label, jobId, source);

  const window = new WebviewWindow(label, {
    title: formatMessage(messages.aiSummaryTitle, { title }),
    url: buildJobWindowUrl("/ai-summary", source, jobId, scopeToken),
    width: 1120,
    height: 860,
    minWidth: 960,
    minHeight: 720,
    resizable: true,
    center: true,
  });

  return window;
}

export async function openMeetingNotesWindow(
  jobId: string,
  title: string,
  source: ProcessingMode,
) {
  const label = "meeting-notes";
  const existing = await WebviewWindow.getByLabel(label);
  const messages = getCurrentMessages().windows;

  if (existing) {
    await existing.close();
  }

  const scopeToken = await issueJobWindowScope(label, jobId, source);

  const window = new WebviewWindow(label, {
    title: formatMessage(messages.meetingNotesTitle, { title }),
    url: buildJobWindowUrl("/meeting-notes", source, jobId, scopeToken),
    width: 1120,
    height: 860,
    minWidth: 920,
    minHeight: 720,
    resizable: true,
    center: true,
  });

  return window;
}

function issueJobWindowScope(windowLabel: string, jobId: string, source: ProcessingMode) {
  return invoke<string>("issue_job_window_scope", {
    windowLabel,
    jobId,
    source,
  });
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
