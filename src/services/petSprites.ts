import fallbackPetImageUrl from "@/assets/pet-angel.png";
import type { JobStage, PetMood, PetStage } from "@/types/meeting";

type PetImageGroup = "stand" | "work" | "sleepy" | "snow" | "coffee" | "read-books" | "Snow-King";
type PetFrameEntry = {
  frame: number;
  url: string;
};

const petImageModules = import.meta.glob("../assets/images/pet/*.png", {
  eager: true,
  import: "default",
}) as Record<string, string>;

const moodImageGroupMap: Record<PetMood, PetImageGroup> = {
  idle: "stand",
  cheerful: "snow",
  excited: "work",
  proud: "Snow-King",
  needy: "coffee",
  sleepy: "sleepy",
  bored: "read-books",
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

  const matched = filename.match(/^(.+)-(\d+)\.png$/);
  if (!matched) {
    return null;
  }

  const [, groupName, frameValue] = matched;
  const frame = Number.parseInt(frameValue, 10);
  if (!Number.isFinite(frame) || frame <= 0) {
    return null;
  }

  return {
    group: groupName as PetImageGroup,
    frame,
  };
}

function getPetImageGroupByMood(mood: PetMood = "idle") {
  return moodImageGroupMap[mood] ?? moodImageGroupMap.idle;
}

export function getPetImageGroupForEnvironment(environmentState?: JobStage | null, mood: PetMood = "idle") {
  switch (environmentState) {
    case "queued":
      return "coffee";
    case "transcribing":
    case "speaker_processing":
    case "summarizing":
      return "work";
    case "completed":
      return "Snow-King";
    case "failed":
      return "sleepy";
    case "uploaded":
      return "read-books";
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
const coverImageUrl = new URL("../assets/images/cover.png", import.meta.url).href;

export function getPetCoverUrl() {
  return coverImageUrl;
}

export function getPetSpriteUrl(mood: PetMood = "idle", frameIndex = 0) {
  const group = getPetImageGroupByMood(mood);
  const frames = petImageGroups.get(group);

  if (!frames?.length) {
    return fallbackPetImageUrl;
  }

  const safeIndex = Math.abs(frameIndex) % frames.length;
  return frames[safeIndex]?.url ?? frames[0]?.url ?? fallbackPetImageUrl;
}

export function getPetSpriteUrlForEnvironment(
  environmentState: JobStage | null | undefined,
  mood: PetMood = "idle",
  frameIndex = 0,
) {
  const group = getPetImageGroupForEnvironment(environmentState, mood);
  const frames = petImageGroups.get(group);

  if (!frames?.length) {
    return fallbackPetImageUrl;
  }

  const safeIndex = Math.abs(frameIndex) % frames.length;
  return frames[safeIndex]?.url ?? frames[0]?.url ?? fallbackPetImageUrl;
}

export function getPetSpriteFrameCount(mood: PetMood = "idle") {
  const group = getPetImageGroupByMood(mood);
  return petImageGroups.get(group)?.length ?? 1;
}

export function getPetSpriteFrameCountForEnvironment(environmentState: JobStage | null | undefined, mood: PetMood = "idle") {
  const group = getPetImageGroupForEnvironment(environmentState, mood);
  return petImageGroups.get(group)?.length ?? 1;
}

export function getPetSpriteScale(stage: PetStage = "baby") {
  return stageScaleMap[stage] ?? 1;
}
