import { confirm } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import { useAiStore } from "@/features/ai/stores/useAiStore";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { Button, Switch, TextInput } from "@/shared/components/ui";
import { formatMessage, getMessages } from "@/shared/i18n";
import { publishEntityChanged } from "@/shared/services/ui/windows";
import { handleEditorWindowCloseRequested, setCurrentWindowTitle } from "@/shared/services/tauri/window";
import type { AiSummaryTemplate } from "@/shared/types/meeting";

export default function TemplateEditorView() {
  const aiStore = useAiStore();
  const meetingStore = useMeetingStore();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [draft, setDraft] = useState<AiSummaryTemplate>(() => aiStore.createTemplate());
  const [errorMessage, setErrorMessage] = useState("");
  const [dirty, setDirty] = useState(false);
  const messages = getMessages(meetingStore.settings.locale).templates;
  const commonMessages = getMessages(meetingStore.settings.locale).common;
  const selectedTemplate = selectedId ? aiStore.getTemplateById(selectedId) ?? null : null;

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
    await aiStore.ensureTemplatesLoaded();
    const templateId = new URLSearchParams(window.location.search).get("id");

    if (templateId) {
      const template = aiStore.getTemplateById(templateId);
      if (template) {
        setSelectedId(template.id);
        setDraft({ ...template });
        setDirty(false);
        await syncWindowTitle(true);
        return;
      }
    }

    setSelectedId(null);
    setDraft(aiStore.createTemplate());
    setDirty(false);
    await syncWindowTitle(false);
  }

  function patchDraft(patch: Partial<AiSummaryTemplate>) {
    setDraft((current) => ({ ...current, ...patch }));
    setDirty(true);
  }

  function validateDraft() {
    if (!draft.name.trim()) {
      return messages.validationName;
    }
    if (!draft.prompt.trim()) {
      return messages.validationPrompt;
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
    if (draft.builtin) {
      setErrorMessage(messages.builtinReadonly);
      return;
    }

    const validation = validateDraft();
    if (validation) {
      setErrorMessage(validation);
      return;
    }

    try {
      await aiStore.saveTemplate({
        ...draft,
        name: draft.name.trim(),
        description: draft.description.trim(),
        prompt: draft.prompt.trim(),
      });
      setSelectedId(draft.id);
      setDraft({ ...(aiStore.getTemplateById(draft.id) ?? draft) });
      setDirty(false);
      setErrorMessage("");
      await publishEntityChanged({ entity: "template", id: draft.id, action: "saved" }).catch(() => undefined);
      await syncWindowTitle(true);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  }

  async function duplicateTemplate() {
    if (!selectedTemplate) {
      return;
    }

    const duplicated = aiStore.duplicateTemplate(selectedTemplate.id);
    if (!duplicated) {
      return;
    }

    try {
      await aiStore.insertTemplate(duplicated);
      setSelectedId(duplicated.id);
      setDraft({ ...(aiStore.getTemplateById(duplicated.id) ?? duplicated) });
      setDirty(false);
      setErrorMessage("");
      await publishEntityChanged({ entity: "template", id: duplicated.id, action: "saved" }).catch(() => undefined);
      await syncWindowTitle(true);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  }

  async function removeTemplate() {
    if (!selectedTemplate || selectedTemplate.builtin) {
      return;
    }

    const confirmed = await confirm(formatMessage(messages.deleteConfirm, { name: selectedTemplate.name }), {
      title: messages.deleteTitle,
      kind: "warning",
      okLabel: commonMessages.delete,
      cancelLabel: commonMessages.cancel,
    });

    if (!confirmed) {
      return;
    }

    try {
      await aiStore.deleteTemplate(selectedTemplate.id);
      await publishEntityChanged({ entity: "template", id: selectedTemplate.id, action: "deleted" }).catch(() => undefined);
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
    setDraft(aiStore.createTemplate());
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
              <label htmlFor="template-name">{messages.name}</label>
              <TextInput id="template-name" value={draft.name} onChange={(value) => patchDraft({ name: value })} readOnly={draft.builtin} />
            </div>
            <div className="field">
              <label htmlFor="template-description">{messages.description}</label>
              <TextInput id="template-description" value={draft.description} onChange={(value) => patchDraft({ description: value })} readOnly={draft.builtin} />
            </div>

            <div className="native-editor-switches">
              <Switch checked={draft.includeSpeakerByDefault} disabled={draft.builtin} id="template-include-speaker" label={messages.includeSpeakerDefault} onChange={(checked) => patchDraft({ includeSpeakerByDefault: checked })} wrapperClassName="native-editor-switch" />
              <Switch checked={draft.includeTimestampByDefault} disabled={draft.builtin} id="template-include-timestamp" label={messages.includeTimestampDefault} onChange={(checked) => patchDraft({ includeTimestampByDefault: checked })} wrapperClassName="native-editor-switch" />
            </div>

            <div className="field">
              <label htmlFor="template-prompt">{messages.prompt}</label>
              <textarea id="template-prompt" value={draft.prompt} onChange={(event) => patchDraft({ prompt: event.target.value })} readOnly={draft.builtin} placeholder={messages.promptPlaceholder} />
            </div>
          </div>
        </div>

        <footer className="native-editor-actions">
          <div className="native-editor-leading-actions">
            {selectedTemplate && (
              <Button type="button" variant="secondary" onClick={duplicateTemplate}>
                {messages.duplicate}
              </Button>
            )}
            {selectedTemplate && !selectedTemplate.builtin && (
              <Button type="button" variant="danger" onClick={removeTemplate}>
                {commonMessages.delete}
              </Button>
            )}
          </div>
          <div className="native-editor-primary-actions">
            <Button type="button" variant="secondary" onClick={() => void resetDraft()}>
              {messages.reset}
            </Button>
            <Button type="button" variant="primary" disabled={draft.builtin} onClick={save}>
              {messages.save}
            </Button>
          </div>
        </footer>
      </article>
    </section>
  );
}
