import type { MessageTree } from "@/shared/i18n/messages/types";
import type { NavIconKey } from "@/shared/services/ui/navIcons";

export type SettingsSectionId =
  | "appearance"
  | "theme"
  | "runtime-overview"
  | "processing-defaults"
  | "local-runtime"
  | "remote-compatibility"
  | "diagnostics";

export type SettingsNavigationItem = {
  id: SettingsSectionId;
  label: string;
  description: string;
  icon: NavIconKey;
};

export type SettingsNavigationGroup = {
  label: string;
  items: SettingsNavigationItem[];
};

let activeSettingsSection: SettingsSectionId = "appearance";
const listeners = new Set<() => void>();

export function getSettingsNavigationGroups(
  messages: MessageTree["settings"],
): SettingsNavigationGroup[] {
  return [
    {
      label: messages.personalizationGroup,
      items: [
        {
          id: "appearance",
          label: messages.appearance,
          description: messages.appearanceHint,
          icon: "appearance",
        },
        {
          id: "theme",
          label: messages.themeSection,
          description: messages.themeSectionHint,
          icon: "accent",
        },
      ],
    },
    {
      label: messages.processingGroup,
      items: [
        {
          id: "runtime-overview",
          label: messages.runtimeOverview,
          description: messages.runtimeOverviewHint,
          icon: "mode",
        },
        {
          id: "processing-defaults",
          label: messages.processingDefaults,
          description: messages.processingDefaultsHint,
          icon: "processing",
        },
        {
          id: "local-runtime",
          label: messages.localRuntime,
          description: messages.localRuntimeHint,
          icon: "chip",
        },
      ],
    },
    {
      label: messages.systemGroup,
      items: [
        {
          id: "remote-compatibility",
          label: messages.remoteCompatibility,
          description: messages.remoteCompatibilityHint,
          icon: "remote",
        },
        {
          id: "diagnostics",
          label: messages.diagnostics,
          description: messages.diagnosticsHint,
          icon: "diagnostics",
        },
      ],
    },
  ];
}

export function getActiveSettingsSection() {
  return activeSettingsSection;
}

export function setActiveSettingsSection(section: SettingsSectionId) {
  if (activeSettingsSection === section) {
    return;
  }
  activeSettingsSection = section;
  listeners.forEach((listener) => listener());
}

export function subscribeActiveSettingsSection(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
