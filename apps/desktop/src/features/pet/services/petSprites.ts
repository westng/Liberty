import type { JobStage, MeetingJob, PetEventLedgerEntry, PetMood, PetStage } from "@/shared/types/meeting";

export type PetImageGroup =
  | "crush"
  | "defecate"
  | "drive"
  | "eat"
  | "pants"
  | "read"
  | "rope"
  | "run"
  | "slack"
  | "sleep"
  | "snow"
  | "toy"
  | "work";
type PetFrameEntry = {
  frame: number;
  url: string;
};

const petImageModules = import.meta.glob("/src/assets/images/*/*.png", {
  eager: true,
  import: "default",
}) as Record<string, string>;

const moodImageGroupMap: Record<PetMood, PetImageGroup> = {
  idle: "slack",
  cheerful: "snow",
  excited: "run",
  proud: "eat",
  needy: "toy",
  sleepy: "sleep",
  bored: "read",
};

const stageScaleMap: Record<PetStage, number> = {
  baby: 1,
  growing: 1.04,
  mature: 1.08,
};

function parsePetFrameFilename(path: string): { group: PetImageGroup; frame: number } | null {
  const filename = path.split("/").pop() ?? "";
  if (!filename.endsWith(".png") || filename.startsWith("._")) {
    return null;
  }

  const matched = filename.match(/^(\d+)\.png$/);
  if (!matched) {
    return null;
  }

  const groupName = path.split("/").at(-2);
  if (!isPetImageGroup(groupName)) {
    return null;
  }

  const [, frameValue] = matched;
  const frame = Number.parseInt(frameValue, 10);
  if (!Number.isFinite(frame) || frame <= 0) {
    return null;
  }

  return {
    group: groupName,
    frame,
  };
}

function isPetImageGroup(value: string | undefined): value is PetImageGroup {
  return (
    value === "crush" ||
    value === "defecate" ||
    value === "drive" ||
    value === "eat" ||
    value === "pants" ||
    value === "read" ||
    value === "rope" ||
    value === "run" ||
    value === "slack" ||
    value === "sleep" ||
    value === "snow" ||
    value === "toy" ||
    value === "work"
  );
}

function getPetImageGroupByMood(mood: PetMood = "idle") {
  return moodImageGroupMap[mood] ?? moodImageGroupMap.idle;
}

export function getPetEnvironmentState(jobs: Pick<MeetingJob, "overallStatus">[]): JobStage | null {
  if (jobs.some((job) => job.overallStatus === "transcribing")) {
    return "transcribing";
  }
  if (jobs.some((job) => job.overallStatus === "speaker_processing")) {
    return "speaker_processing";
  }
  if (jobs.some((job) => job.overallStatus === "summarizing")) {
    return "summarizing";
  }
  if (jobs.some((job) => job.overallStatus === "queued")) {
    return "queued";
  }

  return null;
}

export function getPetImageGroupForEnvironment(environmentState?: JobStage | null, mood: PetMood = "idle") {
  switch (environmentState) {
    case "queued":
      return "slack";
    case "transcribing":
    case "speaker_processing":
    case "summarizing":
      return "work";
    case "completed":
      return "snow";
    case "failed":
      return "pants";
    case "uploaded":
      return "read";
    case "idle":
    default:
      return getPetImageGroupByMood(mood);
  }
}

function buildPetImageGroups() {
  const groups = new Map<PetImageGroup, PetFrameEntry[]>();

  Object.entries(petImageModules).forEach(([path, url]) => {
    const parsed = parsePetFrameFilename(path);
    if (!parsed) {
      return;
    }

    const existing = groups.get(parsed.group) ?? [];
    existing.push({ frame: parsed.frame, url });
    groups.set(parsed.group, existing);
  });

  groups.forEach((frames) => {
    frames.sort((left, right) => left.frame - right.frame);
  });

  return groups;
}

const petImageGroups = buildPetImageGroups();

function getFirstAvailablePetImageUrl(...groups: PetImageGroup[]) {
  for (const group of groups) {
    const firstFrame = petImageGroups.get(group)?.[0]?.url;
    if (firstFrame) {
      return firstFrame;
    }
  }

  return Object.values(petImageModules)[0] ?? "";
}

const coverImageUrl = getFirstAvailablePetImageUrl("slack", "snow", "work");

export function getPetCoverUrl() {
  return coverImageUrl;
}

export function getPetSpriteUrlForGroup(group: PetImageGroup, frameIndex = 0) {
  const frames = petImageGroups.get(group);

  if (!frames?.length) {
    return coverImageUrl;
  }

  const safeIndex = Math.abs(frameIndex) % frames.length;
  return frames[safeIndex]?.url ?? frames[0]?.url ?? coverImageUrl;
}

export function getPetSpriteUrl(mood: PetMood = "idle", frameIndex = 0) {
  const group = getPetImageGroupByMood(mood);
  return getPetSpriteUrlForGroup(group, frameIndex);
}

export function getPetSpriteUrlForEnvironment(
  environmentState: JobStage | null | undefined,
  mood: PetMood = "idle",
  frameIndex = 0,
) {
  const group = getPetImageGroupForEnvironment(environmentState, mood);
  return getPetSpriteUrlForGroup(group, frameIndex);
}

export function getPetSpriteFrameCount(mood: PetMood = "idle") {
  const group = getPetImageGroupByMood(mood);
  return petImageGroups.get(group)?.length ?? 1;
}

export function getPetSpriteFrameCountForGroup(group: PetImageGroup) {
  return petImageGroups.get(group)?.length ?? 1;
}

export function getPetSpriteFrameCountForEnvironment(environmentState: JobStage | null | undefined, mood: PetMood = "idle") {
  const group = getPetImageGroupForEnvironment(environmentState, mood);
  return getPetSpriteFrameCountForGroup(group);
}

export function getPetSpriteScale(stage: PetStage = "baby") {
  return stageScaleMap[stage] ?? 1;
}

type PetVisualActionInput = {
  environmentState?: JobStage | null;
  latestEvent?: PetEventLedgerEntry | null;
  mood?: PetMood;
  nowMs?: number;
};

const RECENT_EVENT_ACTION_HOLD_MS = 45_000;
const NEEDY_AFTER_MS = 30_000;
const DAILY_ACTION_BUCKET_MS = 2 * 60_000;
const dailyIdleActions: PetImageGroup[] = [
  "slack",
  "toy",
  "rope",
  "drive",
  "crush",
  "defecate",
  "eat",
  "pants",
  "read",
  "run",
  "sleep",
  "snow",
  "work",
];

export function resolvePetVisualAction({
  environmentState,
  latestEvent,
  mood = "idle",
  nowMs = Date.now(),
}: PetVisualActionInput): PetImageGroup {
  if (environmentState) {
    return getPetImageGroupForEnvironment(environmentState, mood);
  }

  const latestEventAt = parsePetEventTime(latestEvent);
  const idleMs = latestEventAt ? Math.max(0, nowMs - latestEventAt) : 0;

  if (latestEvent && idleMs <= RECENT_EVENT_ACTION_HOLD_MS) {
    return getPetImageGroupForRecentEvent(latestEvent, mood);
  }

  if (!latestEventAt) {
    return getDailyIdleAction(0, nowMs);
  }

  if (idleMs >= NEEDY_AFTER_MS) {
    return getDailyIdleAction(latestEventAt, idleMs);
  }

  return getPetImageGroupByMood(mood);
}

function parsePetEventTime(event?: PetEventLedgerEntry | null) {
  const value = event?.eventTime;
  if (!value) {
    return 0;
  }

  const parsed = Date.parse(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

function getPetImageGroupForRecentEvent(event: PetEventLedgerEntry, mood: PetMood): PetImageGroup {
  if (event.eventType === "interaction") {
    switch (event.eventSource) {
      case "feed":
        return "eat";
      case "encourage":
        return "rope";
      case "pet":
        return "crush";
      case "tap":
        return "toy";
      default:
        return getPetImageGroupByMood(mood);
    }
  }

  switch (event.eventType) {
    case "job_created":
      return "drive";
    case "transcription_started":
      return "work";
    case "transcription_completed":
    case "ai_summary_completed":
    case "export_completed":
      return "snow";
    case "daily_open":
      return "slack";
    default:
      return getPetImageGroupByMood(mood);
  }
}

function getDailyIdleAction(latestEventAt: number, idleMs: number): PetImageGroup {
  const bucket = Math.floor((latestEventAt + idleMs) / DAILY_ACTION_BUCKET_MS);
  return dailyIdleActions[bucket % dailyIdleActions.length] ?? "slack";
}
