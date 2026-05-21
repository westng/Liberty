import { confirm, message } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import { useAiStore } from "@/features/ai/stores/useAiStore";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { formatMessage, getMessages } from "@/shared/i18n";
import type { AiModelConfig } from "@/shared/types/meeting";

export default function ModelEditorView() {
  const aiStore = useAiStore();
  const meetingStore = useMeetingStore();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [draft, setDraft] = useState<AiModelConfig>(() => aiStore.createModel());
  const [errorMessage, setErrorMessage] = useState("");
  const messages = getMessages(meetingStore.settings.locale).models;
  const commonMessages = getMessages(meetingStore.settings.locale).common;
  const selectedModel = selectedId ? aiStore.getModelById(selectedId) ?? null : null;

  useEffect(() => {
    void initialize();
  }, []);

  async function initialize() {
    await aiStore.ensureLoaded();
    const modelId = new URLSearchParams(window.location.search).get("id");

    if (modelId) {
      const model = aiStore.getModelById(modelId);
      if (model) {
        setSelectedId(model.id);
        setDraft({ ...model });
        await syncWindowTitle(true);
        return;
      }
    }

    setSelectedId(null);
    setDraft(aiStore.createModel());
    await syncWindowTitle(false);
  }

  function patchDraft(patch: Partial<AiModelConfig>) {
    setDraft((current) => ({ ...current, ...patch }));
  }

  function validateDraft() {
    if (!draft.name.trim()) {
      return messages.validationName;
    }
    if (!draft.baseUrl.trim()) {
      return messages.validationBaseUrl;
    }
    if (!selectedId && !draft.apiKey.trim()) {
      return messages.validationApiKey;
    }
    if (!draft.model.trim()) {
      return messages.validationModel;
    }
    return "";
  }

  async function syncWindowTitle(isEdit: boolean) {
    try {
      await getCurrentWindow().setTitle(isEdit ? messages.editorEditTitle : messages.editorNewTitle);
    } catch {
      // ignore
    }
  }

  async function save() {
    const validation = validateDraft();
    const isEditing = !!selectedId;

    if (validation) {
      setErrorMessage(validation);
      return;
    }

    await aiStore.saveModel({
      ...draft,
      name: draft.name.trim(),
      baseUrl: draft.baseUrl.trim(),
      apiKey: draft.apiKey.trim(),
      model: draft.model.trim(),
    });

    setSelectedId(draft.id);
    setDraft({ ...(aiStore.getModelById(draft.id) ?? draft) });
    setErrorMessage("");
    await syncWindowTitle(true);
    await message(
      formatMessage(isEditing ? messages.saveSuccessUpdated : messages.saveSuccessCreated, { name: draft.name }),
      { title: messages.title, kind: "info" },
    );
  }

  async function removeModel() {
    if (!selectedModel) {
      return;
    }

    const confirmed = await confirm(formatMessage(messages.deleteConfirm, { name: selectedModel.name }), {
      title: messages.deleteTitle,
      kind: "warning",
      okLabel: commonMessages.delete,
      cancelLabel: commonMessages.cancel,
    });
    if (!confirmed) {
      return;
    }

    await aiStore.deleteModel(selectedModel.id);
    resetDraft();
  }

  function resetDraft() {
    setSelectedId(null);
    setDraft(aiStore.createModel());
    setErrorMessage("");
    void syncWindowTitle(false);
  }

  return (
    <section className="editor-window-shell native-editor-window">
      <article className="surface editor-window-card native-editor-card">
        <div className="section-heading">
          <h3>{selectedId ? messages.editorEditTitle : messages.editorNewTitle}</h3>
        </div>

        <div className="native-editor-layout">
          <aside className="native-editor-aside">
            <strong>{messages.title}</strong>
            <p>{messages.copy}</p>
            <span>{selectedId ? messages.editorEditTitle : messages.editorNewTitle}</span>
          </aside>

          <div className="field-grid native-editor-form">
            <div className="field">
              <label htmlFor="model-name">{messages.name}</label>
              <input id="model-name" value={draft.name} onChange={(event) => patchDraft({ name: event.target.value })} placeholder={messages.namePlaceholder} />
            </div>
            <div className="field">
              <label htmlFor="model-base-url">{messages.baseUrl}</label>
              <input id="model-base-url" value={draft.baseUrl} onChange={(event) => patchDraft({ baseUrl: event.target.value })} placeholder={messages.baseUrlPlaceholder} />
            </div>
            <div className="field">
              <label htmlFor="model-api-key">{messages.apiKey}</label>
              <input id="model-api-key" value={draft.apiKey} onChange={(event) => patchDraft({ apiKey: event.target.value })} type="password" placeholder={messages.apiKeyPlaceholder} autoComplete="off" />
            </div>
            <div className="field">
              <label htmlFor="model-id">{messages.model}</label>
              <input id="model-id" value={draft.model} onChange={(event) => patchDraft({ model: event.target.value })} placeholder={messages.modelPlaceholder} />
              <p className="field-copy">{messages.modelHelp}</p>
            </div>
            <div className="field-grid two-col">
              <label className="toggle-field">
                <input checked={draft.enabled} onChange={(event) => patchDraft({ enabled: event.target.checked })} type="checkbox" />
                <span>{messages.enabledSwitch}</span>
              </label>
              <label className="toggle-field">
                <input checked={draft.isDefault} onChange={(event) => patchDraft({ isDefault: event.target.checked })} type="checkbox" />
                <span>{messages.defaultSwitch}</span>
              </label>
            </div>
          </div>
        </div>

        {errorMessage && <div className="note-block error-block">{errorMessage}</div>}

        <div className="button-row">
          <button className="primary-button" type="button" onClick={save}>
            {messages.save}
          </button>
          <button className="secondary-button" type="button" onClick={resetDraft}>
            {messages.reset}
          </button>
          {selectedId && (
            <button className="text-button danger-text" type="button" onClick={removeModel}>
              {commonMessages.delete}
            </button>
          )}
        </div>
      </article>
    </section>
  );
}
