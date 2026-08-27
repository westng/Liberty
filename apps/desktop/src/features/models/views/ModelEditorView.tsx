import { confirm, message } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import { useAiStore } from "@/features/ai/stores/useAiStore";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { Button, PasswordInput, Switch, TextInput } from "@/shared/components/ui";
import { formatMessage, getMessages } from "@/shared/i18n";
import { publishEntityChanged } from "@/shared/services/ui/windows";
import { handleEditorWindowCloseRequested, setCurrentWindowTitle } from "@/shared/services/tauri/window";
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
      await handleEditorWindowCloseRequested(event, dirty, () =>
        confirm(messages.reset, {
          title: commonMessages.closeWindow,
          kind: "warning",
          okLabel: commonMessages.closeWindow,
          cancelLabel: commonMessages.cancel,
        }),
      );
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
      <article
        className="editor-window-card native-editor-card"
        aria-label={selectedId ? messages.editorEditTitle : messages.editorNewTitle}
      >
        <div className="native-editor-body">
          {errorMessage && <div className="note-block error-block" role="alert">{errorMessage}</div>}
          <div className="field-grid native-editor-form">
            <div className="field">
              <label htmlFor="model-name">{messages.name}</label>
              <TextInput id="model-name" value={draft.name} onChange={(value) => patchDraft({ name: value })} placeholder={messages.namePlaceholder} />
            </div>
            <div className="field">
              <label htmlFor="model-base-url">{messages.baseUrl}</label>
              <TextInput id="model-base-url" value={draft.baseUrl} onChange={(value) => patchDraft({ baseUrl: value })} placeholder={messages.baseUrlPlaceholder} />
            </div>
            <div className="field">
              <label htmlFor="model-api-key">{messages.apiKey}</label>
              <PasswordInput
                id="model-api-key"
                value={apiKey}
                onChange={(value) => updateApiKey(value)}
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
                <Button
                  type="button"
                  variant={storedCredentialAction === "clear" ? "text" : "danger"}
                  onClick={storedCredentialAction === "clear" ? keepCredential : clearCredential}
                >
                  {storedCredentialAction === "clear" ? messages.keepCredential : messages.clearCredential}
                </Button>
              )}
            </div>
            <div className="field">
              <label htmlFor="model-id">{messages.model}</label>
              <TextInput
                aria-describedby="model-id-help"
                id="model-id"
                value={draft.model}
                onChange={(value) => patchDraft({ model: value })}
                placeholder={messages.modelPlaceholder}
              />
              <p className="field-copy" id="model-id-help">{messages.modelHelp}</p>
            </div>
            <div className="native-editor-switches">
              <Switch checked={draft.enabled} id="model-enabled" label={messages.enabledSwitch} onChange={(checked) => patchDraft({ enabled: checked })} wrapperClassName="native-editor-switch" />
              <Switch checked={draft.isDefault} id="model-default" label={messages.defaultSwitch} onChange={(checked) => patchDraft({ isDefault: checked })} wrapperClassName="native-editor-switch" />
            </div>
          </div>
        </div>

        <footer className="native-editor-actions">
          <div className="native-editor-leading-actions">
            {selectedId && (
              <Button type="button" variant="danger" onClick={removeModel}>
                {commonMessages.delete}
              </Button>
            )}
          </div>
          <div className="native-editor-primary-actions">
            <Button type="button" variant="secondary" onClick={() => void resetDraft()}>
              {messages.reset}
            </Button>
            <Button type="button" variant="primary" onClick={save}>
              {messages.save}
            </Button>
          </div>
        </footer>
      </article>
    </section>
  );
}
