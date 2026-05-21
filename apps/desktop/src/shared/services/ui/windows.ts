import { emitTo } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { formatMessage, getCurrentMessages } from "@/shared/i18n";
import { createLocalPetService } from "@/shared/services/tauri/pet";

const PET_STORE_ITEM_WINDOW_LABEL = "pet-store-item-detail";
const PET_STORE_ITEM_SHOW_EVENT = "pet-store-item:show";
const petService = createLocalPetService();
let petStoreItemWindowRequest: Promise<WebviewWindow> | null = null;

export async function openAiSummaryWindow(jobId: string, title: string) {
  const label = "ai-summary";
  const existing = await WebviewWindow.getByLabel(label);
  const messages = getCurrentMessages().windows;

  if (existing) {
    await existing.close();
  }

  const window = new WebviewWindow(label, {
    title: formatMessage(messages.aiSummaryTitle, { title }),
    url: `/ai-summary?jobId=${encodeURIComponent(jobId)}`,
    width: 1120,
    height: 860,
    minWidth: 960,
    minHeight: 720,
    resizable: true,
    center: true,
  });

  return window;
}

export async function openMeetingNotesWindow(jobId: string, title: string) {
  const label = "meeting-notes";
  const existing = await WebviewWindow.getByLabel(label);
  const messages = getCurrentMessages().windows;

  if (existing) {
    await existing.close();
  }

  const window = new WebviewWindow(label, {
    title: formatMessage(messages.meetingNotesTitle, { title }),
    url: `/meeting-notes?jobId=${encodeURIComponent(jobId)}`,
    width: 1120,
    height: 860,
    minWidth: 920,
    minHeight: 720,
    resizable: true,
    center: true,
  });

  return window;
}

export async function openModelEditorWindow(modelId?: string) {
  const label = "model-editor";
  const existing = await WebviewWindow.getByLabel(label);
  const messages = getCurrentMessages().windows;

  if (existing) {
    await existing.close();
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
  const existing = await WebviewWindow.getByLabel(label);
  const messages = getCurrentMessages().windows;

  if (existing) {
    await existing.close();
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
  const existing = await WebviewWindow.getByLabel(label);
  const messages = getCurrentMessages().windows;

  if (existing) {
    await existing.close();
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
  if (petStoreItemWindowRequest) {
    const window = await petStoreItemWindowRequest;
    await showPetStoreItemInWindow(window, itemKey, title);
    return window;
  }

  petStoreItemWindowRequest = openSinglePetStoreItemWindow(itemKey, title).finally(() => {
    petStoreItemWindowRequest = null;
  });
  return petStoreItemWindowRequest;
}

async function openSinglePetStoreItemWindow(itemKey: string, title: string) {
  const messages = getCurrentMessages().windows;
  const windowTitle = formatMessage(messages.petStoreItemTitle, { title });
  await closeLegacyPetStoreItemWindows();

  const existing = await WebviewWindow.getByLabel(PET_STORE_ITEM_WINDOW_LABEL);

  if (existing) {
    await showPetStoreItemInWindow(existing, itemKey, title);
    return existing;
  }

  await setPetStoreItemDetailState(itemKey);
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

async function showPetStoreItemInWindow(window: WebviewWindow, itemKey: string, title: string) {
  const messages = getCurrentMessages().windows;
  await window.setTitle(formatMessage(messages.petStoreItemTitle, { title }));
  await emitPetStoreItemShow(itemKey);
  await window.show();
  await window.setFocus();
}

async function emitPetStoreItemShow(itemKey: string) {
  await setPetStoreItemDetailState(itemKey);
  await emitTo(PET_STORE_ITEM_WINDOW_LABEL, PET_STORE_ITEM_SHOW_EVENT, { itemKey });
}

async function setPetStoreItemDetailState(itemKey: string) {
  await petService.setStoreItemDetailItem(itemKey);
}

async function closeLegacyPetStoreItemWindows() {
  const windows = await WebviewWindow.getAll();
  await Promise.all(
    windows
      .filter((window) => window.label.startsWith("pet-store-item-") && window.label !== PET_STORE_ITEM_WINDOW_LABEL)
      .map((window) => window.close().catch(() => undefined)),
  );
}
