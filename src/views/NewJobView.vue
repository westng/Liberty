<script setup lang="ts">
import { open } from "@tauri-apps/plugin-dialog";
import { computed, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { useMeetingStore } from "@/composables/useMeetingStore";
import type { MeetingSourceFile } from "@/types/meeting";

const router = useRouter();
const store = useMeetingStore();

const title = ref("");
const hotwordsText = ref(store.settings.value.defaultHotwords);
const lang = ref("zh-CN");
const enableSpeaker = ref(true);
const files = ref<MeetingSourceFile[]>([]);
const fileInput = ref<HTMLInputElement | null>(null);
const isSubmitting = ref(false);
const submitError = ref("");
const isLocalMode = computed(() => store.localMode.value);
const summaryTemplate = computed(
  () => store.settings.value.summaryTemplate.trim() || "默认会议纪要模板",
);
const serviceModeLabel = computed(() => {
  if (store.localMode.value) {
    return "本地 Python";
  }

  if (store.settings.value.backendUrl.trim()) {
    return "在线服务";
  }

  return "未配置环境";
});

const recentJobs = computed(() => store.jobs.value.slice(0, 4));
const activeJobs = computed(() =>
  store.jobs.value.filter((job) =>
    ["queued", "transcribing", "speaker_processing", "summarizing"].includes(
      job.overallStatus,
    ),
  ).length,
);
const failedJobs = computed(
  () => store.jobs.value.filter((job) => job.overallStatus === "failed").length,
);

watch(
  () => store.settings.value,
  (settings, previous) => {
    const previousHotwords = previous?.defaultHotwords ?? "";

    if (!hotwordsText.value.trim() || hotwordsText.value === previousHotwords) {
      hotwordsText.value = settings.defaultHotwords;
    }
  },
  { deep: true },
);

function inferKind(fileName: string): "audio" | "video" {
  return /\.(mp4|mov|mkv)$/i.test(fileName) ? "video" : "audio";
}

function humanSize(size?: number) {
  if (!size) {
    return "未知大小";
  }

  const mb = size / 1024 / 1024;
  return `${mb.toFixed(1)} MB`;
}

function fileNameToTitle(fileName: string) {
  return fileName.replace(/\.[^.]+$/, "").trim();
}

function fileToSource(file: File): MeetingSourceFile {
  return {
    id: crypto.randomUUID(),
    name: file.name,
    sizeLabel: humanSize(file.size),
    kind: inferKind(file.name),
  };
}

function addFiles(next: MeetingSourceFile[]) {
  const lastFile = next.at(-1);

  if (isLocalMode.value) {
    files.value = lastFile ? [lastFile] : files.value;
    if (!title.value.trim() && lastFile) {
      title.value = fileNameToTitle(lastFile.name);
    }
    return;
  }

  files.value = [...files.value, ...next];

  if (!title.value.trim() && lastFile) {
    title.value = fileNameToTitle(lastFile.name);
  }
}

async function pickFiles() {
  try {
    const selected = await open({
      multiple: !isLocalMode.value,
      directory: false,
      filters: [
        {
          name: "Meeting Media",
          extensions: ["m4a", "mp3", "wav", "aac", "flac", "mp4", "mov", "mkv"],
        },
      ],
    });

    if (!selected) {
      return;
    }

    const normalized = Array.isArray(selected) ? selected : [selected];

    addFiles(
      normalized.map((path) => {
        const name = path.split("/").pop() ?? path;

        return {
          id: crypto.randomUUID(),
          name,
          path,
          sizeLabel: "本地路径",
          kind: inferKind(name),
        } satisfies MeetingSourceFile;
      }),
    );
  } catch {
    fileInput.value?.click();
  }
}

function onNativeFileChange(event: Event) {
  const target = event.target as HTMLInputElement;
  const selected = Array.from(target.files ?? []).map(fileToSource);
  addFiles(selected);
  target.value = "";
}

function removeFile(id: string) {
  files.value = files.value.filter((file) => file.id !== id);
}

function clearFiles() {
  files.value = [];
}

async function submit() {
  submitError.value = "";

  if (!files.value.length || !title.value.trim()) {
    return;
  }

  if (isLocalMode.value && files.value.some((file) => !file.path)) {
    submitError.value = "本地 Python 模式需要通过桌面原生文件选择器获取可读路径。";
    return;
  }

  isSubmitting.value = true;

  try {
    const job = await store.createJob({
      title: title.value.trim(),
      files: files.value,
      hotwords: hotwordsText.value
        .split(",")
        .map((item) => item.trim())
        .filter(Boolean),
      lang: lang.value,
      enableSpeaker: enableSpeaker.value,
      summaryTemplate: summaryTemplate.value,
    });

    await router.push(`/jobs/${job.id}`);
  } catch (error) {
    submitError.value =
      error instanceof Error ? error.message : "创建任务失败，请检查本地运行环境配置。";
  } finally {
    isSubmitting.value = false;
  }
}
</script>

<template>
  <section class="view-stack">
    <div class="workspace-grid workspace-grid-home">
      <div class="workspace-primary">
        <article class="surface workbench-hero new-job-hero">
          <div class="job-title-line workbench-hero-head">
            <div>
              <h3>创建会议任务</h3>
              <p class="section-copy">
                先完成标题和文件输入，参数放到下面按需调整。
              </p>
            </div>
            <button class="text-button" type="button" @click="router.push('/jobs')">
              查看任务列表
            </button>
          </div>

          <div class="summary-inline">
            <span>全部任务 {{ store.jobs.value.length }}</span>
            <span>处理中 {{ activeJobs }}</span>
            <span>失败 {{ failedJobs }}</span>
            <span>{{ serviceModeLabel }}</span>
          </div>
        </article>

        <article class="surface workspace-primary new-job-primary-card">
          <div class="section-heading">
            <div>
              <h3>基本信息</h3>
            </div>
          </div>

          <div class="field task-title-row">
            <label for="job-title">任务标题</label>
            <input
              id="job-title"
              v-model="title"
              placeholder="例如：产品周会 2026-03-25"
            />
          </div>

          <div class="surface surface-subtle upload-pane new-job-upload-card">
            <div class="section-heading summary-centered-heading">
              <h3>输入文件</h3>
            </div>

            <input
              ref="fileInput"
              type="file"
              accept=".m4a,.mp3,.wav,.aac,.flac,.mp4,.mov,.mkv"
              :multiple="!isLocalMode"
              hidden
              @change="onNativeFileChange"
            />

            <div class="drop-zone new-job-file-box" :class="{ 'has-files': files.length }">
              <button
                v-if="!files.length"
                class="drop-zone-button"
                type="button"
                @click="pickFiles"
              >
                <div class="drop-zone-copy">
                  <strong>添加文件</strong>
                  <p class="muted">
                    {{ isLocalMode ? "桌面原生文件选择器" : "支持音频与视频文件" }}
                  </p>
                </div>
              </button>

              <template v-else>
                <div class="new-job-file-box-head">
                  <span class="job-meta-line">已选择 {{ files.length }} 个文件</span>
                  <div class="new-job-file-box-actions">
                    <button class="text-button" type="button" @click="pickFiles">
                      {{ isLocalMode ? "重新选择" : "继续添加" }}
                    </button>
                    <button class="text-button danger-text" type="button" @click="clearFiles">
                      清空列表
                    </button>
                  </div>
                </div>

                <div class="file-list new-job-file-list">
                  <div v-for="file in files" :key="file.id" class="new-job-file-row">
                    <div class="new-job-file-name">
                      {{ file.name }}
                    </div>
                    <button class="text-button danger-text" type="button" @click="removeFile(file.id)">
                      移除
                    </button>
                  </div>
                </div>
              </template>
            </div>
          </div>

          <div v-if="submitError" class="note-block error-block">
            {{ submitError }}
          </div>

          <div class="button-row align-end task-actions">
            <button
              class="primary-button"
              type="button"
              :disabled="isSubmitting || !title.trim() || !files.length"
              @click="submit"
            >
              {{ isSubmitting ? "正在创建..." : "创建任务" }}
            </button>
          </div>
        </article>

        <article class="surface workspace-primary new-job-settings-card">
          <div class="section-heading">
            <div>
              <h3>高级设置</h3>
            </div>
          </div>

          <div class="summary-list new-job-settings-list">
            <div class="note-block new-job-setting-item">
              <div class="new-job-setting-head">
                <div>
                  <strong>语言</strong>
                </div>
                <div class="new-job-setting-control">
                  <select id="job-lang" v-model="lang">
                    <option value="zh-CN">中文</option>
                    <option value="en-US">英文</option>
                    <option value="ja-JP">日文</option>
                  </select>
                </div>
              </div>
              <p class="job-meta-line new-job-setting-copy">
                决定转写和后续总结默认使用的语言。
              </p>
            </div>

            <div class="note-block new-job-setting-item">
              <div class="new-job-setting-head">
                <div>
                  <strong>说话人</strong>
                </div>
                <div class="new-job-setting-control">
                  <label class="toggle-field">
                    <input v-model="enableSpeaker" type="checkbox" />
                    <span>{{ enableSpeaker ? "开启" : "关闭" }}</span>
                  </label>
                </div>
              </div>
              <p class="job-meta-line new-job-setting-copy">
                开启后会尝试区分讲话人，关闭时只生成普通逐字稿。
              </p>
            </div>

            <div class="note-block new-job-setting-item">
              <div class="new-job-setting-head">
                <div>
                  <strong>热词</strong>
                </div>
              </div>
              <textarea
                id="job-hotwords"
                v-model="hotwordsText"
                placeholder="使用英文逗号分隔，例如：SeACo-Paraformer, FunASR, 招投标"
              />
              <p class="job-meta-line new-job-setting-copy">
                建议只保留专有名词、项目名和行业术语。
              </p>
            </div>

          </div>
        </article>
      </div>

      <div class="side-stack new-job-side-stack">
        <article class="surface side-panel new-job-side-panel">
          <div class="section-heading">
            <h3>当前状态</h3>
          </div>

          <div class="metric-strip metric-strip-tight">
            <div class="metric-pill">
              <span class="job-meta-line">全部任务</span>
              <strong>{{ store.jobs.value.length }}</strong>
            </div>

            <div class="metric-pill">
              <span class="job-meta-line">处理中</span>
              <strong>{{ activeJobs }}</strong>
            </div>

            <div class="metric-pill">
              <span class="job-meta-line">失败</span>
              <strong>{{ failedJobs }}</strong>
            </div>
          </div>

          <div class="summary-list">
            <div class="file-pill">
              <div>
                <strong>{{ serviceModeLabel }}</strong>
                <div class="job-meta-line">
                  {{
                    isLocalMode
                      ? "使用本机 Python 环境和原生文件路径"
                      : store.settings.value.backendUrl.trim()
                        ? store.settings.value.backendUrl
                        : "请先完成处理环境配置"
                  }}
                </div>
              </div>
            </div>

            <div class="file-pill">
              <div>
                <strong>文件规则</strong>
                <div class="job-meta-line">
                  {{ isLocalMode ? "本地模式每次只处理 1 个文件" : "可一次加入多个文件" }}
                </div>
              </div>
            </div>
          </div>

          <div class="button-row">
            <button class="secondary-button" type="button" @click="router.push('/settings')">
              系统设置
            </button>
            <button class="text-button" type="button" @click="router.push('/jobs')">
              任务列表
            </button>
          </div>
        </article>

        <article class="surface side-panel">
          <div class="section-heading summary-centered-heading">
            <h3>最近任务</h3>
            <button class="text-button" type="button" @click="router.push('/jobs')">
              查看全部
            </button>
          </div>

          <div v-if="recentJobs.length" class="job-list compact-list">
            <div
              v-for="job in recentJobs"
              :key="job.id"
              class="job-item compact-job"
            >
              <div class="job-title-line">
                <div>
                  <h4>{{ job.title }}</h4>
                  <div class="job-meta-line">
                    {{ job.sourceFiles.length }} 个文件 · {{ job.durationMinutes }} 分钟
                  </div>
                </div>
                <button class="text-button" type="button" @click="router.push(`/jobs/${job.id}`)">
                  打开
                </button>
              </div>
            </div>
          </div>

          <div v-else class="empty-state compact-empty">
            还没有任务记录。
          </div>
        </article>
      </div>
    </div>
  </section>
</template>
