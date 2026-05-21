import type { LocaleCode, PetInventoryItem, PetStoreCatalogItemState, PetStoreState } from "@/shared/types/meeting";

export type PetStoreDisplayItem = PetStoreCatalogItemState | PetInventoryItem;

export const shopImageModules = import.meta.glob("/src/assets/images/shop/**/*.png", {
  eager: true,
  import: "default",
}) as Record<string, string>;

export const shopImageUrlMap = Object.fromEntries(
  Object.entries(shopImageModules).map(([path, url]) => {
    const fileName = path.split("/").pop() ?? "";
    return [fileName.replace(/\.png$/, ""), url];
  }),
) as Record<string, string>;

export const categoryAccents: Record<string, string> = {
  pet: "#8f96a3",
  cosmetic: "#ff7aa8",
  theme: "#61b86b",
  tool: "#f6c04f",
  food: "#f58a4c",
  badge: "#5f7dff",
};

export const petBondTiers = ["first_meet", "familiar", "grow_together", "deep_bond", "forever_partner", "bond_forever"];

export function isCatalogItem(item: PetStoreDisplayItem): item is PetStoreCatalogItemState {
  return "item" in item;
}

export function findCatalogItem(storeState: PetStoreState | null, itemKey: string) {
  return storeState?.catalog.find((item) => item.item.itemKey === itemKey);
}

export function findInventoryItem(storeState: PetStoreState | null, itemKey: string) {
  return storeState?.inventory.find((item) => item.itemKey === itemKey);
}

export function itemCatalog(storeState: PetStoreState | null, item: PetStoreDisplayItem) {
  return isCatalogItem(item) ? item.item : findCatalogItem(storeState, item.itemKey)?.item;
}

export function itemName(storeState: PetStoreState | null, item: PetStoreDisplayItem, locale: LocaleCode) {
  const catalog = itemCatalog(storeState, item);
  if (!catalog) {
    return isCatalogItem(item) ? item.item.itemKey : item.itemKey;
  }
  return locale === "en-US" ? catalog.nameEn : catalog.nameZh;
}

export function itemDescription(storeState: PetStoreState | null, item: PetStoreDisplayItem, locale: LocaleCode) {
  const catalog = itemCatalog(storeState, item);
  if (!catalog) {
    return locale === "en-US" ? "Unlocked local item." : "已解锁的本地物品。";
  }
  return locale === "en-US" ? catalog.descriptionEn : catalog.descriptionZh;
}

export function itemAssetKey(storeState: PetStoreState | null, item: PetStoreDisplayItem) {
  return itemCatalog(storeState, item)?.assetKey ?? "gift_box";
}

export function itemAccent(item: PetStoreDisplayItem) {
  const itemType = isCatalogItem(item) ? item.item.itemType : item.itemType;
  return categoryAccents[itemType] ?? "#8f96a3";
}

export function shopImageUrl(imageKey: string) {
  return shopImageUrlMap[imageKey] ?? shopImageUrlMap.gift_box ?? "";
}

export function itemTypeLabel(itemType: string, locale: LocaleCode) {
  if (locale === "en-US") {
    return itemType === "pet"
      ? "Pet"
      : itemType === "cosmetic"
        ? "Cosmetic"
        : itemType === "theme"
          ? "Scene"
          : itemType === "tool"
            ? "Tool"
            : itemType === "food"
              ? "Food"
              : "Badge";
  }

  return itemType === "pet"
    ? "宠物"
    : itemType === "cosmetic"
      ? "装扮"
      : itemType === "theme"
        ? "场景"
        : itemType === "tool"
          ? "道具"
          : itemType === "food"
            ? "食物"
            : "徽章";
}

export function rarityLabel(rarity: string, locale: LocaleCode) {
  const labels: Record<string, { zh: string; en: string }> = {
    first_meet: { zh: "小小初遇", en: "First Encounter" },
    familiar: { zh: "轻轻熟悉", en: "Getting Familiar" },
    grow_together: { zh: "一起成长", en: "Growing Together" },
    deep_bond: { zh: "深深羁绊", en: "Deep Bond" },
    forever_partner: { zh: "永远伙伴", en: "Forever Partner" },
    bond_forever: { zh: "不离不弃", en: "Never Apart" },
  };
  const label = labels[rarity];
  if (!label) {
    return rarity;
  }
  return locale === "en-US" ? label.en : label.zh;
}

export function rarityToneClass(rarity: string) {
  return `rarity-${rarity.replaceAll("_", "-")}`;
}

export function priceLabel(priceLp: number, locale: LocaleCode) {
  return priceLp > 0 ? `${priceLp} LP` : (locale === "en-US" ? "Free" : "免费");
}

export function growthValueLabel(value: number, locale: LocaleCode) {
  return locale === "en-US" ? `Growth +${value}` : `成长值 +${value}`;
}

export function lockReason(item: PetStoreCatalogItemState, locale: LocaleCode) {
  return locale === "en-US" ? item.lockedReasonEn : item.lockedReasonZh;
}
