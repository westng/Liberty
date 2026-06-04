import "./PetDailyCheckInView.css";
import { ask } from "@tauri-apps/plugin-dialog";
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

type CalendarDayStatus = "signed" | "today" | "makeup" | "missed" | "future" | "idle";

type RewardCalendarCell = {
  date: string;
  day: number;
  status: CalendarDayStatus;
  reward: PetDailyCheckInRewardPreview | null;
};

export default function PetDailyCheckInView() {
  const meetingStore = useMeetingStore();
  const locale = meetingStore.settings.locale;
  const isEnglish = locale === "en-US";
  const [checkInState, setCheckInState] = useState<PetDailyCheckInState | null>(null);
  const [loading, setLoading] = useState(false);
  const [claiming, setClaiming] = useState(false);
  const [repairing, setRepairing] = useState(false);
  const [calendarMonthOffset, setCalendarMonthOffset] = useState(0);
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
  const makeupAvailable = Boolean(checkInState?.makeupAvailable);
  const makeupDate = checkInState?.makeupDate ?? "";
  const makeupTicketQuantity = checkInState?.makeupTicketQuantity ?? 0;
  const canRepair = makeupAvailable && makeupTicketQuantity > 0;
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
  const calendarCells = useMemo(
    () => buildRewardCalendar(checkInState, rewards, calendarMonthOffset),
    [calendarMonthOffset, checkInState, rewards],
  );
  const calendarMonthLabel = useMemo(
    () => monthLabel(checkInState?.checkInDate, locale, calendarMonthOffset),
    [calendarMonthOffset, checkInState?.checkInDate, locale],
  );
  const weekdayLabels = useMemo(() => calendarWeekdayLabels(locale), [locale]);

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
    if (makeupAvailable) {
      const confirmed = await ask(
        isEnglish
          ? "Start again from today? Missed days in the current gap can no longer be repaired."
          : "确定从今日重新开始吗？当前断签缺口将不能再补签。",
        {
          title: isEnglish ? "Restart check-in streak" : "重新开始签到",
          kind: "warning",
          okLabel: isEnglish ? "Restart Today" : "从今日开始",
          cancelLabel: isEnglish ? "Cancel" : "取消",
        },
      );
      if (!confirmed) {
        return;
      }
    }
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

  async function repairDailyCheckIn() {
    setRepairing(true);
    setErrorMessage("");
    try {
      const result = await petService.repairDailyCheckIn();
      setCheckInState(result.state);
      setToastMessage(
        isEnglish
          ? `Repaired ${result.entry.checkInDate}. Ticket x1 used.`
          : `已补签 ${result.entry.checkInDate}，消耗 1 张补签票券。`,
      );
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setRepairing(false);
    }
  }

  function buttonLabel() {
    if (claiming) {
      return isEnglish ? "Claiming..." : "领取中...";
    }
    if (checkedInToday) {
      return isEnglish ? "Checked In Today" : "今日已签到";
    }
    if (makeupAvailable) {
      return isEnglish ? "Restart Today" : "从今日重新开始";
    }
    return isEnglish ? "Check In" : "签到领取";
  }

  function repairButtonLabel() {
    if (repairing) {
      return isEnglish ? "Repairing..." : "补签中...";
    }
    if (!makeupTicketQuantity) {
      return isEnglish ? "Need Ticket" : "需要补签票券";
    }
    return isEnglish ? `Repair ${makeupDate}` : `补签 ${makeupDate}`;
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
                    {makeupAvailable
                      ? isEnglish
                        ? "Missed check-in can be repaired"
                        : "断签可以补回"
                      : checkedInToday
                      ? isEnglish
                        ? "Today's reward is secured"
                        : "今日福利已入袋"
                      : isEnglish
                        ? "Daily reward is waiting"
                        : "今日福利待领取"}
                  </h2>
                  <p>
                    {makeupAvailable
                      ? isEnglish
                        ? `Use one make-up ticket to repair ${makeupDate} before restarting from today.`
                        : `先消耗 1 张补签票券补回 ${makeupDate}，再继续保持连续签到。`
                      : checkedInToday
                      ? isEnglish
                        ? `Come back tomorrow for Day ${currentStreak + 1}. Your streak is still alive.`
                        : `明天回来领取第 ${currentStreak + 1} 天奖励，连续签到正在保持中。`
                      : isEnglish
                        ? `Claim Day ${nextStreakDay} to collect LP, growth, and milestone items.`
                        : `领取第 ${nextStreakDay} 天奖励，获得 LP、成长值和里程碑物品。`}
                  </p>
                  <div className="pet-check-in-spotlight-actions">
                    {makeupAvailable && (
                      <button
                        className="primary-button"
                        type="button"
                        disabled={repairing || claiming || !canRepair}
                        onClick={repairDailyCheckIn}
                      >
                        {repairButtonLabel()}
                      </button>
                    )}
                    <button
                      className={makeupAvailable ? "text-button" : "primary-button"}
                      type="button"
                      disabled={claiming || repairing || checkedInToday || !checkInState}
                      onClick={claimDailyCheckIn}
                    >
                      {buttonLabel()}
                    </button>
                    <span className={`pet-check-in-status ${checkedInToday ? "done" : "pending"}`}>
                      {makeupAvailable
                        ? isEnglish
                          ? `${makeupTicketQuantity} ticket${makeupTicketQuantity === 1 ? "" : "s"}`
                          : `补签票券 ×${makeupTicketQuantity}`
                        : checkedInToday
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
                        ? "This month shows completed check-ins, repairable gaps, today, and upcoming rewards."
                        : "本月日历展示已签到、可补签、今日状态和即将到来的奖励。"}
                    </p>
                  </div>
                </div>
                <div className="pet-check-in-calendar">
                  <div className="pet-check-in-calendar-head">
                    <strong>{calendarMonthLabel}</strong>
                    <div className="pet-check-in-calendar-nav">
                      <button type="button" className="text-button" onClick={() => setCalendarMonthOffset((value) => value - 1)}>
                        {isEnglish ? "Previous" : "上月"}
                      </button>
                      <button type="button" className="text-button" disabled={calendarMonthOffset === 0} onClick={() => setCalendarMonthOffset(0)}>
                        {isEnglish ? "Current" : "本月"}
                      </button>
                      <button type="button" className="text-button" onClick={() => setCalendarMonthOffset((value) => value + 1)}>
                        {isEnglish ? "Next" : "下月"}
                      </button>
                    </div>
                  </div>
                  <div className="pet-check-in-calendar-weekdays">
                    {weekdayLabels.map((label) => (
                      <span key={label}>{label}</span>
                    ))}
                  </div>
                  <div className="pet-check-in-calendar-grid">
                    {calendarCells.map((cell, index) => cell.date ? (
                      <RewardCalendarDay
                        key={cell.date}
                        cell={cell}
                        locale={locale}
                        storeState={storeState}
                      />
                    ) : (
                      <div key={`blank-${index}`} className="pet-check-in-calendar-blank" />
                    ))}
                  </div>
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
                <span>{isEnglish ? "Make-up Tickets" : "补签票券"}</span>
                <strong>{makeupTicketQuantity}</strong>
              </div>
              <div>
                <span>{isEnglish ? "Missed Days" : "断签天数"}</span>
                <strong>{checkInState?.missedDays ?? 0}</strong>
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

function buildRewardCalendar(
  state: PetDailyCheckInState | null,
  rewards: PetDailyCheckInRewardPreview[],
  monthOffset: number,
): RewardCalendarCell[] {
  if (!state) {
    return [];
  }
  const today = parseLocalDate(state.checkInDate) ?? new Date();
  const visibleMonth = addMonths(today, monthOffset);
  const year = visibleMonth.getFullYear();
  const month = visibleMonth.getMonth();
  const firstDay = new Date(year, month, 1);
  const daysInMonth = new Date(year, month + 1, 0).getDate();
  const leadingBlanks = mondayFirstWeekdayIndex(firstDay);
  const rewardByCycleDay = new Map(rewards.map((reward) => [reward.cycleDay, reward]));
  const entryByDate = new Map(state.history.map((entry) => [entry.checkInDate, entry]));
  const missedDates = missedDateSet(state);
  const anchorDate = state.makeupAvailable && state.makeupDate ? state.makeupDate : state.checkInDate;
  const anchorCycleDay = state.nextCycleDay;
  const cells: RewardCalendarCell[] = [];

  for (let index = 0; index < leadingBlanks; index += 1) {
    cells.push({ date: "", day: 0, status: "idle", reward: null });
  }

  for (let day = 1; day <= daysInMonth; day += 1) {
    const date = formatLocalDate(new Date(year, month, day));
    const entry = entryByDate.get(date);
    const reward = rewardForCalendarDate({
      date,
      entry,
      state,
      anchorDate,
      anchorCycleDay,
      rewardByCycleDay,
    });
    cells.push({
      date,
      day,
      status: calendarStatusForDate(date, state, entry, missedDates),
      reward,
    });
  }

  return cells;
}

function rewardForCalendarDate({
  date,
  entry,
  state,
  anchorDate,
  anchorCycleDay,
  rewardByCycleDay,
}: {
  date: string;
  entry?: PetDailyCheckInEntry;
  state: PetDailyCheckInState;
  anchorDate: string;
  anchorCycleDay: number;
  rewardByCycleDay: Map<number, PetDailyCheckInRewardPreview>;
}) {
  if (entry) {
    return {
      cycleDay: entry.cycleDay,
      rewardLp: entry.rewardLp,
      growthValue: entry.growthValue,
      items: entry.rewardItems,
    };
  }
  if (date === state.makeupDate || date === state.checkInDate) {
    return state.todayReward;
  }
  const offset = daysBetween(anchorDate, date);
  if (offset < 0) {
    return null;
  }
  const projectedCycleDay = anchorCycleDay + offset;
  return rewardByCycleDay.get(projectedCycleDay) ?? {
    cycleDay: projectedCycleDay,
    rewardLp: 20,
    growthValue: 5,
    items: [],
  };
}

function calendarStatusForDate(
  date: string,
  state: PetDailyCheckInState,
  entry: PetDailyCheckInEntry | undefined,
  missedDates: Set<string>,
): CalendarDayStatus {
  if (entry) {
    return "signed";
  }
  if (date === state.makeupDate) {
    return "makeup";
  }
  if (missedDates.has(date)) {
    return "missed";
  }
  if (date === state.checkInDate) {
    return "today";
  }
  if (date > state.checkInDate) {
    return "future";
  }
  return "idle";
}

function missedDateSet(state: PetDailyCheckInState) {
  const missedDates = new Set<string>();
  if (!state.missedDays || !state.history.length || state.checkedInToday) {
    return missedDates;
  }
  const latest = state.history[0];
  const latestDate = parseLocalDate(latest.checkInDate);
  const today = parseLocalDate(state.checkInDate);
  if (!latestDate || !today) {
    return missedDates;
  }
  for (
    let cursor = addDays(latestDate, 1);
    cursor < today;
    cursor = addDays(cursor, 1)
  ) {
    missedDates.add(formatLocalDate(cursor));
  }
  return missedDates;
}

function calendarStatusLabel(status: CalendarDayStatus, locale: LocaleCode) {
  const isEnglish = locale === "en-US";
  switch (status) {
    case "signed":
      return isEnglish ? "Done" : "已签";
    case "today":
      return isEnglish ? "Today" : "今日";
    case "makeup":
      return isEnglish ? "Repair" : "补签";
    case "missed":
      return isEnglish ? "Missed" : "断签";
    case "future":
      return isEnglish ? "Soon" : "待到";
    default:
      return "";
  }
}

function calendarWeekdayLabels(locale: LocaleCode) {
  return locale === "en-US"
    ? ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
    : ["一", "二", "三", "四", "五", "六", "日"];
}

function monthLabel(date: string | undefined, locale: LocaleCode, monthOffset: number) {
  const value = date ? parseLocalDate(date) : null;
  if (!value) {
    return "--";
  }
  return new Intl.DateTimeFormat(locale, { year: "numeric", month: "long" }).format(addMonths(value, monthOffset));
}

function mondayFirstWeekdayIndex(date: Date) {
  return (date.getDay() + 6) % 7;
}

function parseLocalDate(value: string) {
  const [year, month, day] = value.split("-").map((part) => Number.parseInt(part, 10));
  if (!year || !month || !day) {
    return null;
  }
  return new Date(year, month - 1, day);
}

function formatLocalDate(date: Date) {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function addDays(date: Date, days: number) {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate() + days);
}

function addMonths(date: Date, months: number) {
  return new Date(date.getFullYear(), date.getMonth() + months, 1);
}

function daysBetween(start: string, end: string) {
  const startDate = parseLocalDate(start);
  const endDate = parseLocalDate(end);
  if (!startDate || !endDate) {
    return -1;
  }
  const oneDayMs = 24 * 60 * 60 * 1000;
  return Math.round((endDate.getTime() - startDate.getTime()) / oneDayMs);
}

function RewardCalendarDay({
  cell,
  locale,
  storeState,
}: {
  cell: RewardCalendarCell;
  locale: LocaleCode;
  storeState: PetStoreState | null;
}) {
  const isEnglish = locale === "en-US";
  const hasItemReward = Boolean(cell.reward?.items.length);
  const statusLabel = calendarStatusLabel(cell.status, locale);
  return (
    <article className={`pet-check-in-calendar-day ${cell.status} ${hasItemReward ? "milestone" : ""}`}>
      <div className="pet-check-in-calendar-day-head">
        <strong>{cell.day}</strong>
        <span>{statusLabel}</span>
      </div>
      {cell.reward && (
        <div className="pet-check-in-calendar-reward">
          <span>{isEnglish ? `Day ${cell.reward.cycleDay}` : `第 ${cell.reward.cycleDay} 天`}</span>
          <small>+{cell.reward.rewardLp} LP</small>
          <div className="pet-check-in-calendar-items">
            {cell.reward.items.length ? cell.reward.items.slice(0, 2).map((item) => (
              <RewardItemIcon key={item.itemKey} item={item} locale={locale} storeState={storeState} />
            )) : <em>{isEnglish ? "Base" : "基础"}</em>}
          </div>
        </div>
      )}
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
