import { useSyncExternalStore } from "react";
import {
  applyLocalPetWorkflowEvent,
  createLocalPetService,
  PET_STATE_CHANGED_EVENT,
} from "@/shared/services/tauri/pet";
import type {
  PetCosmeticUnlock,
  PetEventLedgerEntry,
  PetInteractionAction,
  PetProfile,
  PetSettings,
  PetStoreState,
  PetWorkflowEventInput,
} from "@/shared/types/meeting";

type PetState = {
  profile: PetProfile | null;
  settings: PetSettings | null;
  storeState: PetStoreState | null;
  cosmetics: PetCosmeticUnlock[];
  events: PetEventLedgerEntry[];
  loaded: boolean;
  loading: boolean;
};

let state: PetState = {
  profile: null,
  settings: null,
  storeState: null,
  cosmetics: [],
  events: [],
  loaded: false,
  loading: false,
};

const petService = createLocalPetService();
const listeners = new Set<() => void>();
let didBindPetStateChangedEvent = false;

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
  bindPetStateChangedEvent();
  if (state.loaded && !force) {
    return;
  }

  setState({ loading: true });
  try {
    const [profile, settings, storeState, cosmetics, events] = await Promise.all([
      petService.getProfile(),
      petService.getSettings(),
      petService.getStoreState(),
      petService.listCosmeticUnlocks(),
      petService.listEventLedger(20),
    ]);
    setState({
      profile,
      settings,
      storeState,
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
  const storeState = await petService.getStoreState();
  setState({ profile, storeState });
  return profile;
}

async function applyInteraction(action: PetInteractionAction) {
  const profile = await petService.applyInteraction(action);
  const [storeState, events] = await Promise.all([
    petService.getStoreState(),
    petService.listEventLedger(20),
  ]);
  setState({ profile, storeState, events });
  return profile;
}

export async function applyPetWorkflowEvent(input: PetWorkflowEventInput) {
  const profile = await applyLocalPetWorkflowEvent(input);
  const [storeState, events] = await Promise.all([
    petService.getStoreState(),
    petService.listEventLedger(20),
  ]);
  setState({ profile, storeState, events });
  return profile;
}

const actions = {
  loadPetState,
  refresh,
  saveProfile,
  saveSettings,
  applyInteraction,
  applyWorkflowEvent: applyPetWorkflowEvent,
};

function bindPetStateChangedEvent() {
  if (didBindPetStateChangedEvent || typeof window === "undefined") {
    return;
  }

  didBindPetStateChangedEvent = true;
  window.addEventListener(PET_STATE_CHANGED_EVENT, () => {
    if (state.loaded) {
      void refresh();
    }
  });
}

export function usePetStore() {
  bindPetStateChangedEvent();
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
