import { confirm } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { formatMessage, getMessages } from "@/shared/i18n";
import { createLocalMembersService } from "@/shared/services/tauri/members";
import { publishEntityChanged } from "@/shared/services/ui/windows";
import { destroyCurrentWindow, setCurrentWindowTitle } from "@/shared/services/tauri/window";
import type { MeetingMember } from "@/shared/types/meeting";

const membersService = createLocalMembersService();

function createDraft(member?: MeetingMember): MeetingMember {
  return {
    id: member?.id ?? crypto.randomUUID(),
    name: member?.name ?? "",
    department: member?.department ?? "",
    sortOrder: member?.sortOrder ?? 0,
    isRecorder: member?.isRecorder ?? false,
    createdAt: member?.createdAt ?? new Date().toISOString(),
    updatedAt: member?.updatedAt ?? new Date().toISOString(),
  };
}

export default function MemberEditorView() {
  const meetingStore = useMeetingStore();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState("");
  const [draft, setDraft] = useState<MeetingMember>(() => createDraft());
  const [dirty, setDirty] = useState(false);
  const messages = getMessages(meetingStore.settings.locale).members;
  const commonMessages = getMessages(meetingStore.settings.locale).common;

  useEffect(() => {
    void initialize().catch((error) => {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    });
  }, []);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | null = null;
    void getCurrentWindow().onCloseRequested(async (event) => {
      if (!dirty) return;
      event.preventDefault();
      const shouldClose = await confirm(messages.reset, {
        title: commonMessages.closeWindow,
        kind: "warning",
        okLabel: commonMessages.closeWindow,
        cancelLabel: commonMessages.cancel,
      });
      if (shouldClose) await destroyCurrentWindow();
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
    await meetingStore.ensureSettingsLoaded();
    const loadedMembers = await membersService.listMembers();
    const memberId = new URLSearchParams(window.location.search).get("id");

    if (memberId) {
      const member = loadedMembers.find((item) => item.id === memberId);
      if (member) {
        setSelectedId(member.id);
        setDraft(createDraft(member));
        setDirty(false);
        await syncWindowTitle(true);
        return;
      }
    }

    await syncWindowTitle(false);
  }

  function patchDraft(patch: Partial<MeetingMember>) {
    setDraft((current) => ({ ...current, ...patch }));
    setDirty(true);
  }

  function validateDraft() {
    if (!draft.name.trim()) {
      return messages.validationName;
    }
    if (!Number.isInteger(Number(draft.sortOrder))) {
      return messages.validationSortOrder;
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

  async function saveMember() {
    const validation = validateDraft();
    if (validation) {
      setErrorMessage(validation);
      return;
    }

    const now = new Date().toISOString();
    const nextMember: MeetingMember = {
      id: draft.id,
      name: draft.name.trim(),
      department: draft.department.trim(),
      sortOrder: Number(draft.sortOrder),
      isRecorder: draft.isRecorder,
      createdAt: draft.createdAt,
      updatedAt: now,
    };

    try {
      await membersService.saveMember(nextMember);
      setSelectedId(nextMember.id);
      setDraft({ ...nextMember });
      setDirty(false);
      setErrorMessage("");
      await publishEntityChanged({ entity: "member", id: nextMember.id, action: "saved" }).catch(() => undefined);
      await syncWindowTitle(true);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  }

  async function removeMember() {
    if (!selectedId) {
      return;
    }

    const confirmed = await confirm(formatMessage(messages.deleteConfirm, { name: draft.name.trim() }), {
      title: messages.deleteTitle,
      kind: "warning",
      okLabel: commonMessages.delete,
      cancelLabel: commonMessages.cancel,
    });
    if (!confirmed) {
      return;
    }

    try {
      await membersService.deleteMember(selectedId);
      await publishEntityChanged({ entity: "member", id: selectedId, action: "deleted" }).catch(() => undefined);
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
    setDraft(createDraft());
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
              <label htmlFor="member-name">{messages.name}</label>
              <input id="member-name" value={draft.name} onChange={(event) => patchDraft({ name: event.target.value })} placeholder={messages.namePlaceholder} />
            </div>
            <div className="field">
              <label htmlFor="member-department">{messages.department}</label>
              <input id="member-department" value={draft.department} onChange={(event) => patchDraft({ department: event.target.value })} placeholder={messages.departmentPlaceholder} />
            </div>
            <div className="field">
              <label htmlFor="member-sort-order">{messages.sortOrder}</label>
              <input id="member-sort-order" value={draft.sortOrder} onChange={(event) => patchDraft({ sortOrder: Number(event.target.value) })} type="number" step="1" placeholder={messages.sortOrderPlaceholder} />
            </div>
            <div className="field-grid two-col">
              <label className="toggle-field">
                <input checked={draft.isRecorder} onChange={(event) => patchDraft({ isRecorder: event.target.checked })} type="checkbox" />
                <span>{messages.recorderSwitch}</span>
              </label>
            </div>
          </div>
        </div>

        {errorMessage && <div className="note-block error-block">{errorMessage}</div>}

        <div className="button-row">
          <button className="primary-button" type="button" onClick={saveMember}>
            {messages.save}
          </button>
          <button className="secondary-button" type="button" onClick={() => void resetDraft()}>
            {messages.reset}
          </button>
          {selectedId && (
            <button className="text-button danger-text" type="button" onClick={removeMember}>
              {commonMessages.delete}
            </button>
          )}
        </div>
      </article>
    </section>
  );
}
