import "./WorkGameView.css";
import mineMapImage from "@/assets/images/work-maps/mine-map.webp";
import factoryMapImage from "@/assets/images/work-maps/factory-map.webp";
import convenienceStoreMapImage from "@/assets/images/work-maps/convenience-store-map.webp";
import mineWorkerShallowImage from "@/assets/images/work-maps/content/mine-worker-shallow-content.png";
import mineWorkerDeepImage from "@/assets/images/work-maps/content/mine-worker-deep-content.png";
import mineWorkerGlowingImage from "@/assets/images/work-maps/content/mine-worker-glowing-content.png";
import mineWorkerStandardFrame1 from "@/assets/images/work-maps/content/mine-worker-standard-frame-1.png";
import mineWorkerStandardFrame2 from "@/assets/images/work-maps/content/mine-worker-standard-frame-2.png";
import mineWorkerStandardFrame3 from "@/assets/images/work-maps/content/mine-worker-standard-frame-3.png";
import mineWorkerStandardFrame4 from "@/assets/images/work-maps/content/mine-worker-standard-frame-4.png";
import mineWorkerStandardFrame5 from "@/assets/images/work-maps/content/mine-worker-standard-frame-5.png";
import mineWorkerStandardFrame6 from "@/assets/images/work-maps/content/mine-worker-standard-frame-6.png";
import mineWorkerStandardFrame7 from "@/assets/images/work-maps/content/mine-worker-standard-frame-7.png";
import mineWorkerStandardFrame8 from "@/assets/images/work-maps/content/mine-worker-standard-frame-8.png";
import mineWorkerStandardFrame9 from "@/assets/images/work-maps/content/mine-worker-standard-frame-9.png";
import factoryWorkerStandardFrame1 from "@/assets/images/work-maps/content/factory-worker-standard-frame-1.png";
import factoryWorkerStandardFrame2 from "@/assets/images/work-maps/content/factory-worker-standard-frame-2.png";
import factoryWorkerStandardFrame3 from "@/assets/images/work-maps/content/factory-worker-standard-frame-3.png";
import factoryWorkerStandardFrame4 from "@/assets/images/work-maps/content/factory-worker-standard-frame-4.png";
import factoryWorkerStandardFrame5 from "@/assets/images/work-maps/content/factory-worker-standard-frame-5.png";
import factoryWorkerStandardFrame6 from "@/assets/images/work-maps/content/factory-worker-standard-frame-6.png";
import factoryWorkerStandardFrame7 from "@/assets/images/work-maps/content/factory-worker-standard-frame-7.png";
import factoryWorkerStandardFrame8 from "@/assets/images/work-maps/content/factory-worker-standard-frame-8.png";
import factoryWorkerStandardFrame9 from "@/assets/images/work-maps/content/factory-worker-standard-frame-9.png";
import storeWorkerStandardFrame1 from "@/assets/images/work-maps/content/store-worker-standard-frame-1.png";
import storeWorkerStandardFrame2 from "@/assets/images/work-maps/content/store-worker-standard-frame-2.png";
import storeWorkerStandardFrame3 from "@/assets/images/work-maps/content/store-worker-standard-frame-3.png";
import storeWorkerStandardFrame4 from "@/assets/images/work-maps/content/store-worker-standard-frame-4.png";
import storeWorkerStandardFrame5 from "@/assets/images/work-maps/content/store-worker-standard-frame-5.png";
import storeWorkerStandardFrame6 from "@/assets/images/work-maps/content/store-worker-standard-frame-6.png";
import storeWorkerStandardFrame7 from "@/assets/images/work-maps/content/store-worker-standard-frame-7.png";
import storeWorkerStandardFrame8 from "@/assets/images/work-maps/content/store-worker-standard-frame-8.png";
import storeWorkerStandardFrame9 from "@/assets/images/work-maps/content/store-worker-standard-frame-9.png";
import factoryBasicAssemblyImage from "@/assets/images/work-maps/content/factory-basic-assembly-content.png";
import factoryRushOrderImage from "@/assets/images/work-maps/content/factory-rush-order-content.png";
import factoryPrecisionCheckImage from "@/assets/images/work-maps/content/factory-precision-check-content.png";
import storeDayShiftImage from "@/assets/images/work-maps/content/store-day-shift-content.png";
import storeEveningShiftImage from "@/assets/images/work-maps/content/store-evening-shift-content.png";
import storeNightShiftImage from "@/assets/images/work-maps/content/store-night-shift-content.png";
import { useRouter } from "@/app/router/RouterContext";
import { useMeetingStore } from "@/features/meeting/stores/useMeetingStore";
import { useWorkGameStore } from "@/features/work-game/stores/useWorkGameStore";
import type { LocaleCode, WorkGameJobConfig, WorkGameRewardLedgerEntry, WorkGameTask, WorkMapStatus } from "@/shared/types/meeting";
import type { CSSProperties } from "react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";

const GAME_ASSETS: Record<string, string> = {
  mine: mineMapImage,
  factory: factoryMapImage,
  "convenience-store": convenienceStoreMapImage,
};

const JOB_CONTENT_ASSETS: Record<string, string> = {
  "mine:shallow-vein": mineWorkerShallowImage,
  "mine:deep-vein": mineWorkerDeepImage,
  "mine:glowing-vein": mineWorkerGlowingImage,
  "factory:basic-assembly": factoryBasicAssemblyImage,
  "factory:rush-order": factoryRushOrderImage,
  "factory:precision-check": factoryPrecisionCheckImage,
  "convenience-store:day-shift": storeDayShiftImage,
  "convenience-store:evening-shift": storeEveningShiftImage,
  "convenience-store:night-shift": storeNightShiftImage,
};

const MINE_WORKER_FRAMES = [
  mineWorkerStandardFrame1,
  mineWorkerStandardFrame2,
  mineWorkerStandardFrame3,
  mineWorkerStandardFrame4,
  mineWorkerStandardFrame5,
  mineWorkerStandardFrame6,
  mineWorkerStandardFrame7,
  mineWorkerStandardFrame8,
  mineWorkerStandardFrame9,
];

const FACTORY_WORKER_FRAMES = [
  factoryWorkerStandardFrame1,
  factoryWorkerStandardFrame2,
  factoryWorkerStandardFrame3,
  factoryWorkerStandardFrame4,
  factoryWorkerStandardFrame5,
  factoryWorkerStandardFrame6,
  factoryWorkerStandardFrame7,
  factoryWorkerStandardFrame8,
  factoryWorkerStandardFrame9,
];

const STORE_WORKER_FRAMES = [
  storeWorkerStandardFrame1,
  storeWorkerStandardFrame2,
  storeWorkerStandardFrame3,
  storeWorkerStandardFrame4,
  storeWorkerStandardFrame5,
  storeWorkerStandardFrame6,
  storeWorkerStandardFrame7,
  storeWorkerStandardFrame8,
  storeWorkerStandardFrame9,
];

const JOB_FRAME_ASSETS: Record<string, string[]> = {
  "mine:shallow-vein": MINE_WORKER_FRAMES,
  "mine:deep-vein": MINE_WORKER_FRAMES,
  "mine:glowing-vein": MINE_WORKER_FRAMES,
  "factory:basic-assembly": FACTORY_WORKER_FRAMES,
  "factory:rush-order": FACTORY_WORKER_FRAMES,
  "factory:precision-check": FACTORY_WORKER_FRAMES,
  "convenience-store:day-shift": STORE_WORKER_FRAMES,
  "convenience-store:evening-shift": STORE_WORKER_FRAMES,
  "convenience-store:night-shift": STORE_WORKER_FRAMES,
};

const JOB_FRAME_OFFSETS: Record<string, number> = {
  "mine:shallow-vein": 0,
  "mine:deep-vein": 3,
  "mine:glowing-vein": 6,
  "factory:basic-assembly": 0,
  "factory:rush-order": 3,
  "factory:precision-check": 6,
  "convenience-store:day-shift": 0,
  "convenience-store:evening-shift": 3,
  "convenience-store:night-shift": 6,
};

const GAME_WORLD_SIZE = { width: 1536, height: 1024 };
const GAME_DEFAULT_VIEWPORT_SIZE = { width: 960, height: 560 };
const GAME_SCENE_EDGE_BLEED = 24;

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
    (viewport.width + GAME_SCENE_EDGE_BLEED) / GAME_WORLD_SIZE.width,
    (viewport.height + GAME_SCENE_EDGE_BLEED) / GAME_WORLD_SIZE.height,
  );
}

function getMaxSceneScale(minScale: number) {
  return Math.max(1.12, minScale * 1.45, minScale + 0.28);
}

function getCenteredSceneOffset(scale: number, viewport: SceneViewportSize): ScenePoint {
  return {
    x: (viewport.width - GAME_WORLD_SIZE.width * scale) / 2,
    y: (viewport.height - GAME_WORLD_SIZE.height * scale) / 2,
  };
}

function clampSceneOffset(offset: ScenePoint, scale: number, viewport: SceneViewportSize): ScenePoint {
  if (viewport.width <= 0 || viewport.height <= 0) {
    return offset;
  }

  const scaledWidth = GAME_WORLD_SIZE.width * scale;
  const scaledHeight = GAME_WORLD_SIZE.height * scale;
  const centered = getCenteredSceneOffset(scale, viewport);

  return {
    x: scaledWidth <= viewport.width ? centered.x : clamp(offset.x, viewport.width - scaledWidth, 0),
    y: scaledHeight <= viewport.height ? centered.y : clamp(offset.y, viewport.height - scaledHeight, 0),
  };
}

function getElementViewport(element: HTMLElement | null): SceneViewportSize {
  if (!element) {
    return GAME_DEFAULT_VIEWPORT_SIZE;
  }
  const rect = element.getBoundingClientRect();
  return {
    width: element.clientWidth || rect.width || GAME_DEFAULT_VIEWPORT_SIZE.width,
    height: element.clientHeight || rect.height || GAME_DEFAULT_VIEWPORT_SIZE.height,
  };
}

type Hotspot = {
  x: number;
  y: number;
  width: number;
  height: number;
  tone: "cool" | "warm" | "mint";
};

const HOTSPOTS: Record<string, Record<number, Hotspot>> = {
  mine: {
    1: { x: 390, y: 392, width: 190, height: 230, tone: "cool" },
    2: { x: 1012, y: 438, width: 190, height: 232, tone: "warm" },
    3: { x: 652, y: 620, width: 220, height: 234, tone: "mint" },
  },
  factory: {
    1: { x: 465, y: 230, width: 166, height: 188, tone: "warm" },
    2: { x: 565, y: 338, width: 166, height: 188, tone: "mint" },
    3: { x: 749, y: 170, width: 166, height: 188, tone: "cool" },
  },
  "convenience-store": {
    1: { x: 459, y: 336, width: 166, height: 188, tone: "warm" },
    2: { x: 739, y: 391, width: 166, height: 188, tone: "mint" },
    3: { x: 1031, y: 386, width: 166, height: 188, tone: "cool" },
  },
};

type WorkGameNotice = {
  title: string;
  message: string;
};

export default function WorkGameView() {
  const router = useRouter();
  const gameKey = router.params.gameKey ?? "mine";
  const meetingStore = useMeetingStore();
  const locale = meetingStore.settings.locale;
  const workGameStore = useWorkGameStore(gameKey);
  const gameState = workGameStore.state;
  const tasks = gameState?.tasks ?? [];
  const [selectedTaskId, setSelectedTaskId] = useState("");
  const [busyTaskId, setBusyTaskId] = useState("");
  const [notice, setNotice] = useState<WorkGameNotice | null>(null);
  const [effectTaskId, setEffectTaskId] = useState("");
  const [animationFrame, setAnimationFrame] = useState(0);
  const [sceneViewport, setSceneViewport] = useState<SceneViewportSize>(GAME_DEFAULT_VIEWPORT_SIZE);
  const [sceneView, setSceneView] = useState<SceneView>(() => {
    const scale = getCoverSceneScale(GAME_DEFAULT_VIEWPORT_SIZE);
    return {
      offset: getCenteredSceneOffset(scale, GAME_DEFAULT_VIEWPORT_SIZE),
      scale,
    };
  });
  const [dragging, setDragging] = useState(false);
  const sceneViewportRef = useRef<HTMLElement | null>(null);
  const sceneWasAdjustedRef = useRef(false);
  const dragStart = useRef({ pointerId: 0, x: 0, y: 0, offsetX: 0, offsetY: 0 });
  const selectedTask = tasks.find((task) => task.id === selectedTaskId)
    ?? tasks.find((task) => task.status === "claimable")
    ?? tasks.find((task) => task.status === "needsCare")
    ?? tasks[0];
  const mapImage = GAME_ASSETS[gameKey];
  const sceneOffset = sceneView.offset;
  const sceneScale = sceneView.scale;
  const totals = useMemo(
    () => ({
      active: tasks.filter((task) => task.status !== "idle").length,
      needsCare: tasks.filter((task) => task.status === "needsCare").length,
      claimable: tasks.filter((task) => task.status === "claimable").length,
    }),
    [tasks],
  );

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
        if (initial) {
          await workGameStore.loadGameState(gameKey, true);
        } else {
          await workGameStore.refresh(gameKey);
        }
      } catch (error) {
        if (!disposed && initial) {
          setNotice({
            title: locale === "en-US" ? "Map unavailable" : "地图不可用",
            message: error instanceof Error ? error.message : String(error),
          });
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
  }, [gameKey, locale]);

  useEffect(() => {
    if (!selectedTaskId && selectedTask) {
      setSelectedTaskId(selectedTask.id);
    }
    if (selectedTaskId && tasks.length && !tasks.some((task) => task.id === selectedTaskId)) {
      setSelectedTaskId(tasks[0].id);
    }
  }, [selectedTaskId, selectedTask, tasks]);

  useEffect(() => {
    const timer = window.setInterval(() => {
      setAnimationFrame((frame) => (frame + 1) % 9);
    }, 170);
    return () => window.clearInterval(timer);
  }, []);

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
  }, [gameKey]);

  function showError(error: unknown) {
    setNotice({
      title: locale === "en-US" ? "Action failed" : "操作失败",
      message: error instanceof Error ? error.message : String(error),
    });
  }

  function triggerEffect(taskId: string) {
    setEffectTaskId(taskId);
    window.setTimeout(() => {
      setEffectTaskId((current) => current === taskId ? "" : current);
    }, 1200);
  }

  async function startTask(task: WorkGameTask) {
    const jobKey = task.job?.jobKey || gameState?.jobs.find((job) => job.slotIndex === task.slotIndex)?.jobKey;
    if (!jobKey || busyTaskId) {
      return;
    }
    setBusyTaskId(task.id);
    setNotice(null);
    try {
      await workGameStore.startTask(gameKey, task.id, jobKey);
      triggerEffect(task.id);
    } catch (error) {
      showError(error);
    } finally {
      setBusyTaskId("");
    }
  }

  async function careTask(task: WorkGameTask) {
    if (busyTaskId) {
      return;
    }
    setBusyTaskId(task.id);
    setNotice(null);
    try {
      await workGameStore.careTask(gameKey, task.id);
      triggerEffect(task.id);
    } catch (error) {
      showError(error);
    } finally {
      setBusyTaskId("");
    }
  }

  async function claimTask(task: WorkGameTask) {
    if (busyTaskId) {
      return;
    }
    setBusyTaskId(task.id);
    setNotice(null);
    try {
      const result = await workGameStore.claimTask(gameKey, task.id);
      triggerEffect(task.id);
      setNotice({
        title: locale === "en-US" ? "Reward claimed" : "奖励已领取",
        message: rewardLine(result.reward, locale),
      });
    } catch (error) {
      showError(error);
    } finally {
      setBusyTaskId("");
    }
  }

  function runTaskAction(task: WorkGameTask) {
    setSelectedTaskId(task.id);
    focusTask(task);
    if (task.status === "idle") {
      void startTask(task);
      return;
    }
    if (task.status === "needsCare") {
      void careTask(task);
      return;
    }
    if (task.status === "claimable") {
      void claimTask(task);
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

  function handleScenePointerUp(event: React.PointerEvent<HTMLDivElement>) {
    if (dragStart.current.pointerId === event.pointerId) {
      setDragging(false);
    }
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

  function handleSceneWheel(event: React.WheelEvent<HTMLDivElement>) {
    event.preventDefault();
    zoomScene(sceneScale - event.deltaY * 0.0008, getSceneViewportPointer(event));
  }

  function focusTask(task: WorkGameTask) {
    setSelectedTaskId(task.id);
    const position = hotspotForTask(gameKey, task);
    sceneWasAdjustedRef.current = true;
    setSceneView((current) => {
      const minScale = getCoverSceneScale(sceneViewport);
      const scale = clamp(current.scale, minScale, getMaxSceneScale(minScale));
      const taskCenter = {
        x: position.x + position.width / 2,
        y: position.y + position.height / 2,
      };
      const offset = {
        x: sceneViewport.width / 2 - taskCenter.x * scale,
        y: sceneViewport.height / 2 - taskCenter.y * scale,
      };
      return {
        scale,
        offset: clampSceneOffset(offset, scale, sceneViewport),
      };
    });
  }

  if (!mapImage) {
    return (
      <section className="view-stack native-page work-game-page">
        <article className="surface work-game-empty">
          <span className="eyebrow">{locale === "en-US" ? "Work Game" : "打工小游戏"}</span>
          <h3>{locale === "en-US" ? "Map not found" : "地图不存在"}</h3>
          <button className="primary-button" type="button" onClick={() => void router.push("/work-market")}>
            {locale === "en-US" ? "Back to Market" : "返回牛马市场"}
          </button>
        </article>
      </section>
    );
  }

  return (
    <section className={`view-stack native-page work-game-page work-game-${gameKey}`}>
      {notice && (
        <div className="work-game-dialog-backdrop" role="presentation" onClick={() => setNotice(null)}>
          <div className="work-game-dialog" role="dialog" aria-modal="true" aria-labelledby="work-game-dialog-title" onClick={(event) => event.stopPropagation()}>
            <span className="eyebrow">{locale === "en-US" ? "Work Game" : "打工小游戏"}</span>
            <h3 id="work-game-dialog-title">{notice.title}</h3>
            <p>{notice.message}</p>
            <div className="work-game-dialog-actions">
              <button className="primary-button" type="button" onClick={() => setNotice(null)}>
                {locale === "en-US" ? "OK" : "知道了"}
              </button>
            </div>
          </div>
        </div>
      )}

      <header className="work-game-hero">
        <div>
          <button className="text-button small-button native-back-link" type="button" onClick={() => void router.push("/work-market")}>
            {locale === "en-US" ? "Back" : "返回牛马市场"}
          </button>
          <h3>{gameState ? gameName(gameState, locale) : fallbackGameName(gameKey, locale)}</h3>
        </div>
        <div className="work-game-stat-strip">
          <span>{locale === "en-US" ? "Active" : "进行中"} <strong>{totals.active}</strong></span>
          <span>{locale === "en-US" ? "Care" : "需照看"} <strong>{totals.needsCare}</strong></span>
          <span>{locale === "en-US" ? "Ready" : "可领取"} <strong>{totals.claimable}</strong></span>
        </div>
      </header>

      <div className="work-game-shell">
        <article
          ref={sceneViewportRef}
          className={`work-game-scene-viewport ${dragging ? "is-dragging" : ""}`}
          onPointerDown={handleScenePointerDown}
          onPointerMove={handleScenePointerMove}
          onPointerUp={handleScenePointerUp}
          onPointerCancel={handleScenePointerUp}
          onPointerLeave={handleScenePointerLeave}
          onWheel={handleSceneWheel}
        >
          <div
            className="work-game-scene-world"
            style={{ transform: `translate3d(${sceneOffset.x}px, ${sceneOffset.y}px, 0) scale(${sceneScale})` }}
          >
            <img
              className="work-game-map-image"
              src={mapImage}
              alt=""
              width={GAME_WORLD_SIZE.width}
              height={GAME_WORLD_SIZE.height}
              draggable="false"
            />
            {tasks.map((task) => {
              const hotspot = hotspotForTask(gameKey, task);
              const status = task.status;
              const contentKey = task.job ? `${gameKey}:${task.job.jobKey}` : "";
              const frameAssets = contentKey ? JOB_FRAME_ASSETS[contentKey] : undefined;
              const frameOffset = JOB_FRAME_OFFSETS[contentKey] ?? 0;
              const contentAsset = frameAssets?.[(animationFrame + frameOffset) % frameAssets.length]
                ?? (contentKey ? JOB_CONTENT_ASSETS[contentKey] : undefined);
              const contentClassName = task.job
                ? `work-game-job-content work-game-job-content-${gameKey} work-game-job-content-${gameKey}-${task.job.jobKey}`
                : "work-game-job-content";
              return (
                <button
                  key={task.id}
                  className={`work-game-hotspot work-game-hotspot-${status} work-game-hotspot-${hotspot.tone} ${selectedTask?.id === task.id ? "selected" : ""}`}
                  style={{
                    "--hotspot-x": `${hotspot.x}px`,
                    "--hotspot-y": `${hotspot.y}px`,
                    "--hotspot-width": `${hotspot.width}px`,
                    "--hotspot-height": `${hotspot.height}px`,
                  } as CSSProperties}
                  type="button"
                  onClick={() => runTaskAction(task)}
                  aria-label={`${jobName(task.job, locale)}: ${taskStatusLabel(task.status, locale)}`}
                >
                  {contentAsset ? (
                    <>
                      <img className={contentClassName} src={contentAsset} alt="" draggable="false" />
                      {contentKey === "convenience-store:day-shift" && (
                        <span className="work-game-job-occluder work-game-job-occluder-store-counter" aria-hidden="true" />
                      )}
                    </>
                  ) : (
                    <span className="work-game-job-marker" aria-hidden="true">
                      <span className="work-game-job-marker-core" />
                    </span>
                  )}
                  <span className="work-game-hotspot-label">
                    <strong>{jobName(task.job, locale)}</strong>
                    <em>{taskStatusLabel(task.status, locale)}</em>
                  </span>
                  <span className="work-game-hotspot-progress" aria-hidden="true">
                    <span style={{ width: `${Math.round((task.progressRatio ?? 0) * 100)}%` }} />
                  </span>
                  {effectTaskId === task.id && <span className="work-game-map-effect" aria-hidden="true" />}
                </button>
              );
            })}
          </div>
        </article>

        <aside className="work-game-hud">
          <article className="work-game-panel work-game-action-panel">
            <div className="section-heading">
              <div>
                <span className="eyebrow">{locale === "en-US" ? "Current Job" : "当前岗位"}</span>
                <h3>{selectedTask ? jobName(selectedTask.job, locale) : (locale === "en-US" ? "No job" : "暂无岗位")}</h3>
              </div>
              {selectedTask && (
                <span className={`work-game-status-chip work-game-status-${selectedTask.status}`}>
                  {taskStatusLabel(selectedTask.status, locale)}
                </span>
              )}
            </div>

            {selectedTask ? (
              <div className="work-game-action-stack">
                <div className="work-game-progress-card">
                  <div>
                    <span>{locale === "en-US" ? "Progress" : "进度"}</span>
                    <strong>{Math.round((selectedTask.progressRatio ?? 0) * 100)}%</strong>
                  </div>
                  <div className="work-game-progress">
                    <span style={{ width: `${Math.round((selectedTask.progressRatio ?? 0) * 100)}%` }} />
                  </div>
                  <small>{progressHint(selectedTask, locale)}</small>
                </div>
                <button
                  className={selectedTask.status === "running" ? "secondary-button work-game-main-action" : "primary-button work-game-main-action"}
                  type="button"
                  disabled={selectedTask.status === "running" || busyTaskId === selectedTask.id}
                  onClick={() => runTaskAction(selectedTask)}
                >
                  {actionLabel(selectedTask, busyTaskId === selectedTask.id, locale)}
                </button>
              </div>
            ) : (
              <div className="empty-state">{locale === "en-US" ? "No jobs yet." : "暂无岗位。"}</div>
            )}
          </article>
        </aside>
      </div>
    </section>
  );
}

function hotspotForTask(gameKey: string, task: WorkGameTask) {
  return HOTSPOTS[gameKey]?.[task.slotIndex]
    ?? { x: 180 + task.slotIndex * 330, y: 380, width: 280, height: 280, tone: "warm" as const };
}

function gameName(state: { nameZh: string; nameEn: string }, locale: LocaleCode) {
  return locale === "en-US" ? state.nameEn : state.nameZh;
}

function fallbackGameName(gameKey: string, locale: LocaleCode) {
  const names: Record<string, { zh: string; en: string }> = {
    mine: { zh: "矿场挖矿", en: "Mine" },
    factory: { zh: "工厂打螺丝", en: "Factory" },
    "convenience-store": { zh: "便利店值班", en: "Convenience Store" },
  };
  const entry = names[gameKey] ?? names.mine;
  return locale === "en-US" ? entry.en : entry.zh;
}

function jobName(job: WorkGameJobConfig | undefined, locale: LocaleCode) {
  if (!job) {
    return locale === "en-US" ? "Open Slot" : "空闲岗位";
  }
  return locale === "en-US" ? job.nameEn : job.nameZh;
}

function taskStatusLabel(status: WorkMapStatus, locale: LocaleCode) {
  const labels: Record<WorkMapStatus, { zh: string; en: string }> = {
    locked: { zh: "未开放", en: "Locked" },
    idle: { zh: "空闲", en: "Idle" },
    running: { zh: "进行中", en: "Running" },
    needsCare: { zh: "需要照看", en: "Needs Care" },
    claimable: { zh: "可领取", en: "Ready" },
  };
  const entry = labels[status] ?? labels.idle;
  return locale === "en-US" ? entry.en : entry.zh;
}

function actionLabel(task: WorkGameTask, busy: boolean, locale: LocaleCode) {
  if (busy) {
    return locale === "en-US" ? "Working" : "处理中";
  }
  if (task.status === "idle") {
    return locale === "en-US" ? "Start Job" : "开始打工";
  }
  if (task.status === "needsCare") {
    return locale === "en-US" ? "Care Job" : "照看岗位";
  }
  if (task.status === "claimable") {
    return locale === "en-US" ? "Claim Reward" : "领取奖励";
  }
  return locale === "en-US" ? `Next care in ${formatSeconds(task.remainingSeconds, locale)}` : `距离下次照看 ${formatSeconds(task.remainingSeconds, locale)}`;
}

function progressHint(task: WorkGameTask, locale: LocaleCode) {
  if (task.status === "idle") {
    return locale === "en-US" ? "Start this slot when you are ready." : "准备好后即可开始这个岗位。";
  }
  if (task.status === "needsCare") {
    return locale === "en-US" ? "A short care action is waiting." : "现在需要完成一次短照看动作。";
  }
  if (task.status === "claimable") {
    return locale === "en-US" ? "Reward is ready to claim." : "奖励已经可以领取。";
  }
  return locale === "en-US" ? `Next checkpoint in ${formatSeconds(task.remainingSeconds, locale)}.` : `下个节点还需 ${formatSeconds(task.remainingSeconds, locale)}。`;
}

function rewardLine(entry: WorkGameRewardLedgerEntry, locale: LocaleCode) {
  const items = entry.rewards.map((reward) => `${reward.itemKey} x${reward.quantity}`).join(" · ");
  const lp = entry.lpReward > 0 ? `LP +${entry.lpReward}` : "";
  const line = [items, lp].filter(Boolean).join(" · ");
  return line || (locale === "en-US" ? "Reward claimed." : "已领取奖励。");
}

function formatSeconds(seconds: number, locale: LocaleCode) {
  if (seconds <= 0) {
    return locale === "en-US" ? "soon" : "即将到达";
  }
  const minutes = Math.ceil(seconds / 60);
  if (minutes < 60) {
    return locale === "en-US" ? `${minutes} min` : `${minutes} 分钟`;
  }
  return locale === "en-US" ? `${Math.ceil(minutes / 60)} hr` : `${Math.ceil(minutes / 60)} 小时`;
}
