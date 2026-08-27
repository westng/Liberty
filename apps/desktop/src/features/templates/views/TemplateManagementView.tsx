import { confirm } from "@tauri-apps/plugin-dialog";
import SemiAvatar from "@douyinfe/semi-ui/lib/es/avatar";
import SemiCard from "@douyinfe/semi-ui/lib/es/card";
import SemiDivider from "@douyinfe/semi-ui/lib/es/divider";
import SemiEmpty from "@douyinfe/semi-ui/lib/es/empty";
import SemiSpace from "@douyinfe/semi-ui/lib/es/space";
import SemiTag from "@douyinfe/semi-ui/lib/es/tag";
import SemiTypography from "@douyinfe/semi-ui/lib/es/typography";
import { useEffect, useMemo } from "react";
import { useAiStore } from "@/features/ai/stores/useAiStore";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { Button } from "@/shared/components/ui";
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
  const latestTemplate = [...templates].sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))[0] ?? null;

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
    <section className="native-page resource-management-page">
      <header className="resource-management-header">
        <div>
          <h2>{messages.title}</h2>
          <p>{messages.copy}</p>
        </div>
        <Button onClick={() => void openTemplateEditorWindow()} variant="primary">{messages.add}</Button>
      </header>

      <section className="resource-management-metrics" aria-label={messages.listTitle}>
        <ManagementMetric label={messages.total} value={templates.length} />
        <ManagementMetric label={messages.builtin} value={builtinCount} />
        <ManagementMetric label={messages.custom} value={customCount} />
        <ManagementMetric label={messages.latest} value={latestTemplate?.name ?? commonMessages.noData} />
      </section>

      <SemiDivider className="resource-management-divider" />

      <section className="resource-management-workspace">
        {templates.length > 0 ? (
          <ul className="resource-management-card-grid" aria-label={messages.listTitle}>
            {templates.map((template) => (
              <li
                key={template.id}
                className="resource-management-card-item"
                onDoubleClick={(event) => {
                  if (!(event.target instanceof Element) || !event.target.closest("button")) {
                    void openTemplateEditorWindow(template.id);
                  }
                }}
              >
                <SemiCard
                  bordered={false}
                  className="resource-management-card resource-management-template-card"
                  footer={(
                    <SemiSpace align="center" className="resource-management-card-actions" spacing={4}>
                      <Button
                        aria-label={`${commonMessages.edit}: ${template.name}`}
                        onClick={() => void openTemplateEditorWindow(template.id)}
                        size="small"
                        variant="text"
                      >
                        {commonMessages.edit}
                      </Button>
                      {!template.builtin && (
                        <Button
                          aria-label={`${commonMessages.delete}: ${template.name}`}
                          onClick={() => void removeTemplate(template)}
                          size="small"
                          variant="danger"
                        >
                          {commonMessages.delete}
                        </Button>
                      )}
                    </SemiSpace>
                  )}
                  header={(
                    <div className="resource-management-card-header">
                      <SemiSpace align="center" className="resource-management-primary" spacing={12}>
                        <SemiAvatar className="resource-management-avatar" color="blue" shape="square" size="40px">
                          T
                        </SemiAvatar>
                        <SemiTypography.Text
                          className="resource-management-card-title"
                          component="h3"
                          ellipsis={{ showTooltip: true }}
                          strong
                        >
                          {template.name}
                        </SemiTypography.Text>
                      </SemiSpace>
                      <SemiSpace className="resource-management-tag-row" spacing={6} wrap>
                        <SemiTag color={template.builtin ? "blue" : "grey"} shape="circle" type="light">
                          {template.builtin ? messages.builtin : messages.custom}
                        </SemiTag>
                      </SemiSpace>
                    </div>
                  )}
                  headerLine={false}
                >
                  <dl className="resource-management-card-details">
                    <div className="resource-management-template-description-row">
                      <dt>{messages.description}</dt>
                      <dd>
                        <SemiTypography.Text
                          className="resource-management-template-description"
                          ellipsis={{ rows: 2, showTooltip: true }}
                        >
                          {template.description || messages.emptyDescription}
                        </SemiTypography.Text>
                      </dd>
                    </div>
                    <div>
                      <dt>{messages.updatedAt}</dt>
                      <dd>
                        <time className="resource-management-time" dateTime={template.updatedAt}>
                          {formatUpdatedAt(template.updatedAt)}
                        </time>
                      </dd>
                    </div>
                  </dl>
                </SemiCard>
              </li>
            ))}
          </ul>
        ) : (
          <SemiEmpty className="resource-management-empty" description={messages.copy} title={messages.empty} />
        )}
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
