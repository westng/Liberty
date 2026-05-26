import "./PetStoreView.css";
import { ask } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import {
  findCatalogItem as findCatalogItemInState,
  growthValueLabel as formatGrowthValueLabel,
  itemAccent as resolveItemAccent,
  itemAssetKey as resolveItemAssetKey,
  itemDescription as resolveItemDescription,
  itemName as resolveItemName,
  itemTypeLabel as formatItemTypeLabel,
  lockReason as resolveLockReason,
  petBondTiers,
  priceLabel as formatPriceLabel,
  rarityLabel as formatRarityLabel,
  rarityToneClass,
  shopImageUrl as resolveShopImageUrl,
} from "@/features/pet-store/services/petStorePresentation";
import { createLocalPetService } from "@/shared/services/tauri/pet";
import { openPetStoreItemWindow } from "@/shared/services/ui/windows";
import type {
  PetInventoryItem,
  PetStoreCatalogItemState,
  PetStoreState,
} from "@/shared/types/meeting";

type StoreSection = "store" | "inventory";
type StoreCategory = "all" | "pet" | "cosmetic" | "theme" | "tool" | "food" | "badge";

const petService = createLocalPetService();

export default function PetStoreView() {
  const meetingStore = useMeetingStore();
  const [activeSection, setActiveSection] = useState<StoreSection>("store");
  const [activeCategory, setActiveCategory] = useState<StoreCategory>("all");
  const [storeState, setStoreState] = useState<PetStoreState | null>(null);
  const [loading, setLoading] = useState(false);
  const [actionKey, setActionKey] = useState("");
  const [errorMessage, setErrorMessage] = useState("");
  const [toastMessage, setToastMessage] = useState("");
  const [purchaseDraft, setPurchaseDraft] = useState<PetStoreCatalogItemState | null>(null);
  const [purchaseQuantity, setPurchaseQuantity] = useState("1");
  const [useDraft, setUseDraft] = useState<PetInventoryItem | null>(null);
  const [useQuantity, setUseQuantity] = useState("1");
  const locale = meetingStore.settings.locale;
  const isEnglish = locale === "en-US";
  const catalogItems = storeState?.catalog ?? [];
  const inventoryItems = (storeState?.inventory ?? []).filter((item) => item.quantity > 0);
  const wallet = storeState?.wallet;
  const profile = storeState?.profile;
  const profileLevel = profile?.levelSnapshot?.level ?? profile?.level ?? 1;
  const counters = storeState?.counters ?? [];
  const equipment = storeState?.equipment;
  const visibleCatalogItems = activeCategory === "all"
    ? catalogItems
    : catalogItems.filter((item) => item.item.itemType === activeCategory);
  const visibleInventoryItems = activeCategory === "all"
    ? inventoryItems
    : inventoryItems.filter((item) => item.itemType === activeCategory);
  const purchasableCount = catalogItems.filter((item) => item.purchasable).length;
  const lockedCount = catalogItems.filter((item) => item.status === "locked").length;
  const ownedCount = inventoryItems.length;
  const nextUnlockItem = catalogItems.find((item) => item.status === "locked");
  const categories: { key: StoreCategory; label: string }[] = [
    { key: "all", label: isEnglish ? "All" : "全部" },
    { key: "pet", label: isEnglish ? "Pets" : "宠物" },
    { key: "cosmetic", label: isEnglish ? "Cosmetics" : "装扮" },
    { key: "theme", label: isEnglish ? "Scenes" : "场景" },
    { key: "tool", label: isEnglish ? "Tools" : "道具" },
    { key: "food", label: isEnglish ? "Food" : "食物" },
    { key: "badge", label: isEnglish ? "Badges" : "徽章" },
  ];

  useEffect(() => {
    void loadStoreState();
    window.addEventListener("focus", handleWindowFocus);
    return () => window.removeEventListener("focus", handleWindowFocus);
  }, []);

  useEffect(() => {
    if (!toastMessage) {
      return;
    }
    const timer = window.setTimeout(() => setToastMessage(""), 1800);
    return () => window.clearTimeout(timer);
  }, [toastMessage]);

  function handleWindowFocus() {
    void loadStoreState();
  }

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

  function openPurchaseDialog(item: PetStoreCatalogItemState) {
    const maxQuantity = maxPurchaseQuantity(item);
    setPurchaseDraft(item);
    setPurchaseQuantity(String(Math.min(1, maxQuantity)));
    setErrorMessage("");
  }

  function closePurchaseDialog() {
    setPurchaseDraft(null);
    setPurchaseQuantity("1");
  }

  async function purchaseItem(item: PetStoreCatalogItemState, quantity: number) {
    await runStoreAction(item.item.itemKey, async () => {
      setStoreState(await petService.purchaseStoreItem(item.item.itemKey, quantity));
      setToastMessage(itemPurchaseToast(item, quantity));
    });
  }

  async function confirmPurchaseDraft() {
    if (!purchaseDraft) {
      return;
    }

    const quantity = normalizedPurchaseQuantity(purchaseDraft);
    await purchaseItem(purchaseDraft, quantity);
    closePurchaseDialog();
  }

  async function equipItem(itemKey: string) {
    await runStoreAction(itemKey, async () => {
      setStoreState(await petService.equipInventoryItem(itemKey));
      setToastMessage(isEnglish ? "Equipped." : "已装备。");
    });
  }

  async function unequipSlot(slot: string) {
    await runStoreAction(slot, async () => {
      setStoreState(await petService.unequipInventorySlot(slot));
      setToastMessage(isEnglish ? "Unequipped." : "已取消装备。");
    });
  }

  function openUseDialog(item: PetInventoryItem) {
    setUseDraft(item);
    setUseQuantity("1");
    setErrorMessage("");
  }

  function closeUseDialog() {
    setUseDraft(null);
    setUseQuantity("1");
  }

  async function useItem(item: PetInventoryItem, quantity = 1) {
    if (item.itemKey === "gift-box-tool") {
      await openGiftBox(item);
      return;
    }

    const confirmed = await ask(
      isEnglish
        ? `Use ${quantity} x ${itemName(item)}? You currently own ${item.quantity}.`
        : `确认使用「${itemName(item)}」×${quantity} 吗？当前持有 ${item.quantity} 个。`,
      {
        title: isEnglish ? "Confirm Use" : "确认使用",
        kind: "info",
        okLabel: isEnglish ? "Use" : "使用",
        cancelLabel: isEnglish ? "Cancel" : "取消",
      },
    );

    if (!confirmed) {
      return;
    }

    await runStoreAction(item.itemKey, async () => {
      setStoreState(await petService.useInventoryItem(item.itemKey, quantity));
      setToastMessage(itemUseToastMessage(item, quantity));
    });
  }

  async function confirmUseDraft() {
    if (!useDraft) {
      return;
    }
    if (useDraft.itemKey === "gift-box-tool") {
      await openGiftBox(useDraft);
      closeUseDialog();
      return;
    }
    const quantity = normalizedUseQuantity(useDraft);
    await runStoreAction(useDraft.itemKey, async () => {
      setStoreState(await petService.useInventoryItem(useDraft.itemKey, quantity));
      setToastMessage(itemUseToastMessage(useDraft, quantity));
    });
    closeUseDialog();
  }

  async function openGiftBox(item: PetInventoryItem) {
    await runStoreAction(item.itemKey, async () => {
      const result = await petService.openGiftBox();
      setStoreState(result.state);
      const prizeName = locale === "en-US" ? result.prize.nameEn : result.prize.nameZh;
      setToastMessage(
        result.duplicate
          ? isEnglish
            ? `${prizeName} already owned, converted to ${result.duplicateCompensationLp} LP.`
            : `开出重复「${prizeName}」，已转为 ${result.duplicateCompensationLp} LP。`
          : isEnglish
            ? `Gift box opened: ${prizeName}.`
            : `惊喜礼盒开出「${prizeName}」。`,
      );
    });
  }

  async function runStoreAction(key: string, action: () => Promise<void>) {
    setActionKey(key);
    setErrorMessage("");
    try {
      await action();
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setActionKey("");
    }
  }

  function findCatalogItem(itemKey: string) {
    return findCatalogItemInState(storeState, itemKey);
  }

  function itemName(item: PetStoreCatalogItemState | PetInventoryItem) {
    return resolveItemName(storeState, item, locale);
  }

  function itemDescription(item: PetStoreCatalogItemState | PetInventoryItem) {
    return resolveItemDescription(storeState, item, locale);
  }

  function itemUseToastMessage(item: PetInventoryItem, quantity = 1) {
    const catalogState = findCatalogItem(item.itemKey);
    const name = itemName(item);

    if (item.itemType === "food") {
      const growthValue = (catalogState?.growthValue ?? 0) * quantity;
      return isEnglish
        ? `${name} x${quantity} shared a small warm moment. Growth +${growthValue}.`
        : `「${name}」×${quantity} 已好好收下，成长值 +${growthValue}。这份照顾被它记住了。`;
    }

    const lowerKey = item.itemKey.toLowerCase();
    if (lowerKey.includes("energy") || lowerKey.includes("vigor")) {
      return isEnglish
        ? `${name} is active. Your companion feels ready to keep going.`
        : `「${name}」已生效，伙伴的活力被轻轻拉满了。`;
    }
    if (lowerKey.includes("charm") || lowerKey.includes("rings") || lowerKey.includes("bell")) {
      return isEnglish
        ? `${name} is active. The bond feels warmer now.`
        : `「${name}」已生效，这份陪伴感更暖了一点。`;
    }
    if (lowerKey.includes("ticket") || lowerKey.includes("coin") || lowerKey.includes("crystal")) {
      return isEnglish
        ? `${name} is ready. A little lucky moment has been saved for later.`
        : `「${name}」已准备好，一个小小幸运时刻被悄悄存下了。`;
    }
    if (lowerKey.includes("stopwatch") || lowerKey.includes("stone")) {
      return isEnglish
        ? `${name} is active. Focus mode feels steadier.`
        : `「${name}」已生效，专注节奏稳住了。`;
    }

    return isEnglish
      ? `${name} is used. Your companion received this little care.`
      : `「${name}」已使用，伙伴收到这份小小心意了。`;
  }

  function giftBoxClaimLabel(item: PetStoreCatalogItemState) {
    if (item.item.itemKey !== "gift-box-tool" || item.dailyFreeLimit <= 0) {
      return "";
    }
    return isEnglish
      ? `${item.dailyFreeRemaining}/${item.dailyFreeLimit} free left`
      : `今日免费剩余 ${item.dailyFreeRemaining}/${item.dailyFreeLimit}`;
  }

  function purchasePriceText(item: PetStoreCatalogItemState) {
    return giftBoxClaimLabel(item) || priceLabel(item.item.priceLp);
  }

  function itemPurchaseToast(item: PetStoreCatalogItemState, quantity: number) {
    if (item.item.itemKey === "gift-box-tool") {
      return isEnglish
        ? `Gift Box x${quantity} claimed for free today.`
        : `今日免费「${itemName(item)}」×${quantity} 已加入个人仓库。`;
    }
    return isEnglish
      ? `${itemName(item)} x${quantity} added to your inventory.`
      : `「${itemName(item)}」×${quantity} 已加入你的个人仓库。`;
  }

  function itemAssetKey(item: PetStoreCatalogItemState | PetInventoryItem) {
    return resolveItemAssetKey(storeState, item);
  }

  function itemAccent(item: PetStoreCatalogItemState | PetInventoryItem) {
    return resolveItemAccent(item);
  }

  function inventorySourceLabel(source: string) {
    const labels: Record<string, { zh: string; en: string }> = {
      default: { zh: "默认解锁", en: "Default" },
      growth: { zh: "成长解锁", en: "Growth" },
      purchase: { zh: "购买获得", en: "Purchased" },
      achievement: { zh: "成就获得", en: "Achievement" },
      daily_blind_box: { zh: "每日盲盒", en: "Daily Blind Box" },
      daily_check_in: { zh: "每日签到", en: "Daily Check-in" },
      daily_free_store: { zh: "每日免费领取", en: "Daily Free Claim" },
      gift_box_reward: { zh: "惊喜礼盒", en: "Gift Box" },
      blind_box_reward: { zh: "盲盒奖励", en: "Blind Box Reward" },
      blind_box_duplicate: { zh: "盲盒重复补偿", en: "Blind Box Duplicate" },
    };
    const label = labels[source];
    if (!label) {
      return isEnglish ? "Unknown" : "未知";
    }
    return isEnglish ? label.en : label.zh;
  }

  function itemTypeLabel(itemType: string) {
    return formatItemTypeLabel(itemType, locale);
  }

  function rarityLabel(rarity: string) {
    return formatRarityLabel(rarity, locale);
  }

  function milestoneLabel(counterKey: string) {
    const labels: Record<string, { zh: string; en: string }> = {
      tasks_created: { zh: "创建任务", en: "Tasks created" },
      transcriptions_completed: { zh: "完成转写", en: "Transcriptions completed" },
      summaries_completed: { zh: "完成 AI 总结", en: "AI summaries completed" },
      exports_completed: { zh: "导出结果", en: "Exports completed" },
      active_days: { zh: "活跃天数", en: "Active days" },
      dark_theme_days: { zh: "深色主题使用天数", en: "Dark-theme days" },
    };
    const label = labels[counterKey];
    if (!label) {
      return isEnglish ? "Unknown milestone" : "未知里程碑";
    }
    return isEnglish ? label.en : label.zh;
  }

  function storeActionLabel(item: PetStoreCatalogItemState) {
    if (item.status === "coming_soon") {
      return isEnglish ? "Coming Soon" : "即将开放";
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
    if (item.status === "daily_limit") {
      return isEnglish ? "Claimed" : "今日已领完";
    }
    if (item.item.itemKey === "gift-box-tool" && item.dailyFreeRemaining > 0) {
      return isEnglish ? "Free Claim" : "免费领取";
    }
    if (item.item.slot === "consumable" && item.purchasable) {
      return isEnglish ? "Purchase" : "购买";
    }
    if (item.equipped) {
      return isEnglish ? "Equipped" : "已装备";
    }
    if (item.owned) {
      return isEnglish ? "Owned" : "已拥有";
    }
    return isEnglish ? "Purchase" : "购买";
  }

  function storeActionDisabled(item: PetStoreCatalogItemState) {
    return actionKey === item.item.itemKey || !item.purchasable;
  }

  async function handleStoreAction(item: PetStoreCatalogItemState) {
    if (item.purchasable) {
      openPurchaseDialog(item);
    }
  }

  function inventoryActionLabel(item: PetInventoryItem) {
    if (item.slot === "consumable") {
      return isEnglish ? "Use" : "使用";
    }
    return item.equipped ? (isEnglish ? "Equipped" : "已装备") : (isEnglish ? "Equip" : "装备");
  }

  function inventoryActionDisabled(item: PetInventoryItem) {
    return actionKey === item.itemKey || item.equipped;
  }

  async function handleInventoryAction(item: PetInventoryItem) {
    if (item.slot === "consumable") {
      if (item.itemType === "food" && item.quantity > 1) {
        openUseDialog(item);
        return;
      }
      await useItem(item);
      return;
    }
    if (!item.equipped) {
      await equipItem(item.itemKey);
    }
  }

  async function openItemDetail(item: PetStoreCatalogItemState | PetInventoryItem) {
    const itemKey = "item" in item ? item.item.itemKey : item.itemKey;
    await openPetStoreItemWindow(itemKey, itemName(item));
  }

  function shopImageUrl(imageKey: string) {
    return resolveShopImageUrl(imageKey);
  }

  function lockReason(item: PetStoreCatalogItemState) {
    return resolveLockReason(item, locale);
  }

  function priceLabel(priceLp: number) {
    return formatPriceLabel(priceLp, locale);
  }

  function growthValueLabel(value: number) {
    return formatGrowthValueLabel(value, locale);
  }

  function maxPurchaseQuantity(item: PetStoreCatalogItemState) {
    if (item.item.slot !== "consumable") {
      return 1;
    }
    if (item.item.itemKey === "gift-box-tool") {
      return Math.max(1, item.dailyFreeRemaining);
    }
    if (!wallet || item.item.priceLp <= 0) {
      return 99;
    }
    return Math.max(1, Math.min(99, Math.floor(wallet.balance / item.item.priceLp)));
  }

  function normalizedPurchaseQuantity(item: PetStoreCatalogItemState) {
    const parsed = Number.parseInt(purchaseQuantity, 10);
    if (!Number.isFinite(parsed)) {
      return 1;
    }
    return Math.max(1, Math.min(maxPurchaseQuantity(item), parsed));
  }

  function updatePurchaseQuantity(value: string) {
    if (!purchaseDraft) {
      return;
    }
    const digits = value.replace(/\D/g, "");
    if (!digits) {
      setPurchaseQuantity("");
      return;
    }
    setPurchaseQuantity(String(Math.min(maxPurchaseQuantity(purchaseDraft), Number.parseInt(digits, 10))));
  }

  function normalizedUseQuantity(item: PetInventoryItem) {
    const parsed = Number.parseInt(useQuantity, 10);
    if (!Number.isFinite(parsed)) {
      return 1;
    }
    return Math.max(1, Math.min(item.quantity, parsed));
  }

  function updateUseQuantity(value: string) {
    if (!useDraft) {
      return;
    }
    const digits = value.replace(/\D/g, "");
    if (!digits) {
      setUseQuantity("");
      return;
    }
    setUseQuantity(String(Math.min(useDraft.quantity, Number.parseInt(digits, 10))));
  }

  const purchaseQuantityValue = purchaseDraft ? normalizedPurchaseQuantity(purchaseDraft) : 1;
  const purchaseTotalPrice = purchaseDraft ? purchaseDraft.item.priceLp * purchaseQuantityValue : 0;
  const useQuantityValue = useDraft ? normalizedUseQuantity(useDraft) : 1;
  const useGrowthValue = useDraft ? (findCatalogItem(useDraft.itemKey)?.growthValue ?? 0) * useQuantityValue : 0;

  return (
    <section className="view-stack native-page native-split-page pet-store-native-page">
      <div className="pet-store-toast" data-visible={Boolean(toastMessage)} role="status">
        {toastMessage}
      </div>

      {purchaseDraft && (
        <div className="pet-store-modal-backdrop" role="presentation" onClick={closePurchaseDialog}>
          <div className="pet-store-purchase-modal" role="dialog" aria-modal="true" aria-labelledby="pet-store-purchase-title" onClick={(event) => event.stopPropagation()}>
            <div className="pet-store-purchase-heading">
              <h3 id="pet-store-purchase-title">{isEnglish ? "Confirm Purchase" : "确认购买"}</h3>
              <button className="text-button" type="button" onClick={closePurchaseDialog}>
                {isEnglish ? "Cancel" : "取消"}
              </button>
            </div>
            <div className="pet-store-purchase-item">
              <img src={shopImageUrl(itemAssetKey(purchaseDraft))} alt={itemName(purchaseDraft)} />
              <div>
                <strong>{itemName(purchaseDraft)}</strong>
                <span>{purchasePriceText(purchaseDraft)}</span>
              </div>
            </div>
            <label className="pet-store-quantity-field">
              <span>{isEnglish ? "Quantity" : "购买数量"}</span>
              <input
                type="number"
                min={1}
                max={maxPurchaseQuantity(purchaseDraft)}
                step={1}
                value={purchaseQuantity}
                onChange={(event) => updatePurchaseQuantity(event.target.value)}
                onBlur={() => setPurchaseQuantity(String(purchaseQuantityValue))}
                autoFocus
              />
            </label>
            <div className="pet-store-purchase-total">
              <span>{isEnglish ? "Total" : "合计"}</span>
              <strong>{purchaseDraft.item.itemKey === "gift-box-tool" ? (isEnglish ? "Free" : "免费") : `${purchaseTotalPrice} LP`}</strong>
            </div>
            <button className="primary-button" type="button" disabled={actionKey === purchaseDraft.item.itemKey} onClick={confirmPurchaseDraft}>
              {isEnglish ? "Purchase" : "购买"}
            </button>
          </div>
        </div>
      )}

      {useDraft && (
        <div className="pet-store-modal-backdrop" role="presentation" onClick={closeUseDialog}>
          <div className="pet-store-purchase-modal" role="dialog" aria-modal="true" aria-labelledby="pet-store-use-title" onClick={(event) => event.stopPropagation()}>
            <div className="pet-store-purchase-heading">
              <h3 id="pet-store-use-title">{isEnglish ? "Confirm Feeding" : "确认食用"}</h3>
              <button className="text-button" type="button" onClick={closeUseDialog}>
                {isEnglish ? "Cancel" : "取消"}
              </button>
            </div>
            <div className="pet-store-purchase-item">
              <img src={shopImageUrl(itemAssetKey(useDraft))} alt={itemName(useDraft)} />
              <div>
                <strong>{itemName(useDraft)}</strong>
                <span>{isEnglish ? `Owned x${useDraft.quantity}` : `持有 ×${useDraft.quantity}`}</span>
              </div>
            </div>
            <label className="pet-store-quantity-field">
              <span>{isEnglish ? "Quantity to feed" : "食用数量"}</span>
              <input
                type="number"
                min={1}
                max={useDraft.quantity}
                step={1}
                value={useQuantity}
                onChange={(event) => updateUseQuantity(event.target.value)}
                onBlur={() => setUseQuantity(String(useQuantityValue))}
                autoFocus
              />
            </label>
            <div className="pet-store-purchase-total">
              <span>{isEnglish ? "Growth gained" : "获得成长值"}</span>
              <strong>+{useGrowthValue}</strong>
            </div>
            <button className="primary-button" type="button" disabled={actionKey === useDraft.itemKey} onClick={confirmUseDraft}>
              {isEnglish ? "Feed" : "食用"}
            </button>
          </div>
        </div>
      )}

      <article className="surface native-page-hero pet-store-hero">
        <div className="section-heading">
          <div>
            <h3>{isEnglish ? "Pet Store" : "宠物商店"}</h3>
            <p className="section-copy">
              {isEnglish
                ? "Earn Liberty Points through real work, then unlock and equip local companion items."
                : "通过真实工作获得 Liberty 点数，用于解锁、购买和装备桌宠内容。"}
            </p>
          </div>
        </div>

        <div className="pet-store-wallet">
          <span>{isEnglish ? "Liberty Points" : "Liberty 点数"}</span>
          <strong>{wallet?.balance ?? 0} LP</strong>
        </div>
      </article>

      <div className="native-split-layout">
        <article className="surface native-list-panel pet-store-list-panel">
          <div className="section-heading pet-store-list-heading">
            <div className="pet-store-list-title">
              <h3>{isEnglish ? "Pet Shelf" : "宠物货架"}</h3>
              <div className="pet-store-tabs" role="tablist">
                <button type="button" className={activeSection === "store" ? "active" : ""} onClick={() => setActiveSection("store")}>
                  {isEnglish ? "Store" : "商店"}
                </button>
                <button type="button" className={activeSection === "inventory" ? "active" : ""} onClick={() => setActiveSection("inventory")}>
                  {isEnglish ? "Inventory" : "个人仓库"}
                </button>
              </div>
            </div>
            <div className="pet-store-category-tabs" aria-label="Pet store categories">
              {categories.map((category) => (
                <button
                  key={category.key}
                  type="button"
                  className={activeCategory === category.key ? "active" : ""}
                  onClick={() => setActiveCategory(category.key)}
                >
                  {category.label}
                </button>
              ))}
            </div>
          </div>

          {errorMessage && (
            <div className="pet-store-message error">{errorMessage}</div>
          )}

          {loading ? (
            <div className="empty-state">{isEnglish ? "Loading pet store..." : "正在加载宠物商店..."}</div>
          ) : activeSection === "store" ? (
            <div className="pet-store-grid">
              {visibleCatalogItems.map((item) => (
                <article
                  key={item.item.itemKey}
                  className={`pet-store-card status-${item.status}`}
                  tabIndex={0}
                  title={isEnglish ? "Double-click to open details" : "双击打开商品详情"}
                  onDoubleClick={() => openItemDetail(item)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      void openItemDetail(item);
                    }
                  }}
                >
                  <div className="pet-store-card-media" style={{ "--item-accent": itemAccent(item) } as React.CSSProperties}>
                    <img src={shopImageUrl(itemAssetKey(item))} alt={itemName(item)} loading="lazy" />
                  </div>
                  <div className="pet-store-card-copy">
                    <div className="pet-store-card-title">
                      <strong>{itemName(item)}</strong>
                      <span>{itemTypeLabel(item.item.itemType)}</span>
                    </div>
                    <div className={`pet-store-card-meta ${rarityToneClass(item.item.rarity)}`}>
                      <span>{rarityLabel(item.item.rarity)}</span>
                      {item.item.itemType === "food" && item.growthValue > 0 && (
                        <span className="pet-store-growth-chip">{growthValueLabel(item.growthValue)}</span>
                      )}
                      {item.quantity > 0 && <span>×{item.quantity}</span>}
                    </div>
                    <p>{lockReason(item) || itemDescription(item)}</p>
                  </div>
                  <div className="pet-store-card-footer">
                    <span>{purchasePriceText(item)}</span>
                    <button className="text-button" type="button" disabled={storeActionDisabled(item)} onClick={() => handleStoreAction(item)}>
                      {storeActionLabel(item)}
                    </button>
                  </div>
                </article>
              ))}
            </div>
          ) : visibleInventoryItems.length ? (
            <div className="pet-store-grid">
              {visibleInventoryItems.map((item) => (
                <article
                  key={item.id}
                  className={`pet-store-card inventory-card ${item.equipped ? "equipped" : ""}`}
                  tabIndex={0}
                  title={isEnglish ? "Double-click to open details" : "双击打开商品详情"}
                  onDoubleClick={() => openItemDetail(item)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      void openItemDetail(item);
                    }
                  }}
                >
                  <div className="pet-store-card-media" style={{ "--item-accent": itemAccent(item) } as React.CSSProperties}>
                    <img src={shopImageUrl(itemAssetKey(item))} alt={itemName(item)} loading="lazy" />
                  </div>
                  <div className="pet-store-card-copy">
                    <div className="pet-store-card-title">
                      <strong>{itemName(item)}</strong>
                      <span>{itemTypeLabel(item.itemType)}</span>
                    </div>
                    <div className="pet-store-card-meta">
                      <span>{inventorySourceLabel(item.source)}</span>
                      {item.itemType === "food" && (findCatalogItem(item.itemKey)?.growthValue ?? 0) > 0 && (
                        <span className="pet-store-growth-chip">{growthValueLabel(findCatalogItem(item.itemKey)?.growthValue ?? 0)}</span>
                      )}
                      <span>{isEnglish ? `Owned x${item.quantity}` : `持有 ×${item.quantity}`}</span>
                    </div>
                    <p>{itemDescription(item)}</p>
                  </div>
                  <div className="pet-store-card-footer">
                    <span>
                      {item.equipped
                        ? (isEnglish ? "Equipped" : "已装备")
                        : item.slot === "consumable"
                          ? (isEnglish ? `Qty ${item.quantity}` : `数量 ${item.quantity}`)
                          : (isEnglish ? "Owned" : "已拥有")}
                    </span>
                    <button className="text-button" type="button" disabled={inventoryActionDisabled(item)} onClick={() => handleInventoryAction(item)}>
                      {inventoryActionLabel(item)}
                    </button>
                  </div>
                </article>
              ))}
            </div>
          ) : (
            <div className="empty-state">{isEnglish ? "No items in this category yet." : "当前分类暂无内容。"}</div>
          )}
        </article>

        <aside className="pet-store-side-stack">
          <section className="surface native-inspector-panel">
            <div className="section-heading">
              <h3>{isEnglish ? "Store Overview" : "商店概览"}</h3>
            </div>
            <div className="native-stat-list">
              <div>
                <span>{isEnglish ? "Level" : "等级"}</span>
                <strong>{profileLevel}</strong>
              </div>
              <div>
                <span>{isEnglish ? "LP Balance" : "LP 余额"}</span>
                <strong>{wallet?.balance ?? 0}</strong>
              </div>
              <div>
                <span>{isEnglish ? "Inventory" : "个人仓库"}</span>
                <strong>{ownedCount}</strong>
              </div>
              <div>
                <span>{isEnglish ? "Purchasable" : "可购买"}</span>
                <strong>{purchasableCount}</strong>
              </div>
              <div>
                <span>{isEnglish ? "Locked" : "未解锁"}</span>
                <strong>{lockedCount}</strong>
              </div>
            </div>
          </section>

          <section className="surface native-inspector-panel">
            <div className="section-heading">
              <h3>{isEnglish ? "Bond Tiers" : "陪伴阶梯说明"}</h3>
            </div>
            <div className="pet-level-guide">
              <strong>{isEnglish ? "Current pet: Never Apart" : "当前宠物：不离不弃"}</strong>
              <div className="pet-bond-tier-list">
                {petBondTiers.map((tier) => (
                  <span key={tier} className={`${rarityToneClass(tier)} ${tier === "bond_forever" ? "active" : ""}`}>
                    {rarityLabel(tier)}
                  </span>
                ))}
              </div>
              <p>
                {isEnglish
                  ? "Pet rarity describes companionship depth. Libby is the highest tier: Never Apart."
                  : "宠物稀有度描述陪伴羁绊深度。当前 Libby 是最高阶：不离不弃。"}
              </p>
            </div>
          </section>

          <section className="surface native-inspector-panel">
            <div className="pet-equipment-panel">
              <span>{isEnglish ? "Equipped" : "当前装备"}</span>
              <button type="button" disabled>{equipment?.currentPet ? itemName(equipment.currentPet) : "Libby"}</button>
              <button type="button" disabled={!equipment?.accessory} onClick={() => unequipSlot("accessory")}>
                {equipment?.accessory ? itemName(equipment.accessory) : (isEnglish ? "No accessory" : "未装备配饰")}
              </button>
              <button type="button" disabled={!equipment?.scene} onClick={() => unequipSlot("scene")}>
                {equipment?.scene ? itemName(equipment.scene) : (isEnglish ? "No scene" : "未装备场景")}
              </button>
              <button type="button" disabled={!equipment?.badge} onClick={() => unequipSlot("badge")}>
                {equipment?.badge ? itemName(equipment.badge) : (isEnglish ? "No badge" : "未装备徽章")}
              </button>
            </div>

            <div className="native-inspector-note">
              <span>{isEnglish ? "Next Unlock" : "下一个解锁"}</span>
              <strong>{nextUnlockItem ? itemName(nextUnlockItem) : (isEnglish ? "All available" : "暂无锁定目标")}</strong>
              <p>
                {nextUnlockItem
                  ? lockReason(nextUnlockItem)
                  : (isEnglish ? "Keep completing work to earn LP and achievements." : "继续完成任务、转写、总结和导出以获得 LP 与成就。")}
              </p>
            </div>

            <div className="pet-store-counters">
              <span>{isEnglish ? "Milestones" : "里程碑"}</span>
              {counters.map((counter) => (
                <div key={counter.counterKey}>
                  <span>{milestoneLabel(counter.counterKey)}</span>
                  <strong>{counter.counterValue}</strong>
                </div>
              ))}
            </div>
          </section>
        </aside>
      </div>
    </section>
  );
}
