import { confirm, message, open, save } from "@tauri-apps/plugin-dialog";
import SemiAvatar from "@douyinfe/semi-ui/lib/es/avatar";
import SemiDivider from "@douyinfe/semi-ui/lib/es/divider";
import SemiEmpty from "@douyinfe/semi-ui/lib/es/empty";
import SemiSpace from "@douyinfe/semi-ui/lib/es/space";
import SemiTable from "@douyinfe/semi-ui/lib/es/table";
import type { ColumnProps } from "@douyinfe/semi-ui/lib/es/table";
import SemiTag from "@douyinfe/semi-ui/lib/es/tag";
import SemiTypography from "@douyinfe/semi-ui/lib/es/typography";
import { useEffect, useMemo, useState } from "react";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { Button } from "@/shared/components/ui";
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
  const latestMember = [...members].sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))[0] ?? null;

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

  function formatUpdatedAt(value: string) {
    return new Date(value).toLocaleString(meetingStore.settings.locale, {
      year: "2-digit",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  const columns: ColumnProps<MeetingMember>[] = [
    {
      title: messages.name,
      dataIndex: "name",
      width: 260,
      render: (_value, member) => (
        <SemiSpace align="center" className="resource-management-primary" spacing={12}>
          <SemiAvatar
            className="resource-management-avatar resource-management-avatar--circle"
            color="blue"
            shape="circle"
            size="36px"
          >
            {member.name.trim().slice(0, 1).toLocaleUpperCase(meetingStore.settings.locale) || "?"}
          </SemiAvatar>
          <SemiTypography.Text
            className="resource-management-primary-text"
            ellipsis={{ showTooltip: true }}
            strong
          >
            {member.name}
          </SemiTypography.Text>
        </SemiSpace>
      ),
    },
    {
      title: messages.department,
      dataIndex: "department",
      width: 240,
      render: (_value, member) => (
        <SemiTypography.Text className="resource-management-text" ellipsis={{ showTooltip: true }}>
          {member.department || messages.emptyDepartment}
        </SemiTypography.Text>
      ),
    },
    {
      title: messages.sortOrder,
      dataIndex: "sortOrder",
      width: 110,
      render: (_value, member) => (
        <SemiTypography.Text className="resource-management-text" type="secondary">
          {member.sortOrder}
        </SemiTypography.Text>
      ),
    },
    {
      title: messages.role,
      width: 150,
      render: (_value, member) => (
        <SemiTag color={member.isRecorder ? "blue" : "grey"} shape="circle" type="light">
          {member.isRecorder ? messages.recorderTag : messages.normalTag}
        </SemiTag>
      ),
    },
    {
      title: messages.updatedAt,
      dataIndex: "updatedAt",
      width: 170,
      render: (_value, member) => (
        <time className="resource-management-time" dateTime={member.updatedAt}>
          <SemiTypography.Text type="secondary">{formatUpdatedAt(member.updatedAt)}</SemiTypography.Text>
        </time>
      ),
    },
    {
      title: messages.actions,
      align: "right",
      fixed: "right",
      width: 160,
      render: (_value, member) => (
        <SemiSpace align="center" className="resource-management-table-actions" spacing={4}>
          <Button
            onClick={(event) => {
              event.stopPropagation();
              void openMemberEditorWindow(member.id);
            }}
            size="small"
            variant="text"
          >
            {commonMessages.edit}
          </Button>
          <Button
            onClick={(event) => {
              event.stopPropagation();
              void removeMember(member);
            }}
            size="small"
            variant="danger"
          >
            {commonMessages.delete}
          </Button>
        </SemiSpace>
      ),
    },
  ];

  return (
    <section className="native-page resource-management-page">
      <header className="resource-management-header">
        <div>
          <h2>{messages.title}</h2>
          <p>{messages.copy}</p>
        </div>
        <div className="resource-management-actions">
          <Button onClick={() => void importMembers()} variant="secondary">{commonMessages.import}</Button>
          <Button onClick={() => void exportMembers()} variant="secondary">{commonMessages.export}</Button>
          <Button onClick={() => void openMemberEditorWindow()} variant="primary">{messages.add}</Button>
        </div>
      </header>

      <section className="resource-management-metrics" aria-label={messages.listTitle}>
        <ManagementMetric label={messages.total} value={members.length} />
        <ManagementMetric label={messages.departmentCount} value={departmentCount} />
        <ManagementMetric label={messages.recorder} value={recorderMember?.name ?? commonMessages.notSet} />
        <ManagementMetric label={messages.latest} value={latestMember?.name ?? commonMessages.noData} />
      </section>

      <SemiDivider className="resource-management-divider" />

      <section className="resource-management-workspace">
        <div className="resource-management-notice">{messages.importHint}</div>
        {errorMessage && <div className="resource-management-error" role="alert">{errorMessage}</div>}
        <SemiTable<MeetingMember>
          aria-busy={loading}
          aria-label={messages.listTitle}
          className="resource-management-table"
          columns={columns}
          dataSource={sortedMembers}
          empty={(
            <SemiEmpty
              className="resource-management-empty"
              description={loading ? messages.loading : messages.copy}
              title={loading ? messages.loading : messages.empty}
            />
          )}
          loading={loading}
          onRow={(member) => ({
            onDoubleClick: () => {
              if (member) {
                void openMemberEditorWindow(member.id);
              }
            },
          })}
          pagination={false}
          rowKey={(member) => member ? member.id : ""}
          scroll={{ x: 1090 }}
        />
      </section>
    </section>
  );
}

function ManagementMetric({ label, value }: { label: string; value: number | string }) {
  return (
    <div className="resource-management-metric">
      <span>{label}</span>
      <strong title={String(value)}>{value}</strong>
    </div>
  );
}
