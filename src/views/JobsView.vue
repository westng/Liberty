<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { RouterLink } from "vue-router";
import StatusBadge from "@/components/StatusBadge.vue";
import { useMeetingStore } from "@/composables/useMeetingStore";

const store = useMeetingStore();
const deletingJobId = ref<string | null>(null);

const sortedJobs = computed(() =>
  [...store.jobs.value].sort((left, right) => right.createdAt.localeCompare(left.createdAt)),
);

const completedJobs = computed(
  () => store.jobs.value.filter((job) => job.overallStatus === "completed").length,
);

const processingJobs = computed(
  () =>
    store.jobs.value.filter((job) =>
      ["queued", "transcribing", "speaker_processing", "summarizing"].includes(
        job.overallStatus,
      ),
    ).length,
);

onMounted(() => {
  void store.refreshJobs();
});

function isDeleting(jobId: string) {
  return deletingJobId.value === jobId;
}

function isDeleteDisabled(status: string) {
  return ["queued", "transcribing", "speaker_processing", "summarizing"].includes(status);
}

async function deleteJob(jobId: string) {
  const job = store.getJobById(jobId);

  if (!job || isDeleteDisabled(job.overallStatus)) {
    return;
  }

  const confirmed = window.confirm(`确认删除任务“${job.title}”吗？删除后无法恢复。`);
  if (!confirmed) {
    return;
  }

  deletingJobId.value = jobId;

  try {
    await store.deleteJob(jobId);
  } finally {
    deletingJobId.value = null;
  }
}

function formatCreatedAt(value: string) {
  return new Date(value).toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatFileDuration(minutes: number) {
  if (!minutes || minutes <= 0) {
    return "待处理";
  }

  const totalMinutes = Math.max(1, Math.round(minutes));
  const hours = Math.floor(totalMinutes / 60);
  const remainingMinutes = totalMinutes % 60;

  if (hours <= 0) {
    return `${remainingMinutes} 分钟`;
  }

  if (remainingMinutes === 0) {
    return `${hours} 小时`;
  }

  return `${hours} 小时 ${remainingMinutes} 分钟`;
}

function formatProcessingDuration(seconds?: number) {
  if (typeof seconds !== "number" || seconds < 0) {
    return "未完成";
  }

  const totalSeconds = Math.max(0, Math.round(seconds));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const remainingSeconds = totalSeconds % 60;

  if (hours > 0) {
    return `${hours}小时 ${String(minutes).padStart(2, "0")}分`;
  }

  return `${minutes}分 ${String(remainingSeconds).padStart(2, "0")}秒`;
}
</script>

<template>
  <section class="view-stack">
    <article class="surface">
      <div class="section-heading">
        <div>
          <h3>所有会议任务</h3>
          <p class="section-copy">
            统一查看上传、转写、说话人处理和纪要生成阶段。
          </p>
        </div>
      </div>
      <div class="summary-inline">
        <span>总任务 {{ sortedJobs.length }}</span>
        <span>处理中 {{ processingJobs }}</span>
        <span>已完成 {{ completedJobs }}</span>
      </div>
    </article>

    <article class="surface">
      <div class="section-heading">
        <div>
          <h3>任务队列</h3>
          <p class="section-copy">按任务名、时长、状态和操作整理，减少在卡片间来回扫描。</p>
        </div>
      </div>

      <div class="jobs-table">
        <div class="jobs-table-head">
          <span>任务</span>
          <span>文件信息</span>
          <span>处理时间</span>
          <span>创建时间</span>
          <span>状态</span>
          <span>操作</span>
        </div>

        <div v-for="job in sortedJobs" :key="job.id" class="jobs-row">
          <div class="jobs-primary">
            <strong>{{ job.title }}</strong>
            <div class="job-meta-line">
              {{ job.sourceFiles.map((file) => file.name).join(" · ") }}
            </div>
          </div>

          <div class="jobs-cell">
            <strong>{{ job.sourceFiles.length }} 个文件</strong>
            <div class="job-meta-line">
              文件时长 {{ formatFileDuration(job.durationMinutes) }}
            </div>
            <div class="job-meta-line">
              {{ job.enableSpeaker ? "含说话人分离" : "仅转写" }}
            </div>
          </div>

          <div class="jobs-cell">
            <strong>{{ formatProcessingDuration(job.processingDurationSeconds) }}</strong>
            <div class="job-meta-line">
              {{ job.overallStatus === "completed" ? "处理完成" : job.overallStatus === "failed" ? "处理失败" : "处理中" }}
            </div>
          </div>

          <div class="jobs-cell">
            {{ formatCreatedAt(job.createdAt) }}
          </div>

          <div class="jobs-cell">
            <StatusBadge :status="job.overallStatus" />
          </div>

          <div class="jobs-actions">
            <RouterLink class="text-button" :to="`/jobs/${job.id}`">
              详情
            </RouterLink>
            <RouterLink
              v-if="job.overallStatus === 'completed'"
              class="primary-button small-button"
              :to="`/jobs/${job.id}/workbench`"
            >
              工作台
            </RouterLink>
            <button
              v-if="job.overallStatus === 'failed'"
              class="secondary-button small-button"
              type="button"
              @click="store.retryJob(job.id)"
            >
              重试
            </button>
            <button
              class="text-button small-button jobs-delete-button"
              type="button"
              :disabled="isDeleteDisabled(job.overallStatus) || isDeleting(job.id)"
              :title="isDeleteDisabled(job.overallStatus) ? '处理中暂不可删除' : '删除任务'"
              @click="deleteJob(job.id)"
            >
              {{ isDeleting(job.id) ? "删除中..." : "删除" }}
            </button>
          </div>
        </div>
      </div>
    </article>
  </section>
</template>
