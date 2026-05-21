import { confirm, message, open, save } from "@tauri-apps/plugin-dialog";
import { useEffect, useMemo, useState } from "react";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { formatMessage, getMessages } from "@/shared/i18n";
import { createLocalMembersService } from "@/shared/services/tauri/members";
import { openMemberEditorWindow } from "@/shared/services/ui/windows";
import type { MeetingMember } from "@/shared/types/meeting";

const membersService = createLocalMembersService();

export default function MemberManagementView() {
  const meetingStore = useMeetingStore();
  const messages = getMessages(meetingStore.settings.locale).members;
  const commonMessages = getMessages(meetingStore.settings.locale).common;
  const [members, setMembers] = useState<MeetingMember[]>([]);
  const [loading, setLoading] = useState(false);
  const [errorMessage, setErrorMessage] = useState("");
  const sortedMembers = useMemo(
    () => [...members].sort((left, right) => {
      if (left.sortOrder !== right.sortOrder) {
        return left.sortOrder - right.sortOrder;
      }
      const updatedAtDiff = right.updatedAt.localeCompare(left.updatedAt);
      if (updatedAtDiff !== 0) {
        return updatedAtDiff;
      }
      return left.name.localeCompare(right.name, meetingStore.settings.locale);
    }),
    [members, meetingStore.settings.locale],
  );
  const recorderMember = members.find((item) => item.isRecorder) ?? null;
  const departmentCount = new Set(members.map((item) => item.department.trim()).filter(Boolean)).size;
  const latestMember = sortedMembers[0] ?? null;

  useEffect(() => {
    void loadMembers();
    window.addEventListener("focus", handleWindowFocus);
    return () => window.removeEventListener("focus", handleWindowFocus);
  }, []);

  function handleWindowFocus() {
    void loadMembers();
  }

  async function loadMembers() {
    setLoading(true);
    try {
      setMembers(await membersService.listMembers());
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setLoading(false);
    }
  }

  async function removeMember(member: MeetingMember) {
    const confirmed = await confirm(formatMessage(messages.deleteConfirm, { name: member.name }), {
      title: messages.deleteTitle,
      kind: "warning",
      okLabel: commonMessages.delete,
      cancelLabel: commonMessages.cancel,
    });

    if (!confirmed) {
      return;
    }

    try {
      await membersService.deleteMember(member.id);
      await loadMembers();
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  }

  async function importMembers() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Excel", extensions: ["xlsx"] }],
    });

    if (!selected || Array.isArray(selected)) {
      return;
    }

    try {
      const result = await membersService.importMembersExcel(selected);
      await loadMembers();
      await message(formatMessage(messages.importSuccess, { created: result.created, updated: result.updated }), {
        title: messages.title,
        kind: "info",
      });
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  }

  async function exportMembers() {
    const filePath = await save({
      defaultPath: "人员信息.xlsx",
      filters: [{ name: "Excel", extensions: ["xlsx"] }],
    });

    if (!filePath) {
      return;
    }

    try {
      await membersService.exportMembersExcel(filePath);
      await message(formatMessage(messages.exportSuccess, { path: filePath }), {
        title: messages.title,
        kind: "info",
      });
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  }

  return (
    <section className="view-stack native-page native-split-page resource-native-page model-page-stack">
      <article className="surface native-page-hero resource-native-hero">
        <div className="section-heading">
          <div>
            <h3>{messages.title}</h3>
            <p className="section-copy">{messages.copy}</p>
          </div>
          <div className="button-row">
            <button className="secondary-button" type="button" onClick={importMembers}>
              {commonMessages.import}
            </button>
            <button className="secondary-button" type="button" onClick={exportMembers}>
              {commonMessages.export}
            </button>
            <button className="primary-button" type="button" onClick={() => openMemberEditorWindow()}>
              {messages.add}
            </button>
          </div>
        </div>
        <div className="summary-inline">
          <span>{messages.total} {members.length}</span>
          <span>{messages.recorder} {recorderMember?.name ?? commonMessages.notSet}</span>
        </div>
        <p className="section-copy">{messages.importHint}</p>
        {errorMessage && <div className="note-block error-block">{errorMessage}</div>}
      </article>

      <div className="native-split-layout">
        <article className="surface native-list-panel model-list-card">
          <div className="section-heading model-management-header">
            <h3>{messages.listTitle}</h3>
          </div>
          {sortedMembers.length ? (
            <div className="model-list-rows">
              {sortedMembers.map((member) => (
                <article key={member.id} className="model-list-row" onClick={() => openMemberEditorWindow(member.id)}>
                  <div className="model-row-main member-row-main">
                    <strong>{member.name}</strong>
                    <span>{member.department || messages.emptyDepartment}</span>
                  </div>
                  <div className="model-row-side">
                    <span className="record-meta">#{member.sortOrder}</span>
                    <span className={`record-tag ${member.isRecorder ? "active" : ""}`}>
                      {member.isRecorder ? messages.recorderTag : messages.normalTag}
                    </span>
                    <button
                      className="text-button"
                      type="button"
                      onClick={(event) => {
                        event.stopPropagation();
                        void openMemberEditorWindow(member.id);
                      }}
                    >
                      {commonMessages.edit}
                    </button>
                    <button
                      className="text-button danger-text"
                      type="button"
                      onClick={(event) => {
                        event.stopPropagation();
                        void removeMember(member);
                      }}
                    >
                      {commonMessages.delete}
                    </button>
                  </div>
                </article>
              ))}
            </div>
          ) : (
            <div className="empty-state">{loading ? commonMessages.noData : messages.empty}</div>
          )}
        </article>

        <aside className="surface native-inspector-panel">
          <div className="section-heading">
            <h3>{messages.recorder}</h3>
          </div>
          <div className="native-stat-list">
            <div>
              <span>{messages.total}</span>
              <strong>{members.length}</strong>
            </div>
            <div>
              <span>{messages.department}</span>
              <strong>{departmentCount}</strong>
            </div>
            <div>
              <span>{messages.recorder}</span>
              <strong>{recorderMember?.name ?? commonMessages.dash}</strong>
            </div>
          </div>
          <div className="native-inspector-note">
            <span>{messages.listTitle}</span>
            <strong>{latestMember?.name ?? commonMessages.noData}</strong>
            <p>{latestMember?.department || messages.emptyDepartment}</p>
          </div>
        </aside>
      </div>
    </section>
  );
}
