import "./FarmWorkView.css";
import cropBlueberryOverlayImage from "@/assets/images/farm-layers/crop-blueberry-overlay.png";
import cropCarrotOverlayImage from "@/assets/images/farm-layers/crop-carrot-overlay.png";
import cropCornOverlayImage from "@/assets/images/farm-layers/crop-corn-overlay.png";
import cropPotatoOverlayImage from "@/assets/images/farm-layers/crop-potato-overlay.png";
import cropPumpkinOverlayImage from "@/assets/images/farm-layers/crop-pumpkin-overlay.png";
import cropStrawberryOverlayImage from "@/assets/images/farm-layers/crop-strawberry-overlay.png";
import cropTomatoOverlayImage from "@/assets/images/farm-layers/crop-tomato-overlay.png";
import cropWheatOverlayImage from "@/assets/images/farm-layers/crop-wheat-overlay.png";
import dogWalkSpritesheetImage from "@/assets/images/farm-layers/dog-patrol/dog-walk-spritesheet.png";
import farmBackgroundImage from "@/assets/images/farm-layers/farm-background.png";
import plotCarrotSoilImage from "@/assets/images/farm-layers/plot-carrot-soil.png";
import plotTomatoSoilImage from "@/assets/images/farm-layers/plot-tomato-soil.png";
import plotWheatSoilImage from "@/assets/images/farm-layers/plot-wheat-soil.png";
import { useRouter } from "@/app/router/RouterContext";
import { useFarmStore } from "@/features/farm-work/stores/useFarmStore";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { createLocalPetService } from "@/shared/services/tauri/pet";
import type { FarmCropConfig, FarmPlot, LocaleCode } from "@/shared/types/meeting";
import type { CSSProperties } from "react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";

const FARM_WORLD_SIZE = { width: 1536, height: 1024 };
const FARM_DEFAULT_VIEWPORT_SIZE = { width: 960, height: 560 };
const FARM_SCENE_EDGE_BLEED = 24;

type ScenePoint = { x: number; y: number };
type SceneViewportSize = { width: number; height: number };
type SceneView = { offset: ScenePoint; scale: number };

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}

function getCoverSceneScale(viewport: SceneViewportSize) {
  if (viewport.width <= 0 || viewport.height <= 0) {
    return 0.6;
  }
  return Math.max(
    (viewport.width + FARM_SCENE_EDGE_BLEED) / FARM_WORLD_SIZE.width,
    (viewport.height + FARM_SCENE_EDGE_BLEED) / FARM_WORLD_SIZE.height,
  );
}

function getMaxSceneScale(minScale: number) {
  return Math.max(1.12, minScale * 1.45, minScale + 0.28);
}

function getCenteredSceneOffset(scale: number, viewport: SceneViewportSize): ScenePoint {
  return {
    x: (viewport.width - FARM_WORLD_SIZE.width * scale) / 2,
    y: (viewport.height - FARM_WORLD_SIZE.height * scale) / 2,
  };
}

function clampSceneOffset(offset: ScenePoint, scale: number, viewport: SceneViewportSize): ScenePoint {
  if (viewport.width <= 0 || viewport.height <= 0) {
    return offset;
  }

  const scaledWidth = FARM_WORLD_SIZE.width * scale;
  const scaledHeight = FARM_WORLD_SIZE.height * scale;
  const centered = getCenteredSceneOffset(scale, viewport);

  return {
    x: scaledWidth <= viewport.width ? centered.x : clamp(offset.x, viewport.width - scaledWidth, 0),
    y: scaledHeight <= viewport.height ? centered.y : clamp(offset.y, viewport.height - scaledHeight, 0),
  };
}

function getElementViewport(element: HTMLElement | null): SceneViewportSize {
  if (!element) {
    return FARM_DEFAULT_VIEWPORT_SIZE;
  }
  const rect = element.getBoundingClientRect();
  return {
    width: element.clientWidth || rect.width || FARM_DEFAULT_VIEWPORT_SIZE.width,
    height: element.clientHeight || rect.height || FARM_DEFAULT_VIEWPORT_SIZE.height,
  };
}

const PLOT_HIT_AREAS: Record<number, { x: number; y: number; width: number; height: number }> = {
  1: { x: 210, y: 410, width: 330, height: 330 },
  2: { x: 505, y: 438, width: 330, height: 330 },
  3: { x: 800, y: 410, width: 330, height: 330 },
};

const PLOT_LAYERS: Record<number, { soil: string }> = {
  1: { soil: plotWheatSoilImage },
  2: { soil: plotCarrotSoilImage },
  3: { soil: plotTomatoSoilImage },
};

const CROP_LAYERS: Record<string, string> = {
  wheat: cropWheatOverlayImage,
  carrot: cropCarrotOverlayImage,
  tomato: cropTomatoOverlayImage,
  pumpkin: cropPumpkinOverlayImage,
  corn: cropCornOverlayImage,
  strawberry: cropStrawberryOverlayImage,
  blueberry: cropBlueberryOverlayImage,
  potato: cropPotatoOverlayImage,
};

const SUPPORTED_VISUAL_CROP_KEYS = new Set(Object.keys(CROP_LAYERS));
const petService = createLocalPetService();

type FarmEffect = {
  id: string;
  plotId: string;
  type: "plant" | "water" | "harvest";
};

type FarmDialog = {
  title: string;
  message: string;
  actionLabel?: string;
  actionRoute?: string;
};

type PlantDialog = {
  plotId: string;
};

export default function FarmWorkView() {
  const router = useRouter();
  const meetingStore = useMeetingStore();
  const farmStore = useFarmStore();
  const locale = meetingStore.settings.locale;
  const farmState = farmStore.farmState;
  const plots = farmState?.plots ?? [];
  const crops = (farmState?.crops ?? []).filter((crop) => SUPPORTED_VISUAL_CROP_KEYS.has(crop.cropKey));
  const [selectedPlotId, setSelectedPlotId] = useState<string>("");
  const [selectedCropKey, setSelectedCropKey] = useState<string>("");
  const [busyPlotId, setBusyPlotId] = useState<string>("");
  const [sceneViewport, setSceneViewport] = useState<SceneViewportSize>(FARM_DEFAULT_VIEWPORT_SIZE);
  const [sceneView, setSceneView] = useState<SceneView>(() => {
    const scale = getCoverSceneScale(FARM_DEFAULT_VIEWPORT_SIZE);
    return {
      offset: getCenteredSceneOffset(scale, FARM_DEFAULT_VIEWPORT_SIZE),
      scale,
    };
  });
  const [dragging, setDragging] = useState(false);
  const [effects, setEffects] = useState<FarmEffect[]>([]);
  const [seedInventory, setSeedInventory] = useState<Record<string, number>>({});
  const [farmDialog, setFarmDialog] = useState<FarmDialog | null>(null);
  const [plantDialog, setPlantDialog] = useState<PlantDialog | null>(null);
  const sceneViewportRef = useRef<HTMLElement | null>(null);
  const sceneWasAdjustedRef = useRef(false);
  const seedInventoryPromiseRef = useRef<Promise<void> | null>(null);
  const seedInventoryVersionRef = useRef(0);
  const dragStart = useRef({ pointerId: 0, x: 0, y: 0, offsetX: 0, offsetY: 0 });
  const sceneOffset = sceneView.offset;
  const sceneScale = sceneView.scale;
  const selectedPlot = plots.find((plot) => plot.id === selectedPlotId) ?? plots[0];

  useEffect(() => {
    let disposed = false;
    let polling = false;
    let rerunRequested = false;
    let timer: number | undefined;

    const schedule = () => {
      if (disposed || document.hidden) {
        return;
      }
      timer = window.setTimeout(() => void poll(false), 5_000);
    };
    const poll = async (initial: boolean) => {
      if (disposed || document.hidden) {
        return;
      }
      if (polling) {
        rerunRequested = true;
        return;
      }
      polling = true;
      try {
        await Promise.all([
          initial ? farmStore.loadFarmState(true) : farmStore.refresh(),
          loadSeedInventory(),
        ]);
      } catch (error) {
        if (!disposed && initial) {
          showFarmDialog(error instanceof Error ? error.message : String(error));
        }
      } finally {
        polling = false;
        if (rerunRequested && !disposed && !document.hidden) {
          rerunRequested = false;
          void poll(false);
        } else {
          schedule();
        }
      }
    };
    const handleVisibilityChange = () => {
      if (timer !== undefined) {
        window.clearTimeout(timer);
        timer = undefined;
      }
      if (!document.hidden) {
        void poll(false);
      }
    };

    document.addEventListener("visibilitychange", handleVisibilityChange);
    void poll(true);
    return () => {
      disposed = true;
      if (timer !== undefined) {
        window.clearTimeout(timer);
      }
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, []);

  useEffect(() => {
    if (!selectedPlotId && plots[0]) {
      setSelectedPlotId(plots[0].id);
    }
  }, [plots, selectedPlotId]);

  useEffect(() => {
    if (selectedCropKey && !crops.some((crop) => crop.cropKey === selectedCropKey)) {
      setSelectedCropKey("");
    }
  }, [crops, selectedCropKey]);

  useLayoutEffect(() => {
    const element = sceneViewportRef.current;
    if (!element) {
      return;
    }

    const syncViewport = () => {
      const nextViewport = getElementViewport(element);
      setSceneViewport(nextViewport);
      const coverScale = getCoverSceneScale(nextViewport);
      setSceneView((current) => {
        if (!sceneWasAdjustedRef.current) {
          return {
            scale: coverScale,
            offset: getCenteredSceneOffset(coverScale, nextViewport),
          };
        }

        const nextScale = clamp(current.scale, coverScale, getMaxSceneScale(coverScale));
        return {
          scale: nextScale,
          offset: clampSceneOffset(current.offset, nextScale, nextViewport),
        };
      });
    };

    syncViewport();
    const resizeObserver = new ResizeObserver(syncViewport);
    resizeObserver.observe(element);
    return () => resizeObserver.disconnect();
  }, []);

  const totals = useMemo(
    () => ({
      active: plots.filter((plot) => plot.status !== "empty").length,
      needsWater: plots.filter((plot) => plot.status === "needs_water").length,
      mature: plots.filter((plot) => plot.status === "mature").length,
    }),
    [plots],
  );

  function pushEffect(plotId: string, type: FarmEffect["type"]) {
    const effect = { id: `${type}-${plotId}-${Date.now()}`, plotId, type };
    setEffects((current) => [...current, effect]);
    window.setTimeout(() => {
      setEffects((current) => current.filter((item) => item.id !== effect.id));
    }, 1400);
  }

  function loadSeedInventory(forceAfterCurrent = false): Promise<void> {
    if (seedInventoryPromiseRef.current) {
      if (forceAfterCurrent) {
        return seedInventoryPromiseRef.current.then(() => loadSeedInventory());
      }
      return seedInventoryPromiseRef.current;
    }
    const requestVersion = seedInventoryVersionRef.current;
    const request = (async () => {
      try {
        const storeState = await petService.getStoreState();
        if (requestVersion === seedInventoryVersionRef.current) {
          setSeedInventory(Object.fromEntries(storeState.inventory.map((item) => [item.itemKey, item.quantity])));
        }
      } catch (error) {
        showFarmDialog(error instanceof Error ? error.message : String(error));
      } finally {
        seedInventoryPromiseRef.current = null;
      }
    })();
    seedInventoryPromiseRef.current = request;
    return request;
  }

  function showFarmDialog(message: string, title?: string, action?: Pick<FarmDialog, "actionLabel" | "actionRoute">) {
    setFarmDialog({
      title: title ?? (locale === "en-US" ? "Farm Notice" : "农场提示"),
      message,
      ...action,
    });
  }

  function showMissingSeedDialog(crop: FarmCropConfig) {
    showFarmDialog(
      locale === "en-US"
        ? `No ${crop.nameEn} seeds. Buy seeds in the pet store first, then return to plant.`
        : `缺少${crop.nameZh}种子。请先去宠物商店购买种子，再回到农场播种。`,
      locale === "en-US" ? "Seeds Required" : "缺少种子",
      {
        actionLabel: locale === "en-US" ? "Go Pet Store" : "去宠物商店",
        actionRoute: "/pet-store",
      },
    );
  }

  function showActionError(error: unknown) {
    showFarmDialog(
      error instanceof Error ? error.message : String(error),
      locale === "en-US" ? "Action Failed" : "操作失败",
    );
  }

  async function handleDialogAction() {
    const route = farmDialog?.actionRoute;
    setFarmDialog(null);
    if (route) {
      await router.push(route);
    }
  }

  function hasSeed(crop: FarmCropConfig) {
    return (seedInventory[crop.seedItemKey] ?? 0) > 0;
  }

  async function refreshSeedInventoryAfterPlant() {
    await loadSeedInventory(true);
  }

  function openPlantDialog(plot: FarmPlot) {
    if (plot.status !== "empty") {
      return;
    }
    setSelectedPlotId(plot.id);
    setPlantDialog({ plotId: plot.id });
  }

  async function plantCropInPlot(plotId: string, crop: FarmCropConfig) {
    const plot = plots.find((item) => item.id === plotId);
    if (!plot || plot.status !== "empty") {
      return;
    }
    if (!hasSeed(crop)) {
      showMissingSeedDialog(crop);
      return;
    }
    setBusyPlotId(plot.id);
    setFarmDialog(null);
    setPlantDialog(null);
    setSelectedCropKey(crop.cropKey);
    seedInventoryVersionRef.current += 1;
    try {
      await farmStore.plantCrop(plot.id, crop.cropKey);
      await refreshSeedInventoryAfterPlant();
      pushEffect(plot.id, "plant");
    } catch (error) {
      showActionError(error);
    } finally {
      setBusyPlotId("");
    }
  }

  async function waterSelectedPlot() {
    if (!selectedPlot || selectedPlot.status !== "needs_water") {
      return;
    }
    setBusyPlotId(selectedPlot.id);
    setFarmDialog(null);
    try {
      await farmStore.waterPlot(selectedPlot.id);
      pushEffect(selectedPlot.id, "water");
    } catch (error) {
      showActionError(error);
    } finally {
      setBusyPlotId("");
    }
  }

  async function harvestSelectedPlot() {
    if (!selectedPlot || selectedPlot.status !== "mature") {
      return;
    }
    setBusyPlotId(selectedPlot.id);
    setFarmDialog(null);
    seedInventoryVersionRef.current += 1;
    try {
      await farmStore.harvestPlot(selectedPlot.id);
      pushEffect(selectedPlot.id, "harvest");
      await loadSeedInventory(true);
    } catch (error) {
      showActionError(error);
    } finally {
      setBusyPlotId("");
    }
  }

  async function runPlotAction(plot: FarmPlot) {
    setSelectedPlotId(plot.id);
    if (busyPlotId) {
      return;
    }
    if (plot.status === "empty") {
      openPlantDialog(plot);
      return;
    }
    if (plot.status === "needs_water") {
      setBusyPlotId(plot.id);
      setFarmDialog(null);
      try {
        await farmStore.waterPlot(plot.id);
        pushEffect(plot.id, "water");
      } catch (error) {
        showActionError(error);
      } finally {
        setBusyPlotId("");
      }
      return;
    }
    if (plot.status === "mature") {
      setBusyPlotId(plot.id);
      setFarmDialog(null);
      seedInventoryVersionRef.current += 1;
      try {
        await farmStore.harvestPlot(plot.id);
        pushEffect(plot.id, "harvest");
        await loadSeedInventory(true);
      } catch (error) {
        showActionError(error);
      } finally {
        setBusyPlotId("");
      }
    }
  }

  function handleScenePointerDown(event: React.PointerEvent<HTMLDivElement>) {
    if ((event.target as HTMLElement).closest("button")) {
      return;
    }
    event.currentTarget.setPointerCapture(event.pointerId);
    dragStart.current = {
      pointerId: event.pointerId,
      x: event.clientX,
      y: event.clientY,
      offsetX: sceneOffset.x,
      offsetY: sceneOffset.y,
    };
    setDragging(true);
  }

  function handleScenePointerMove(event: React.PointerEvent<HTMLDivElement>) {
    if (!dragging || dragStart.current.pointerId !== event.pointerId) {
      return;
    }
    const nextX = dragStart.current.offsetX + event.clientX - dragStart.current.x;
    const nextY = dragStart.current.offsetY + event.clientY - dragStart.current.y;
    sceneWasAdjustedRef.current = true;
    setSceneView((current) => ({
      ...current,
      offset: clampSceneOffset({ x: nextX, y: nextY }, current.scale, sceneViewport),
    }));
  }

  function handleScenePointerLeave(event: React.PointerEvent<HTMLDivElement>) {
    if (dragStart.current.pointerId === event.pointerId) {
      setDragging(false);
    }
  }

  function getSceneViewportPointer(event: React.PointerEvent<HTMLDivElement> | React.WheelEvent<HTMLDivElement>) {
    const rect = event.currentTarget.getBoundingClientRect();
    return {
      x: event.clientX - rect.left,
      y: event.clientY - rect.top,
    };
  }

  function zoomScene(nextScale: number, anchor: ScenePoint) {
    sceneWasAdjustedRef.current = true;
    setSceneView((current) => {
      const minScale = getCoverSceneScale(sceneViewport);
      const scale = clamp(nextScale, minScale, getMaxSceneScale(minScale));
      const scaleRatio = scale / current.scale;
      const offset = {
        x: anchor.x - (anchor.x - current.offset.x) * scaleRatio,
        y: anchor.y - (anchor.y - current.offset.y) * scaleRatio,
      };
      return {
        scale,
        offset: clampSceneOffset(offset, scale, sceneViewport),
      };
    });
  }

  function handleScenePointerUp(event: React.PointerEvent<HTMLDivElement>) {
    if (dragStart.current.pointerId === event.pointerId) {
      setDragging(false);
    }
  }

  function handleSceneWheel(event: React.WheelEvent<HTMLDivElement>) {
    event.preventDefault();
    zoomScene(sceneScale - event.deltaY * 0.0008, getSceneViewportPointer(event));
  }

  function focusPlot(plot: FarmPlot) {
    setSelectedPlotId(plot.id);
    const position = PLOT_HIT_AREAS[plot.plotIndex] ?? { x: 760, y: 512, width: 330, height: 330 };
    sceneWasAdjustedRef.current = true;
    setSceneView((current) => {
      const minScale = getCoverSceneScale(sceneViewport);
      const scale = clamp(current.scale, minScale, getMaxSceneScale(minScale));
      const plotCenter = {
        x: position.x + position.width / 2,
        y: position.y + position.height / 2,
      };
      const offset = {
        x: sceneViewport.width / 2 - plotCenter.x * scale,
        y: sceneViewport.height / 2 - plotCenter.y * scale,
      };
      return {
        scale,
        offset: clampSceneOffset(offset, scale, sceneViewport),
      };
    });
  }

  function handlePlotClick(plot: FarmPlot) {
    focusPlot(plot);
    if (plot.status === "planted") {
      return;
    }
    void runPlotAction(plot);
  }

  return (
    <section className="view-stack native-page farm-work-page">
      {farmDialog && (
        <div className="farm-dialog-backdrop" role="presentation" onClick={() => setFarmDialog(null)}>
          <div className="farm-dialog" role="dialog" aria-modal="true" aria-labelledby="farm-dialog-title" onClick={(event) => event.stopPropagation()}>
            <div className="farm-dialog-heading">
              <span className="eyebrow">{locale === "en-US" ? "Farm" : "农场"}</span>
              <h3 id="farm-dialog-title">{farmDialog.title}</h3>
            </div>
            <p>{farmDialog.message}</p>
            <div className="farm-dialog-actions">
              <button className="text-button" type="button" onClick={() => setFarmDialog(null)}>
                {locale === "en-US" ? "Close" : "关闭"}
              </button>
              {farmDialog.actionLabel && (
                <button className="primary-button" type="button" onClick={() => void handleDialogAction()}>
                  {farmDialog.actionLabel}
                </button>
              )}
            </div>
          </div>
        </div>
      )}

      {plantDialog && (
        <div className="farm-dialog-backdrop" role="presentation" onClick={() => setPlantDialog(null)}>
          <div className="farm-dialog farm-plant-dialog" role="dialog" aria-modal="true" aria-labelledby="farm-plant-dialog-title" onClick={(event) => event.stopPropagation()}>
            <div className="farm-dialog-heading">
              <span className="eyebrow">{locale === "en-US" ? "Planting" : "播种"}</span>
              <h3 id="farm-plant-dialog-title">{locale === "en-US" ? "Choose a Crop" : "选择要种植的农作物"}</h3>
            </div>
            <p>
              {locale === "en-US"
                ? "Pick one crop to plant in this empty plot. Seeds are consumed when planting succeeds."
                : "请选择要在这块空地种植的农作物。播种成功后会消耗对应种子。"}
            </p>
            <div className="farm-plant-choice-list">
              {crops.map((crop) => {
                const seedCount = seedInventory[crop.seedItemKey] ?? 0;
                const unavailable = seedCount <= 0 || busyPlotId === plantDialog.plotId;
                return (
                  <button
                    key={crop.cropKey}
                    className={`farm-plant-choice ${seedCount <= 0 ? "unavailable" : ""}`}
                    type="button"
                    disabled={unavailable}
                    onClick={() => void plantCropInPlot(plantDialog.plotId, crop)}
                  >
                    <span>
                      <strong>{cropName(crop, locale)}</strong>
                      <small>{formatDuration(crop.durationSeconds, locale)} · {crop.waterRequired} {locale === "en-US" ? "waters" : "次浇水"}</small>
                    </span>
                    <em>{locale === "en-US" ? `Seeds x${seedCount}` : `种子 ×${seedCount}`}</em>
                  </button>
                );
              })}
            </div>
            <div className="farm-dialog-actions">
              <button className="text-button" type="button" onClick={() => setPlantDialog(null)}>
                {locale === "en-US" ? "Cancel" : "取消"}
              </button>
              <button className="primary-button" type="button" onClick={() => void router.push("/pet-store")}>
                {locale === "en-US" ? "Buy Seeds" : "购买种子"}
              </button>
            </div>
          </div>
        </div>
      )}

      <header className="farm-work-hero">
        <div>
          <button className="text-button small-button native-back-link" type="button" onClick={() => void router.push("/work-market")}>
            {locale === "en-US" ? "Back" : "返回牛马市场"}
          </button>
          <h3>{locale === "en-US" ? "Farm" : "农场种菜"}</h3>
        </div>
        <div className="farm-work-stat-strip">
          <span>{locale === "en-US" ? "Active" : "进行中"} <strong>{totals.active}</strong></span>
          <span>{locale === "en-US" ? "Care" : "需浇水"} <strong>{totals.needsWater}</strong></span>
          <span>{locale === "en-US" ? "Ready" : "可收获"} <strong>{totals.mature}</strong></span>
        </div>
      </header>

      <div className="farm-game-shell">
        <article
          ref={sceneViewportRef}
          className={`farm-scene-viewport ${dragging ? "is-dragging" : ""}`}
          onPointerDown={handleScenePointerDown}
          onPointerMove={handleScenePointerMove}
          onPointerUp={handleScenePointerUp}
          onPointerCancel={handleScenePointerUp}
          onPointerLeave={handleScenePointerLeave}
          onWheel={handleSceneWheel}
        >
          <div
            className="farm-scene-world"
            style={{ transform: `translate3d(${sceneOffset.x}px, ${sceneOffset.y}px, 0) scale(${sceneScale})` }}
          >
            <img
              className="farm-scene-backdrop"
              src={farmBackgroundImage}
              alt=""
              draggable="false"
              width={FARM_WORLD_SIZE.width}
              height={FARM_WORLD_SIZE.height}
            />
            <div className="farm-patrol-dog" aria-hidden="true">
              <span className="farm-patrol-dog-facing">
                <span
                  className="farm-patrol-dog-sprite"
                  style={{ backgroundImage: `url(${dogWalkSpritesheetImage})` }}
                />
              </span>
            </div>
            {plots.map((plot) => {
              const area = PLOT_HIT_AREAS[plot.plotIndex] ?? { x: 360 + plot.plotIndex * 360, y: 430, width: 330, height: 330 };
              const layer = PLOT_LAYERS[plot.plotIndex];
              const cropLayer = plot.status === "empty" || !plot.cropKey ? undefined : CROP_LAYERS[plot.cropKey];
              const plotEffects = effects.filter((effect) => effect.plotId === plot.id);
              return (
                <button
                  key={plot.id}
                  className={`farm-map-plot farm-map-plot-${plot.status} ${selectedPlot?.id === plot.id ? "selected" : ""}`}
                  style={{
                    "--plot-x": `${area.x}px`,
                    "--plot-y": `${area.y}px`,
                    "--plot-width": `${area.width}px`,
                    "--plot-height": `${area.height}px`,
                  } as CSSProperties}
                  type="button"
                  onClick={() => handlePlotClick(plot)}
                  aria-label={`${locale === "en-US" ? "Plot" : "地块"} ${plot.plotIndex}: ${plotStatusLabel(plot, locale)}`}
                >
                  {layer?.soil && (
                    <img
                      className="farm-plot-art-layer farm-plot-soil-layer"
                      src={layer.soil}
                      alt=""
                      draggable="false"
                      width={area.width}
                      height={area.height}
                    />
                  )}
                  {cropLayer && (
                    <img
                      className={`farm-plot-art-layer farm-plot-crop-layer farm-plot-crop-layer-${cropStage(plot)}`}
                      src={cropLayer}
                      alt=""
                      draggable="false"
                      width={area.width}
                      height={area.height}
                    />
                  )}
                  <span className="farm-map-plot-label">
                    <strong>{plot.crop ? cropName(plot.crop, locale) : (locale === "en-US" ? "Empty" : "空地")}</strong>
                    <em>{plotStatusLabel(plot, locale)}</em>
                  </span>
                  <span className="farm-map-progress">
                    <span style={{ width: `${Math.round((plot.progressRatio ?? 0) * 100)}%` }} />
                  </span>
                  {plotEffects.map((effect) => (
                    <span key={effect.id} className={`farm-effect farm-effect-${effect.type}`} aria-hidden="true" />
                  ))}
                  {plot.status !== "planted" && (
                    <span className="farm-plot-actions" aria-hidden="true">
                      {busyPlotId === plot.id && (locale === "en-US" ? "Working" : "处理中")}
                      {busyPlotId !== plot.id && plot.status === "empty" && (locale === "en-US" ? "Plant" : "播种")}
                      {busyPlotId !== plot.id && plot.status === "needs_water" && (locale === "en-US" ? "Water" : "浇水")}
                      {busyPlotId !== plot.id && plot.status === "mature" && (locale === "en-US" ? "Harvest" : "收获")}
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        </article>

        <aside className="farm-game-hud">
          <article className="farm-panel farm-action-panel">
            <div className="section-heading">
              <div>
                <span className="eyebrow">{locale === "en-US" ? "Selected Plot" : "当前地块"}</span>
                <h3>{selectedPlot ? `${locale === "en-US" ? "Plot" : "地块"} ${selectedPlot.plotIndex}` : locale === "en-US" ? "No plot" : "暂无地块"}</h3>
              </div>
            </div>

            {selectedPlot ? (
              <div className="farm-action-stack">
                <div className="farm-selected-status">
                  <strong>{plotStatusLabel(selectedPlot, locale)}</strong>
                  <span>{selectedPlot.crop ? cropName(selectedPlot.crop, locale) : locale === "en-US" ? "Choose a crop to plant." : "选择作物后即可播种。"}</span>
                </div>

                {selectedPlot.status === "empty" && (
                  <button className="primary-button farm-main-action" type="button" disabled={busyPlotId === selectedPlot.id} onClick={() => openPlantDialog(selectedPlot)}>
                    {locale === "en-US" ? "Choose Crop" : "选择农作物"}
                  </button>
                )}

                {selectedPlot.status === "planted" && (
                  <button className="secondary-button farm-main-action" type="button" disabled>
                    {locale === "en-US" ? "Next care in " : "距离下次照看 "}
                    {formatSeconds(selectedPlot.remainingSeconds, locale)}
                  </button>
                )}

                {selectedPlot.status === "needs_water" && (
                  <button className="primary-button farm-main-action" type="button" disabled={busyPlotId === selectedPlot.id} onClick={waterSelectedPlot}>
                    {busyPlotId === selectedPlot.id ? (locale === "en-US" ? "Watering" : "浇水中") : locale === "en-US" ? "Water" : "浇水"}
                  </button>
                )}

                {selectedPlot.status === "mature" && (
                  <button className="primary-button farm-main-action" type="button" disabled={busyPlotId === selectedPlot.id} onClick={harvestSelectedPlot}>
                    {busyPlotId === selectedPlot.id ? (locale === "en-US" ? "Harvesting" : "收获中") : locale === "en-US" ? "Harvest" : "收获"}
                  </button>
                )}
              </div>
            ) : (
              <div className="empty-state">{locale === "en-US" ? "No farm plots yet." : "暂无农场地块。"}</div>
            )}
          </article>
        </aside>
      </div>
    </section>
  );
}

function cropStage(plot: FarmPlot) {
  if (plot.status === "empty") {
    return "empty";
  }
  if (plot.status === "mature") {
    return "mature";
  }
  if (plot.status === "needs_water") {
    return "care";
  }
  if ((plot.progressRatio ?? 0) > 0.62) {
    return "grown";
  }
  return "sprout";
}

function cropName(crop: FarmCropConfig, locale: LocaleCode) {
  return locale === "en-US" ? crop.nameEn : crop.nameZh;
}

function plotStatusLabel(plot: FarmPlot, locale: LocaleCode) {
  if (locale === "en-US") {
    switch (plot.status) {
      case "planted":
        return "Growing";
      case "needs_water":
        return "Needs water";
      case "mature":
        return "Ready";
      default:
        return "Empty";
    }
  }
  switch (plot.status) {
    case "planted":
      return "成长中";
    case "needs_water":
      return "需要浇水";
    case "mature":
      return "可收获";
    default:
      return "空地";
  }
}

function formatDuration(seconds: number, locale: LocaleCode) {
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) {
    return locale === "en-US" ? `${minutes} min` : `${minutes} 分钟`;
  }
  return locale === "en-US" ? `${Math.round(minutes / 60)} hr` : `${Math.round(minutes / 60)} 小时`;
}

function formatSeconds(seconds: number, locale: LocaleCode) {
  if (seconds <= 0) {
    return locale === "en-US" ? "soon" : "即将到达";
  }
  const minutes = Math.ceil(seconds / 60);
  return locale === "en-US" ? `${minutes} min` : `${minutes} 分钟`;
}
