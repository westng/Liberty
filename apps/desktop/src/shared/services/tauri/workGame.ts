import { invoke } from "@tauri-apps/api/core";
import { grantPreviewInventoryItem, notifyPetStateChanged } from "@/shared/services/tauri/pet";
import type {
  WorkGameClaimResult,
  WorkGameJobConfig,
  WorkGameRewardLedgerEntry,
  WorkGameState,
  WorkGameTask,
  WorkMapStatus,
} from "@/shared/types/meeting";

const previewJobs: WorkGameJobConfig[] = [
  makeJob("mine", "shallow-vein", 1, "浅层矿脉", "Shallow Vein", "短周期矿点，适合快速收获工具和少量 LP。", "A short mining loop for tools and a little LP.", 600, 1, "energy-capsule-tool", 1, "rune-stone-tool", 12, 3, 6, ["敲击矿点", "清理碎石"], ["Tap ore", "Clear rocks"]),
  makeJob("mine", "deep-vein", 2, "深层矿脉", "Deep Vein", "中周期矿点，需要加固支架，收益更稳定。", "A deeper mining loop with steadier rewards.", 1500, 2, "rune-stone-tool", 1, "rainbow-crystal-tool", 16, 8, 14, ["清理碎石", "加固支架", "装载矿车"], ["Clear rocks", "Brace beams", "Load cart"]),
  makeJob("mine", "glowing-vein", 3, "闪光富矿", "Glowing Vein", "长周期富矿，低概率带出惊喜礼盒。", "A long rich-vein loop with a small gift-box chance.", 2700, 3, "rainbow-crystal-tool", 1, "gift-box-tool", 14, 15, 24, ["定位富矿", "加固支架", "装载矿车"], ["Mark vein", "Brace beams", "Load cart"]),
  makeJob("factory", "basic-assembly", 1, "基础装配", "Basic Assembly", "短周期工位，稳定产出 LP 和基础工具。", "A short station with stable LP and tool rewards.", 480, 1, "energy-capsule-tool", 1, "star-coin-tool", 10, 5, 9, ["按序拧螺丝", "打包出货"], ["Tighten screws", "Pack order"]),
  makeJob("factory", "rush-order", 2, "加急订单", "Rush Order", "中周期订单，照看传送带后获得更高 LP。", "A mid-cycle order with higher LP after line care.", 1080, 2, "energy-drink-tool", 1, "stopwatch-tool", 18, 10, 16, ["清理卡料", "质检盖章", "打包出货"], ["Clear jam", "Inspect stamp", "Pack order"]),
  makeJob("factory", "precision-check", 3, "精密质检", "Precision Check", "长周期质检岗位，适合收获秒表和稀有工具。", "A longer inspection station for stopwatch and rare tools.", 2100, 3, "stopwatch-tool", 1, "golden-bell-tool", 14, 16, 25, ["按序拧螺丝", "质检盖章", "整理工位"], ["Tighten screws", "Inspect stamp", "Reset station"]),
  makeJob("convenience-store", "day-shift", 1, "白班", "Day Shift", "短周期值班，主要获得食物和少量 LP。", "A short store shift for food and a little LP.", 720, 1, "sandwich-food", 1, "bubble-tea-food", 18, 4, 8, ["收银结账", "补齐货架"], ["Checkout", "Restock shelf"]),
  makeJob("convenience-store", "evening-shift", 2, "晚班", "Evening Shift", "中周期值班，兼顾补货、加热便当和顾客需求。", "A mid-cycle shift with restocking and customer care.", 1440, 2, "bento-box-food", 1, "gift-box-tool", 10, 9, 15, ["加热便当", "补齐货架", "收银结账"], ["Heat meal", "Restock shelf", "Checkout"]),
  makeJob("convenience-store", "night-shift", 3, "夜班", "Night Shift", "长周期夜班，低压力但有更好的礼盒和道具概率。", "A longer night shift with better gift and tool chances.", 2400, 3, "fruit-tart-food", 1, "magic-potion-tool", 16, 14, 22, ["清洁门口", "处理顾客需求", "整理货架"], ["Clean entrance", "Help customer", "Tidy shelf"]),
];

let previewStates = new Map<string, WorkGameState>();

export function createLocalWorkGameService() {
  if (!("__TAURI_INTERNALS__" in window)) {
    return createPreviewWorkGameService();
  }

  return {
    getState: (gameKey: string) => invoke<WorkGameState>("get_work_game_state", { input: { gameKey } }),
    startTask: async (gameKey: string, taskId: string, jobKey: string) => {
      const state = await invoke<WorkGameState>("start_work_game_task", { input: { gameKey, taskId, jobKey } });
      notifyPetStateChanged("work-game-start");
      return state;
    },
    careTask: async (gameKey: string, taskId: string) => {
      const state = await invoke<WorkGameState>("care_work_game_task", { input: { gameKey, taskId } });
      notifyPetStateChanged("work-game-care");
      return state;
    },
    claimTask: async (gameKey: string, taskId: string) => {
      const result = await invoke<WorkGameClaimResult>("claim_work_game_task", { input: { gameKey, taskId } });
      notifyPetStateChanged("work-game-claim");
      return result;
    },
  };
}

export function createPreviewWorkGameMaps() {
  return ["mine", "factory", "convenience-store"].map((gameKey) => {
    const state = getPreviewState(gameKey);
    return {
      id: state.gameKey,
      nameZh: state.nameZh,
      nameEn: state.nameEn,
      descriptionZh: state.descriptionZh,
      descriptionEn: state.descriptionEn,
      category: state.gameKey,
      status: state.mapStatus,
      route: `/work-game/${state.gameKey}`,
      outputs: state.gameKey === "mine" ? ["tool", "rare_tool", "lp"] : state.gameKey === "factory" ? ["lp", "tool"] : ["food", "gift_box", "lp"],
      enabled: true,
      summary: {
        status: state.mapStatus,
        activePlots: state.tasks.filter((task) => task.status !== "idle").length,
        needsCarePlots: state.tasks.filter((task) => task.status === "needsCare").length,
        maturePlots: state.tasks.filter((task) => task.status === "claimable").length,
      },
    };
  });
}

function createPreviewWorkGameService() {
  return {
    getState: async (gameKey: string) => getPreviewState(gameKey),
    startTask: async (gameKey: string, taskId: string, jobKey: string) => {
      const state = getPreviewState(gameKey);
      const job = state.jobs.find((item) => item.jobKey === jobKey) ?? state.jobs[0];
      const nextTasks = state.tasks.map((task) =>
        task.id === taskId
          ? makePreviewTask(gameKey, task.slotIndex, job, "running", 0.34, Math.max(60, Math.round(job.durationSeconds / (job.careRequired + 1))))
          : task,
      );
      return setPreviewState({ ...state, tasks: nextTasks, mapStatus: statusFromTasks(nextTasks), updatedAt: new Date().toISOString() });
    },
    careTask: async (gameKey: string, taskId: string) => {
      const state = getPreviewState(gameKey);
      const nextTasks = state.tasks.map((task) =>
        task.id === taskId && task.job
          ? makePreviewTask(
              gameKey,
              task.slotIndex,
              task.job,
              task.stageIndex + 1 >= task.job.careRequired ? "claimable" : "running",
              task.stageIndex + 1 >= task.job.careRequired ? 1 : Math.min(0.92, (task.progressRatio ?? 0) + 0.2),
              task.stageIndex + 1 >= task.job.careRequired ? 0 : Math.max(60, Math.round(task.job.durationSeconds / (task.job.careRequired + 1))),
              Math.min(task.job.careRequired, task.stageIndex + 1),
            )
          : task,
      );
      return setPreviewState({ ...state, tasks: nextTasks, mapStatus: statusFromTasks(nextTasks), updatedAt: new Date().toISOString() });
    },
    claimTask: async (gameKey: string, taskId: string) => {
      const state = getPreviewState(gameKey);
      const task = state.tasks.find((item) => item.id === taskId);
      const job = task?.job ?? state.jobs[0];
      const reward: WorkGameRewardLedgerEntry = {
        id: `preview-work-reward-${Date.now()}`,
        gameKey,
        taskId,
        jobKey: job.jobKey,
        rewards: [{ itemKey: job.primaryRewardItemKey, quantity: job.primaryRewardQuantity, rewardType: "primary" }],
        lpReward: job.lpMin,
        createdAt: new Date().toISOString(),
      };
      for (const item of reward.rewards) {
        grantPreviewInventoryItem(item.itemKey, item.quantity, "work_game_reward");
      }
      const nextTasks = state.tasks.map((item) =>
        item.id === taskId ? makePreviewTask(gameKey, item.slotIndex, state.jobs.find((jobItem) => jobItem.slotIndex === item.slotIndex), "idle", 0, 0) : item,
      );
      const nextState = setPreviewState({ ...state, tasks: nextTasks, rewards: [reward, ...state.rewards].slice(0, 12), mapStatus: statusFromTasks(nextTasks), updatedAt: new Date().toISOString() });
      return { state: nextState, reward };
    },
  };
}

function getPreviewState(gameKey: string) {
  const existing = previewStates.get(gameKey);
  if (existing) {
    return existing;
  }
  const created = createPreviewState(gameKey);
  previewStates.set(gameKey, created);
  return created;
}

function setPreviewState(state: WorkGameState) {
  previewStates.set(state.gameKey, state);
  return state;
}

function createPreviewState(gameKey: string): WorkGameState {
  const jobs = previewJobs.filter((job) => job.gameKey === gameKey);
  const names = gameNames(gameKey);
  const tasks = jobs.map((job) => {
    if (job.slotIndex === 1) {
      return makePreviewTask(gameKey, job.slotIndex, job, "running", 0.46, 120);
    }
    if (job.slotIndex === 2) {
      return makePreviewTask(gameKey, job.slotIndex, job, "needsCare", 0.64, 0);
    }
    return makePreviewTask(gameKey, job.slotIndex, job, "claimable", 1, 0);
  });
  return {
    gameKey,
    ...names,
    mapStatus: statusFromTasks(tasks),
    tasks,
    jobs,
    rewards: [],
    updatedAt: new Date().toISOString(),
  };
}

function makePreviewTask(
  gameKey: string,
  slotIndex: number,
  job: WorkGameJobConfig | undefined,
  status: WorkMapStatus,
  progressRatio: number,
  remainingSeconds: number,
  stageIndex?: number,
): WorkGameTask {
  const now = new Date().toISOString();
  return {
    id: `${gameKey}-slot-${slotIndex}`,
    gameKey,
    slotIndex,
    jobKey: status === "idle" ? "" : (job?.jobKey ?? ""),
    status,
    stageIndex: stageIndex ?? (status === "idle" ? 0 : status === "claimable" ? (job?.careRequired ?? 1) : 1),
    startedAt: status === "idle" ? undefined : now,
    lastCaredAt: status === "claimable" ? now : undefined,
    nextCareAt: status === "needsCare" ? now : undefined,
    claimableAt: status === "claimable" ? now : undefined,
    updatedAt: now,
    job,
    progressRatio,
    remainingSeconds,
  };
}

function makeJob(
  gameKey: string,
  jobKey: string,
  slotIndex: number,
  nameZh: string,
  nameEn: string,
  descriptionZh: string,
  descriptionEn: string,
  durationSeconds: number,
  careRequired: number,
  primaryRewardItemKey: string,
  primaryRewardQuantity: number,
  bonusRewardItemKey: string | undefined,
  bonusChancePercent: number,
  lpMin: number,
  lpMax: number,
  careActionsZh: string[],
  careActionsEn: string[],
): WorkGameJobConfig {
  return {
    gameKey,
    jobKey,
    slotIndex,
    nameZh,
    nameEn,
    descriptionZh,
    descriptionEn,
    durationSeconds,
    careRequired,
    primaryRewardItemKey,
    primaryRewardQuantity,
    bonusRewardItemKey,
    bonusChancePercent,
    lpMin,
    lpMax,
    careActionsZh,
    careActionsEn,
  };
}

function gameNames(gameKey: string) {
  if (gameKey === "factory") {
    return {
      nameZh: "工厂打螺丝",
      nameEn: "Factory",
      descriptionZh: "接装配订单、处理生产线，稳定获得 LP 和工具。",
      descriptionEn: "Run assembly orders for steady LP and tool rewards.",
    };
  }
  if (gameKey === "convenience-store") {
    return {
      nameZh: "便利店值班",
      nameEn: "Convenience Store",
      descriptionZh: "完成收银、补货和清洁，获得食物、礼盒和日常 LP。",
      descriptionEn: "Handle checkout, restocking, and cleaning for food, gifts, and LP.",
    };
  }
  return {
    nameZh: "矿场挖矿",
    nameEn: "Mine",
    descriptionZh: "选择矿脉、照看矿点，收获工具、稀有道具和 LP。",
    descriptionEn: "Pick a vein, care for the mine, and earn tools, rare items, and LP.",
  };
}

function statusFromTasks(tasks: WorkGameTask[]): WorkMapStatus {
  if (tasks.some((task) => task.status === "claimable")) {
    return "claimable";
  }
  if (tasks.some((task) => task.status === "needsCare")) {
    return "needsCare";
  }
  if (tasks.some((task) => task.status === "running")) {
    return "running";
  }
  return "idle";
}
