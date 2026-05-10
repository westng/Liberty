import { computed, reactive, toRefs } from "vue";
import { createLocalPetService } from "@/shared/services/tauri/pet";
import type {
  PetCosmeticUnlock,
  PetEventLedgerEntry,
  PetInteractionAction,
  PetProfile,
  PetSettings,
  PetWorkflowEventInput,
} from "@/shared/types/meeting";

const petService = createLocalPetService();

const state = reactive({
  profile: null as PetProfile | null,
  settings: null as PetSettings | null,
  cosmetics: [] as PetCosmeticUnlock[],
  events: [] as PetEventLedgerEntry[],
  loaded: false,
  loading: false,
});

async function loadPetState(force = false) {
  if (state.loaded && !force) {
    return;
  }

  state.loading = true;
  try {
    const [profile, settings, cosmetics, events] = await Promise.all([
      petService.getProfile(),
      petService.getSettings(),
      petService.listCosmeticUnlocks(),
      petService.listEventLedger(20),
    ]);
    state.profile = profile;
    state.settings = settings;
    state.cosmetics = cosmetics;
    state.events = events;
    state.loaded = true;
  } finally {
    state.loading = false;
  }
}

export function usePetStore() {
  const stageProgress = computed(() => {
    const experience = state.profile?.experience ?? 0;
    return experience % 20;
  });

  async function refresh() {
    await loadPetState(true);
  }

  async function saveSettings(partial: Omit<PetSettings, "petId" | "updatedAt">) {
    state.settings = await petService.saveSettings(partial);
    return state.settings;
  }

  async function saveProfile(partial: Pick<PetProfile, "name">) {
    state.profile = await petService.saveProfile(partial);
    return state.profile;
  }

  async function applyInteraction(action: PetInteractionAction) {
    state.profile = await petService.applyInteraction(action);
    state.events = await petService.listEventLedger(20);
    return state.profile;
  }

  async function applyWorkflowEvent(input: PetWorkflowEventInput) {
    state.profile = await petService.applyWorkflowEvent(input);
    state.events = await petService.listEventLedger(20);
    return state.profile;
  }

  return {
    ...toRefs(state),
    stageProgress,
    loadPetState,
    refresh,
    saveProfile,
    saveSettings,
    applyInteraction,
    applyWorkflowEvent,
  };
}
