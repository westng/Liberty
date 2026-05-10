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
export type PetMood = "idle" | "cheerful" | "excited" | "proud" | "needy" | "sleepy" | "bored";
export type PetStage = "baby" | "growing" | "mature";
export type PetInteractionAction = "tap" | "pet" | "feed" | "encourage";
export type PetWorkflowEventType =
  | "job_created"
  | "transcription_started"
  | "transcription_completed"
  | "ai_summary_completed"
  | "export_completed"
  | "daily_open";

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

export interface MeetingMember {
  id: string;
  name: string;
  department: string;
  sortOrder: number;
  isRecorder: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface MeetingMemberImportResult {
  created: number;
  updated: number;
}

export interface PetProfile {
  id: string;
  name: string;
  level: number;
  experience: number;
  stage: PetStage;
  currentMood: PetMood;
  createdAt: string;
  updatedAt: string;
}

export interface PetSettings {
  petId: string;
  desktopEnabled: boolean;
  alwaysOnTop: boolean;
  muted: boolean;
  focusModeEnabled: boolean;
  proactiveLevel: number;
  lastWindowX?: number;
  lastWindowY?: number;
  updatedAt: string;
}

export interface PetCosmeticUnlock {
  id: string;
  petId: string;
  cosmeticType: string;
  cosmeticKey: string;
  unlockedAt: string;
  equipped: boolean;
}

export interface PetEventLedgerEntry {
  id: string;
  petId: string;
  eventType: string;
  eventSource: string;
  eventValue: number;
  eventTime: string;
  metadata?: string;
}

export interface PetWorkflowEventInput {
  eventType: PetWorkflowEventType;
  metadata?: string;
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

export interface AppUpdateStatus {
  status:
    | "idle"
    | "checking"
    | "updateAvailable"
    | "upToDate"
    | "downloading"
    | "installing"
    | "restartRequired"
    | "error"
    | "unsupported";
  platform: "macos" | "windows" | "unsupported";
  channel: string;
  currentVersion: string;
  latestVersion?: string;
  lastCheckedAt?: string;
  releaseNotes?: string;
  pubDate?: string;
  message?: string;
  downloadPercent?: number;
  feedUrl: string;
  canAutoInstall: boolean;
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

export interface ProcessMetrics {
  cpuPercent: number;
  memoryMb: number;
}
