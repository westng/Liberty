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
  | "repair_required"
  | "unsupported";
export type PetMood = "idle" | "cheerful" | "excited" | "proud" | "needy" | "sleepy" | "bored";
export type PetStage =
  | "first_meet"
  | "familiar"
  | "steady_companion"
  | "grow_together"
  | "tacit_bond"
  | "deep_bond"
  | "long_company"
  | "bond_forever"
  | "baby"
  | "growing"
  | "mature";
export type PetInteractionAction = "tap" | "pet" | "feed" | "encourage";
export type PetWorkflowEventType =
  | "job_created"
  | "transcription_started"
  | "transcription_completed"
  | "ai_summary_completed"
  | "export_completed"
  | "daily_open"
  | "dark_theme_used";

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
  apiKeyRef?: string;
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

export interface PetLevelSnapshot {
  level: number;
  currentLevelExp: number;
  nextLevelRequired: number;
  totalExperience: number;
  currentStage: PetStage;
  currentStageLabelZh: string;
  currentStageLabelEn: string;
  nextStage?: PetStage;
  nextStageLevel?: number;
  progressRatio: number;
  isMaxLevel: boolean;
}

export interface PetProfile {
  id: string;
  name: string;
  level: number;
  experience: number;
  stage: PetStage;
  levelSnapshot: PetLevelSnapshot;
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

export interface PetWallet {
  petId: string;
  currencyKey: string;
  balance: number;
  lifetimeEarned: number;
  lifetimeSpent: number;
  updatedAt: string;
}

export interface PetInventoryItem {
  id: string;
  petId: string;
  itemKey: string;
  itemType: string;
  slot: string;
  quantity: number;
  equipped: boolean;
  source: string;
  purchasedAt: string;
  updatedAt: string;
}

export interface PetEconomyEntry {
  id: string;
  petId: string;
  entryType: string;
  currencyKey: string;
  amount: number;
  balanceAfter: number;
  sourceType: string;
  sourceKey: string;
  metadata?: string;
  createdAt: string;
}

export interface PetMilestoneCounter {
  petId: string;
  counterKey: string;
  counterValue: number;
  lastEventKey: string;
  updatedAt: string;
}

export interface PetStoreCatalogItem {
  itemKey: string;
  itemType: "pet" | "cosmetic" | "theme" | "tool" | "food" | "badge" | "none";
  slot: "pet" | "accessory" | "scene" | "badge" | "consumable" | "none";
  nameZh: string;
  nameEn: string;
  descriptionZh: string;
  descriptionEn: string;
  rarity: string;
  priceLp: number;
  levelGate: number;
  stageGate: string;
  milestoneGate: string;
  assetKey: string;
  growthValue: number;
  enabled: boolean;
  sortOrder: number;
}

export interface PetStoreCatalogItemState {
  item: PetStoreCatalogItem;
  owned: boolean;
  equipped: boolean;
  quantity: number;
  growthValue: number;
  dailyFreeLimit: number;
  dailyFreeClaimed: number;
  dailyFreeRemaining: number;
  purchasable: boolean;
  lockedReasonZh: string;
  lockedReasonEn: string;
  status: "equipped" | "owned" | "coming_soon" | "locked" | "achievement" | "insufficient" | "daily_limit" | "available";
}

export interface PetEquipmentState {
  currentPet?: PetInventoryItem;
  accessory?: PetInventoryItem;
  scene?: PetInventoryItem;
  badge?: PetInventoryItem;
}

export interface PetStoreState {
  profile: PetProfile;
  wallet: PetWallet;
  catalog: PetStoreCatalogItemState[];
  inventory: PetInventoryItem[];
  equipment: PetEquipmentState;
  counters: PetMilestoneCounter[];
  economy: PetEconomyEntry[];
}

export interface PetBlindBoxDrawEntry {
  id: string;
  petId: string;
  drawDate: string;
  itemKey: string;
  itemType: PetStoreCatalogItem["itemType"];
  quantity: number;
  duplicateCompensationLp: number;
  createdAt: string;
}

export interface PetBlindBoxPoolItem {
  item: PetStoreCatalogItem;
  owned: boolean;
  weight: number;
  duplicateCompensationLp: number;
}

export interface PetBlindBoxState {
  drawDate: string;
  dailyLimit: number;
  usedToday: number;
  remainingToday: number;
  pool: PetBlindBoxPoolItem[];
  history: PetBlindBoxDrawEntry[];
  storeState: PetStoreState;
}

export interface PetBlindBoxDrawResult {
  state: PetBlindBoxState;
  draw: PetBlindBoxDrawEntry;
  prize: PetStoreCatalogItem;
  duplicate: boolean;
}

export interface PetRewardItem {
  itemKey: string;
  itemType: PetStoreCatalogItem["itemType"];
  quantity: number;
  duplicateCompensationLp: number;
}

export interface PetDailyCheckInEntry {
  id: string;
  petId: string;
  checkInDate: string;
  streakCount: number;
  cycleDay: number;
  rewardLp: number;
  growthValue: number;
  rewardItems: PetRewardItem[];
  createdAt: string;
}

export interface PetDailyCheckInRewardPreview {
  cycleDay: number;
  rewardLp: number;
  growthValue: number;
  items: PetRewardItem[];
}

export interface PetDailyCheckInState {
  checkInDate: string;
  checkedInToday: boolean;
  currentStreak: number;
  nextCycleDay: number;
  cycleLength: number;
  todayReward: PetDailyCheckInRewardPreview;
  rewards: PetDailyCheckInRewardPreview[];
  history: PetDailyCheckInEntry[];
  storeState: PetStoreState;
}

export interface PetDailyCheckInClaimResult {
  state: PetDailyCheckInState;
  entry: PetDailyCheckInEntry;
  duplicate: boolean;
}

export interface PetGiftBoxOpenResult {
  state: PetStoreState;
  prize: PetStoreCatalogItem;
  duplicate: boolean;
  duplicateCompensationLp: number;
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

export type PlatformValidationLevel = "primary" | "extended";

export interface SupportedPlatform {
  id: string;
  label: string;
  rustTarget: string;
  validationLevel: PlatformValidationLevel;
}

export interface SecurityBaselineStatus {
  cspEnabled: boolean;
  scopedCapabilities: boolean;
  credentialStoreRequired: boolean;
}

export interface DiagnosticsReport {
  appVersion: string;
  currentPlatform?: SupportedPlatform;
  supportedPlatforms: SupportedPlatform[];
  databasePath?: string;
  schemaVersion: number;
  runtimeStatus: string;
  desktopPetDiagnosticLogPath?: string;
  desktopPetDiagnosticLogTail: string;
  securityBaseline: SecurityBaselineStatus;
}
