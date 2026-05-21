import "./PetManagementView.css";
import { message } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { icons as f7Icons } from "@iconify-json/f7";
import { getIconData, iconToSVG } from "@iconify/utils";
import { useEffect, useMemo, useState } from "react";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { usePetStore } from "@/features/pet/stores/usePetStore";
import {
  getPetEnvironmentState,
  getPetSpriteFrameCountForGroup,
  getPetSpriteScale,
  getPetSpriteUrlForGroup,
  resolvePetVisualAction,
} from "@/features/pet/services/petSprites";
import { formatPetEventDetail, formatPetEventTitle, formatPetEventValue } from "@/features/pet/services/petEventFormatters";
import { applyDesktopPetState, openExtraDesktopPet } from "@/shared/services/tauri/pet";
import { promptPetName } from "@/shared/services/tauri/system";
import type { LocaleCode, PetInteractionAction, PetSettings, PetStage } from "@/shared/types/meeting";

const ANIMATION_FRAME_INTERVAL_MS = 1000;

type SettingsForm = Omit<PetSettings, "petId" | "updatedAt">;

const defaultSettingsForm: SettingsForm = {
  desktopEnabled: true,
  alwaysOnTop: true,
  muted: false,
  focusModeEnabled: false,
  proactiveLevel: 2,
  lastWindowX: undefined,
  lastWindowY: undefined,
};

export default function PetManagementView() {
  const meetingStore = useMeetingStore();
  const petStore = usePetStore();
  const [petNameSaving, setPetNameSaving] = useState(false);
  const [extraPetOpening, setExtraPetOpening] = useState(false);
  const [petCoverFrameIndex, setPetCoverFrameIndex] = useState(0);
  const [petVisualStateNow, setPetVisualStateNow] = useState(Date.now());
  const [settingsForm, setSettingsForm] = useState<SettingsForm>(defaultSettingsForm);
  const locale = meetingStore.settings.locale;
  const levelSnapshot = petStore.levelSnapshot;
  const stageLabel = levelSnapshot
    ? (locale === "en-US" ? levelSnapshot.currentStageLabelEn : levelSnapshot.currentStageLabelZh)
    : stageLabelFromStage(petStore.profile?.stage, locale);
  const moodLabel = (() => {
    switch (petStore.profile?.currentMood) {
      case "cheerful":
        return locale === "en-US" ? "Cheerful" : "开心";
      case "excited":
        return locale === "en-US" ? "Excited" : "兴奋";
      case "proud":
        return locale === "en-US" ? "Proud" : "得意";
      case "needy":
        return locale === "en-US" ? "Needs interaction" : "想互动";
      case "sleepy":
        return locale === "en-US" ? "Sleepy" : "犯困";
      case "bored":
        return locale === "en-US" ? "Bored" : "无聊";
      default:
        return locale === "en-US" ? "Idle" : "待机";
    }
  })();
  const progressRatio = Math.round((petStore.levelProgressRatio ?? 0) * 100);
  const nextStageLabel = levelSnapshot?.nextStage
    ? `${stageLabelFromStage(levelSnapshot.nextStage, locale)} Lv.${levelSnapshot.nextStageLevel ?? ""}`.trim()
    : (locale === "en-US" ? "Max companion stage" : "最高陪伴阶段");
  const petCoverMood = petStore.profile?.currentMood ?? "idle";
  const petCoverEnvironmentState = getPetEnvironmentState(meetingStore.jobs);
  const petCoverAction = resolvePetVisualAction({
    environmentState: petCoverEnvironmentState,
    latestEvent: petStore.events[0],
    mood: petCoverMood,
    nowMs: petVisualStateNow,
  });
  const petCoverUrl = getPetSpriteUrlForGroup(petCoverAction, petCoverFrameIndex);
  const petCoverStyle = {
    transform: `scale(${getPetSpriteScale(levelSnapshot?.currentStage ?? petStore.profile?.stage ?? "first_meet")})`,
  };
  const petActionIcons = useMemo(() => createPetActionIcons(), []);
  const petInteractionActions = [
    {
      action: "tap" as const,
      label: locale === "en-US" ? "Tap" : "点击",
      description: locale === "en-US" ? "Check in with your companion" : "叫它一下，增加陪伴感",
      icon: petActionIcons.tap,
    },
    {
      action: "pet" as const,
      label: locale === "en-US" ? "Pet" : "抚摸",
      description: locale === "en-US" ? "Comfort your companion" : "安抚一下，提升心情",
      icon: petActionIcons.pet,
    },
    {
      action: "feed" as const,
      label: locale === "en-US" ? "Feed" : "投喂",
      description: locale === "en-US" ? "Restore energy" : "补充活力，保持状态",
      icon: petActionIcons.feed,
    },
    {
      action: "encourage" as const,
      label: locale === "en-US" ? "Encourage" : "鼓励",
      description: locale === "en-US" ? "Give it a boost" : "打打气，进入积极状态",
      icon: petActionIcons.encourage,
    },
  ];
  const equippedCosmetics = petStore.cosmetics.filter((item) => item.equipped).map((item) => {
    let label = item.cosmeticKey;
    if (item.cosmeticKey === "sprout-ribbon") {
      label = locale === "en-US" ? "Sprout Ribbon" : "新芽丝带";
    }
    if (item.cosmeticKey === "golden-bell") {
      label = locale === "en-US" ? "Golden Bell" : "金色铃铛";
    }
    return { ...item, label };
  });

  useEffect(() => {
    void (async () => {
      await petStore.loadPetState();
      syncSettingsForm();
    })();
    window.addEventListener("focus", handleWindowFocus);
    const refreshTimer = window.setInterval(() => {
      setPetVisualStateNow(Date.now());
      void petStore.refresh();
    }, 4000);
    return () => {
      window.removeEventListener("focus", handleWindowFocus);
      window.clearInterval(refreshTimer);
    };
  }, []);

  useEffect(() => {
    const coverAnimationTimer = window.setInterval(() => {
      const frameCount = getPetSpriteFrameCountForGroup(petCoverAction);
      setPetCoverFrameIndex((current) => (current + 1) % Math.max(frameCount, 1));
    }, ANIMATION_FRAME_INTERVAL_MS);
    return () => window.clearInterval(coverAnimationTimer);
  }, [petCoverAction]);

  useEffect(() => {
    setPetCoverFrameIndex(0);
  }, [petCoverAction]);

  useEffect(() => {
    syncSettingsForm();
  }, [petStore.settings]);

  async function handleWindowFocus() {
    await petStore.refresh();
    syncSettingsForm();
  }

  function syncSettingsForm() {
    const settings = petStore.settings;
    if (!settings) {
      return;
    }
    setSettingsForm({
      desktopEnabled: settings.desktopEnabled,
      alwaysOnTop: settings.alwaysOnTop,
      muted: settings.muted,
      focusModeEnabled: settings.focusModeEnabled,
      proactiveLevel: settings.proactiveLevel,
      lastWindowX: settings.lastWindowX,
      lastWindowY: settings.lastWindowY,
    });
  }

  function patchSettingsForm(patch: Partial<SettingsForm>) {
    setSettingsForm((current) => ({ ...current, ...patch }));
  }

  async function saveSettings() {
    const savedSettings = await petStore.saveSettings({
      ...settingsForm,
      proactiveLevel: Number(settingsForm.proactiveLevel),
    });

    setSettingsForm({
      desktopEnabled: savedSettings.desktopEnabled,
      alwaysOnTop: savedSettings.alwaysOnTop,
      muted: savedSettings.muted,
      focusModeEnabled: savedSettings.focusModeEnabled,
      proactiveLevel: savedSettings.proactiveLevel,
      lastWindowX: savedSettings.lastWindowX,
      lastWindowY: savedSettings.lastWindowY,
    });

    try {
      await applyDesktopPetState(savedSettings, "pet-settings");
    } catch (error) {
      console.error("[pet-settings] settings saved but native pet sync failed", error);
    }

    await focusCurrentWindow();
    await message(locale === "en-US" ? "Pet desktop settings have been saved." : "桌面行为设置已保存。", {
      title: locale === "en-US" ? "Pet Center" : "宠物中心",
      kind: "info",
    });
  }

  async function openAnotherDesktopPet() {
    setExtraPetOpening(true);
    try {
      const status = await openExtraDesktopPet();
      if (!settingsForm.desktopEnabled) {
        patchSettingsForm({ desktopEnabled: true });
        await petStore.refresh();
        syncSettingsForm();
      }
      await focusCurrentWindow();
      await message(
        locale === "en-US"
          ? `Desktop pet opened. Active pets: ${status.instanceCount}.`
          : `已多开一个桌面宠物，当前共 ${status.instanceCount} 个。`,
        {
          title: locale === "en-US" ? "Pet Center" : "宠物中心",
          kind: "info",
        },
      );
    } catch (error) {
      const content = error instanceof Error ? error.message : String(error);
      await focusCurrentWindow();
      await message(content || (locale === "en-US" ? "Failed to open another desktop pet." : "多开桌面宠物失败。"), {
        title: locale === "en-US" ? "Pet Center" : "宠物中心",
        kind: "error",
      });
    } finally {
      setExtraPetOpening(false);
    }
  }

  async function triggerInteraction(action: PetInteractionAction) {
    try {
      await petStore.applyInteraction(action);
    } catch (error) {
      const rawContent = error instanceof Error ? error.message : String(error);
      const content = normalizePetInteractionError(rawContent, locale);
      await focusCurrentWindow();
      await message(content || (locale === "en-US" ? "Today's interaction limit has been reached." : "今天的互动已经达到上限。"), {
        title: locale === "en-US" ? "Pet Center" : "宠物中心",
        kind: "warning",
      });
    }
  }

  async function openPetNameEditor() {
    if (petNameSaving) {
      return;
    }

    const nextName = await promptPetName({
      title: locale === "en-US" ? "Edit Pet Name" : "修改宠物名字",
      message: locale === "en-US" ? "Enter a new pet name." : "请输入新的宠物名字。",
      defaultValue: petStore.profile?.name ?? "Libby",
    });

    if (nextName === null) {
      return;
    }

    await savePetNameValue(nextName);
  }

  async function savePetNameValue(rawName: string) {
    const nextName = rawName.trim().slice(0, 24);
    if (!nextName || nextName === (petStore.profile?.name ?? "")) {
      return;
    }

    setPetNameSaving(true);
    try {
      await petStore.saveProfile({ name: nextName });
      await focusCurrentWindow();
      await message(locale === "en-US" ? "Pet name has been saved." : "宠物名字已保存。", {
        title: locale === "en-US" ? "Pet Center" : "宠物中心",
        kind: "info",
      });
    } finally {
      setPetNameSaving(false);
    }
  }

  return (
    <section className="view-stack native-page pet-native-page pet-page-stack">
      <div className="pet-center-layout">
        <article className="surface pet-companion-card">
          <div className="section-heading pet-companion-header">
            <h3>{locale === "en-US" ? "Desktop Companion" : "桌面伙伴"}</h3>
          </div>

          <div className="pet-companion-content">
            <div className="pet-hero-body">
              <div className="pet-interaction-rail" aria-label={locale === "en-US" ? "Pet interactions" : "宠物互动"}>
                {petInteractionActions.map((item) => (
                  <button
                    key={item.action}
                    className="secondary-button pet-action-button"
                    type="button"
                    aria-label={`${item.label}：${item.description}`}
                    title={item.description}
                    onClick={() => triggerInteraction(item.action)}
                  >
                    <span className="pet-action-icon" dangerouslySetInnerHTML={{ __html: item.icon }} />
                    <span className="pet-action-tooltip">{item.description}</span>
                  </button>
                ))}
              </div>

              <div className="pet-visual-panel">
                <div className="pet-avatar-shell pet-avatar-shell-large" data-stage={levelSnapshot?.currentStage ?? petStore.profile?.stage ?? "first_meet"}>
                  <img className="pet-cover-image" src={petCoverUrl} style={petCoverStyle} alt="" draggable="false" />
                </div>
                <div className="pet-identity-row">
                  <strong title={petStore.profile?.name ?? "Libby"}>{petStore.profile?.name ?? "Libby"}</strong>
                  <button
                    className="pet-name-edit-button"
                    type="button"
                    aria-label={locale === "en-US" ? "Edit pet name" : "修改宠物名字"}
                    title={locale === "en-US" ? "Edit pet name" : "修改宠物名字"}
                    disabled={petNameSaving}
                    onClick={openPetNameEditor}
                  >
                    <span dangerouslySetInnerHTML={{ __html: petActionIcons.edit }} />
                  </button>
                  <span>{stageLabel} · {moodLabel}</span>
                </div>
              </div>

              <div className="pet-companion-copy">
                <div className="pet-status-grid">
                  <div className="pet-status-tile">
                    <span>{locale === "en-US" ? "Level" : "等级"}</span>
                    <strong>{levelSnapshot?.level ?? petStore.profile?.level ?? 1}</strong>
                  </div>
                  <div className="pet-status-tile">
                    <span>{locale === "en-US" ? "Total Growth" : "累计成长"}</span>
                    <strong>{levelSnapshot?.totalExperience ?? petStore.profile?.experience ?? 0}</strong>
                  </div>
                  <div className="pet-status-tile">
                    <span>{locale === "en-US" ? "Outfit" : "佩戴"}</span>
                    <strong>{equippedCosmetics[0]?.label ?? (locale === "en-US" ? "None" : "未佩戴")}</strong>
                  </div>
                  <div className="pet-status-tile">
                    <span>{locale === "en-US" ? "Rewards" : "奖励"}</span>
                    <strong>{petStore.cosmetics.length}</strong>
                  </div>
                </div>

                <div className="pet-progress-card">
                  <div className="pet-stat-row">
                    <span>{petStore.isMaxLevel ? (locale === "en-US" ? "Companion Value" : "满级陪伴值") : (locale === "en-US" ? "Level Progress" : "本级成长")}</span>
                    <strong>
                      {petStore.isMaxLevel
                        ? (levelSnapshot?.totalExperience ?? petStore.profile?.experience ?? 0)
                        : `${petStore.currentLevelExp}/${petStore.nextLevelRequired}`}
                    </strong>
                  </div>
                  <div className="pet-progress">
                    <div className="pet-progress-bar" style={{ width: `${progressRatio}%` }} />
                  </div>
                  <div className="pet-stat-row pet-next-stage-row">
                    <span>{locale === "en-US" ? "Next stage" : "下一阶段"}</span>
                    <strong>{nextStageLabel}</strong>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </article>

        <aside className="pet-control-stack">
          <article className="surface pet-settings-panel">
            <div className="section-heading">
              <h3>{locale === "en-US" ? "Desktop Behavior" : "桌面行为"}</h3>
            </div>

            <div className="pet-settings-form">
              <label className="toggle-row">
                <span>{locale === "en-US" ? "Enable desktop pet" : "启用桌宠"}</span>
                <input checked={settingsForm.desktopEnabled} onChange={(event) => patchSettingsForm({ desktopEnabled: event.target.checked })} type="checkbox" />
              </label>
              <label className="toggle-row">
                <span>{locale === "en-US" ? "Always on top" : "始终置顶"}</span>
                <input checked={settingsForm.alwaysOnTop} onChange={(event) => patchSettingsForm({ alwaysOnTop: event.target.checked })} type="checkbox" />
              </label>
              <label className="toggle-row">
                <span>{locale === "en-US" ? "Mute prompts" : "静音提示"}</span>
                <input checked={settingsForm.muted} onChange={(event) => patchSettingsForm({ muted: event.target.checked })} type="checkbox" />
              </label>
              <label className="toggle-row">
                <span>{locale === "en-US" ? "Focus mode" : "专注模式"}</span>
                <input checked={settingsForm.focusModeEnabled} onChange={(event) => patchSettingsForm({ focusModeEnabled: event.target.checked })} type="checkbox" />
              </label>
              <label className="range-row">
                <span>{locale === "en-US" ? "Proactive level" : "主动程度"}</span>
                <input value={settingsForm.proactiveLevel} onChange={(event) => patchSettingsForm({ proactiveLevel: Number(event.target.value) })} type="range" min="0" max="3" step="1" />
                <strong>{settingsForm.proactiveLevel}</strong>
              </label>

              <button className="secondary-button desktop-pet-open-button" type="button" disabled={extraPetOpening} onClick={openAnotherDesktopPet}>
                {extraPetOpening ? (locale === "en-US" ? "Opening" : "正在打开") : (locale === "en-US" ? "Open Another Pet" : "多开桌宠")}
              </button>
              <button className="primary-button" type="button" onClick={saveSettings}>
                {locale === "en-US" ? "Save Settings" : "保存设置"}
              </button>
            </div>
          </article>
        </aside>
      </div>

      <div className="pet-secondary-grid">
        <article className="surface pet-ledger-panel">
          <div className="section-heading">
            <h3>{locale === "en-US" ? "Recent Events" : "最近事件"}</h3>
          </div>

          {petStore.events.length ? (
            <div className="pet-ledger-list">
              {petStore.events.map((entry) => (
                <article key={entry.id} className="pet-ledger-item">
                  <div>
                    <strong>{formatPetEventTitle(entry, locale)}</strong>
                    <p>{formatPetEventDetail(entry, locale)}</p>
                  </div>
                  <div className="pet-ledger-side">
                    <span>{formatPetEventValue(entry, locale)}</span>
                    <span>{new Date(entry.eventTime).toLocaleString(locale)}</span>
                  </div>
                </article>
              ))}
            </div>
          ) : (
            <div className="empty-state">{locale === "en-US" ? "No pet events yet." : "还没有宠物事件。"}</div>
          )}
        </article>
      </div>
    </section>
  );
}

function createIconSvg(name: string) {
  const iconData = getIconData(f7Icons, name);
  const iconSvg = iconData
    ? iconToSVG(iconData, {
        height: "1em",
        width: "1em",
      })
    : null;

  return iconSvg ? `<svg viewBox="${iconSvg.attributes.viewBox}" width="1em" height="1em" aria-hidden="true">${iconSvg.body}</svg>` : "";
}

async function focusCurrentWindow() {
  await getCurrentWindow().setFocus().catch(() => undefined);
}

function createPetActionIcons() {
  return {
    edit: createIconSvg("pencil-circle"),
    tap: createIconSvg("hand-raised"),
    pet: createIconSvg("heart"),
    feed: createIconSvg("gift"),
    encourage: createIconSvg("bolt"),
  };
}

function normalizePetInteractionError(content: string, locale: string) {
  if (locale === "en-US" && /今天的.*互动已经达到上限/.test(content)) {
    return "Today's interaction limit has been reached.";
  }

  return content;
}

function stageLabelFromStage(stage: PetStage | undefined, locale: LocaleCode) {
  const labels: Record<string, { zh: string; en: string }> = {
    first_meet: { zh: "小小初遇", en: "First Encounter" },
    familiar: { zh: "轻轻熟悉", en: "Getting Familiar" },
    steady_companion: { zh: "稳定陪伴", en: "Steady Companion" },
    grow_together: { zh: "一起成长", en: "Growing Together" },
    tacit_bond: { zh: "默契养成", en: "Tacit Bond" },
    deep_bond: { zh: "深深羁绊", en: "Deep Bond" },
    long_company: { zh: "长久相伴", en: "Long Company" },
    bond_forever: { zh: "不离不弃", en: "Never Apart" },
    baby: { zh: "小小初遇", en: "First Encounter" },
    growing: { zh: "一起成长", en: "Growing Together" },
    mature: { zh: "深深羁绊", en: "Deep Bond" },
  };
  const label = labels[stage ?? "first_meet"] ?? labels.first_meet;
  return locale === "en-US" ? label.en : label.zh;
}
