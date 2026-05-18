import type { LocaleCode } from "@/shared/types/meeting";
import { enUSMessages } from "./messages/en-US";
import { zhCNMessages } from "./messages/zh-CN";
import type { MessageTree } from "./messages/types";

export type { MessageTree };

const messages: Record<LocaleCode, MessageTree> = {
  "zh-CN": zhCNMessages,
  "en-US": enUSMessages,
};

export function resolveLocale(locale?: string | null): LocaleCode {
  return locale === "en-US" ? "en-US" : "zh-CN";
}

export function getCurrentLocale(): LocaleCode {
  if (typeof document === "undefined") {
    return "zh-CN";
  }

  return resolveLocale(document.documentElement.lang);
}

export function formatMessage(template: string, values: Record<string, string | number>) {
  return template.replace(/\{(\w+)\}/g, (_, key: string) => String(values[key] ?? ""));
}

export function getMessages(locale: LocaleCode): MessageTree {
  return messages[locale] ?? messages["zh-CN"];
}

export function getCurrentMessages(): MessageTree {
  return getMessages(getCurrentLocale());
}
