import { useSyncExternalStore } from "react";
import { createLocalPetService } from "@/shared/services/tauri/pet";
import type {
  PetCosmeticUnlock,
  PetEventLedgerEntry,
  PetInteractionAction,
  PetProfile,
  PetSettings,
  PetWorkflowEventInput,
} from "@/shared/types/meeting";

type PetState = {
  profile: PetProfile | null;
  settings: PetSettings | null;
  cosmetics: PetCosmeticUnlock[];
  events: PetEventLedgerEntry[];
  loaded: boolean;
  loading: boolean;
};

let state: PetState = {
  profile: null,
  settings: null,
  cosmetics: [],
  events: [],
  loaded: false,
  loading: false,
};

const petService = createLocalPetService();
const listeners = new Set<() => void>();

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function getSnapshot() {
  return state;
}

function setState(patch: Partial<PetState>) {
  state = { ...state, ...patch };
  for (const listener of listeners) {
    listener();
  }
}

async function loadPetState(force = false) {
  if (state.loaded && !force) {
    return;
  }

  setState({ loading: true });
  try {
    const [profile, settings, cosmetics, events] = await Promise.all([
      petService.getProfile(),
      petService.getSettings(),
      petService.listCosmeticUnlocks(),
      petService.listEventLedger(20),
    ]);
    setState({
      profile,
      settings,
      cosmetics,
      events,
      loaded: true,
    });
  } finally {
    setState({ loading: false });
  }
}

async function refresh() {
  await loadPetState(true);
}

async function saveSettings(partial: Omit<PetSettings, "petId" | "updatedAt">) {
  const settings = await petService.saveSettings(partial);
  setState({ settings });
  return settings;
}

async function saveProfile(partial: Pick<PetProfile, "name">) {
  const profile = await petService.saveProfile(partial);
  setState({ profile });
  return profile;
}

async function applyInteraction(action: PetInteractionAction) {
  const [profile, events] = await Promise.all([
    petService.applyInteraction(action),
    petService.listEventLedger(20),
  ]);
  setState({ profile, events });
  return profile;
}

async function applyWorkflowEvent(input: PetWorkflowEventInput) {
  const [profile, events] = await Promise.all([
    petService.applyWorkflowEvent(input),
    petService.listEventLedger(20),
  ]);
  setState({ profile, events });
  return profile;
}

const actions = {
  loadPetState,
  refresh,
  saveProfile,
  saveSettings,
  applyInteraction,
  applyWorkflowEvent,
};

export function usePetStore() {
  const snapshot = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
  const levelSnapshot = snapshot.profile?.levelSnapshot;

  return {
    ...snapshot,
    levelSnapshot,
    currentLevelExp: levelSnapshot?.currentLevelExp ?? 0,
    nextLevelRequired: levelSnapshot?.nextLevelRequired ?? 0,
    levelProgressRatio: levelSnapshot?.progressRatio ?? 0,
    isMaxLevel: levelSnapshot?.isMaxLevel ?? false,
    ...actions,
  };
}

export type PetStore = ReturnType<typeof usePetStore>;
