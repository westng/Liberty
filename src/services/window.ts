import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { formatMessage, getCurrentMessages } from "@/services/i18n";

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
