import "./PetStoreItemDetailView.css";
import { useEffect, useState } from "react";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import {
  findCatalogItem,
  findInventoryItem,
  growthValueLabel,
  itemAccent,
  itemAssetKey,
  itemDescription,
  itemName,
  itemTypeLabel,
  lockReason,
  priceLabel,
  rarityLabel,
  rarityToneClass,
  shopImageUrl,
} from "@/features/pet-store/services/petStorePresentation";
import { createLocalPetService } from "@/shared/services/tauri/pet";
import type { PetStoreCatalogItemState, PetStoreState } from "@/shared/types/meeting";

const petService = createLocalPetService();

export default function PetStoreItemDetailView() {
  const meetingStore = useMeetingStore();
  const locale = meetingStore.settings.locale;
  const isEnglish = locale === "en-US";
  const [storeState, setStoreState] = useState<PetStoreState | null>(null);
  const [loading, setLoading] = useState(true);
  const [errorMessage, setErrorMessage] = useState("");
  const itemKey = new URLSearchParams(window.location.search).get("itemKey")?.trim() ?? "";
  const catalogItem = findCatalogItem(storeState, itemKey);
  const inventoryItem = findInventoryItem(storeState, itemKey);
  const displayItem = catalogItem ?? inventoryItem ?? null;
  const catalog = catalogItem?.item;
  const ownedQuantity = catalogItem?.quantity ?? inventoryItem?.quantity ?? 0;
  const statusLabel = catalogItem ? catalogStatusLabel(catalogItem) : inventoryStatusLabel();
  const imageUrl = displayItem ? shopImageUrl(itemAssetKey(storeState, displayItem)) : "";
  const accent = displayItem ? itemAccent(displayItem) : "#8f96a3";

  useEffect(() => {
    void loadStoreState();
  }, []);

  async function loadStoreState() {
    setLoading(true);
    setErrorMessage("");
    try {
      setStoreState(await petService.getStoreState());
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setLoading(false);
    }
  }

  function catalogStatusLabel(item: PetStoreCatalogItemState) {
    if (item.equipped) {
      return isEnglish ? "Equipped" : "已装备";
    }
    if (item.owned) {
      return isEnglish ? "Owned" : "已拥有";
    }
    if (item.status === "locked") {
      return isEnglish ? "Locked" : "未解锁";
    }
    if (item.status === "achievement") {
      return isEnglish ? "Achievement" : "成就获得";
    }
    if (item.status === "insufficient") {
      return isEnglish ? "Need LP" : "LP 不足";
    }
    if (item.status === "coming_soon") {
      return isEnglish ? "Coming Soon" : "即将开放";
    }
    return isEnglish ? "Available" : "可购买";
  }

  function inventoryStatusLabel() {
    if (!inventoryItem) {
      return isEnglish ? "Not Found" : "未找到";
    }
    if (inventoryItem.equipped) {
      return isEnglish ? "Equipped" : "已装备";
    }
    return inventoryItem.slot === "consumable"
      ? (isEnglish ? `Owned x${inventoryItem.quantity}` : `持有 ×${inventoryItem.quantity}`)
      : (isEnglish ? "Owned" : "已拥有");
  }

  function stageGateLabel(stageGate: string) {
    if (!stageGate) {
      return isEnglish ? "None" : "无";
    }
    return rarityLabel(stageGate, locale);
  }

  function sourceLabel(source: string) {
    const labels: Record<string, { zh: string; en: string }> = {
      default: { zh: "默认解锁", en: "Default" },
      growth: { zh: "成长解锁", en: "Growth" },
      purchase: { zh: "购买获得", en: "Purchased" },
      achievement: { zh: "成就获得", en: "Achievement" },
      daily_blind_box: { zh: "每日盲盒", en: "Daily Blind Box" },
      blind_box_reward: { zh: "盲盒奖励", en: "Blind Box Reward" },
      blind_box_duplicate: { zh: "盲盒重复补偿", en: "Blind Box Duplicate" },
    };
    const label = labels[source];
    if (!label) {
      return isEnglish ? "Unknown" : "未知";
    }
    return isEnglish ? label.en : label.zh;
  }

  if (loading) {
    return <section className="pet-item-detail-page"><div className="pet-item-detail-empty">{isEnglish ? "Loading item details..." : "正在加载商品详情..."}</div></section>;
  }

  if (errorMessage || !displayItem || !catalog) {
    return (
      <section className="pet-item-detail-page">
        <div className="pet-item-detail-empty">
          {errorMessage || (isEnglish ? "Item details are unavailable." : "无法找到该商品详情。")}
        </div>
      </section>
    );
  }

  const detailRows = [
    { label: isEnglish ? "Type" : "分类", value: itemTypeLabel(catalog.itemType, locale) },
    { label: isEnglish ? "Rarity" : "羁绊阶梯", value: rarityLabel(catalog.rarity, locale) },
    { label: isEnglish ? "Status" : "状态", value: statusLabel },
    { label: isEnglish ? "Price" : "价格", value: priceLabel(catalog.priceLp, locale) },
    { label: isEnglish ? "Owned" : "持有", value: String(ownedQuantity) },
    { label: isEnglish ? "Required level" : "等级要求", value: `Lv.${catalog.levelGate}` },
    { label: isEnglish ? "Stage gate" : "阶段要求", value: stageGateLabel(catalog.stageGate) },
    { label: isEnglish ? "Milestone" : "里程碑", value: catalog.milestoneGate || (isEnglish ? "None" : "无") },
  ];

  if (inventoryItem) {
    detailRows.push({ label: isEnglish ? "Source" : "来源", value: sourceLabel(inventoryItem.source) });
  }

  if (catalogItem && catalogItem.growthValue > 0) {
    detailRows.push({ label: isEnglish ? "Growth" : "成长值", value: growthValueLabel(catalogItem.growthValue, locale) });
  }

  return (
    <section className="pet-item-detail-page" style={{ "--item-accent": accent } as React.CSSProperties}>
      <div className="pet-item-detail-layout">
        <aside className="pet-item-visual-column">
          <div className="pet-item-flip-card" tabIndex={0} aria-label={isEnglish ? "3D item preview" : "3D 商品预览"}>
            <div className="pet-item-flip-inner">
              <div className="pet-item-flip-face pet-item-flip-front">
                <img src={imageUrl} alt={itemName(storeState, displayItem, locale)} />
              </div>
              <div className="pet-item-flip-face pet-item-flip-back">
                <span>{itemTypeLabel(catalog.itemType, locale)}</span>
                <strong>{rarityLabel(catalog.rarity, locale)}</strong>
                <small>{priceLabel(catalog.priceLp, locale)}</small>
              </div>
            </div>
          </div>
          <p className="pet-item-flip-hint">{isEnglish ? "Hover or focus to flip the item card." : "悬停或聚焦图片可查看 3D 翻转背面。"}</p>
        </aside>

        <main className="pet-item-content-column">
          <div className="pet-item-kicker">
            <span>{itemTypeLabel(catalog.itemType, locale)}</span>
            <span className={rarityToneClass(catalog.rarity)}>{rarityLabel(catalog.rarity, locale)}</span>
          </div>
          <h1>{itemName(storeState, displayItem, locale)}</h1>
          <p className="pet-item-description">{itemDescription(storeState, displayItem, locale)}</p>

          {catalogItem && lockReason(catalogItem, locale) && (
            <div className="pet-item-lock-note">{lockReason(catalogItem, locale)}</div>
          )}

          <div className="pet-item-detail-grid">
            {detailRows.map((row) => (
              <div key={row.label}>
                <span>{row.label}</span>
                <strong>{row.value}</strong>
              </div>
            ))}
          </div>
        </main>
      </div>
    </section>
  );
}
