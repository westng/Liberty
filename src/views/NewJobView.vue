<script setup lang="ts">
import { open } from "@tauri-apps/plugin-dialog";
import { computed, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { useMeetingStore } from "@/composables/useMeetingStore";
import { formatMessage, getMessages } from "@/services/i18n";
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
const messages = computed(() => getMessages(store.settings.value.locale).newJob);
const commonMessages = computed(() => getMessages(store.settings.value.locale).common);
const jobsMessages = computed(() => getMessages(store.settings.value.locale).jobs);
const summaryTemplate = computed(
  () => store.settings.value.summaryTemplate.trim() || messages.value.defaultSummaryTemplateName,
);
const serviceModeLabel = computed(() => {
  if (store.localMode.value) {
    return messages.value.localPython;
  }

  if (store.settings.value.backendUrl.trim()) {
    return messages.value.remoteService;
  }

  return messages.value.envMissing;
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
    return commonMessages.value.unknownSize;
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
          sizeLabel: commonMessages.value.localPath,
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
    submitError.value = messages.value.localPathRequired;
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
      error instanceof Error ? error.message : messages.value.createFailed;
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
              <h3>{{ messages.heroTitle }}</h3>
              <p class="section-copy">
                {{ messages.heroCopy }}
              </p>
            </div>
            <button class="text-button" type="button" @click="router.push('/jobs')">
              {{ messages.viewJobs }}
            </button>
          </div>

          <div class="summary-inline">
            <span>{{ messages.allJobs }} {{ store.jobs.value.length }}</span>
            <span>{{ messages.processing }} {{ activeJobs }}</span>
            <span>{{ messages.failed }} {{ failedJobs }}</span>
            <span>{{ serviceModeLabel }}</span>
          </div>
        </article>

        <article class="surface workspace-primary new-job-primary-card">
          <div class="section-heading">
            <div>
              <h3>{{ messages.basicInfo }}</h3>
            </div>
          </div>

          <div class="field task-title-row">
            <label for="job-title">{{ messages.jobTitle }}</label>
            <input
              id="job-title"
              v-model="title"
              :placeholder="messages.titlePlaceholder"
            />
          </div>

          <div class="surface surface-subtle upload-pane new-job-upload-card">
            <div class="section-heading summary-centered-heading">
              <h3>{{ messages.inputFiles }}</h3>
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
                  <strong>{{ messages.addFiles }}</strong>
                  <p class="muted">
                    {{ isLocalMode ? messages.desktopFilePicker : messages.mediaSupported }}
                  </p>
                </div>
              </button>

              <template v-else>
                <div class="new-job-file-box-head">
                  <span class="job-meta-line">
                    {{ formatMessage(messages.selectedFiles, { count: files.length }) }}
                  </span>
                  <div class="new-job-file-box-actions">
                    <button class="text-button" type="button" @click="pickFiles">
                      {{ isLocalMode ? messages.reselect : messages.continueAdding }}
                    </button>
                    <button class="text-button danger-text" type="button" @click="clearFiles">
                      {{ messages.clearList }}
                    </button>
                  </div>
                </div>

                <div class="file-list new-job-file-list">
                  <div v-for="file in files" :key="file.id" class="new-job-file-row">
                    <div class="new-job-file-name">
                      {{ file.name }}
                    </div>
                    <button class="text-button danger-text" type="button" @click="removeFile(file.id)">
                      {{ commonMessages.remove }}
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
              {{ isSubmitting ? messages.creating : messages.createJob }}
            </button>
          </div>
        </article>

        <article class="surface workspace-primary new-job-settings-card">
          <div class="section-heading">
            <div>
              <h3>{{ messages.advancedSettings }}</h3>
            </div>
          </div>

          <div class="summary-list new-job-settings-list">
            <div class="note-block new-job-setting-item">
              <div class="new-job-setting-head">
                <div>
                  <strong>{{ messages.language }}</strong>
                </div>
                <div class="new-job-setting-control">
                  <select id="job-lang" v-model="lang">
                    <option value="zh-CN">{{ messages.langZh }}</option>
                    <option value="en-US">{{ messages.langEn }}</option>
                    <option value="ja-JP">{{ messages.langJa }}</option>
                  </select>
                </div>
              </div>
              <p class="job-meta-line new-job-setting-copy">
                {{ messages.languageHint }}
              </p>
            </div>

            <div class="note-block new-job-setting-item">
              <div class="new-job-setting-head">
                <div>
                  <strong>{{ messages.speaker }}</strong>
                </div>
                <div class="new-job-setting-control">
                  <label class="toggle-field">
                    <input v-model="enableSpeaker" type="checkbox" />
                    <span>{{ enableSpeaker ? commonMessages.enabled : commonMessages.disabled }}</span>
                  </label>
                </div>
              </div>
              <p class="job-meta-line new-job-setting-copy">
                {{ messages.speakerHint }}
              </p>
            </div>

            <div class="note-block new-job-setting-item">
              <div class="new-job-setting-head">
                <div>
                  <strong>{{ messages.hotwords }}</strong>
                </div>
              </div>
              <textarea
                id="job-hotwords"
                v-model="hotwordsText"
                :placeholder="messages.hotwordsPlaceholder"
              />
              <p class="job-meta-line new-job-setting-copy">
                {{ messages.hotwordsHint }}
              </p>
            </div>

          </div>
        </article>
      </div>

      <div class="side-stack new-job-side-stack">
        <article class="surface side-panel new-job-side-panel">
          <div class="section-heading">
            <h3>{{ messages.currentStatus }}</h3>
          </div>

          <div class="metric-strip metric-strip-tight">
            <div class="metric-pill">
              <span class="job-meta-line">{{ messages.allJobs }}</span>
              <strong>{{ store.jobs.value.length }}</strong>
            </div>

            <div class="metric-pill">
              <span class="job-meta-line">{{ messages.processing }}</span>
              <strong>{{ activeJobs }}</strong>
            </div>

            <div class="metric-pill">
              <span class="job-meta-line">{{ messages.failed }}</span>
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
                      ? messages.localModeHint
                      : store.settings.value.backendUrl.trim()
                        ? store.settings.value.backendUrl
                        : messages.pendingEnvHint
                  }}
                </div>
              </div>
            </div>

            <div class="file-pill">
              <div>
                <strong>{{ messages.fileRule }}</strong>
                <div class="job-meta-line">
                  {{ isLocalMode ? messages.localFileRule : messages.remoteFileRule }}
                </div>
              </div>
            </div>
          </div>

          <div class="button-row">
            <button class="secondary-button" type="button" @click="router.push('/settings')">
              {{ messages.settings }}
            </button>
            <button class="text-button" type="button" @click="router.push('/jobs')">
              {{ messages.jobs }}
            </button>
          </div>
        </article>

        <article class="surface side-panel">
          <div class="section-heading summary-centered-heading">
            <h3>{{ messages.recentJobs }}</h3>
            <button class="text-button" type="button" @click="router.push('/jobs')">
              {{ messages.viewAll }}
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
                    {{ formatMessage(messages.fileCount, { count: job.sourceFiles.length }) }} · {{ formatMessage(jobsMessages.minutes, { count: job.durationMinutes }) }}
                  </div>
                </div>
                <button class="text-button" type="button" @click="router.push(`/jobs/${job.id}`)">
                  {{ commonMessages.open }}
                </button>
              </div>
            </div>
          </div>

          <div v-else class="empty-state compact-empty">
            {{ messages.noTasks }}
          </div>
        </article>
      </div>
    </div>
  </section>
</template>
