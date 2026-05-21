import { confirm } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import { useAiStore } from "@/features/ai/stores/useAiStore";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { formatMessage, getMessages } from "@/shared/i18n";
import type { AiSummaryTemplate } from "@/shared/types/meeting";

export default function TemplateEditorView() {
  const aiStore = useAiStore();
  const meetingStore = useMeetingStore();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [draft, setDraft] = useState<AiSummaryTemplate>(() => aiStore.createTemplate());
  const [errorMessage, setErrorMessage] = useState("");
  const messages = getMessages(meetingStore.settings.locale).templates;
  const commonMessages = getMessages(meetingStore.settings.locale).common;
  const selectedTemplate = selectedId ? aiStore.getTemplateById(selectedId) ?? null : null;

  useEffect(() => {
    void initialize();
  }, []);

  async function initialize() {
    await aiStore.ensureLoaded();
    const templateId = new URLSearchParams(window.location.search).get("id");

    if (templateId) {
      const template = aiStore.getTemplateById(templateId);
      if (template) {
        setSelectedId(template.id);
        setDraft({ ...template });
        await syncWindowTitle(true);
        return;
      }
    }

    setSelectedId(null);
    setDraft(aiStore.createTemplate());
    await syncWindowTitle(false);
  }

  function patchDraft(patch: Partial<AiSummaryTemplate>) {
    setDraft((current) => ({ ...current, ...patch }));
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
      await getCurrentWindow().setTitle(isEdit ? messages.editorEditTitle : messages.editorNewTitle);
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

    await aiStore.saveTemplate({
      ...draft,
      name: draft.name.trim(),
      description: draft.description.trim(),
      prompt: draft.prompt.trim(),
    });

    setSelectedId(draft.id);
    setDraft({ ...(aiStore.getTemplateById(draft.id) ?? draft) });
    setErrorMessage("");
    await syncWindowTitle(true);
  }

  async function duplicateTemplate() {
    if (!selectedTemplate) {
      return;
    }

    const duplicated = aiStore.duplicateTemplate(selectedTemplate.id);
    if (!duplicated) {
      return;
    }

    await aiStore.insertTemplate(duplicated);
    setSelectedId(duplicated.id);
    setDraft({ ...(aiStore.getTemplateById(duplicated.id) ?? duplicated) });
    setErrorMessage("");
    await syncWindowTitle(true);
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

    await aiStore.deleteTemplate(selectedTemplate.id);
    resetDraft();
  }

  function resetDraft() {
    setSelectedId(null);
    setDraft(aiStore.createTemplate());
    setErrorMessage("");
    void syncWindowTitle(false);
  }

  return (
    <section className="editor-window-shell native-editor-window">
      <article className="surface editor-window-card native-editor-card">
        <div className="section-heading">
          <h3>{selectedId ? messages.editorEditTitle : messages.editorNewTitle}</h3>
          {selectedTemplate && (
            <button className="secondary-button" type="button" onClick={duplicateTemplate}>
              {messages.duplicate}
            </button>
          )}
        </div>

        <div className="native-editor-layout">
          <aside className="native-editor-aside">
            <strong>{messages.title}</strong>
            <p>{messages.copy}</p>
            <span>{draft.builtin ? messages.builtin : messages.custom}</span>
          </aside>

          <div className="field-grid native-editor-form">
            <div className="field-grid two-col">
              <div className="field">
                <label htmlFor="template-name">{messages.name}</label>
                <input id="template-name" value={draft.name} onChange={(event) => patchDraft({ name: event.target.value })} readOnly={draft.builtin} />
              </div>
              <div className="field">
                <label htmlFor="template-description">{messages.description}</label>
                <input id="template-description" value={draft.description} onChange={(event) => patchDraft({ description: event.target.value })} readOnly={draft.builtin} />
              </div>
            </div>

            <div className="field-grid two-col">
              <label className="toggle-field">
                <input checked={draft.includeSpeakerByDefault} onChange={(event) => patchDraft({ includeSpeakerByDefault: event.target.checked })} type="checkbox" disabled={draft.builtin} />
                <span>{messages.includeSpeakerDefault}</span>
              </label>
              <label className="toggle-field">
                <input checked={draft.includeTimestampByDefault} onChange={(event) => patchDraft({ includeTimestampByDefault: event.target.checked })} type="checkbox" disabled={draft.builtin} />
                <span>{messages.includeTimestampDefault}</span>
              </label>
            </div>

            <div className="field">
              <label htmlFor="template-prompt">{messages.prompt}</label>
              <textarea id="template-prompt" value={draft.prompt} onChange={(event) => patchDraft({ prompt: event.target.value })} readOnly={draft.builtin} placeholder={messages.promptPlaceholder} />
            </div>
          </div>
        </div>

        {errorMessage && <div className="note-block error-block">{errorMessage}</div>}

        <div className="button-row">
          <button className="primary-button" type="button" disabled={draft.builtin} onClick={save}>
            {messages.save}
          </button>
          <button className="secondary-button" type="button" onClick={resetDraft}>
            {messages.reset}
          </button>
          {selectedTemplate && !selectedTemplate.builtin && (
            <button className="text-button danger-text" type="button" onClick={removeTemplate}>
              {commonMessages.delete}
            </button>
          )}
        </div>
      </article>
    </section>
  );
}
