import { confirm, message } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import { useAiStore } from "@/features/ai/stores/useAiStore";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { formatMessage, getMessages } from "@/shared/i18n";
import { publishEntityChanged } from "@/shared/services/ui/windows";
import { destroyCurrentWindow, setCurrentWindowTitle } from "@/shared/services/tauri/window";
import type { AiModelConfig, AiModelCredentialUpdate } from "@/shared/types/meeting";

type StoredCredentialAction = "keep" | "clear";

export default function ModelEditorView() {
  const aiStore = useAiStore();
  const meetingStore = useMeetingStore();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [draft, setDraft] = useState<AiModelConfig>(() => aiStore.createModel());
  const [apiKey, setApiKey] = useState("");
  const [storedCredentialAction, setStoredCredentialAction] = useState<StoredCredentialAction>("keep");
  const [errorMessage, setErrorMessage] = useState("");
  const [dirty, setDirty] = useState(false);
  const messages = getMessages(meetingStore.settings.locale).models;
  const commonMessages = getMessages(meetingStore.settings.locale).common;
  const selectedModel = selectedId ? aiStore.getModelById(selectedId) ?? null : null;

  useEffect(() => {
    void initialize().catch((error) => {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    });
  }, []);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | null = null;
    void getCurrentWindow().onCloseRequested(async (event) => {
      if (!dirty) {
        return;
      }
      event.preventDefault();
      const shouldClose = await confirm(messages.reset, {
        title: commonMessages.closeWindow,
        kind: "warning",
        okLabel: commonMessages.closeWindow,
        cancelLabel: commonMessages.cancel,
      });
      if (shouldClose) {
        await destroyCurrentWindow();
      }
    }).then((stop) => {
      if (active) unlisten = stop;
      else stop();
    });
    return () => {
      active = false;
      unlisten?.();
    };
  }, [dirty, messages, commonMessages]);

  async function initialize() {
    await aiStore.ensureModelsLoaded();
    const modelId = new URLSearchParams(window.location.search).get("id");

    if (modelId) {
      const model = aiStore.getModelById(modelId);
      if (model) {
        setSelectedId(model.id);
        setDraft({ ...model });
        setApiKey("");
        setStoredCredentialAction("keep");
        setDirty(false);
        await syncWindowTitle(true);
        return;
      }
    }

    setSelectedId(null);
    setDraft(aiStore.createModel());
    setApiKey("");
    setStoredCredentialAction("keep");
    setDirty(false);
    await syncWindowTitle(false);
  }

  function patchDraft(patch: Partial<AiModelConfig>) {
    setDraft((current) => ({ ...current, ...patch }));
    setDirty(true);
  }

  function updateApiKey(value: string) {
    setApiKey(value);
    setStoredCredentialAction("keep");
    setDirty(true);
  }

  function clearCredential() {
    setApiKey("");
    setStoredCredentialAction("clear");
    setDirty(true);
  }

  function keepCredential() {
    setStoredCredentialAction("keep");
    setDirty(true);
  }

  function credentialUpdate(): AiModelCredentialUpdate {
    const value = apiKey.trim();
    if (value) {
      return { action: "set", value };
    }
    return { action: storedCredentialAction };
  }

  function validateDraft() {
    if (!draft.name.trim()) {
      return messages.validationName;
    }
    if (!draft.baseUrl.trim()) {
      return messages.validationBaseUrl;
    }
    if (!selectedId && !apiKey.trim()) {
      return messages.validationApiKey;
    }
    if (!draft.model.trim()) {
      return messages.validationModel;
    }
    return "";
  }

  async function syncWindowTitle(isEdit: boolean) {
    try {
      await setCurrentWindowTitle(isEdit ? messages.editorEditTitle : messages.editorNewTitle);
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

    try {
      await aiStore.saveModel({
        id: draft.id,
        name: draft.name.trim(),
        baseUrl: draft.baseUrl.trim(),
        model: draft.model.trim(),
        enabled: draft.enabled,
        isDefault: draft.isDefault,
        createdAt: draft.createdAt,
        updatedAt: draft.updatedAt,
        credential: credentialUpdate(),
      });
      setSelectedId(draft.id);
      setDraft({ ...(aiStore.getModelById(draft.id) ?? draft) });
      setApiKey("");
      setStoredCredentialAction("keep");
      setDirty(false);
      setErrorMessage("");
      await publishEntityChanged({ entity: "model", id: draft.id, action: "saved" }).catch(() => undefined);
      await syncWindowTitle(true);
      await message(
        formatMessage(isEditing ? messages.saveSuccessUpdated : messages.saveSuccessCreated, { name: draft.name }),
        { title: messages.title, kind: "info" },
      );
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
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

    try {
      await aiStore.deleteModel(selectedModel.id);
      await publishEntityChanged({ entity: "model", id: selectedModel.id, action: "deleted" }).catch(() => undefined);
      resetDraft(true);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  }

  async function resetDraft(force = false) {
    if (!force && dirty) {
      const confirmed = await confirm(messages.reset, {
        title: messages.reset,
        kind: "warning",
        okLabel: messages.reset,
        cancelLabel: commonMessages.cancel,
      });
      if (!confirmed) return;
    }
    setSelectedId(null);
    setDraft(aiStore.createModel());
    setApiKey("");
    setStoredCredentialAction("keep");
    setDirty(false);
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
              <input
                id="model-api-key"
                value={apiKey}
                onChange={(event) => updateApiKey(event.target.value)}
                type="password"
                placeholder={selectedId ? messages.apiKeyKeepPlaceholder : messages.apiKeyPlaceholder}
                autoComplete="new-password"
              />
              {selectedId && storedCredentialAction === "clear" ? (
                <p className="field-copy danger-text">{messages.credentialClearPending}</p>
              ) : selectedId && draft.credentialPresent ? (
                <p className="field-copy">{messages.credentialStored}</p>
              ) : selectedId ? (
                <p className="field-copy">{messages.credentialMissing}</p>
              ) : null}
              {selectedId && draft.credentialPresent && !apiKey.trim() && (
                <button
                  className={storedCredentialAction === "clear" ? "text-button" : "text-button danger-text"}
                  type="button"
                  onClick={storedCredentialAction === "clear" ? keepCredential : clearCredential}
                >
                  {storedCredentialAction === "clear" ? messages.keepCredential : messages.clearCredential}
                </button>
              )}
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
          <button className="secondary-button" type="button" onClick={() => void resetDraft()}>
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
