import type { LocaleCode, PetEventLedgerEntry } from "@/shared/types/meeting";
import {
  localizedPetLabel,
  petSourceLabels,
  type LocalizedPetLabel,
} from "@/shared/services/petSourceLabels";

const itemLabels: Record<string, LocalizedPetLabel> = {
  "libby-default": { zh: "Libby 初始伙伴", en: "Libby Starter" },
  "bell-accessory": { zh: "陪伴铃铛", en: "Companion Bell" },
  "clover-bow": { zh: "幸运草蝴蝶结", en: "Clover Bow" },
  "strawberry-candy": { zh: "草莓糖果发夹", en: "Strawberry Candy Clip" },
  "bookshelf-scene": { zh: "阅读书架", en: "Reading Bookshelf" },
  "cat-bed-scene": { zh: "软绵猫窝", en: "Cozy Cat Bed" },
  "desk-lamp-scene": { zh: "桌面小灯", en: "Desk Lamp" },
  "flower-pot-scene": { zh: "治愈花盆", en: "Healing Flower Pot" },
  "pillow-scene": { zh: "午睡软枕", en: "Nap Pillow" },
  "sheep-plush-scene": { zh: "小羊玩偶", en: "Sheep Plush" },
  "sofa-scene": { zh: "云朵小沙发", en: "Cloud Sofa" },
  "game-console-tool": { zh: "掌机玩具", en: "Game Console" },
  "gift-box-tool": { zh: "惊喜礼盒", en: "Gift Box" },
  "energy-capsule-tool": { zh: "能量胶囊", en: "Energy Capsule" },
  "energy-drink-tool": { zh: "能量饮料", en: "Energy Drink" },
  "gem-ticket-tool": { zh: "补签票券", en: "Make-up Check-in Ticket" },
  "golden-bell-tool": { zh: "金色铃铛", en: "Golden Bell" },
  "heart-charm-tool": { zh: "爱心护符", en: "Heart Charm" },
  "heart-rings-tool": { zh: "羁绊双环", en: "Heart Rings" },
  "magic-potion-tool": { zh: "魔法药水", en: "Magic Potion" },
  "magic-scroll-tool": { zh: "魔法卷轴", en: "Magic Scroll" },
  "magic-syringe-tool": { zh: "活力针筒", en: "Vigor Syringe" },
  "rainbow-crystal-tool": { zh: "彩虹水晶", en: "Rainbow Crystal" },
  "rune-stone-tool": { zh: "符文石", en: "Rune Stone" },
  "seed-bag-tool": { zh: "成长种子袋", en: "Seed Bag" },
  "sprout-bow-tool": { zh: "新芽蝴蝶结", en: "Sprout Bow" },
  "star-coin-tool": { zh: "星星硬币", en: "Star Coin" },
  "star-ticket-tool": { zh: "星光票券", en: "Star Ticket" },
  "stopwatch-tool": { zh: "专注秒表", en: "Stopwatch" },
  "bell-pudding-food": { zh: "铃铛布丁", en: "Bell Pudding" },
  "bento-box-food": { zh: "元气便当", en: "Bento Box" },
  "bubble-tea-food": { zh: "珍珠奶茶", en: "Bubble Tea" },
  "candy-jar-food": { zh: "糖果罐", en: "Candy Jar" },
  "chocolate-cake-food": { zh: "巧克力蛋糕", en: "Chocolate Cake" },
  "chocolate-chip-cookie-food": { zh: "巧克力曲奇", en: "Chocolate Chip Cookie" },
  "cream-cake-food": { zh: "奶油蛋糕", en: "Cream Cake" },
  "cupcake-food": { zh: "纸杯蛋糕", en: "Cupcake" },
  "custard-pudding-food": { zh: "焦糖布丁", en: "Custard Pudding" },
  "fruit-tart-food": { zh: "水果挞", en: "Fruit Tart" },
  "ice-cream-cone-food": { zh: "冰淇淋甜筒", en: "Ice Cream Cone" },
  "jelly-pudding-food": { zh: "果冻布丁", en: "Jelly Pudding" },
  "pink-donut-food": { zh: "粉色甜甜圈", en: "Pink Donut" },
  "purple-macaron-food": { zh: "紫色马卡龙", en: "Purple Macaron" },
  "sandwich-food": { zh: "元气三明治", en: "Sandwich" },
  "strawberry-shortcake-food": { zh: "草莓奶油蛋糕", en: "Strawberry Shortcake" },
  "wheat-harvest-food": { zh: "小麦", en: "Wheat" },
  "carrot-harvest-food": { zh: "胡萝卜", en: "Carrot" },
  "tomato-harvest-food": { zh: "番茄", en: "Tomato" },
  "pumpkin-harvest-food": { zh: "南瓜", en: "Pumpkin" },
  "corn-harvest-food": { zh: "玉米", en: "Corn" },
  "strawberry-harvest-food": { zh: "草莓", en: "Strawberry" },
  "blueberry-harvest-food": { zh: "蓝莓", en: "Blueberry" },
  "potato-harvest-food": { zh: "土豆", en: "Potato" },
  "baby-bottle-badge": { zh: "小小初遇勋章", en: "First Encounter Badge" },
  "bell-badge": { zh: "提醒铃勋章", en: "Reminder Bell Badge" },
  "calendar-check-badge": { zh: "日程完成勋章", en: "Calendar Check Badge" },
  "clover-badge": { zh: "幸运草勋章", en: "Clover Badge" },
  "crystal-badge": { zh: "晶石总结勋章", en: "Crystal Summary Badge" },
  "friendship-badge": { zh: "深深羁绊勋章", en: "Deep Bond Badge" },
  "heart-badge": { zh: "暖心陪伴勋章", en: "Warm Heart Badge" },
  "lantern-badge": { zh: "夜灯陪伴勋章", en: "Lantern Badge" },
  "laurel-sprout-badge": { zh: "新芽桂冠勋章", en: "Laurel Sprout Badge" },
  "moon-badge": { zh: "月光陪伴勋章", en: "Moon Badge" },
  "mystery-badge": { zh: "神秘伙伴勋章", en: "Mystery Badge" },
  "paw-heart-badge": { zh: "爪印爱心勋章", en: "Paw Heart Badge" },
  "pets-badge": { zh: "伙伴中心勋章", en: "Companion Center Badge" },
  "sheep-badge": { zh: "小羊陪伴勋章", en: "Sheep Companion Badge" },
  "sprout-badge": { zh: "一起成长勋章", en: "Growing Together Badge" },
  "sun-badge": { zh: "阳光交付勋章", en: "Sun Delivery Badge" },
};

function localized(label: LocalizedPetLabel, locale: LocaleCode) {
  return localizedPetLabel(label, locale);
}

export function formatPetEventTitle(entry: PetEventLedgerEntry, locale: LocaleCode) {
  const sourceLabel = petSourceLabels[entry.eventSource] ?? petSourceLabels[entry.eventType];
  if (sourceLabel) {
    return localized(sourceLabel, locale);
  }

  if (isStoreEvent(entry)) {
    return locale === "en-US" ? "Store Event" : "商店事件";
  }
  if (entry.eventType === "workflow") {
    return locale === "en-US" ? "Workflow Event" : "工作流事件";
  }
  if (entry.eventType === "interaction") {
    return locale === "en-US" ? "Interaction" : "互动";
  }
  if (isBlindBoxEvent(entry)) {
    return locale === "en-US" ? "Blind Box Event" : "盲盒事件";
  }
  if (isDailyCheckInEvent(entry)) {
    return locale === "en-US" ? "Daily Check-in" : "每日签到";
  }
  if (isGiftBoxEvent(entry)) {
    return locale === "en-US" ? "Gift Box" : "惊喜礼盒";
  }

  return locale === "en-US" ? "Pet Event" : "宠物事件";
}

export function formatPetEventDetail(entry: PetEventLedgerEntry, locale: LocaleCode) {
  const structuredDetail = formatStructuredMetadata(entry, locale);
  if (structuredDetail) {
    return structuredDetail;
  }

  const value = entry.metadata?.trim();
  if (!value) {
    return locale === "en-US" ? "No extra context." : "无额外上下文。";
  }
  if (entry.eventSource === "daily_open" && value === "Liberty app opened") {
    return locale === "en-US" ? "Liberty app opened." : "Liberty 已启动。";
  }
  if (entry.eventType === "workflow") {
    return workflowEventDetail(entry, locale);
  }
  if (locale === "en-US" && containsChinese(value)) {
    return fallbackEnglishDetail(entry);
  }
  if (locale !== "en-US" && isLikelyRawId(value)) {
    return fallbackChineseDetail(entry);
  }

  return value;
}

export function formatPetEventValue(entry: PetEventLedgerEntry, locale: LocaleCode) {
  if (entry.eventValue <= 0) {
    return locale === "en-US" ? "Recorded" : "已记录";
  }

  return `+${entry.eventValue} XP`;
}

function formatStructuredMetadata(entry: PetEventLedgerEntry, locale: LocaleCode) {
  const metadata = entry.metadata?.trim();
  if (!metadata) {
    return "";
  }
  if (metadata.startsWith("{")) {
    return formatJsonMetadata(entry, metadata, locale);
  }

  const parts = metadata.split("|");
  if (parts.length >= 3 && isStoreEvent(entry)) {
    return formatLegacyStoreMetadata(entry, parts, locale);
  }

  return "";
}

function formatJsonMetadata(entry: PetEventLedgerEntry, metadata: string, locale: LocaleCode) {
  try {
    const value = JSON.parse(metadata) as Partial<Record<"zh" | "en" | "itemKey" | "itemType" | "nameZh" | "nameEn" | "source", string>>;
    const localizedText = locale === "en-US" ? value.en : value.zh;
    if (localizedText?.trim()) {
      return localizedText.trim();
    }
    if (value.itemKey) {
      return itemEventDetail(entry, value.itemKey, locale, value.itemType, {
        zh: value.nameZh,
        en: value.nameEn,
      });
    }
    if (entry.eventType === "interaction") {
      return interactionEventDetail(entry, locale);
    }
    if (entry.eventType === "workflow") {
      return workflowEventDetail(entry, locale);
    }
    if (isBlindBoxEvent(entry)) {
      return blindBoxEventDetail(entry, locale);
    }
    if (isDailyCheckInEvent(entry)) {
      return dailyCheckInEventDetail(entry, locale);
    }
    if (isGiftBoxEvent(entry)) {
      return giftBoxEventDetail(entry, locale);
    }
  } catch {
    if (entry.eventType === "interaction") {
      return interactionEventDetail(entry, locale);
    }
    if (entry.eventType === "workflow") {
      return workflowEventDetail(entry, locale);
    }
    if (isBlindBoxEvent(entry)) {
      return blindBoxEventDetail(entry, locale);
    }
    if (isDailyCheckInEvent(entry)) {
      return dailyCheckInEventDetail(entry, locale);
    }
    if (isGiftBoxEvent(entry)) {
      return giftBoxEventDetail(entry, locale);
    }
  }

  return eventDefaultDetail(entry, locale);
}

function formatLegacyStoreMetadata(entry: PetEventLedgerEntry, parts: string[], locale: LocaleCode) {
  const itemKey = parts[0] ?? entry.eventSource;
  if (locale === "en-US") {
    return itemEventDetail(entry, itemKey, locale);
  }

  const legacyLine = parts.find((part, index) => index >= 2 && containsChinese(part))?.trim() ?? parts.slice(2).join("|").trim();
  return legacyLine || itemEventDetail(entry, itemKey, locale);
}

function itemEventDetail(
  entry: PetEventLedgerEntry,
  itemKey: string,
  locale: LocaleCode,
  itemType = "",
  itemName?: Partial<LocalizedPetLabel>,
) {
  const itemNameLabel = itemLabels[itemKey] ?? {
    zh: itemName?.zh || itemKey,
    en: itemName?.en || itemKey,
  };
  const itemDisplayName = localized(itemNameLabel, locale);
  if (entry.eventType === "store_food") {
    return locale === "en-US"
      ? `${itemDisplayName} was shared with your companion. Growth +${entry.eventValue}.`
      : `已投喂「${itemDisplayName}」，成长值 +${entry.eventValue}。`;
  }
  if (entry.eventType === "store_equip") {
    return locale === "en-US"
      ? `${itemDisplayName} is now equipped.`
      : `已装备「${itemDisplayName}」。`;
  }
  if (entry.eventType === "store_tool") {
    return locale === "en-US" ? `${itemDisplayName} was used.` : `已使用「${itemDisplayName}」。`;
  }
  if (itemType === "badge") {
    return locale === "en-US" ? `${itemDisplayName} badge recorded.` : `「${itemDisplayName}」勋章已记录。`;
  }

  return locale === "en-US" ? `${itemDisplayName} event recorded.` : `「${itemDisplayName}」事件已记录。`;
}

function fallbackEnglishDetail(entry: PetEventLedgerEntry) {
  if (entry.eventType === "store_food") {
    return itemEventDetail(entry, entry.eventSource, "en-US");
  }
  if (isStoreEvent(entry)) {
    return itemEventDetail(entry, entry.eventSource, "en-US");
  }
  if (entry.eventType === "interaction") {
    return interactionEventDetail(entry, "en-US");
  }

  return eventDefaultDetail(entry, "en-US");
}

function fallbackChineseDetail(entry: PetEventLedgerEntry) {
  if (isStoreEvent(entry)) {
    return itemEventDetail(entry, entry.eventSource, "zh-CN");
  }
  if (entry.eventType === "interaction") {
    return interactionEventDetail(entry, "zh-CN");
  }

  return eventDefaultDetail(entry, "zh-CN");
}

function containsChinese(value: string) {
  return /[\u3400-\u9fff]/.test(value);
}

function isLikelyRawId(value: string) {
  return /^[a-z0-9_-]+(:[a-z0-9_-]+)*$/i.test(value);
}

function isStoreEvent(entry: PetEventLedgerEntry) {
  return entry.eventType.startsWith("store_");
}

function isBlindBoxEvent(entry: PetEventLedgerEntry) {
  return entry.eventType.startsWith("blind_box_") || entry.eventSource === "daily_blind_box";
}

function isDailyCheckInEvent(entry: PetEventLedgerEntry) {
  return entry.eventType === "daily_check_in" || entry.eventSource.startsWith("daily_check_in");
}

function isGiftBoxEvent(entry: PetEventLedgerEntry) {
  return entry.eventType.startsWith("gift_box_") || entry.eventSource === "gift-box-tool";
}

function interactionEventDetail(entry: PetEventLedgerEntry, locale: LocaleCode) {
  const details: Record<string, LocalizedPetLabel> = {
    tap: { zh: "你轻轻叫了它一下，伙伴回应了这次互动。", en: "You checked in with your companion and it responded." },
    pet: { zh: "你安抚了伙伴，它的心情变得更轻松。", en: "You comforted your companion and it feels calmer." },
    feed: { zh: "你投喂了伙伴，这次照顾已记录。", en: "You fed your companion and this care was recorded." },
    encourage: { zh: "你鼓励了伙伴，它获得了一点继续陪伴的力量。", en: "You encouraged your companion and it feels ready to keep going." },
  };
  return localized(details[entry.eventSource] ?? { zh: "互动已记录。", en: "Interaction recorded." }, locale);
}

function blindBoxEventDetail(entry: PetEventLedgerEntry, locale: LocaleCode) {
  if (entry.eventType === "blind_box_duplicate") {
    return locale === "en-US"
      ? "Duplicate item converted into LP compensation."
      : "重复物品已转换为 LP 补偿。";
  }
  if (entry.eventType === "blind_box_empty") {
    return locale === "en-US"
      ? "No item this time, but the companionship was recorded."
      : "这次没有获得物品，但陪伴已记录。";
  }
  return locale === "en-US"
    ? "Blind box reward recorded."
    : "盲盒奖励已记录。";
}

function dailyCheckInEventDetail(entry: PetEventLedgerEntry, locale: LocaleCode) {
  if (entry.eventSource === "daily_check_in_makeup") {
    return locale === "en-US"
      ? `Make-up check-in recorded. Growth +${entry.eventValue}.`
      : `补签已记录，成长值 +${entry.eventValue}。`;
  }
  return locale === "en-US"
    ? `Daily check-in recorded. Growth +${entry.eventValue}.`
    : `每日签到已记录，成长值 +${entry.eventValue}。`;
}

function giftBoxEventDetail(entry: PetEventLedgerEntry, locale: LocaleCode) {
  if (entry.eventType === "gift_box_duplicate") {
    return locale === "en-US"
      ? "Duplicate gift-box item converted into LP compensation."
      : "礼盒重复物品已转换为 LP 补偿。";
  }
  return locale === "en-US" ? "Gift box reward recorded." : "惊喜礼盒奖励已记录。";
}

function eventDefaultDetail(entry: PetEventLedgerEntry, locale: LocaleCode) {
  return locale === "en-US"
    ? `${formatPetEventTitle(entry, "en-US")} recorded.`
    : `${formatPetEventTitle(entry, "zh-CN")}已记录。`;
}

function workflowEventDetail(entry: PetEventLedgerEntry, locale: LocaleCode) {
  const details: Record<string, LocalizedPetLabel> = {
    daily_open: { zh: "Liberty 已启动，今日陪伴已记录。", en: "Liberty opened and today's companionship was recorded." },
    job_created: { zh: "新任务已创建，伙伴成长已记录。", en: "A new job was created and companion growth was recorded." },
    transcription_started: { zh: "转写任务已开始，伙伴进入陪伴状态。", en: "Transcription started and the companion is standing by." },
    transcription_completed: { zh: "转写任务已完成，伙伴获得成长。", en: "Transcription completed and companion growth was recorded." },
    ai_summary_completed: { zh: "AI 总结已完成，伙伴获得成长。", en: "AI summary completed and companion growth was recorded." },
    export_completed: { zh: "结果已导出，伙伴获得成长。", en: "Export completed and companion growth was recorded." },
  };
  const detail = details[entry.eventSource];
  if (detail) {
    return localized(detail, locale);
  }

  return locale === "en-US" ? "Workflow event recorded." : "工作流事件已记录。";
}
