import { invoke } from "@tauri-apps/api/core";
import type {
  PetBlindBoxDrawResult,
  PetBlindBoxState,
  PetCosmeticUnlock,
  PetDailyCheckInClaimResult,
  PetDailyCheckInMakeupResult,
  PetDailyCheckInState,
  PetEventLedgerEntry,
  PetGiftBoxOpenResult,
  PetInteractionAction,
  PetProfile,
  PetSettings,
  PetStoreState,
  PetWorkflowEventInput,
} from "@/shared/types/meeting";

type SavePetSettingsInput = Omit<PetSettings, "petId" | "updatedAt">;
type SavePetProfileInput = Pick<PetProfile, "name">;
export const PET_STATE_CHANGED_EVENT = "liberty:pet-state-changed";

export interface DesktopPetStatus {
  visible: boolean;
  instanceCount: number;
}

export function applyDesktopPetState(settings: PetSettings, source = "app") {
  if (!settings.desktopEnabled) {
    return invoke<boolean>("hide_desktop_pet", { source });
  }

  return invoke<boolean>("show_desktop_pet", { source });
}

export function openExtraDesktopPet() {
  return invoke<DesktopPetStatus>("open_extra_desktop_pet");
}

export function createLocalPetService() {
  return {
    getProfile: () => invoke<PetProfile>("get_pet_profile"),
    saveProfile: (input: SavePetProfileInput) => invoke<PetProfile>("save_pet_profile", { input }),
    getSettings: () => invoke<PetSettings>("get_pet_settings"),
    saveSettings: (input: SavePetSettingsInput) => invoke<PetSettings>("save_pet_settings", { input }),
    getStoreState: () => invoke<PetStoreState>("get_pet_store_state"),
    getBlindBoxState: () => invoke<PetBlindBoxState>("get_pet_blind_box_state"),
    drawBlindBox: () => invoke<PetBlindBoxDrawResult>("draw_pet_blind_box"),
    getDailyCheckInState: () => invoke<PetDailyCheckInState>("get_pet_daily_check_in_state"),
    claimDailyCheckIn: async () => {
      const result = await invoke<PetDailyCheckInClaimResult>("claim_pet_daily_check_in");
      notifyPetStateChanged("daily-check-in");
      return result;
    },
    repairDailyCheckIn: async () => {
      const result = await invoke<PetDailyCheckInMakeupResult>("repair_pet_daily_check_in");
      notifyPetStateChanged("daily-check-in-makeup");
      return result;
    },
    purchaseStoreItem: (itemKey: string, quantity = 1) =>
      invoke<PetStoreState>("purchase_pet_store_item", { input: { itemKey, quantity } }),
    equipInventoryItem: (itemKey: string) => invoke<PetStoreState>("equip_pet_inventory_item", { input: { itemKey } }),
    unequipInventorySlot: (slot: string) => invoke<PetStoreState>("unequip_pet_inventory_slot", { input: { slot } }),
    useInventoryItem: (itemKey: string, quantity = 1) =>
      invoke<PetStoreState>("use_pet_inventory_item", { input: { itemKey, quantity } }),
    openGiftBox: async () => {
      const result = await invoke<PetGiftBoxOpenResult>("open_pet_gift_box");
      notifyPetStateChanged("gift-box");
      return result;
    },
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
    openExtraDesktopPet,
  };
}

export async function applyLocalPetWorkflowEvent(input: PetWorkflowEventInput) {
  const profile = await createLocalPetService().applyWorkflowEvent(input);
  notifyPetStateChanged("workflow");
  return profile;
}

export function notifyPetStateChanged(reason: string): void {
  if (typeof window === "undefined") {
    return;
  }

  window.dispatchEvent(new CustomEvent(PET_STATE_CHANGED_EVENT, { detail: { reason } }));
}
