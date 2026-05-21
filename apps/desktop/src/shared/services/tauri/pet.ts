import { invoke } from "@tauri-apps/api/core";
import type {
  PetBlindBoxDrawResult,
  PetBlindBoxState,
  PetCosmeticUnlock,
  PetEventLedgerEntry,
  PetInteractionAction,
  PetProfile,
  PetSettings,
  PetStoreState,
  PetWorkflowEventInput,
} from "@/shared/types/meeting";

type SavePetSettingsInput = Omit<PetSettings, "petId" | "updatedAt">;
type SavePetProfileInput = Pick<PetProfile, "name">;

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
    setStoreItemDetailItem: (itemKey: string) =>
      invoke<string>("set_pet_store_item_detail_item", { input: { itemKey } }),
    getStoreItemDetailItem: () => invoke<string>("get_pet_store_item_detail_item"),
    purchaseStoreItem: (itemKey: string, quantity = 1) =>
      invoke<PetStoreState>("purchase_pet_store_item", { input: { itemKey, quantity } }),
    equipInventoryItem: (itemKey: string) => invoke<PetStoreState>("equip_pet_inventory_item", { input: { itemKey } }),
    unequipInventorySlot: (slot: string) => invoke<PetStoreState>("unequip_pet_inventory_slot", { input: { slot } }),
    useInventoryItem: (itemKey: string, quantity = 1) =>
      invoke<PetStoreState>("use_pet_inventory_item", { input: { itemKey, quantity } }),
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
