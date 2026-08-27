import { confirm, message } from "@tauri-apps/plugin-dialog";
import { IconMoreStroked } from "@douyinfe/semi-icons";
import SemiAvatar from "@douyinfe/semi-ui/lib/es/avatar";
import SemiButton from "@douyinfe/semi-ui/lib/es/button";
import SemiDivider from "@douyinfe/semi-ui/lib/es/divider";
import SemiDropdown from "@douyinfe/semi-ui/lib/es/dropdown";
import SemiEmpty from "@douyinfe/semi-ui/lib/es/empty";
import SemiPagination from "@douyinfe/semi-ui/lib/es/pagination";
import SemiProgress from "@douyinfe/semi-ui/lib/es/progress";
import SemiSpace from "@douyinfe/semi-ui/lib/es/space";
import SemiTable from "@douyinfe/semi-ui/lib/es/table";
import type { ColumnProps } from "@douyinfe/semi-ui/lib/es/table";
import SemiTag from "@douyinfe/semi-ui/lib/es/tag";
import type { TagColor } from "@douyinfe/semi-ui/lib/es/tag/interface";
import SemiTooltip from "@douyinfe/semi-ui/lib/es/tooltip";
import SemiTypography from "@douyinfe/semi-ui/lib/es/typography";
import { useEffect, useMemo, useState } from "react";
import { useRouter } from "@/app/router/RouterContext";
import {
  filterJobQueue,
  isProcessingStage,
  JOB_QUEUE_PAGE_SIZE,
  paginateJobQueue,
  type JobQueueFilter,
} from "@/features/jobs/application/jobQueue";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { Button, Tabs, TextInput } from "@/shared/components/ui";
import { formatMessage, getMessages } from "@/shared/i18n";
import { openJobWorkbenchWindow } from "@/shared/services/ui/windows";
import type { DiarizationStatus, JobStage, MeetingJob } from "@/shared/types/meeting";
import {
  jobDetailPath,
  jobRef,
  jobRefKey,
} from "./jobRoutes";
import "./JobsView.css";

function diarizationLabel(job: MeetingJob, messages: ReturnType<typeof getMessages>["jobs"]) {
  if (!job.enableSpeaker || job.diarizationStatus === "disabled") {
    return messages.transcriptOnly;
  }
  switch (job.diarizationStatus) {
    case "completed":
      return messages.diarizationCompleted;
    case "unavailable":
      return messages.diarizationUnavailable;
    case "failed":
      return messages.diarizationFailed;
    case "legacy_unverified":
      return messages.diarizationUnverified;
    case "pending":
    case "processing":
      return messages.diarizationEnabled;
  }
}

function diarizationTagColor(status: DiarizationStatus): TagColor {
  switch (status) {
    case "completed":
      return "green";
    case "pending":
    case "processing":
      return "blue";
    case "legacy_unverified":
      return "amber";
    case "unavailable":
      return "orange";
    case "failed":
      return "red";
    case "disabled":
      return "grey";
  }
}

function statusTagColor(status: JobStage): TagColor {
  switch (status) {
    case "completed":
      return "green";
    case "failed":
      return "red";
    case "transcribing":
      return "blue";
    case "speaker_processing":
      return "purple";
    case "summarizing":
      return "indigo";
    case "uploaded":
      return "teal";
    case "queued":
      return "cyan";
    case "idle":
      return "grey";
  }
}

function jobProgressPercent(job: MeetingJob) {
  if (job.overallStatus === "completed") {
    return 100;
  }
  if (typeof job.progressPercent !== "number" || !Number.isFinite(job.progressPercent)) {
    return undefined;
  }
  return Math.min(100, Math.max(0, Math.round(job.progressPercent)));
}

function sourceFileMark(job: MeetingJob) {
  const filename = job.sourceFiles[0]?.name ?? "";
  const extension = filename.includes(".") ? filename.split(".").pop()?.trim() : undefined;
  return extension && extension.length <= 4 ? extension.toUpperCase() : "AV";
}

export default function JobsView() {
  const store = useMeetingStore();
  const router = useRouter();
  const [deletingJobId, setDeletingJobId] = useState<string | null>(null);
  const [filter, setFilter] = useState<JobQueueFilter>(readInitialJobQueueFilter);
  const [searchQuery, setSearchQuery] = useState("");
  const [page, setPage] = useState(1);
  const messages = getMessages(store.settings.locale).jobs;
  const commonMessages = getMessages(store.settings.locale).common;
  const statusMessages = getMessages(store.settings.locale).status;
  const operationUnavailable = getMessages(store.settings.locale).workbench.remoteOperationUnavailable;
  const modeJobs = useMemo(
    () => filterJobQueue(store.jobs, store.settings.processingMode, "all", ""),
    [store.jobs, store.settings.processingMode],
  );
  const visibleJobs = useMemo(
    () => filterJobQueue(store.jobs, store.settings.processingMode, filter, searchQuery),
    [filter, searchQuery, store.jobs, store.settings.processingMode],
  );
  const pagination = useMemo(() => paginateJobQueue(visibleJobs, page), [page, visibleJobs]);
  const completedJobs = modeJobs.filter((job) => job.overallStatus === "completed").length;
  const processingJobs = modeJobs.filter((job) => isProcessingStage(job.overallStatus)).length;
  const failedJobs = modeJobs.filter((job) => job.overallStatus === "failed").length;

  useEffect(() => {
    void store.refreshJobs().catch(() => undefined);
  }, []);

  useEffect(() => {
    if (page !== pagination.page) {
      setPage(pagination.page);
    }
  }, [page, pagination.page]);

  function isDeleting(job: MeetingJob) {
    return deletingJobId === jobRefKey(jobRef(job));
  }

  function supports(job: MeetingJob, operation: "jobs.read" | "jobs.result.read" | "jobs.retry" | "jobs.delete") {
    return job.source === "local" || store.canRemoteOperation(operation);
  }

  function supportsResult(job: MeetingJob) {
    return supports(job, "jobs.read") && supports(job, "jobs.result.read");
  }

  function isDeleteDisabled(job: MeetingJob) {
    return isProcessingStage(job.overallStatus) || !supports(job, "jobs.delete");
  }

  async function openJobDetail(job: MeetingJob) {
    if (supports(job, "jobs.read")) {
      await router.push(jobDetailPath(jobRef(job)));
    }
  }

  async function openJobWorkbench(job: MeetingJob) {
    if (supportsResult(job)) {
      await openJobWorkbenchWindow(job.id, job.title, job.source);
    }
  }

  async function deleteJob(job: MeetingJob) {
    if (isDeleteDisabled(job)) {
      return;
    }

    const confirmed = await confirm(formatMessage(messages.deleteConfirm, { title: job.title }), {
      title: messages.deleteTitle,
      kind: "warning",
      okLabel: commonMessages.delete,
      cancelLabel: commonMessages.cancel,
    });
    if (!confirmed) {
      return;
    }

    setDeletingJobId(jobRefKey(jobRef(job)));
    try {
      await store.deleteJob(jobRef(job));
    } finally {
      setDeletingJobId(null);
    }
  }

  async function retryJob(job: MeetingJob) {
    if (!supports(job, "jobs.retry")) {
      return;
    }
    if (job.source === "local" && !store.runtimeStatus.shellReady) {
      await message(commonMessages.modelUnavailableMessage, {
        title: commonMessages.modelUnavailableTitle,
        kind: "warning",
      });
      return;
    }

    await store.retryJob(jobRef(job));
  }

  function formatCreatedAt(value: string) {
    return new Date(value).toLocaleString(store.settings.locale, {
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  function formatFileDuration(minutes: number) {
    if (!minutes || minutes <= 0) {
      return messages.pending;
    }

    const totalMinutes = Math.max(1, Math.round(minutes));
    const hours = Math.floor(totalMinutes / 60);
    const remainingMinutes = totalMinutes % 60;
    if (hours <= 0) {
      return formatMessage(messages.minutes, { count: remainingMinutes });
    }
    if (remainingMinutes === 0) {
      return formatMessage(messages.hours, { count: hours });
    }
    return formatMessage(messages.hoursMinutes, { hours, minutes: remainingMinutes });
  }

  function formatProcessingDuration(seconds?: number) {
    if (typeof seconds !== "number" || seconds < 0) {
      return messages.notCompleted;
    }

    const totalSeconds = Math.max(0, Math.round(seconds));
    const hours = Math.floor(totalSeconds / 3600);
    const minutes = Math.floor((totalSeconds % 3600) / 60);
    const remainingSeconds = totalSeconds % 60;
    if (hours > 0) {
      return formatMessage(messages.processingWithHours, {
        hours,
        minutes: String(minutes).padStart(2, "0"),
      });
    }
    return formatMessage(messages.processingWithMinutes, {
      minutes,
      seconds: String(remainingSeconds).padStart(2, "0"),
    });
  }

  const filters = [
    { key: "all", label: messages.filterAll },
    { key: "processing", label: messages.filterProcessing },
    { key: "completed", label: messages.filterCompleted },
    { key: "failed", label: messages.filterFailed },
  ];
  const columns: ColumnProps<MeetingJob>[] = [
    {
      title: messages.colTask,
      dataIndex: "title",
      width: 250,
      render: (_value, job) => (
        <SemiSpace align="center" className="jobs-queue-task" spacing={12}>
          <SemiAvatar
            className="jobs-queue-file-avatar"
            color="blue"
            shape="square"
            size="36px"
          >
            {sourceFileMark(job)}
          </SemiAvatar>
          <SemiTypography.Text className="jobs-queue-task-title" ellipsis={{ showTooltip: true }} strong>
            {job.title}
          </SemiTypography.Text>
        </SemiSpace>
      ),
    },
    {
      title: messages.colFiles,
      width: 220,
      render: (_value, job) => {
        const sourceNames = job.sourceFiles.map((file) => file.name).join(" · ") || commonMessages.noData;
        return (
          <SemiTypography.Text className="jobs-queue-source-name" ellipsis={{ showTooltip: true }}>
            {sourceNames}
          </SemiTypography.Text>
        );
      },
    },
    {
      title: messages.colFileCount,
      width: 95,
      render: (_value, job) => (
        <SemiTag color="grey" shape="circle" type="light">
          {formatMessage(messages.filesCount, { count: job.sourceFiles.length })}
        </SemiTag>
      ),
    },
    {
      title: messages.colDuration,
      width: 105,
      render: (_value, job) => (
        <SemiTypography.Text className="jobs-queue-media-duration" type="secondary">
          {formatFileDuration(job.durationMinutes)}
        </SemiTypography.Text>
      ),
    },
    {
      title: messages.colDiarization,
      width: 220,
      render: (_value, job) => (
        <SemiTooltip content={diarizationLabel(job, messages)} position="top">
          <SemiTag
            className="jobs-queue-diarization-tag"
            color={diarizationTagColor(job.enableSpeaker ? job.diarizationStatus : "disabled")}
            shape="circle"
            type="light"
          >
            {diarizationLabel(job, messages)}
          </SemiTag>
        </SemiTooltip>
      ),
    },
    {
      title: messages.colCreatedAt,
      width: 145,
      render: (_value, job) => (
        <time className="jobs-queue-created-at" dateTime={job.createdAt}>
          <SemiTypography.Text type="secondary">
            {formatCreatedAt(job.createdAt)}
          </SemiTypography.Text>
        </time>
      ),
    },
    {
      title: messages.colProcessingDuration,
      width: 115,
      render: (_value, job) => (
        <SemiTypography.Text className="jobs-queue-duration" type="secondary">
          {formatProcessingDuration(job.processingDurationSeconds)}
        </SemiTypography.Text>
      ),
    },
    {
      title: messages.colStatus,
      width: 150,
      render: (_value, job) => (
        <SemiTooltip
          condition={job.overallStatus === "failed" && Boolean(job.failureReason?.trim())}
          content={job.failureReason}
          position="top"
        >
          <SemiTag
            aria-label={statusMessages[job.overallStatus]}
            className="jobs-queue-status-tag"
            color={statusTagColor(job.overallStatus)}
            shape="circle"
            type="light"
          >
            {statusMessages[job.overallStatus]}
          </SemiTag>
        </SemiTooltip>
      ),
    },
    {
      title: messages.colProgress,
      width: 145,
      render: (_value, job) => {
        const progressPercent = jobProgressPercent(job);
        const processLabel = job.overallStatus === "completed"
          ? messages.processCompleted
          : job.overallStatus === "failed"
            ? messages.processFailed
            : messages.processRunning;
        return progressPercent === undefined ? (
          <SemiTypography.Text className="jobs-queue-process-label" type="secondary">
            {processLabel}
          </SemiTypography.Text>
        ) : (
          <SemiSpace align="center" className="jobs-queue-progress-row" spacing={8}>
            <SemiProgress
              aria-label={`${processLabel} ${progressPercent}%`}
              className="jobs-queue-progress"
              motion={false}
              percent={progressPercent}
              showInfo={false}
              size="small"
              stroke="var(--accent)"
              strokeWidth={5}
            />
            <SemiTypography.Text className="jobs-queue-progress-value" type="secondary">
              {progressPercent}%
            </SemiTypography.Text>
          </SemiSpace>
        );
      },
    },
    {
      title: messages.colActions,
      align: "right",
      fixed: "right",
      width: 125,
      render: (_value, job) => (
        <SemiSpace align="center" className="jobs-queue-actions" spacing={6}>
          <SemiTooltip
            condition={job.overallStatus === "completed"
              ? !supportsResult(job)
              : !supports(job, "jobs.read")}
            content={store.remoteError ?? operationUnavailable}
            position="top"
          >
            <SemiButton
              disabled={job.overallStatus === "completed" ? !supportsResult(job) : !supports(job, "jobs.read")}
              htmlType="button"
              onClick={(event) => {
                event.stopPropagation();
                void (job.overallStatus === "completed" ? openJobWorkbench(job) : openJobDetail(job));
              }}
              theme={job.overallStatus === "completed" ? "solid" : "light"}
              type={job.overallStatus === "completed" ? "primary" : "secondary"}
            >
              {job.overallStatus === "completed" ? messages.viewResult : messages.details}
            </SemiButton>
          </SemiTooltip>
          {(job.overallStatus === "completed" || job.overallStatus === "failed" || !isDeleteDisabled(job)) && (
            <SemiDropdown
              position="bottomRight"
              render={(
                <SemiDropdown.Menu>
                  {job.overallStatus === "completed" && (
                    <SemiDropdown.Item
                      disabled={!supports(job, "jobs.read")}
                      onClick={(event) => {
                        event.stopPropagation();
                        void openJobDetail(job);
                      }}
                    >
                      {messages.details}
                    </SemiDropdown.Item>
                  )}
                  {job.overallStatus === "failed" && (
                    <SemiDropdown.Item
                      disabled={!supports(job, "jobs.retry")}
                      onClick={(event) => {
                        event.stopPropagation();
                        void retryJob(job);
                      }}
                      type="warning"
                    >
                      {commonMessages.retry}
                    </SemiDropdown.Item>
                  )}
                  <SemiDropdown.Item
                    disabled={isDeleteDisabled(job) || isDeleting(job)}
                    onClick={(event) => {
                      event.stopPropagation();
                      void deleteJob(job);
                    }}
                    type="danger"
                  >
                    {isDeleting(job) ? messages.deleting : commonMessages.delete}
                  </SemiDropdown.Item>
                </SemiDropdown.Menu>
              )}
              trigger="click"
            >
              <SemiButton
                aria-label={messages.moreActions}
                circle
                htmlType="button"
                icon={<IconMoreStroked />}
                onClick={(event) => {
                  event.stopPropagation();
                }}
                theme="borderless"
                type="tertiary"
              />
            </SemiDropdown>
          )}
        </SemiSpace>
      ),
    },
  ];

  return (
    <section className="native-page jobs-queue-page">
      <header className="jobs-queue-header">
        <div>
          <h2>{messages.pageTitle}</h2>
        </div>
        <Button
          disabled={store.settings.processingMode !== "local"}
          onClick={() => void router.push("/jobs/new")}
          title={store.settings.processingMode !== "local" ? operationUnavailable : undefined}
          variant="primary"
        >
          {messages.newJob}
        </Button>
      </header>

      <section className="jobs-queue-metrics" aria-label={messages.queueTitle}>
        <QueueMetric label={messages.total} value={modeJobs.length} />
        <QueueMetric label={messages.processing} value={processingJobs} />
        <QueueMetric label={messages.completed} value={completedJobs} />
        <QueueMetric label={messages.processFailed} value={failedJobs} tone={failedJobs > 0 ? "danger" : "default"} />
      </section>

      <SemiDivider className="jobs-queue-divider" />

      <section className="jobs-queue-workspace">
        <div className="jobs-queue-controls">
          <Tabs
            activeKey={filter}
            appearance="button"
            ariaLabel={messages.filterLabel}
            className="jobs-queue-filters"
            items={filters}
            onChange={(activeKey) => {
              const nextFilter = isJobQueueFilter(activeKey) ? activeKey : "all";
              setFilter(nextFilter);
              window.history.replaceState(
                {},
                "",
                nextFilter === "all" ? "/jobs" : `/jobs?status=${nextFilter}`,
              );
              setPage(1);
            }}
          />
          <div className="jobs-queue-search-group">
            <TextInput
              aria-label={messages.searchPlaceholder}
              className="jobs-queue-search"
              onChange={(value) => {
                setSearchQuery(value);
                setPage(1);
              }}
              placeholder={messages.searchPlaceholder}
              showClear
              value={searchQuery}
            />
            <span>{formatMessage(messages.visibleCount, { visible: visibleJobs.length, total: modeJobs.length })}</span>
          </div>
        </div>

        <SemiTable<MeetingJob>
          aria-label={messages.queueTitle}
          className="jobs-queue-table"
          columns={columns}
          dataSource={pagination.items}
          empty={(
            <SemiEmpty
              className="jobs-queue-empty"
              description={messages.emptyDescription}
              title={messages.emptyTitle}
            />
          )}
          onRow={(job) => ({
            onDoubleClick: () => {
              if (job) {
                void openJobDetail(job);
              }
            },
          })}
          pagination={false}
          rowKey={(job) => job ? jobRefKey(jobRef(job)) : ""}
          scroll={{ x: 1570 }}
        />

        {visibleJobs.length > 0 && (
          <nav className="jobs-queue-pagination" aria-label={messages.paginationLabel}>
            <span className="jobs-queue-pagination-summary" aria-live="polite">
              {formatMessage(messages.paginationSummary, {
                from: pagination.from,
                to: pagination.to,
                total: pagination.total,
              })}
            </span>
            <SemiPagination
              className="jobs-queue-pagination-control"
              currentPage={pagination.page}
              hideOnSinglePage={false}
              nextText={messages.nextPage}
              onPageChange={setPage}
              pageSize={JOB_QUEUE_PAGE_SIZE}
              prevText={messages.previousPage}
              showSizeChanger={false}
              showTotal={false}
              total={pagination.total}
            />
          </nav>
        )}
      </section>
    </section>
  );
}

function QueueMetric({ label, value, tone = "default" }: { label: string; value: number; tone?: "default" | "danger" }) {
  return (
    <div className="jobs-queue-metric" data-tone={tone}>
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function isJobQueueFilter(value: string): value is JobQueueFilter {
  return value === "all" || value === "processing" || value === "completed" || value === "failed";
}

function readInitialJobQueueFilter(): JobQueueFilter {
  const status = new URLSearchParams(window.location.search).get("status") ?? "all";
  return isJobQueueFilter(status) ? status : "all";
}
