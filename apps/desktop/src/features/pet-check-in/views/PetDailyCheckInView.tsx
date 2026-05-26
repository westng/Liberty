import "./PetDailyCheckInView.css";
import { useEffect, useMemo, useState } from "react";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import {
  findCatalogItem,
  itemName,
  rarityToneClass,
  shopImageUrl,
} from "@/features/pet-store/services/petStorePresentation";
import { createLocalPetService } from "@/shared/services/tauri/pet";
import type {
  LocaleCode,
  PetDailyCheckInEntry,
  PetDailyCheckInRewardPreview,
  PetDailyCheckInState,
  PetRewardItem,
  PetStoreState,
} from "@/shared/types/meeting";

const petService = createLocalPetService();

export default function PetDailyCheckInView() {
  const meetingStore = useMeetingStore();
  const locale = meetingStore.settings.locale;
  const isEnglish = locale === "en-US";
  const [checkInState, setCheckInState] = useState<PetDailyCheckInState | null>(null);
  const [loading, setLoading] = useState(false);
  const [claiming, setClaiming] = useState(false);
  const [errorMessage, setErrorMessage] = useState("");
  const [toastMessage, setToastMessage] = useState("");
  const storeState = checkInState?.storeState ?? null;
  const todayReward = checkInState?.todayReward ?? null;
  const rewards = useMemo(
    () => [...(checkInState?.rewards ?? [])].sort((left, right) => left.cycleDay - right.cycleDay),
    [checkInState],
  );
  const currentStreak = checkInState?.currentStreak ?? 0;
  const nextStreakDay = checkInState?.nextCycleDay ?? 1;
  const visibleTrackLength = checkInState?.cycleLength ?? 14;
  const checkedInToday = Boolean(checkInState?.checkedInToday);
  const nextMilestone = useMemo(() => {
    if (!rewards.length) {
      return null;
    }
    return rewards.find((reward) => reward.cycleDay >= nextStreakDay && reward.items.length > 0) ?? null;
  }, [nextStreakDay, rewards]);
  const milestoneTargetDay = nextMilestone?.cycleDay ?? visibleTrackLength;
  const cycleProgress = visibleTrackLength > 0
    ? Math.min(100, Math.round((Math.min(currentStreak, visibleTrackLength) / visibleTrackLength) * 100))
    : 0;

  useEffect(() => {
    void loadState();
  }, []);

  useEffect(() => {
    if (!toastMessage) {
      return;
    }
    const timer = window.setTimeout(() => setToastMessage(""), 2200);
    return () => window.clearTimeout(timer);
  }, [toastMessage]);

  async function loadState() {
    setLoading(true);
    setErrorMessage("");
    try {
      setCheckInState(await petService.getDailyCheckInState());
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setLoading(false);
    }
  }

  async function claimDailyCheckIn() {
    setClaiming(true);
    setErrorMessage("");
    try {
      const result = await petService.claimDailyCheckIn();
      setCheckInState(result.state);
      setToastMessage(
        result.duplicate
          ? isEnglish
            ? "Already checked in today."
            : "今日已经签到过了。"
          : isEnglish
            ? `Checked in. LP +${result.entry.rewardLp}, growth +${result.entry.growthValue}.`
            : `签到完成，LP +${result.entry.rewardLp}，成长值 +${result.entry.growthValue}。`,
      );
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setClaiming(false);
    }
  }

  function buttonLabel() {
    if (claiming) {
      return isEnglish ? "Claiming..." : "领取中...";
    }
    if (checkedInToday) {
      return isEnglish ? "Checked In Today" : "今日已签到";
    }
    return isEnglish ? "Check In" : "签到领取";
  }

  return (
    <section className="view-stack native-page pet-check-in-page">
      <div className="pet-check-in-toast" data-visible={Boolean(toastMessage)} role="status">
        {toastMessage}
      </div>

      {errorMessage && <div className="pet-check-in-message error">{errorMessage}</div>}

      <div className="pet-check-in-center">
        <article className="surface pet-check-in-stage">
          {loading ? (
            <div className="empty-state">{isEnglish ? "Loading check-in..." : "正在加载签到状态..."}</div>
          ) : (
            <>
              <section className={`pet-check-in-spotlight ${checkedInToday ? "is-complete" : "is-ready"}`}>
                <div className="pet-check-in-spotlight-copy">
                  <span className="pet-check-in-eyebrow">{isEnglish ? "Welfare Center" : "签到中心"}</span>
                  <h2>
                    {checkedInToday
                      ? isEnglish
                        ? "Today's reward is secured"
                        : "今日福利已入袋"
                      : isEnglish
                        ? "Daily reward is waiting"
                        : "今日福利待领取"}
                  </h2>
                  <p>
                    {checkedInToday
                      ? isEnglish
                        ? `Come back tomorrow for Day ${currentStreak + 1}. Your streak is still alive.`
                        : `明天回来领取第 ${currentStreak + 1} 天奖励，连续签到正在保持中。`
                      : isEnglish
                        ? `Claim Day ${nextStreakDay} to collect LP, growth, and milestone items.`
                        : `领取第 ${nextStreakDay} 天奖励，获得 LP、成长值和里程碑物品。`}
                  </p>
                  <div className="pet-check-in-spotlight-actions">
                    <button
                      className="primary-button"
                      type="button"
                      disabled={claiming || checkedInToday || !checkInState}
                      onClick={claimDailyCheckIn}
                    >
                      {buttonLabel()}
                    </button>
                    <span className={`pet-check-in-status ${checkedInToday ? "done" : "pending"}`}>
                      {checkedInToday
                        ? isEnglish
                          ? "Claimed"
                          : "已领取"
                        : isEnglish
                          ? "Available today"
                          : "今日可领"}
                    </span>
                  </div>
                </div>

                <div className="pet-check-in-spotlight-art" aria-hidden="true">
                  <div className="pet-check-in-orbit">
                    {(todayReward?.items ?? []).slice(0, 3).map((item, index) => (
                      <RewardItemIcon
                        key={`${item.itemKey}-${index}`}
                        item={item}
                        locale={locale}
                        storeState={storeState}
                      />
                    ))}
                    {!todayReward?.items.length && (
                      <>
                        <span className="pet-check-in-token">LP</span>
                        <span className="pet-check-in-token">{isEnglish ? "EXP" : "成长"}</span>
                      </>
                    )}
                  </div>
                  <div className="pet-check-in-streak-medal">
                    <strong>{currentStreak}</strong>
                    <span>{isEnglish ? "day streak" : "天连续"}</span>
                  </div>
                </div>
              </section>

              <section className="pet-check-in-dashboard">
                <div className="pet-check-in-today-card">
                  <div className="pet-check-in-card-head">
                    <span>{isEnglish ? "Today Package" : "今日福利包"}</span>
                    <strong>{checkInState?.checkInDate ?? "--"}</strong>
                  </div>
                  {todayReward && <RewardSummary reward={todayReward} locale={locale} storeState={storeState} />}
                </div>

                <div className="pet-check-in-progress-card">
                  <div className="pet-check-in-card-head">
                    <span>{isEnglish ? "Cycle Progress" : "周期进度"}</span>
                    <strong>{Math.min(currentStreak, visibleTrackLength)}/{visibleTrackLength}</strong>
                  </div>
                  <div
                    className="pet-check-in-progress"
                    role="progressbar"
                    aria-valuemin={0}
                    aria-valuemax={visibleTrackLength}
                    aria-valuenow={Math.min(currentStreak, visibleTrackLength)}
                  >
                    <span style={{ width: `${cycleProgress}%` }} />
                  </div>
                  <div className="pet-check-in-progress-caption">
                    <span>{isEnglish ? "Next special reward" : "下个特别奖励"}</span>
                    <strong>{isEnglish ? `Day ${milestoneTargetDay}` : `第 ${milestoneTargetDay} 天`}</strong>
                  </div>
                </div>
              </section>

              <section className="pet-check-in-cycle">
                <div className="pet-check-in-section-head">
                  <div>
                    <h3>{isEnglish ? "Reward Calendar" : "奖励日历"}</h3>
                    <p>
                      {isEnglish
                        ? "Special rewards appear on marked days. Completed days stay highlighted."
                        : "标记的天数有特别奖励，已签到的天数会点亮。"}
                    </p>
                  </div>
                </div>
                <div className="pet-check-in-cycle-grid">
                  {rewards.map((reward) => (
                    <RewardDayCard
                      key={reward.cycleDay}
                      reward={reward}
                      locale={locale}
                      storeState={storeState}
                      active={reward.cycleDay === nextStreakDay && !checkedInToday}
                      completed={currentStreak >= reward.cycleDay}
                    />
                  ))}
                </div>
              </section>
            </>
          )}
        </article>

        <aside className="pet-check-in-side-stack">
          <section className="surface native-inspector-panel">
            <div className="section-heading">
              <h3>{isEnglish ? "Next Special Reward" : "下个特别奖励"}</h3>
            </div>
            {nextMilestone ? (
              <MilestonePreview reward={nextMilestone} locale={locale} storeState={storeState} />
            ) : (
              <div className="empty-state">
                {isEnglish ? "Keep checking in. More rewards are on the way." : "继续签到，更多奖励等你领取。"}
              </div>
            )}
          </section>

          <section className="surface native-inspector-panel">
            <div className="section-heading">
              <h3>{isEnglish ? "Reward Overview" : "奖励概览"}</h3>
            </div>
            <div className="native-stat-list">
              <div>
                <span>{isEnglish ? "Streak" : "连续签到"}</span>
                <strong>{currentStreak}</strong>
              </div>
              <div>
                <span>{isEnglish ? "LP Balance" : "LP 余额"}</span>
                <strong>{storeState?.wallet.balance ?? 0}</strong>
              </div>
              <div>
                <span>{isEnglish ? "Inventory" : "仓库物品"}</span>
                <strong>{storeState?.inventory.length ?? 0}</strong>
              </div>
              <div>
                <span>{isEnglish ? "Special Reward" : "特别奖励"}</span>
                <strong>{Math.min(currentStreak, milestoneTargetDay)}/{milestoneTargetDay}</strong>
              </div>
            </div>
          </section>

          <section className="surface native-inspector-panel">
            <div className="section-heading">
              <h3>{isEnglish ? "Recent Records" : "最近记录"}</h3>
            </div>
            <div className="pet-check-in-history">
              {(checkInState?.history ?? []).length ? (
                checkInState?.history.slice(0, 5).map((entry) => (
                  <HistoryRow key={entry.id} entry={entry} locale={locale} storeState={storeState} />
                ))
              ) : (
                <div className="empty-state">{isEnglish ? "No check-ins yet." : "暂无签到记录。"}</div>
              )}
            </div>
          </section>
        </aside>
      </div>
    </section>
  );
}

function RewardSummary({
  reward,
  locale,
  storeState,
}: {
  reward: PetDailyCheckInRewardPreview;
  locale: LocaleCode;
  storeState: PetStoreState | null;
}) {
  const isEnglish = locale === "en-US";
  return (
    <div className="pet-check-in-reward-summary">
      <div>
        <span>LP</span>
        <strong>+{reward.rewardLp}</strong>
      </div>
      <div>
        <span>{isEnglish ? "Growth" : "成长值"}</span>
        <strong>+{reward.growthValue}</strong>
      </div>
      {reward.items.length ? (
        reward.items.map((item) => (
          <RewardItemChip key={item.itemKey} item={item} locale={locale} storeState={storeState} />
        ))
      ) : (
        <div>
          <span>{isEnglish ? "Item" : "物品"}</span>
          <strong>{isEnglish ? "None" : "无"}</strong>
        </div>
      )}
    </div>
  );
}

function RewardDayCard({
  reward,
  locale,
  storeState,
  active,
  completed,
}: {
  reward: PetDailyCheckInRewardPreview;
  locale: LocaleCode;
  storeState: PetStoreState | null;
  active: boolean;
  completed: boolean;
}) {
  const isEnglish = locale === "en-US";
  const hasItemReward = reward.items.length > 0;
  return (
    <article className={`pet-check-in-day-card ${active ? "active" : ""} ${completed ? "completed" : ""} ${hasItemReward ? "milestone" : ""}`}>
      <div className="pet-check-in-day-marker" aria-hidden="true">
        <span>{reward.cycleDay}</span>
      </div>
      <div className="pet-check-in-day-body">
        <div className="pet-check-in-day-head">
          <strong>{isEnglish ? `Day ${reward.cycleDay}` : `第 ${reward.cycleDay} 天`}</strong>
          <span>{completed ? (isEnglish ? "Done" : "已完成") : active ? (isEnglish ? "Today" : "今日") : hasItemReward ? (isEnglish ? "Special" : "特别奖励") : ""}</span>
        </div>
        <div className="pet-check-in-day-values">
          <span>+{reward.rewardLp} LP</span>
          <span>{isEnglish ? `+${reward.growthValue} Growth` : `+${reward.growthValue} 成长`}</span>
        </div>
        <div className="pet-check-in-day-items">
          {reward.items.length ? reward.items.map((item) => (
            <RewardItemIcon key={item.itemKey} item={item} locale={locale} storeState={storeState} />
          )) : <span>{isEnglish ? "Base reward" : "基础奖励"}</span>}
        </div>
      </div>
    </article>
  );
}

function MilestonePreview({
  reward,
  locale,
  storeState,
}: {
  reward: PetDailyCheckInRewardPreview;
  locale: LocaleCode;
  storeState: PetStoreState | null;
}) {
  const isEnglish = locale === "en-US";
  const itemNames = reward.items.map((item) => {
    const catalog = findCatalogItem(storeState, item.itemKey);
    return catalog ? itemName(storeState, catalog, locale) : item.itemKey;
  });
  return (
    <div className="pet-check-in-milestone-preview">
      <span>{isEnglish ? `Day ${reward.cycleDay}` : `第 ${reward.cycleDay} 天奖励`}</span>
      <strong>
        {itemNames.length
          ? itemNames.join("、")
          : isEnglish
            ? "Base welfare package"
            : "基础福利包"}
      </strong>
      <div className="pet-check-in-milestone-items">
        {reward.items.length ? reward.items.map((item) => (
          <RewardItemIcon key={item.itemKey} item={item} locale={locale} storeState={storeState} />
        )) : <span>{isEnglish ? "No item reward" : "无物品奖励"}</span>}
      </div>
      <p>
        +{reward.rewardLp} LP · +{reward.growthValue} {isEnglish ? "Growth" : "成长"}
      </p>
    </div>
  );
}

function RewardItemChip({
  item,
  locale,
  storeState,
}: {
  item: PetRewardItem;
  locale: LocaleCode;
  storeState: PetStoreState | null;
}) {
  const catalog = findCatalogItem(storeState, item.itemKey);
  const name = catalog ? itemName(storeState, catalog, locale) : item.itemKey;
  return (
    <div className="pet-check-in-item-chip">
      <img src={shopImageUrl(catalog?.item.assetKey ?? "gift_box")} alt={name} />
      <span>{name}</span>
      <strong>×{item.quantity}</strong>
    </div>
  );
}

function RewardItemIcon({
  item,
  locale,
  storeState,
}: {
  item: PetRewardItem;
  locale: LocaleCode;
  storeState: PetStoreState | null;
}) {
  const catalog = findCatalogItem(storeState, item.itemKey);
  const name = catalog ? itemName(storeState, catalog, locale) : item.itemKey;
  return (
    <span className={`pet-check-in-item-icon ${catalog ? rarityToneClass(catalog.item.rarity) : ""}`} title={name}>
      <img src={shopImageUrl(catalog?.item.assetKey ?? "gift_box")} alt={name} />
      <small>×{item.quantity}</small>
    </span>
  );
}

function HistoryRow({
  entry,
  locale,
  storeState,
}: {
  entry: PetDailyCheckInEntry;
  locale: LocaleCode;
  storeState: PetStoreState | null;
}) {
  const isEnglish = locale === "en-US";
  const itemNames = entry.rewardItems.map((item) => {
    const catalog = findCatalogItem(storeState, item.itemKey);
    return catalog ? itemName(storeState, catalog, locale) : item.itemKey;
  });
  return (
    <div className="pet-check-in-history-row">
      <div>
        <strong>{entry.checkInDate}</strong>
        <span>{isEnglish ? `Streak ${entry.streakCount}` : `连续 ${entry.streakCount} 天`}</span>
      </div>
      <p>
        +{entry.rewardLp} LP · +{entry.growthValue} {isEnglish ? "Growth" : "成长"}
        {itemNames.length ? ` · ${itemNames.join("、")}` : ""}
      </p>
    </div>
  );
}
