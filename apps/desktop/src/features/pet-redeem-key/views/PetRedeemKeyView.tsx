import "./PetRedeemKeyView.css";
import { useEffect, useMemo, useState } from "react";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { findCatalogItem, itemName, shopImageUrl } from "@/features/pet-store/services/petStorePresentation";
import { createLocalPetService } from "@/shared/services/tauri/pet";
import type {
  LocaleCode,
  PetRedeemKeyRedemption,
  PetRedeemKeyResult,
  PetRedeemKeyRewardItem,
  PetRedeemKeyRewards,
  PetStoreState,
} from "@/shared/types/meeting";

const petService = createLocalPetService();

export default function PetRedeemKeyView() {
  const meetingStore = useMeetingStore();
  const locale = meetingStore.settings.locale;
  const isEnglish = locale === "en-US";
  const [storeState, setStoreState] = useState<PetStoreState | null>(null);
  const [redemptions, setRedemptions] = useState<PetRedeemKeyRedemption[]>([]);
  const [redeemKey, setRedeemKey] = useState("");
  const [lastResult, setLastResult] = useState<PetRedeemKeyResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [redeeming, setRedeeming] = useState(false);
  const [message, setMessage] = useState("");
  const [errorMessage, setErrorMessage] = useState("");
  const normalizedKey = useMemo(() => redeemKey.replace(/\s/g, ""), [redeemKey]);
  const recentRedemptions = redemptions.slice(0, 8);

  useEffect(() => {
    void loadRedeemState();
  }, []);

  async function loadRedeemState() {
    setLoading(true);
    setErrorMessage("");
    try {
      const [nextStoreState, nextRedemptions] = await Promise.all([
        petService.getStoreState(),
        petService.listRedeemKeyRedemptions(20),
      ]);
      setStoreState(nextStoreState);
      setRedemptions(nextRedemptions);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setLoading(false);
    }
  }

  async function redeem() {
    if (!normalizedKey) {
      return;
    }
    setRedeeming(true);
    setErrorMessage("");
    setMessage("");
    try {
      const result = await petService.redeemKey(normalizedKey);
      setLastResult(result);
      setStoreState(result.state);
      setRedemptions((values) => mergeRedemptions(result.redemption, values));
      setRedeemKey("");
      setMessage(result.duplicate
        ? (isEnglish ? "This key was already redeemed on this device." : "该 Key 已在本机兑换过。")
        : (isEnglish ? "Redeemed successfully." : "兑换成功。"));
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setRedeeming(false);
    }
  }

  return (
    <section className="view-stack native-page pet-redeem-key-page">
      <article className="surface native-page-hero pet-redeem-key-hero">
        <div className="section-heading">
          <div>
            <h3>{isEnglish ? "Redeem Center" : "兑换中心"}</h3>
            <p className="section-copy">
              {isEnglish ? "LP, growth, and item rewards" : "LP、成长值与道具奖励"}
            </p>
          </div>
        </div>
        <div className="pet-redeem-key-wallet">
          <span>LP</span>
          <strong>{storeState?.wallet.balance ?? 0}</strong>
          <small>{isEnglish ? "Balance" : "当前余额"}</small>
        </div>
      </article>

      {message && <div className="pet-redeem-key-message">{message}</div>}
      {errorMessage && <div className="pet-redeem-key-message error">{errorMessage}</div>}

      <div className="native-split-layout pet-redeem-key-layout">
        <article className="surface native-list-panel pet-redeem-key-panel">
          <div className="native-list-header">
            <div>
              <span>{isEnglish ? "Redeem" : "兑换"}</span>
              <strong>{isEnglish ? "Key" : "Key"}</strong>
            </div>
          </div>

          <div className="pet-redeem-key-form">
            <textarea
              value={redeemKey}
              rows={5}
              spellCheck={false}
              placeholder="LIB1..."
              onChange={(event) => setRedeemKey(event.target.value)}
            />
            <button
              type="button"
              className="primary-button"
              disabled={loading || redeeming || !normalizedKey}
              onClick={redeem}
            >
              {redeeming ? (isEnglish ? "Redeeming..." : "兑换中...") : (isEnglish ? "Redeem" : "立即兑换")}
            </button>
          </div>

          {lastResult && (
            <RewardResult
              result={lastResult}
              locale={locale}
              storeState={storeState}
            />
          )}
        </article>

        <aside className="surface native-list-panel pet-redeem-key-history-panel">
          <div className="native-list-header">
            <div>
              <span>{isEnglish ? "History" : "历史记录"}</span>
              <strong>{recentRedemptions.length}</strong>
            </div>
          </div>

          <div className="pet-redeem-key-history">
            {recentRedemptions.length ? recentRedemptions.map((redemption) => (
              <RedemptionRow
                key={redemption.id}
                redemption={redemption}
                locale={locale}
                storeState={storeState}
              />
            )) : (
              <div className="empty-state">{isEnglish ? "No records yet." : "暂无兑换记录。"}</div>
            )}
          </div>
        </aside>
      </div>
    </section>
  );
}

function RewardResult({
  result,
  locale,
  storeState,
}: {
  result: PetRedeemKeyResult;
  locale: LocaleCode;
  storeState: PetStoreState | null;
}) {
  const isEnglish = locale === "en-US";
  return (
    <div className="pet-redeem-key-result">
      <div>
        <span>{isEnglish ? "Campaign" : "活动"}</span>
        <strong>{result.redemption.campaignId}</strong>
      </div>
      <RewardSummary rewards={result.rewards} locale={locale} storeState={storeState} />
    </div>
  );
}

function RedemptionRow({
  redemption,
  locale,
  storeState,
}: {
  redemption: PetRedeemKeyRedemption;
  locale: LocaleCode;
  storeState: PetStoreState | null;
}) {
  const isEnglish = locale === "en-US";
  const rewards = parseRewards(redemption.rewardJson);
  return (
    <div className="pet-redeem-key-history-row">
      <div>
        <strong>{redemption.campaignId}</strong>
        <span>{formatDateTime(redemption.redeemedAt, locale)}</span>
      </div>
      <small>{redemption.codePrefix}</small>
      <RewardSummary rewards={rewards} locale={locale} storeState={storeState} compact />
      <em>{isEnglish ? "Redeemed" : "已兑换"}</em>
    </div>
  );
}

function RewardSummary({
  rewards,
  locale,
  storeState,
  compact = false,
}: {
  rewards: PetRedeemKeyRewards;
  locale: LocaleCode;
  storeState: PetStoreState | null;
  compact?: boolean;
}) {
  const isEnglish = locale === "en-US";
  const values = [
    rewards.lp > 0 ? `${rewards.lp} LP` : "",
    rewards.growthValue > 0 ? (isEnglish ? `Growth +${rewards.growthValue}` : `成长 +${rewards.growthValue}`) : "",
  ].filter(Boolean);
  return (
    <div className={`pet-redeem-key-rewards ${compact ? "compact" : ""}`}>
      {values.map((value) => <span key={value}>{value}</span>)}
      {rewards.items.map((item) => (
        <RewardItem key={`${item.itemKey}-${item.quantity}`} item={item} locale={locale} storeState={storeState} />
      ))}
    </div>
  );
}

function RewardItem({
  item,
  locale,
  storeState,
}: {
  item: PetRedeemKeyRewardItem;
  locale: LocaleCode;
  storeState: PetStoreState | null;
}) {
  const catalog = findCatalogItem(storeState, item.itemKey);
  const name = catalog ? itemName(storeState, catalog, locale) : item.itemKey;
  return (
    <span className="pet-redeem-key-item" title={name}>
      {catalog && <img src={shopImageUrl(catalog.item.assetKey)} alt="" />}
      <strong>{name}</strong>
      <small>×{item.quantity}</small>
    </span>
  );
}

function parseRewards(value: string): PetRedeemKeyRewards {
  try {
    const parsed = JSON.parse(value) as PetRedeemKeyRewards;
    return {
      lp: parsed.lp ?? 0,
      growthValue: parsed.growthValue ?? 0,
      items: parsed.items ?? [],
    };
  } catch {
    return { lp: 0, growthValue: 0, items: [] };
  }
}

function formatDateTime(value: string, locale: LocaleCode) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return date.toLocaleString(locale, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function mergeRedemptions(
  redemption: PetRedeemKeyRedemption,
  values: PetRedeemKeyRedemption[],
) {
  return [
    redemption,
    ...values.filter((value) => value.id !== redemption.id),
  ].slice(0, 20);
}
