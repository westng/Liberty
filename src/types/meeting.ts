export type JobStage =
  | "idle"
  | "uploaded"
  | "queued"
  | "transcribing"
  | "speaker_processing"
  | "summarizing"
  | "completed"
  | "failed";

export type AiSummaryRunStatus = "running" | "completed" | "failed";
export type ThemeMode = "auto" | "light" | "dark";
export type LiquidGlassStyle = "transparent" | "tinted";
export type LocaleCode = "zh-CN" | "en-US";
export type LocalAsrDevice = "auto" | "cpu" | "mps" | "cuda";
export type ManagedRuntimeInstallStatus =
  | "missing"
  | "installing"
  | "ready"
  | "failed"
  | "repair_required";

export interface TranscriptSegment {
  id: string;
  startMs: number;
  endMs: number;
  speaker?: string;
  text: string;
}

export interface MeetingSummary {
  overview: string;
  topics: string[];
  decisions: string[];
  actionItems: string[];
  risks?: string[];
  followUps?: string[];
}

export interface AiSummaryActionItem {
  task: string;
  owner?: string;
  dueDate?: string;
}

export interface AiSummaryResult {
  title: string;
  overview: string;
  topics: string[];
  decisions: string[];
  actionItems: AiSummaryActionItem[];
  risks: string[];
  followUps: string[];
}

export interface AiModelConfig {
  id: string;
  name: string;
  baseUrl: string;
  apiKey: string;
  model: string;
  enabled: boolean;
  isDefault: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface AiSummaryTemplate {
  id: string;
  name: string;
  description: string;
  prompt: string;
  includeSpeakerByDefault: boolean;
  includeTimestampByDefault: boolean;
  builtin: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface AiSummaryRun {
  id: string;
  jobId: string;
  modelConfigId: string;
  templateId: string;
  includeSpeaker: boolean;
  includeTimestamp: boolean;
  extraInstructions: string;
  status: AiSummaryRunStatus;
  errorMessage?: string;
  promptPreview?: string;
  rawResponse?: string;
  result?: AiSummaryResult;
  createdAt: string;
  updatedAt: string;
}

export interface MeetingJob {
  id: string;
  title: string;
  sourceFiles: MeetingSourceFile[];
  durationMinutes: number;
  processingStartedAtMs?: number;
  processingFinishedAtMs?: number;
  processingDurationSeconds?: number;
  progressPercent?: number;
  progressMessage?: string;
  createdAt: string;
  hotwords: string[];
  lang: string;
  enableSpeaker: boolean;
  summaryTemplate: string;
  uploadStatus: JobStage;
  asrStatus: JobStage;
  summaryStatus: JobStage;
  overallStatus: JobStage;
  failureReason?: string;
  transcriptSegments: TranscriptSegment[];
  speakerSegments: TranscriptSegment[];
  summary: MeetingSummary;
  summaryRuns: AiSummaryRun[];
  activeSummaryRunId?: string;
  exportFormats: string[];
  lastExportedAt?: string;
  processLog?: string;
}

export interface MeetingSourceFile {
  id: string;
  name: string;
  path?: string;
  sizeLabel: string;
  kind: "audio" | "video";
}

export interface NewMeetingJobInput {
  title: string;
  files: MeetingSourceFile[];
  hotwords: string[];
  lang: string;
  enableSpeaker: boolean;
  summaryTemplate: string;
}

export interface SettingsState {
  themeMode: ThemeMode;
  liquidGlassStyle: LiquidGlassStyle;
  accentColor: string;
  locale: LocaleCode;
  backendUrl: string;
  apiToken: string;
  defaultHotwords: string;
  summaryTemplate: string;
  concurrency: number;
  pythonPath: string;
  runnerScriptPath: string;
  localAsrDevice: LocalAsrDevice;
  localAsrThreads: number;
  localAsrBatchSizeSeconds: number;
}

export interface ManagedRuntimeStatus {
  platformId: string;
  runtimeVersion: string;
  pythonVersion: string;
  status: ManagedRuntimeInstallStatus;
  pythonExecutablePath?: string;
  modelsRoot?: string;
  installRoot?: string;
  lastError?: string;
  installedAt?: string;
  updatedAt: string;
  lastLogPath?: string;
}
