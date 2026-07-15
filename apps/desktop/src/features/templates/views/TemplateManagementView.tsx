import { confirm } from "@tauri-apps/plugin-dialog";
import { useEffect, useMemo } from "react";
import { useAiStore } from "@/features/ai/stores/useAiStore";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { formatMessage, getMessages } from "@/shared/i18n";
import { openTemplateEditorWindow } from "@/shared/services/ui/windows";
import type { AiSummaryTemplate } from "@/shared/types/meeting";

export default function TemplateManagementView() {
  const aiStore = useAiStore();
  const meetingStore = useMeetingStore();
  const messages = getMessages(meetingStore.settings.locale).templates;
  const commonMessages = getMessages(meetingStore.settings.locale).common;
  const templates = useMemo(
    () => [...aiStore.templates].sort((left, right) => {
      if (left.builtin !== right.builtin) {
        return left.builtin ? -1 : 1;
      }
      return right.updatedAt.localeCompare(left.updatedAt);
    }),
    [aiStore.templates],
  );
  const builtinCount = templates.filter((item) => item.builtin).length;
  const customCount = templates.filter((item) => !item.builtin).length;
  const latestTemplate = templates[0] ?? null;

  useEffect(() => {
    void aiStore.ensureTemplatesLoaded();
    window.addEventListener("focus", handleWindowFocus);
    return () => window.removeEventListener("focus", handleWindowFocus);
  }, []);

  function handleWindowFocus() {
    void aiStore.reloadTemplates();
  }

  async function removeTemplate(template: AiSummaryTemplate) {
    if (template.builtin) {
      return;
    }

    const confirmed = await confirm(formatMessage(messages.deleteConfirm, { name: template.name }), {
      title: messages.deleteTitle,
      kind: "warning",
      okLabel: commonMessages.delete,
      cancelLabel: commonMessages.cancel,
    });

    if (!confirmed) {
      return;
    }

    await aiStore.deleteTemplate(template.id);
  }

  function formatUpdatedAt(value: string) {
    return new Date(value).toLocaleString(meetingStore.settings.locale, {
      year: "2-digit",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  return (
    <section className="view-stack native-page native-split-page resource-native-page model-page-stack">
      <article className="surface native-page-hero resource-native-hero">
        <div className="section-heading">
          <div>
            <h3>{messages.title}</h3>
            <p className="section-copy">{messages.copy}</p>
          </div>
          <button className="primary-button" type="button" onClick={() => openTemplateEditorWindow()}>
            {messages.add}
          </button>
        </div>
        <div className="summary-inline">
          <span>{messages.total} {templates.length}</span>
          <span>{messages.builtin} {builtinCount}</span>
          <span>{messages.custom} {customCount}</span>
        </div>
      </article>

      <div className="native-split-layout">
        <article className="surface native-list-panel model-list-card">
          <div className="section-heading model-management-header">
            <h3>{messages.listTitle}</h3>
          </div>
          {templates.length ? (
            <div className="model-list-rows">
              {templates.map((template) => (
                <article key={template.id} className="model-list-row" onClick={() => openTemplateEditorWindow(template.id)}>
                  <div className="model-row-main">
                    <strong>{template.name}</strong>
                    <span>{template.description || messages.emptyDescription}</span>
                  </div>
                  <div className="model-row-side">
                    <span className="record-meta">{template.builtin ? messages.builtin : messages.custom}</span>
                    <span className="record-meta">{formatUpdatedAt(template.updatedAt)}</span>
                    <button
                      className="text-button"
                      type="button"
                      onClick={(event) => {
                        event.stopPropagation();
                        void openTemplateEditorWindow(template.id);
                      }}
                    >
                      {commonMessages.edit}
                    </button>
                    {!template.builtin && (
                      <button
                        className="text-button danger-text"
                        type="button"
                        onClick={(event) => {
                          event.stopPropagation();
                          void removeTemplate(template);
                        }}
                      >
                        {commonMessages.delete}
                      </button>
                    )}
                  </div>
                </article>
              ))}
            </div>
          ) : (
            <div className="empty-state">{messages.empty}</div>
          )}
        </article>

        <aside className="surface native-inspector-panel">
          <div className="section-heading">
            <h3>{messages.listTitle}</h3>
          </div>
          <div className="native-stat-list">
            <div>
              <span>{messages.total}</span>
              <strong>{templates.length}</strong>
            </div>
            <div>
              <span>{messages.builtin}</span>
              <strong>{builtinCount}</strong>
            </div>
            <div>
              <span>{messages.custom}</span>
              <strong>{customCount}</strong>
            </div>
          </div>
          <div className="native-inspector-note">
            <span>{messages.listTitle}</span>
            <strong>{latestTemplate?.name ?? commonMessages.noData}</strong>
            <p>{latestTemplate ? formatUpdatedAt(latestTemplate.updatedAt) : commonMessages.noData}</p>
          </div>
        </aside>
      </div>
    </section>
  );
}
