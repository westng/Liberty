import "./WorkMarketView.css";
import niumaMarketImage from "@/assets/images/work-maps/niuma-market.webp";
import farmCoverImage from "@/assets/images/work-maps/farm-isometric.webp";
import mineCoverImage from "@/assets/images/work-maps/mine-map.webp";
import factoryCoverImage from "@/assets/images/work-maps/factory-map.webp";
import convenienceStoreCoverImage from "@/assets/images/work-maps/convenience-store-map.webp";
import { useRouter } from "@/app/router/RouterContext";
import { useFarmStore } from "@/features/farm-work/stores/useFarmStore";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import type { LocaleCode, WorkMap, WorkMapStatus } from "@/shared/types/meeting";
import { useEffect } from "react";

export default function WorkMarketView() {
  const router = useRouter();
  const meetingStore = useMeetingStore();
  const farmStore = useFarmStore();
  const locale = meetingStore.settings.locale;
  const maps = farmStore.workMarketState?.maps ?? [];
  const coverImages: Record<string, string> = {
    farm: farmCoverImage,
    mine: mineCoverImage,
    factory: factoryCoverImage,
    "convenience-store": convenienceStoreCoverImage,
  };

  useEffect(() => {
    void farmStore.loadFarmState(true);
    const timer = window.setInterval(() => {
      void farmStore.refresh();
    }, 10_000);
    return () => window.clearInterval(timer);
  }, []);

  function openMap(map: WorkMap) {
    if (!map.enabled || !map.route) {
      return;
    }
    void router.push(map.route);
  }

  return (
    <section className="view-stack native-page work-market-page">
      <article className="surface native-page-hero work-market-hero">
        <div className="work-market-hero-copy">
          <span className="eyebrow">{locale === "en-US" ? "Work Games" : "打工地图"}</span>
          <h3>{locale === "en-US" ? "Niuma Market" : "牛马市场"}</h3>
          <p className="section-copy">
            {locale === "en-US"
              ? "Pick a mini-game map and turn small breaks into pet items."
              : "选择一个打工小游戏地图，把碎片时间变成宠物商品收益。"}
          </p>
        </div>
        <div className="work-market-hero-art">
          <img src={niumaMarketImage} alt={locale === "en-US" ? "Niuma Market map lobby" : "牛马市场地图大厅"} />
        </div>
      </article>

      <div className="work-market-grid">
        {maps.map((map) => (
          <button
            key={map.id}
            className={`surface work-map-card ${map.enabled ? "is-enabled" : "is-locked"}`}
            type="button"
            disabled={!map.enabled}
            onClick={() => openMap(map)}
          >
            <div className="work-map-cover">
              {coverImages[map.id] ? (
                <img src={coverImages[map.id]} alt="" />
              ) : (
                <div className={`work-map-placeholder work-map-placeholder-${map.id}`} aria-hidden="true" />
              )}
              <span className={`work-map-status work-map-status-${map.status}`}>
                {statusLabel(map.status, locale)}
              </span>
            </div>
            <div className="work-map-body">
              <div>
                <strong>{locale === "en-US" ? map.nameEn : map.nameZh}</strong>
                <p>{locale === "en-US" ? map.descriptionEn : map.descriptionZh}</p>
              </div>
              {map.enabled ? (
                <div className="work-map-summary">
                  <span>{locale === "en-US" ? "Active" : "进行中"} {map.summary.activePlots}</span>
                  <span>{locale === "en-US" ? "Care" : "需照看"} {map.summary.needsCarePlots}</span>
                  <span>{locale === "en-US" ? "Ready" : "可收获"} {map.summary.maturePlots}</span>
                </div>
              ) : (
                <div className="work-map-summary muted">
                  <span>{locale === "en-US" ? "Coming soon" : "筹备中"}</span>
                </div>
              )}
            </div>
          </button>
        ))}
      </div>
    </section>
  );
}

function statusLabel(status: WorkMapStatus, locale: LocaleCode) {
  const labels: Record<WorkMapStatus, { zh: string; en: string }> = {
    locked: { zh: "未开放", en: "Locked" },
    idle: { zh: "空闲", en: "Idle" },
    running: { zh: "进行中", en: "Running" },
    needsCare: { zh: "需要照看", en: "Needs Care" },
    claimable: { zh: "可收获", en: "Ready" },
  };
  const entry = labels[status] ?? labels.idle;
  return locale === "en-US" ? entry.en : entry.zh;
}
