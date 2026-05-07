import { invoke } from "@tauri-apps/api/core";
import type {
  PetCosmeticUnlock,
  PetEventLedgerEntry,
  PetInteractionAction,
  PetProfile,
  PetSettings,
  PetWorkflowEventInput,
} from "@/types/meeting";

type SavePetSettingsInput = Omit<PetSettings, "petId" | "updatedAt">;
type SavePetProfileInput = Pick<PetProfile, "name">;

export function createLocalPetService() {
  return {
    getProfile: () => invoke<PetProfile>("get_pet_profile"),
    saveProfile: (input: SavePetProfileInput) => invoke<PetProfile>("save_pet_profile", { input }),
    getSettings: () => invoke<PetSettings>("get_pet_settings"),
    saveSettings: (input: SavePetSettingsInput) => invoke<PetSettings>("save_pet_settings", { input }),
    listEventLedger: (limit = 20) => invoke<PetEventLedgerEntry[]>("list_pet_event_ledger", { limit }),
    listCosmeticUnlocks: () => invoke<PetCosmeticUnlock[]>("list_pet_cosmetic_unlocks"),
    applyInteraction: (action: PetInteractionAction) =>
      invoke<PetProfile>("apply_pet_interaction", {
        input: { action },
      }),
    applyWorkflowEvent: (input: PetWorkflowEventInput) =>
      invoke<PetProfile>("apply_pet_workflow_event", {
        input,
      }),
  };
}
