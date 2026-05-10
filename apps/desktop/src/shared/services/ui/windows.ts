import { availableMonitors } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { formatMessage, getCurrentMessages } from "@/shared/i18n";
import type { PetSettings } from "@/shared/types/meeting";

const PET_WINDOW_WIDTH = 320;
const PET_WINDOW_HEIGHT = 220;
const PET_WINDOW_LABEL = "desktop-pet";

function wait(ms: number) {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

async function resolvePetWindowPlacement(
  settings?: Pick<PetSettings, "lastWindowX" | "lastWindowY">,
) {
  if (settings?.lastWindowX == null || settings?.lastWindowY == null) {
    return { center: true, x: undefined, y: undefined };
  }

  const savedX = settings.lastWindowX;
  const savedY = settings.lastWindowY;

  try {
    const monitors = await availableMonitors();
    const isVisibleOnAnyMonitor = monitors.some((monitor) => {
      const { position, size } = monitor.workArea;
      const maxX = position.x + size.width - PET_WINDOW_WIDTH;
      const maxY = position.y + size.height - PET_WINDOW_HEIGHT;

      return (
        savedX >= position.x
        && savedY >= position.y
        && savedX <= maxX
        && savedY <= maxY
      );
    });

    if (!isVisibleOnAnyMonitor) {
      console.warn("[pet-window] saved position is outside current monitors, recentering", {
        x: savedX,
        y: savedY,
      });
      return { center: true, x: undefined, y: undefined };
    }
  } catch (error) {
    console.warn("[pet-window] failed to validate saved monitor position, recentering", error);
    return { center: true, x: undefined, y: undefined };
  }

  return {
    center: false,
    x: savedX,
    y: savedY,
  };
}

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

export async function openPetDesktopWindow(settings?: Pick<PetSettings, "alwaysOnTop" | "lastWindowX" | "lastWindowY">) {
  const placement = await resolvePetWindowPlacement(settings);
  await closePetDesktopWindow().catch(() => false);
  await wait(120);

  const window = new WebviewWindow(PET_WINDOW_LABEL, {
    title: "Liberty Pet",
    url: `/pet-desktop?v=${Date.now()}`,
    width: PET_WINDOW_WIDTH,
    height: PET_WINDOW_HEIGHT,
    minWidth: PET_WINDOW_WIDTH,
    minHeight: PET_WINDOW_HEIGHT,
    maxWidth: PET_WINDOW_WIDTH,
    maxHeight: PET_WINDOW_HEIGHT,
    resizable: false,
    decorations: false,
    transparent: true,
    alwaysOnTop: settings?.alwaysOnTop ?? true,
    skipTaskbar: true,
    shadow: false,
    center: placement.center,
    x: placement.x,
    y: placement.y,
  });

  window.once("tauri://created", () => {
    console.info("[pet-window] created", {
      label: PET_WINDOW_LABEL,
      alwaysOnTop: settings?.alwaysOnTop ?? true,
      x: placement.x,
      y: placement.y,
      center: placement.center,
    });
  }).catch((error) => {
    console.error("[pet-window] failed to attach created listener", error);
  });

  window.once("tauri://error", (event) => {
    console.error("[pet-window] create error", event.payload);
  }).catch((error) => {
    console.error("[pet-window] failed to attach error listener", error);
  });

  window.onCloseRequested(() => {
    console.warn("[pet-window] close requested");
  }).catch((error) => {
    console.error("[pet-window] failed to attach close listener", error);
  });

  return window;
}

export async function openPetDesktopWindowCentered(settings?: Pick<PetSettings, "alwaysOnTop">) {
  return openPetDesktopWindow({
    alwaysOnTop: settings?.alwaysOnTop ?? true,
    lastWindowX: undefined,
    lastWindowY: undefined,
  });
}

export async function getPetDesktopWindow() {
  return WebviewWindow.getByLabel(PET_WINDOW_LABEL);
}

export async function isPetDesktopWindowOpen() {
  const window = await getPetDesktopWindow();
  if (!window) {
    return false;
  }

  try {
    return await window.isVisible();
  } catch {
    return false;
  }
}

export async function ensurePetDesktopWindowVisible(
  settings?: Pick<PetSettings, "alwaysOnTop" | "lastWindowX" | "lastWindowY">,
) {
  await openPetDesktopWindow(settings);
  await wait(180);

  if (await isPetDesktopWindowOpen()) {
    return true;
  }

  await closePetDesktopWindow().catch(() => false);
  await openPetDesktopWindowCentered({
    alwaysOnTop: settings?.alwaysOnTop ?? true,
  });
  await wait(180);
  return isPetDesktopWindowOpen();
}

export async function applyPetDesktopState(
  settings: Pick<PetSettings, "desktopEnabled" | "alwaysOnTop" | "lastWindowX" | "lastWindowY">,
) {
  if (!settings.desktopEnabled) {
    await closePetDesktopWindow().catch(() => false);
    return false;
  }

  return ensurePetDesktopWindowVisible({
    alwaysOnTop: settings.alwaysOnTop,
    lastWindowX: settings.lastWindowX,
    lastWindowY: settings.lastWindowY,
  });
}

export async function closePetDesktopWindow() {
  const window = await getPetDesktopWindow();
  if (!window) {
    return false;
  }

  try {
    await window.close();
  } catch (error) {
    console.error("[pet-window] failed to close window", error);
    throw error;
  }

  for (let attempt = 0; attempt < 20; attempt += 1) {
    await wait(50);
    const next = await getPetDesktopWindow();
    if (!next) {
      return true;
    }
  }

  return false;
}

export async function togglePetDesktopWindow(
  settings?: Pick<PetSettings, "alwaysOnTop" | "lastWindowX" | "lastWindowY">,
) {
  const existing = await getPetDesktopWindow();
  if (existing) {
    await closePetDesktopWindow().catch(() => false);
    return false;
  }

  return ensurePetDesktopWindowVisible(settings);
}

export async function hidePetDesktopWindow() {
  const window = await getPetDesktopWindow();
  if (!window) {
    return null;
  }

  await closePetDesktopWindow();
  return null;
}
