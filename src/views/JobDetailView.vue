<script setup lang="ts">
import { computed } from "vue";
import { RouterLink, useRoute } from "vue-router";
import StatusBadge from "@/components/StatusBadge.vue";
import { useMeetingStore } from "@/composables/useMeetingStore";

const route = useRoute();
const store = useMeetingStore();

const job = computed(() => store.getJobById(route.params.id as string));

const stages = computed(() => {
  if (!job.value) {
    return [];
  }

  return [
    { label: "上传完成", status: job.value.uploadStatus },
    { label: "转写引擎", status: job.value.asrStatus },
    { label: "AI 总结", status: job.value.summaryStatus },
    { label: "总状态", status: job.value.overallStatus },
  ];
});
</script>

<template>
  <section class="view-stack">
    <div v-if="job" class="detail-grid">
      <article class="surface detail-hero full-span">
        <div class="job-title-line detail-hero-head">
          <div>
            <h3>{{ job.title }}</h3>
            <p class="section-copy">
              {{ job.sourceFiles.map((file) => file.name).join(" · ") }}
            </p>
          </div>
          <StatusBadge :status="job.overallStatus" />
        </div>

        <div class="detail-stage-grid">
          <div v-for="stage in stages" :key="stage.label" class="detail-stage-card">
            <span class="detail-stage-label">{{ stage.label }}</span>
            <StatusBadge :status="stage.status" />
          </div>
        </div>

        <div class="detail-hero-footer">
          <div class="summary-inline">
            <span>输入文件 {{ job.sourceFiles.length }}</span>
            <span>热词 {{ job.hotwords.length }}</span>
            <span>说话人 {{ job.enableSpeaker ? "开启" : "关闭" }}</span>
          </div>

          <div class="button-row">
            <RouterLink
              v-if="job.overallStatus === 'completed'"
              class="primary-button"
              :to="`/jobs/${job.id}/workbench`"
            >
              查看结果工作台
            </RouterLink>
            <button
              v-if="job.overallStatus === 'failed'"
              class="secondary-button"
              type="button"
              @click="store.retryJob(job.id)"
            >
              重试任务
            </button>
          </div>
        </div>
      </article>

      <article class="surface detail-main-column">
        <div class="section-heading">
          <h3>输入文件</h3>
        </div>
        <div class="file-list">
          <div v-for="file in job.sourceFiles" :key="file.id" class="file-pill">
            <div>
              <strong>{{ file.name }}</strong>
              <div class="job-meta-line">
                {{ file.kind === "audio" ? "音频" : "视频" }} · {{ file.sizeLabel }}
              </div>
            </div>
          </div>
        </div>
      </article>

      <article class="surface detail-side-column">
        <div class="section-heading">
          <h3>任务设置</h3>
        </div>
        <div class="metric-strip metric-strip-tight">
          <div class="metric-pill">
            <span class="muted">语言</span>
            <strong>{{ job.lang }}</strong>
          </div>
          <div class="metric-pill">
            <span class="muted">说话人分离</span>
            <strong>{{ job.enableSpeaker ? "开启" : "关闭" }}</strong>
          </div>
          <div class="metric-pill">
            <span class="muted">热词</span>
            <strong>{{ job.hotwords.length }}</strong>
          </div>
        </div>
      </article>

      <article class="surface detail-log-card full-span">
        <div class="section-heading">
          <h3>异常信息</h3>
        </div>
        <div v-if="job.failureReason" class="note-block error-block">
          {{ job.failureReason }}
        </div>
        <pre
          v-if="job.processLog"
          class="log-block"
        ><code>{{ job.processLog }}</code></pre>
        <div v-else class="empty-state">
          当前没有错误记录。
        </div>
      </article>
    </div>

    <div v-else class="empty-state">
      没有找到这个任务。
    </div>
  </section>
</template>
