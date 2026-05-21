import "./PetBlindBoxView.css";
import { useEffect, useMemo, useState } from "react";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { PetBlindBoxThreeStage } from "@/features/pet-blind-box/views/PetBlindBoxThreeStage";
import {
  itemAccent,
  itemDescription,
  itemName,
  itemTypeLabel,
  rarityLabel,
  rarityToneClass,
  shopImageUrl,
} from "@/features/pet-store/services/petStorePresentation";
import { createLocalPetService } from "@/shared/services/tauri/pet";
import type {
  LocaleCode,
  PetBlindBoxDrawEntry,
  PetBlindBoxDrawResult,
  PetBlindBoxPoolItem,
  PetBlindBoxState,
  PetStoreCatalogItem,
  PetStoreState,
} from "@/shared/types/meeting";

const petService = createLocalPetService();

export default function PetBlindBoxView() {
  const meetingStore = useMeetingStore();
  const locale = meetingStore.settings.locale;
  const isEnglish = locale === "en-US";
  const [blindBoxState, setBlindBoxState] = useState<PetBlindBoxState | null>(null);
  const [lastResult, setLastResult] = useState<PetBlindBoxDrawResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [drawing, setDrawing] = useState(false);
  const [stagePhase, setStagePhase] = useState<"idle" | "drawing" | "result">("idle");
  const [resultBurstKey, setResultBurstKey] = useState(0);
  const [errorMessage, setErrorMessage] = useState("");
  const storeState = blindBoxState?.storeState ?? null;
  const remainingToday = blindBoxState?.remainingToday ?? 0;
  const dailyLimit = blindBoxState?.dailyLimit ?? 10;
  const usedToday = blindBoxState?.usedToday ?? 0;
  const fullPrizePool = useMemo(() => [...(blindBoxState?.pool ?? [])].sort((left, right) => left.item.sortOrder - right.item.sortOrder), [blindBoxState]);
  const todayDraws = useMemo(
    () => (blindBoxState?.history ?? [])
      .filter((entry) => entry.drawDate === blindBoxState?.drawDate)
      .sort((left, right) => Date.parse(left.createdAt) - Date.parse(right.createdAt))
      .slice(0, dailyLimit),
    [blindBoxState, dailyLimit],
  );
  const hasEmptyPrize = lastResult?.prize.itemType === "none";

  useEffect(() => {
    void loadBlindBoxState();
  }, []);

  async function loadBlindBoxState() {
    setLoading(true);
    setErrorMessage("");
    try {
      setBlindBoxState(await petService.getBlindBoxState());
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setLoading(false);
    }
  }

  async function drawBlindBox() {
    setDrawing(true);
    setStagePhase("drawing");
    setErrorMessage("");
    try {
      await delay(5000);
      const result = await petService.drawBlindBox();
      setLastResult(result);
      setBlindBoxState(result.state);
      setResultBurstKey((value) => value + 1);
      setStagePhase("result");
    } catch (error) {
      setStagePhase("idle");
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setDrawing(false);
    }
  }

  function catalogName(item: PetStoreCatalogItem) {
    return locale === "en-US" ? item.nameEn : item.nameZh;
  }

  function catalogDescription(item: PetStoreCatalogItem) {
    return locale === "en-US" ? item.descriptionEn : item.descriptionZh;
  }

  function displayName(itemKey: string, fallbackType: string) {
    const item = findCatalogItem(storeState, itemKey);
    if (!item) {
      return itemKey || fallbackType;
    }
    return catalogName(item);
  }

  function displayImage(itemKey: string) {
    const item = findCatalogItem(storeState, itemKey);
    return shopImageUrl(item?.assetKey ?? "gift_box");
  }

  function historyMeta(itemKey: string, fallbackType: string) {
    const item = findCatalogItem(storeState, itemKey);
    return item
      ? `${itemTypeLabel(item.itemType, locale)} · ${rarityLabel(item.rarity, locale)}`
      : itemTypeLabel(fallbackType, locale);
  }

  function resultTitle() {
    if (!lastResult) {
      return isEnglish ? "No box opened yet" : "还没有开启盲盒";
    }
    return catalogName(lastResult.prize);
  }

  function resultDescription() {
    if (!lastResult) {
      return isEnglish
        ? "Each opening grants one result from the full pet store pool."
        : "每次开启都会从完整宠物商店奖池中得到一个结果。";
    }
    if (lastResult.prize.itemType === "none") {
      return isEnglish
        ? "No item this time. The attempt was recorded and the next box may still surprise you."
        : "这次没有获得物品，次数已记录，下一次仍可能开出惊喜。";
    }
    if (lastResult.duplicate) {
      return isEnglish
        ? `Already owned. Converted to ${lastResult.draw.duplicateCompensationLp} LP.`
        : `已拥有，自动转为 ${lastResult.draw.duplicateCompensationLp} LP 补偿。`;
    }
    return catalogDescription(lastResult.prize);
  }

  return (
    <section className="view-stack native-page pet-blind-box-page">
      <article className="surface native-page-hero pet-blind-box-hero">
        <div className="section-heading">
          <div>
            <h3>{isEnglish ? "Daily Blind Box" : "每日盲盒"}</h3>
            <p className="section-copy">
              {isEnglish
                ? "Open up to ten companion boxes each day. Rewards come from the pet store, excluding pets."
                : "每天最多开启 10 次伙伴盲盒，奖励来自宠物商店内容，但不包含宠物本体。"}
            </p>
          </div>
        </div>
        <div className="pet-blind-box-counter">
          <span>{isEnglish ? "Remaining Today" : "今日剩余"}</span>
          <strong>{remainingToday}</strong>
          <small>{isEnglish ? `${usedToday}/${dailyLimit} used` : `已用 ${usedToday}/${dailyLimit}`}</small>
        </div>
      </article>

      {errorMessage && <div className="pet-blind-box-message error">{errorMessage}</div>}

      <div className="native-split-layout">
        <article className="surface native-list-panel pet-blind-box-draw-panel">
          <div className={`pet-blind-box-stage phase-${stagePhase}`}>
            <PetBlindBoxThreeStage
              pool={fullPrizePool}
              phase={stagePhase}
              resultPrize={lastResult?.prize ?? null}
            />

            <div className="pet-blind-box-start">
              <button
                className="primary-button"
                type="button"
                disabled={loading || drawing || remainingToday <= 0}
                onClick={drawBlindBox}
              >
                {drawing
                  ? (isEnglish ? "Drawing..." : "抽奖中...")
                  : remainingToday <= 0
                    ? (isEnglish ? "No Attempts Left" : "今日次数已用完")
                    : (isEnglish ? "Start Draw" : "开始抽奖")}
              </button>
            </div>
          </div>

          {lastResult && (
            <div key={resultBurstKey} className={`pet-blind-box-result has-result ${hasEmptyPrize ? "empty-result" : ""}`}>
              <div className="pet-blind-box-result-flash" aria-hidden="true" />
              <span>{isEnglish ? "Latest Reward" : "最新奖励"}</span>
              <h3>{resultTitle()}</h3>
              <p>{resultDescription()}</p>
            </div>
          )}

          <section className="pet-blind-box-preview">
            <section className="pet-blind-box-today">
              <div className="section-heading">
                <h3>{isEnglish ? "Today Results" : "今日抽取结果"}</h3>
                <span>{isEnglish ? `${todayDraws.length}/${dailyLimit}` : `${todayDraws.length}/${dailyLimit}`}</span>
              </div>
              {todayDraws.length ? (
                <div className="pet-blind-box-today-scroll">
                  <div
                    className="pet-blind-box-today-track"
                    style={{ "--today-scroll-duration": `${Math.max(12, todayDraws.length * 4)}s` } as React.CSSProperties}
                  >
                    {todayDraws.map((entry) => (
                      <TodayDrawCard
                        key={`primary-${entry.id}`}
                        entry={entry}
                        locale={locale}
                        storeState={storeState}
                      />
                    ))}
                    {todayDraws.map((entry) => (
                      <TodayDrawCard
                        key={`loop-${entry.id}`}
                        entry={entry}
                        locale={locale}
                        storeState={storeState}
                        ariaHidden
                      />
                    ))}
                  </div>
                </div>
              ) : (
                <div className="pet-blind-box-today-empty">{isEnglish ? "No opens today." : "今日还没有抽取记录。"}</div>
              )}
            </section>

            <div className="section-heading">
              <h3>{isEnglish ? "Full Prize Pool" : "完整奖池"}</h3>
            </div>
            {loading ? (
              <div className="empty-state">{isEnglish ? "Loading blind box..." : "正在加载每日盲盒..."}</div>
            ) : (
              <div className="pet-blind-box-pool-scroll">
                <div className="pet-blind-box-prize-grid">
                  {fullPrizePool.map((poolItem) => (
                  <PrizeCard
                    key={poolItem.item.itemKey}
                    poolItem={poolItem}
                    locale={locale}
                    storeState={storeState}
                  />
                  ))}
                </div>
              </div>
            )}
          </section>
        </article>

        <aside className="pet-blind-box-side-stack">
          <section className="surface native-inspector-panel">
            <div className="section-heading">
              <h3>{isEnglish ? "Rules" : "规则"}</h3>
            </div>
            <div className="native-stat-list">
              <div>
                <span>{isEnglish ? "Daily Attempts" : "每日次数"}</span>
                <strong>{dailyLimit}</strong>
              </div>
              <div>
                <span>{isEnglish ? "Pool Items" : "奖池数量"}</span>
                <strong>{blindBoxState?.pool.length ?? 0}</strong>
              </div>
              <div>
                <span>{isEnglish ? "Excluded" : "不包含"}</span>
                <strong>{isEnglish ? "Pets" : "宠物"}</strong>
              </div>
            </div>
            <div className="native-inspector-note">
              <span>{isEnglish ? "Duplicate Rule" : "重复规则"}</span>
              <strong>{isEnglish ? "Consumables stack, owned collectibles convert to LP." : "消耗品叠加，已拥有收藏品转 LP。"}</strong>
            </div>
          </section>

          <section className="surface native-inspector-panel">
            <div className="section-heading">
              <h3>{isEnglish ? "Recent Opens" : "最近开启"}</h3>
            </div>
            <div className="pet-blind-box-history">
              {(blindBoxState?.history ?? []).length ? (
                blindBoxState?.history.map((entry) => (
                  <article key={entry.id}>
                    <img src={displayImage(entry.itemKey)} alt="" />
                    <div>
                      <strong>{displayName(entry.itemKey, entry.itemType)}</strong>
                      <span>{historyMeta(entry.itemKey, entry.itemType)}</span>
                    </div>
                    <small>
                      {entry.itemType === "none"
                        ? (isEnglish ? "Empty" : "空")
                        : entry.duplicateCompensationLp > 0
                          ? `+${entry.duplicateCompensationLp} LP`
                          : `×${entry.quantity}`}
                    </small>
                  </article>
                ))
              ) : (
                <div className="empty-state">{isEnglish ? "No history yet." : "暂无开启记录。"}</div>
              )}
            </div>
          </section>
        </aside>
      </div>
    </section>
  );
}

function TodayDrawCard({
  entry,
  locale,
  storeState,
  ariaHidden = false,
}: {
  entry: PetBlindBoxDrawEntry;
  locale: LocaleCode;
  storeState: PetStoreState | null;
  ariaHidden?: boolean;
}) {
  const item = findCatalogItem(storeState, entry.itemKey);
  const isEmpty = entry.itemType === "none";
  const title = item ? catalogName(item, locale) : entry.itemKey;
  const meta = isEmpty
    ? itemTypeLabel(entry.itemType, locale)
    : entry.duplicateCompensationLp > 0
      ? `+${entry.duplicateCompensationLp} LP`
      : `×${entry.quantity}`;

  return (
    <article className={`pet-blind-box-today-card ${isEmpty ? "empty-prize" : ""}`} aria-hidden={ariaHidden}>
      <img src={shopImageUrl(item?.assetKey ?? "gift_box")} alt={title} />
      <strong>{title}</strong>
      <span>{meta}</span>
    </article>
  );
}

function PrizeCard({
  poolItem,
  locale,
  storeState,
}: {
  poolItem: PetBlindBoxPoolItem;
  locale: LocaleCode;
  storeState: PetStoreState | null;
}) {
  const stateItem = storeState?.catalog.find((item) => item.item.itemKey === poolItem.item.itemKey);
  const displayItem = stateItem ?? {
    item: poolItem.item,
    owned: poolItem.owned,
    equipped: false,
    quantity: 0,
    growthValue: 0,
    purchasable: false,
    lockedReasonZh: "",
    lockedReasonEn: "",
    status: "available" as const,
  };

  return (
    <article className={`pet-blind-box-prize-card ${poolItem.item.itemType === "none" ? "empty-prize" : ""}`} style={{ "--item-accent": itemAccent(displayItem) } as React.CSSProperties}>
      <img src={shopImageUrl(poolItem.item.assetKey)} alt={catalogName(poolItem.item, locale)} />
      <div>
        <strong>{itemName(storeState, displayItem, locale)}</strong>
        <span className={rarityToneClass(poolItem.item.rarity)}>
          {poolItem.item.itemType === "none"
            ? itemTypeLabel(poolItem.item.itemType, locale)
            : `${itemTypeLabel(poolItem.item.itemType, locale)} · ${rarityLabel(poolItem.item.rarity, locale)}`}
        </span>
        <p>{itemDescription(storeState, displayItem, locale)}</p>
      </div>
    </article>
  );
}

function findCatalogItem(storeState: PetStoreState | null, itemKey: string) {
  return storeState?.catalog.find((item) => item.item.itemKey === itemKey)?.item ?? null;
}

function catalogName(item: PetStoreCatalogItem, locale: LocaleCode) {
  return locale === "en-US" ? item.nameEn : item.nameZh;
}

function delay(duration: number) {
  return new Promise<void>((resolve) => {
    window.setTimeout(resolve, duration);
  });
}
