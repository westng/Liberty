import type { LocaleCode } from "@/shared/types/meeting";

export type LocalizedPetLabel = {
  zh: string;
  en: string;
};

export const petSourceLabels: Record<string, LocalizedPetLabel> = {
  default: { zh: "默认解锁", en: "Default" },
  growth: { zh: "成长解锁", en: "Growth" },
  purchase: { zh: "购买获得", en: "Purchased" },
  achievement: { zh: "成就获得", en: "Achievement" },
  interaction: { zh: "互动", en: "Interaction" },
  workflow: { zh: "工作流", en: "Workflow" },
  store_pet: { zh: "切换伙伴", en: "Changed Companion" },
  store_equip: { zh: "装备物品", en: "Equipped Item" },
  store_food: { zh: "投喂食物", en: "Fed Food" },
  store_tool: { zh: "使用道具", en: "Used Item" },
  daily_free_store: { zh: "每日免费领取", en: "Daily Free Claim" },
  daily_blind_box: { zh: "每日盲盒", en: "Daily Blind Box" },
  blind_box_reward: { zh: "盲盒奖励", en: "Blind Box Reward" },
  blind_box_duplicate: { zh: "盲盒重复补偿", en: "Blind Box Duplicate" },
  blind_box_empty: { zh: "盲盒空奖", en: "Empty Blind Box" },
  gift_box_reward: { zh: "惊喜礼盒", en: "Gift Box Reward" },
  gift_box_duplicate: { zh: "礼盒重复补偿", en: "Gift Box Duplicate" },
  farm_harvest: { zh: "农场收获", en: "Farm Harvest" },
  redeem_key: { zh: "兑换奖励", en: "Redeem Reward" },
  daily_check_in: { zh: "每日签到", en: "Daily Check-in" },
  daily_check_in_makeup: { zh: "补签", en: "Make-up Check-in" },
  daily_check_in_duplicate: { zh: "签到重复补偿", en: "Check-in Duplicate" },
  daily_check_in_makeup_duplicate: { zh: "补签重复补偿", en: "Make-up Check-in Duplicate" },
  tap: { zh: "点击", en: "Tap" },
  pet: { zh: "抚摸", en: "Pet" },
  feed: { zh: "投喂", en: "Feed" },
  encourage: { zh: "鼓励", en: "Encourage" },
  job_created: { zh: "创建任务", en: "Job Created" },
  daily_open: { zh: "每日上线", en: "Daily Open" },
  transcription_started: { zh: "开始转写", en: "Transcription Started" },
  transcription_completed: { zh: "转写完成", en: "Transcription Completed" },
  ai_summary_completed: { zh: "AI 总结完成", en: "AI Summary Completed" },
  export_completed: { zh: "导出完成", en: "Export Completed" },
};

export function localizedPetLabel(label: LocalizedPetLabel, locale: LocaleCode) {
  return locale === "en-US" ? label.en : label.zh;
}

export function petSourceLabel(source: string, locale: LocaleCode) {
  const label = petSourceLabels[source];
  return label ? localizedPetLabel(label, locale) : null;
}

export function petSourceLabelOrUnknown(source: string, locale: LocaleCode) {
  return petSourceLabel(source, locale) ?? (locale === "en-US" ? "Unknown" : "未知");
}
