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
  PetRedeemKeyRedemption,
  PetRedeemKeyResult,
  PetSettings,
  PetStoreCatalogItem,
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
  if (!("__TAURI_INTERNALS__" in window)) {
    return createPreviewPetService();
  }

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
    purchaseStoreItem: async (itemKey: string, quantity = 1) => {
      const result = await invoke<PetStoreState>("purchase_pet_store_item", { input: { itemKey, quantity } });
      notifyPetStateChanged("purchase");
      return result;
    },
    equipInventoryItem: (itemKey: string) => invoke<PetStoreState>("equip_pet_inventory_item", { input: { itemKey } }),
    unequipInventorySlot: (slot: string) => invoke<PetStoreState>("unequip_pet_inventory_slot", { input: { slot } }),
    useInventoryItem: (itemKey: string, quantity = 1) =>
      invoke<PetStoreState>("use_pet_inventory_item", { input: { itemKey, quantity } }),
    openGiftBox: async () => {
      const result = await invoke<PetGiftBoxOpenResult>("open_pet_gift_box");
      notifyPetStateChanged("gift-box");
      return result;
    },
    redeemKey: async (key: string) => {
      const result = await invoke<PetRedeemKeyResult>("redeem_pet_key", { input: { key } });
      notifyPetStateChanged("redeem-key");
      return result;
    },
    listRedeemKeyRedemptions: (limit = 20) =>
      invoke<PetRedeemKeyRedemption[]>("list_pet_redeem_key_redemptions", { limit }),
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

const previewSeedItems = [
  ["wheat-seed", "小麦种子", "Wheat Seeds", "农场播种消耗品，可种出小麦。", "A farm planting consumable used to grow wheat.", "first_meet", 4, "wheat_seed"],
  ["carrot-seed", "胡萝卜种子", "Carrot Seeds", "农场播种消耗品，可种出胡萝卜。", "A farm planting consumable used to grow carrots.", "first_meet", 6, "carrot_seed"],
  ["tomato-seed", "番茄种子", "Tomato Seeds", "农场播种消耗品，可种出番茄。", "A farm planting consumable used to grow tomatoes.", "familiar", 9, "tomato_seed"],
  ["pumpkin-seed", "南瓜种子", "Pumpkin Seeds", "农场播种消耗品，可种出高收益南瓜。", "A farm planting consumable used to grow high-yield pumpkins.", "grow_together", 14, "pumpkin_seed"],
  ["corn-seed", "玉米种子", "Corn Seeds", "农场播种消耗品，可种出玉米。", "A farm planting consumable used to grow corn.", "familiar", 8, "corn_seed"],
  ["strawberry-seed", "草莓种子", "Strawberry Seeds", "农场播种消耗品，可种出草莓。", "A farm planting consumable used to grow strawberries.", "grow_together", 10, "strawberry_seed"],
  ["blueberry-seed", "蓝莓种子", "Blueberry Seeds", "农场播种消耗品，可种出蓝莓。", "A farm planting consumable used to grow blueberries.", "deep_bond", 15, "blueberry_seed"],
  ["potato-seed", "土豆种子", "Potato Seeds", "农场播种消耗品，可种出土豆。", "A farm planting consumable used to grow potatoes.", "first_meet", 5, "potato_seed"],
] as const;

const previewHarvestItems = [
  ["wheat-harvest-food", "小麦", "Wheat", "农场收获的小麦，可作为基础投喂食物。", "Wheat harvested from the farm, usable as a basic feeding item.", "first_meet", 18, "harvest_wheat", 4],
  ["carrot-harvest-food", "胡萝卜", "Carrot", "农场收获的胡萝卜，清爽稳定的投喂食物。", "Carrots harvested from the farm, a steady fresh feeding item.", "first_meet", 24, "harvest_carrot", 6],
  ["tomato-harvest-food", "番茄", "Tomato", "农场收获的番茄，成熟后带来一份元气照顾。", "Tomatoes harvested from the farm, a bright feeding item.", "familiar", 32, "harvest_tomato", 8],
  ["pumpkin-harvest-food", "南瓜", "Pumpkin", "农场收获的南瓜，饱满扎实的高级投喂食物。", "Pumpkins harvested from the farm, a hearty premium feeding item.", "grow_together", 58, "harvest_pumpkin", 14],
  ["corn-harvest-food", "玉米", "Corn", "农场收获的玉米，适合持续补充活力。", "Corn harvested from the farm, good for steady energy.", "familiar", 30, "harvest_corn", 8],
  ["strawberry-harvest-food", "草莓", "Strawberry", "农场收获的草莓，甜甜的小份投喂食物。", "Strawberries harvested from the farm, a sweet feeding item.", "grow_together", 38, "harvest_strawberry", 10],
  ["blueberry-harvest-food", "蓝莓", "Blueberry", "农场收获的蓝莓，小而珍贵的浆果投喂食物。", "Blueberries harvested from the farm, a small premium berry item.", "deep_bond", 48, "harvest_blueberry", 12],
  ["potato-harvest-food", "土豆", "Potato", "农场收获的土豆，便宜稳定的基础投喂食物。", "Potatoes harvested from the farm, a reliable basic feeding item.", "first_meet", 20, "harvest_potato", 5],
] as const;

let previewStoreState: PetStoreState = createPreviewStoreState();

function createPreviewPetService() {
  return {
    getProfile: async () => previewStoreState.profile,
    saveProfile: async (input: SavePetProfileInput) => {
      previewStoreState = { ...previewStoreState, profile: { ...previewStoreState.profile, ...input, updatedAt: new Date().toISOString() } };
      notifyPetStateChanged("preview-profile");
      return previewStoreState.profile;
    },
    getSettings: async () => createPreviewSettings(),
    saveSettings: async (input: SavePetSettingsInput) => ({ ...createPreviewSettings(), ...input, updatedAt: new Date().toISOString() }),
    getStoreState: async () => previewStoreState,
    getBlindBoxState: async () => createPreviewBlindBoxState(),
    drawBlindBox: async (): Promise<PetBlindBoxDrawResult> => Promise.reject(new Error("Preview blind box is unavailable.")),
    getDailyCheckInState: async () => Promise.reject(new Error("Preview check-in is unavailable.")),
    claimDailyCheckIn: async () => Promise.reject(new Error("Preview check-in is unavailable.")),
    repairDailyCheckIn: async () => Promise.reject(new Error("Preview check-in is unavailable.")),
    purchaseStoreItem: async (itemKey: string, quantity = 1) => {
      const itemState = previewStoreState.catalog.find((entry) => entry.item.itemKey === itemKey);
      if (!itemState || !itemState.purchasable) {
        throw new Error("商品暂不可购买。");
      }
      const purchaseQuantity = Math.max(1, Math.min(99, Math.floor(quantity)));
      const totalPrice = itemState.item.priceLp * purchaseQuantity;
      if (previewStoreState.wallet.balance < totalPrice) {
        throw new Error("LP 余额不足。");
      }
      const now = new Date().toISOString();
      previewStoreState = upsertPreviewInventory({
        ...previewStoreState,
        wallet: {
          ...previewStoreState.wallet,
          balance: previewStoreState.wallet.balance - totalPrice,
          lifetimeSpent: previewStoreState.wallet.lifetimeSpent + totalPrice,
          updatedAt: now,
        },
      }, itemState.item, purchaseQuantity, "purchase", now);
      notifyPetStateChanged("purchase");
      return previewStoreState;
    },
    equipInventoryItem: async () => previewStoreState,
    unequipInventorySlot: async () => previewStoreState,
    useInventoryItem: async () => previewStoreState,
    openGiftBox: async () => Promise.reject(new Error("Preview gift box is unavailable.")),
    redeemKey: async () => Promise.reject(new Error("Preview redeem key is unavailable.")),
    listRedeemKeyRedemptions: async () => [],
    listEventLedger: async () => [],
    listCosmeticUnlocks: async () => [],
    applyInteraction: async () => previewStoreState.profile,
    applyWorkflowEvent: async () => previewStoreState.profile,
    openExtraDesktopPet,
  };
}

export function consumePreviewInventoryItem(itemKey: string, quantity = 1) {
  if ("__TAURI_INTERNALS__" in window) {
    return;
  }
  const now = new Date().toISOString();
  const inventoryItem = previewStoreState.inventory.find((item) => item.itemKey === itemKey);
  if (!inventoryItem || inventoryItem.quantity < quantity) {
    throw new Error("缺少种子，请先去宠物商店购买。");
  }
  previewStoreState = {
    ...previewStoreState,
    inventory: previewStoreState.inventory
      .map((item) => item.itemKey === itemKey ? { ...item, quantity: item.quantity - quantity, updatedAt: now } : item)
      .filter((item) => item.quantity > 0),
    catalog: previewStoreState.catalog.map((entry) =>
      entry.item.itemKey === itemKey
        ? { ...entry, owned: inventoryItem.quantity - quantity > 0, quantity: Math.max(0, inventoryItem.quantity - quantity), status: "available" }
        : entry,
    ),
  };
  notifyPetStateChanged("farm-plant");
}

export function grantPreviewInventoryItem(itemKey: string, quantity = 1, source = "farm_harvest") {
  if ("__TAURI_INTERNALS__" in window) {
    return;
  }
  const itemState = previewStoreState.catalog.find((entry) => entry.item.itemKey === itemKey);
  if (!itemState) {
    return;
  }
  previewStoreState = upsertPreviewInventory(previewStoreState, itemState.item, Math.max(1, quantity), source, new Date().toISOString());
  notifyPetStateChanged(source);
}

function createPreviewStoreState(): PetStoreState {
  const now = new Date().toISOString();
  const profile = createPreviewProfile(now);
  const wallet = {
    petId: profile.id,
    currencyKey: "LP",
    balance: 120,
    lifetimeEarned: 120,
    lifetimeSpent: 0,
    updatedAt: now,
  };
  return {
    profile,
    wallet,
    catalog: [
      ...previewHarvestItems.map(createPreviewHarvestCatalogState),
      ...previewSeedItems.map(createPreviewSeedCatalogState),
    ],
    inventory: [],
    equipment: {},
    counters: [],
    economy: [],
  };
}

function createPreviewSeedCatalogState(seed: (typeof previewSeedItems)[number], index: number) {
  const item = {
    itemKey: seed[0],
    itemType: "seed" as const,
    slot: "consumable" as const,
    nameZh: seed[1],
    nameEn: seed[2],
    descriptionZh: seed[3],
    descriptionEn: seed[4],
    rarity: seed[5],
    priceLp: seed[6],
    levelGate: 1,
    stageGate: "",
    milestoneGate: "",
    assetKey: seed[7],
    growthValue: 0,
    enabled: true,
    sortOrder: 700 + index * 10,
  };
  return createPreviewCatalogState(item);
}

function createPreviewHarvestCatalogState(item: (typeof previewHarvestItems)[number], index: number) {
  const catalogItem = {
    itemKey: item[0],
    itemType: "food" as const,
    slot: "consumable" as const,
    nameZh: item[1],
    nameEn: item[2],
    descriptionZh: item[3],
    descriptionEn: item[4],
    rarity: item[5],
    priceLp: item[6],
    levelGate: 1,
    stageGate: "",
    milestoneGate: "",
    assetKey: item[7],
    growthValue: item[8],
    enabled: true,
    sortOrder: 660 + index * 2,
  };
  return createPreviewCatalogState(catalogItem);
}

function createPreviewCatalogState(item: PetStoreCatalogItem) {
  return {
    item,
    owned: false,
    equipped: false,
    quantity: 0,
    growthValue: item.growthValue,
    dailyFreeLimit: 0,
    dailyFreeClaimed: 0,
    dailyFreeRemaining: 0,
    purchasable: true,
    lockedReasonZh: "",
    lockedReasonEn: "",
    status: "available" as const,
  };
}

function upsertPreviewInventory(state: PetStoreState, item: PetStoreCatalogItem, quantity: number, source: string, now: string): PetStoreState {
  const existing = state.inventory.find((inventoryItem) => inventoryItem.itemKey === item.itemKey);
  const nextQuantity = (existing?.quantity ?? 0) + quantity;
  const inventory = existing
    ? state.inventory.map((inventoryItem) =>
        inventoryItem.itemKey === item.itemKey
          ? { ...inventoryItem, quantity: nextQuantity, source, updatedAt: now }
          : inventoryItem,
      )
    : [
        ...state.inventory,
        {
          id: `preview-inventory-${item.itemKey}`,
          petId: state.profile.id,
          itemKey: item.itemKey,
          itemType: item.itemType,
          slot: item.slot,
          quantity,
          equipped: false,
          source,
          purchasedAt: now,
          updatedAt: now,
        },
      ];
  return {
    ...state,
    inventory,
    catalog: state.catalog.map((entry) =>
      entry.item.itemKey === item.itemKey
        ? { ...entry, owned: true, quantity: nextQuantity, status: "available" }
        : entry,
    ),
  };
}

function createPreviewProfile(now: string) {
  return {
    id: "preview-pet",
    name: "Libby",
    level: 8,
    experience: 680,
    stage: "grow_together" as const,
    currentMood: "cheerful" as const,
    createdAt: now,
    updatedAt: now,
    levelSnapshot: {
      level: 8,
      currentLevelExp: 80,
      nextLevelRequired: 120,
      totalExperience: 680,
      currentStage: "grow_together" as const,
      currentStageLabelZh: "一起成长",
      currentStageLabelEn: "Growing Together",
      nextStage: "tacit_bond" as const,
      nextStageLevel: 30,
      progressRatio: 0.66,
      isMaxLevel: false,
    },
  };
}

function createPreviewSettings() {
  return {
    petId: "preview-pet",
    desktopEnabled: false,
    alwaysOnTop: true,
    muted: false,
    focusModeEnabled: false,
    proactiveLevel: 1,
    updatedAt: new Date().toISOString(),
  };
}

function createPreviewBlindBoxState(): PetBlindBoxState {
  return {
    drawDate: new Date().toISOString().slice(0, 10),
    dailyLimit: 0,
    usedToday: 0,
    remainingToday: 0,
    pool: [],
    history: [],
    storeState: previewStoreState,
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
