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
    <section className="native-page resource-management-page">
      <header className="resource-management-header">
        <div>
          <h2>{messages.title}</h2>
          <p>{messages.copy}</p>
        </div>
        <Button onClick={() => void openModelEditorWindow()} variant="primary">{messages.add}</Button>
      </header>

      <section className="resource-management-metrics" aria-label={messages.listTitle}>
        <ManagementMetric label={messages.total} value={models.length} />
        <ManagementMetric label={messages.enabled} value={enabledCount} />
        <ManagementMetric label={messages.disabledTag} value={disabledCount} />
        <ManagementMetric label={messages.defaultLabel} value={defaultModel?.name ?? commonMessages.notSet} />
      </section>

      <SemiDivider className="resource-management-divider" />

      <section className="resource-management-workspace">
        {models.length > 0 ? (
          <ul className="resource-management-card-grid" aria-label={messages.listTitle}>
            {models.map((model) => (
              <li
                key={model.id}
                aria-label={model.name}
                className="resource-management-model-card-item"
                onDoubleClick={(event) => {
                  if (!(event.target instanceof Element) || !event.target.closest("button")) {
                    void openModelEditorWindow(model.id);
                  }
                }}
              >
                <SemiCard
                  bordered={false}
                  className="resource-management-model-card"
                  footer={(
                    <SemiSpace align="center" className="resource-management-card-actions" spacing={4}>
                      <Button
                        aria-label={`${commonMessages.edit}: ${model.name}`}
                        onClick={() => void openModelEditorWindow(model.id)}
                        size="small"
                        variant="text"
                      >
                        {commonMessages.edit}
                      </Button>
                      <Button
                        aria-label={`${commonMessages.delete}: ${model.name}`}
                        onClick={() => void removeModel(model)}
                        size="small"
                        variant="danger"
                      >
                        {commonMessages.delete}
                      </Button>
                    </SemiSpace>
                  )}
                  header={(
                    <div className="resource-management-card-header">
                      <SemiSpace align="center" className="resource-management-primary" spacing={12}>
                        <SemiAvatar
                          className="resource-management-avatar resource-management-model-avatar"
                          color="blue"
                          shape="square"
                          size="40px"
                        >
                          AI
                        </SemiAvatar>
                        <SemiTypography.Text
                          className="resource-management-card-title"
                          ellipsis={{ showTooltip: true }}
                          strong
                        >
                          {model.name}
                        </SemiTypography.Text>
                      </SemiSpace>
                      <SemiSpace className="resource-management-tag-row" spacing={6} wrap>
                        <SemiTag color={model.enabled ? "green" : "grey"} shape="circle" type="light">
                          {model.enabled ? messages.enabledTag : messages.disabledTag}
                        </SemiTag>
                        {model.isDefault && (
                          <SemiTag color="blue" shape="circle" type="light">
                            {messages.defaultTag}
                          </SemiTag>
                        )}
                      </SemiSpace>
                    </div>
                  )}
                  headerLine={false}
                >
                  <dl className="resource-management-card-details">
                    <div>
                      <dt>{messages.model}</dt>
                      <dd>
                        <SemiTypography.Text ellipsis={{ showTooltip: true }}>{model.model}</SemiTypography.Text>
                      </dd>
                    </div>
                    <div>
                      <dt>{messages.updatedAt}</dt>
                      <dd>
                        <time className="resource-management-time" dateTime={model.updatedAt}>
                          {formatUpdatedAt(model.updatedAt)}
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
