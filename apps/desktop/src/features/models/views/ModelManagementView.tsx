import { confirm } from "@tauri-apps/plugin-dialog";
import { useEffect, useMemo } from "react";
import { useAiStore } from "@/features/ai/stores/useAiStore";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { formatMessage, getMessages } from "@/shared/i18n";
import { openModelEditorWindow } from "@/shared/services/ui/windows";
import type { AiModelConfig } from "@/shared/types/meeting";

export default function ModelManagementView() {
  const aiStore = useAiStore();
  const meetingStore = useMeetingStore();
  const messages = getMessages(meetingStore.settings.locale).models;
  const commonMessages = getMessages(meetingStore.settings.locale).common;
  const models = useMemo(
    () => [...aiStore.models].sort((left, right) => right.updatedAt.localeCompare(left.updatedAt)),
    [aiStore.models],
  );
  const defaultModel = models.find((model) => model.isDefault && model.enabled)
    ?? models.find((model) => model.isDefault)
    ?? null;
  const enabledCount = models.filter((model) => model.enabled).length;
  const disabledCount = models.length - enabledCount;
  const latestModel = models[0] ?? null;

  useEffect(() => {
    void aiStore.ensureModelsLoaded();
    window.addEventListener("focus", handleWindowFocus);
    return () => window.removeEventListener("focus", handleWindowFocus);
  }, []);

  function handleWindowFocus() {
    void aiStore.reloadModels();
  }

  async function removeModel(model: AiModelConfig) {
    const confirmed = await confirm(formatMessage(messages.deleteConfirm, { name: model.name }), {
      title: messages.deleteTitle,
      kind: "warning",
      okLabel: commonMessages.delete,
      cancelLabel: commonMessages.cancel,
    });
    if (!confirmed) {
      return;
    }

    await aiStore.deleteModel(model.id);
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
          <button className="primary-button" type="button" onClick={() => openModelEditorWindow()}>
            {messages.add}
          </button>
        </div>
        <div className="summary-inline">
          <span>{messages.total} {models.length}</span>
          <span>{messages.enabled} {enabledCount}</span>
          <span>{messages.defaultLabel} {defaultModel?.name ?? commonMessages.notSet}</span>
        </div>
      </article>

      <div className="native-split-layout">
        <article className="surface native-list-panel model-list-card">
          <div className="section-heading model-management-header">
            <h3>{messages.listTitle}</h3>
          </div>
          {models.length ? (
            <div className="model-list-rows">
              {models.map((model) => (
                <article key={model.id} className="model-list-row" onClick={() => openModelEditorWindow(model.id)}>
                  <div className="model-row-main">
                    <strong>{model.name}</strong>
                    <span>{model.model}</span>
                  </div>
                  <div className="model-row-side">
                    <span className="record-meta">{formatUpdatedAt(model.updatedAt)}</span>
                    <span className="record-meta">{model.isDefault ? messages.defaultTag : model.enabled ? messages.enabledTag : messages.disabledTag}</span>
                    <button
                      className="text-button"
                      type="button"
                      onClick={(event) => {
                        event.stopPropagation();
                        void openModelEditorWindow(model.id);
                      }}
                    >
                      {commonMessages.edit}
                    </button>
                    <button
                      className="text-button danger-text"
                      type="button"
                      onClick={(event) => {
                        event.stopPropagation();
                        void removeModel(model);
                      }}
                    >
                      {commonMessages.delete}
                    </button>
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
            <h3>{messages.defaultLabel}</h3>
          </div>
          <div className="native-stat-list">
            <div>
              <span>{messages.total}</span>
              <strong>{models.length}</strong>
            </div>
            <div>
              <span>{messages.enabled}</span>
              <strong>{enabledCount}</strong>
            </div>
            <div>
              <span>{messages.disabledTag}</span>
              <strong>{disabledCount}</strong>
            </div>
          </div>
          <div className="native-inspector-note">
            <span>{messages.defaultLabel}</span>
            <strong>{defaultModel?.name ?? commonMessages.notSet}</strong>
            <p>{latestModel ? formatUpdatedAt(latestModel.updatedAt) : commonMessages.noData}</p>
          </div>
        </aside>
      </div>
    </section>
  );
}
