import { invoke } from "@tauri-apps/api/core";
import { consumePreviewInventoryItem, grantPreviewInventoryItem, notifyPetStateChanged } from "@/shared/services/tauri/pet";
import { createPreviewWorkGameMaps } from "@/shared/services/tauri/workGame";
import type {
  FarmCropConfig,
  FarmHarvestLedgerEntry,
  FarmHarvestResult,
  FarmPlot,
  FarmState,
  WorkMarketState,
} from "@/shared/types/meeting";

const previewCrops: FarmCropConfig[] = [
  {
    cropKey: "wheat",
    seedItemKey: "wheat-seed",
    nameZh: "小麦",
    nameEn: "Wheat",
    descriptionZh: "短周期作物，适合测试播种手感。",
    descriptionEn: "A short-cycle crop for testing planting flow.",
    durationSeconds: 300,
    waterRequired: 1,
    primaryRewardItemKey: "wheat-harvest-food",
    primaryRewardQuantity: 1,
    bonusChancePercent: 0,
    lpMin: 1,
    lpMax: 3,
  },
  {
    cropKey: "carrot",
    seedItemKey: "carrot-seed",
    nameZh: "胡萝卜",
    nameEn: "Carrot",
    descriptionZh: "稳定收益作物。",
    descriptionEn: "A steady reward crop.",
    durationSeconds: 900,
    waterRequired: 2,
    primaryRewardItemKey: "carrot-harvest-food",
    primaryRewardQuantity: 1,
    bonusRewardItemKey: "cupcake-food",
    bonusChancePercent: 20,
    lpMin: 3,
    lpMax: 6,
  },
  {
    cropKey: "tomato",
    seedItemKey: "tomato-seed",
    nameZh: "番茄",
    nameEn: "Tomato",
    descriptionZh: "中周期作物，成熟后更显眼。",
    descriptionEn: "A mid-cycle crop with a clear mature state.",
    durationSeconds: 1800,
    waterRequired: 2,
    primaryRewardItemKey: "tomato-harvest-food",
    primaryRewardQuantity: 1,
    bonusRewardItemKey: "energy-drink-tool",
    bonusChancePercent: 24,
    lpMin: 6,
    lpMax: 10,
  },
  {
    cropKey: "pumpkin",
    seedItemKey: "pumpkin-seed",
    nameZh: "南瓜",
    nameEn: "Pumpkin",
    descriptionZh: "长周期高收益作物。",
    descriptionEn: "A long-cycle crop with richer rewards.",
    durationSeconds: 3600,
    waterRequired: 3,
    primaryRewardItemKey: "pumpkin-harvest-food",
    primaryRewardQuantity: 1,
    bonusRewardItemKey: "gift-box-tool",
    bonusChancePercent: 30,
    lpMin: 10,
    lpMax: 18,
  },
  {
    cropKey: "corn",
    seedItemKey: "corn-seed",
    nameZh: "玉米",
    nameEn: "Corn",
    descriptionZh: "中短周期作物，收获活力道具。",
    descriptionEn: "A medium-short crop that yields energy tools.",
    durationSeconds: 1200,
    waterRequired: 2,
    primaryRewardItemKey: "corn-harvest-food",
    primaryRewardQuantity: 1,
    bonusRewardItemKey: "energy-drink-tool",
    bonusChancePercent: 22,
    lpMin: 4,
    lpMax: 8,
  },
  {
    cropKey: "strawberry",
    seedItemKey: "strawberry-seed",
    nameZh: "草莓",
    nameEn: "Strawberry",
    descriptionZh: "甜点收益作物。",
    descriptionEn: "A dessert-yielding crop.",
    durationSeconds: 1500,
    waterRequired: 2,
    primaryRewardItemKey: "strawberry-harvest-food",
    primaryRewardQuantity: 1,
    bonusRewardItemKey: "pink-donut-food",
    bonusChancePercent: 24,
    lpMin: 5,
    lpMax: 9,
  },
  {
    cropKey: "blueberry",
    seedItemKey: "blueberry-seed",
    nameZh: "蓝莓",
    nameEn: "Blueberry",
    descriptionZh: "长周期稀有作物。",
    descriptionEn: "A long-cycle rare crop.",
    durationSeconds: 2700,
    waterRequired: 3,
    primaryRewardItemKey: "blueberry-harvest-food",
    primaryRewardQuantity: 1,
    bonusRewardItemKey: "rainbow-crystal-tool",
    bonusChancePercent: 26,
    lpMin: 8,
    lpMax: 14,
  },
  {
    cropKey: "potato",
    seedItemKey: "potato-seed",
    nameZh: "土豆",
    nameEn: "Potato",
    descriptionZh: "稳定基础作物。",
    descriptionEn: "A steady starter crop.",
    durationSeconds: 720,
    waterRequired: 1,
    primaryRewardItemKey: "potato-harvest-food",
    primaryRewardQuantity: 1,
    bonusRewardItemKey: "sandwich-food",
    bonusChancePercent: 16,
    lpMin: 2,
    lpMax: 5,
  },
];

let previewFarmState: FarmState = createPreviewFarmState();

export function createLocalFarmService() {
  if (!("__TAURI_INTERNALS__" in window)) {
    return createPreviewFarmService();
  }

  return {
    getWorkMarketState: () => invoke<WorkMarketState>("get_work_market_state"),
    getFarmState: () => invoke<FarmState>("get_farm_state"),
    plantCrop: async (plotId: string, cropKey: string) => {
      const state = await invoke<FarmState>("plant_farm_crop", { input: { plotId, cropKey } });
      notifyPetStateChanged("farm-plant");
      return state;
    },
    waterPlot: (plotId: string) => invoke<FarmState>("water_farm_plot", { input: { plotId } }),
    harvestPlot: async (plotId: string) => {
      const result = await invoke<FarmHarvestResult>("harvest_farm_plot", { input: { plotId } });
      notifyPetStateChanged("farm-harvest");
      return result;
    },
    listHarvestLedger: (limit = 20) =>
      invoke<FarmHarvestLedgerEntry[]>("list_farm_harvest_ledger", { limit }),
  };
}

function createPreviewFarmService() {
  return {
    getWorkMarketState: async () => createPreviewWorkMarketState(previewFarmState),
    getFarmState: async () => previewFarmState,
    plantCrop: async (plotId: string, cropKey: string) => {
      const crop = previewCrops.find((item) => item.cropKey === cropKey) ?? previewCrops[0];
      consumePreviewInventoryItem(crop.seedItemKey, 1);
      previewFarmState = {
        ...previewFarmState,
        plots: previewFarmState.plots.map((plot) =>
          plot.id === plotId
            ? makePreviewPlot({
                id: plot.id,
                plotIndex: plot.plotIndex,
                crop,
                status: "planted",
                progressRatio: 0.28,
                remainingSeconds: Math.max(60, Math.round(crop.durationSeconds * 0.72)),
              })
            : plot,
        ),
        updatedAt: new Date().toISOString(),
      };
      previewFarmState.mapStatus = previewMapStatus(previewFarmState);
      return previewFarmState;
    },
    waterPlot: async (plotId: string) => {
      previewFarmState = {
        ...previewFarmState,
        plots: previewFarmState.plots.map((plot) =>
          plot.id === plotId && plot.crop
            ? makePreviewPlot({
                id: plot.id,
                plotIndex: plot.plotIndex,
                crop: plot.crop,
                status: "mature",
                progressRatio: 1,
                remainingSeconds: 0,
              })
            : plot,
        ),
        mapStatus: "claimable",
        updatedAt: new Date().toISOString(),
      };
      return previewFarmState;
    },
    harvestPlot: async (plotId: string) => {
      const harvestedPlot = previewFarmState.plots.find((plot) => plot.id === plotId);
      const harvest: FarmHarvestLedgerEntry = {
        id: `preview-harvest-${Date.now()}`,
        plotId,
        cropKey: harvestedPlot?.cropKey || "wheat",
        rewards: [{ itemKey: harvestedPlot?.crop?.primaryRewardItemKey || "wheat-harvest-food", quantity: 1, rewardType: "primary" }],
        lpReward: 3,
        createdAt: new Date().toISOString(),
      };
      for (const reward of harvest.rewards) {
        grantPreviewInventoryItem(reward.itemKey, reward.quantity, "farm_harvest");
      }
      previewFarmState = {
        ...previewFarmState,
        plots: previewFarmState.plots.map((plot) =>
          plot.id === plotId
            ? makePreviewPlot({
                id: plot.id,
                plotIndex: plot.plotIndex,
                crop: undefined,
                status: "empty",
                progressRatio: 0,
                remainingSeconds: 0,
              })
            : plot,
        ),
        harvests: [harvest, ...previewFarmState.harvests].slice(0, 12),
        updatedAt: new Date().toISOString(),
      };
      previewFarmState.mapStatus = previewMapStatus(previewFarmState);
      return { state: previewFarmState, harvest };
    },
    listHarvestLedger: async () => previewFarmState.harvests,
  };
}

function createPreviewFarmState(): FarmState {
  const now = new Date().toISOString();
  return {
    crops: previewCrops,
    harvests: [],
    mapStatus: "needsCare",
    updatedAt: now,
    plots: [
      makePreviewPlot({ id: "preview-plot-1", plotIndex: 1, crop: previewCrops[0], status: "planted", progressRatio: 0.58, remainingSeconds: 120 }),
      makePreviewPlot({ id: "preview-plot-2", plotIndex: 2, crop: previewCrops[1], status: "needs_water", progressRatio: 0.58, remainingSeconds: 0 }),
      makePreviewPlot({ id: "preview-plot-3", plotIndex: 3, crop: previewCrops[2], status: "mature", progressRatio: 1, remainingSeconds: 0 }),
    ],
  };
}

function makePreviewPlot(input: {
  id: string;
  plotIndex: number;
  crop?: FarmCropConfig;
  status: FarmPlot["status"];
  progressRatio: number;
  remainingSeconds: number;
}): FarmPlot {
  const now = new Date().toISOString();
  return {
    id: input.id,
    plotIndex: input.plotIndex,
    cropKey: input.crop?.cropKey ?? "",
    status: input.status,
    stageIndex: input.status === "empty" ? 0 : input.status === "mature" ? 3 : 1,
    plantedAt: input.crop ? now : undefined,
    lastWateredAt: input.status === "mature" ? now : undefined,
    nextCareAt: input.status === "needs_water" ? now : undefined,
    matureAt: input.status === "mature" ? now : undefined,
    updatedAt: now,
    crop: input.crop,
    progressRatio: input.progressRatio,
    remainingSeconds: input.remainingSeconds,
  };
}

function previewMapStatus(farmState: FarmState) {
  if (farmState.plots.some((plot) => plot.status === "mature")) {
    return "claimable";
  }
  if (farmState.plots.some((plot) => plot.status === "needs_water")) {
    return "needsCare";
  }
  if (farmState.plots.some((plot) => plot.status === "planted")) {
    return "running";
  }
  return "idle";
}

function createPreviewWorkMarketState(farmState: FarmState): WorkMarketState {
  return {
    updatedAt: farmState.updatedAt,
    maps: [
      {
        id: "farm",
        nameZh: "农场种菜",
        nameEn: "Farm",
        descriptionZh: "派宠物入场种菜，收获宠物商品与 LP。",
        descriptionEn: "Send your pet into the farm to grow pet-store rewards and LP.",
        category: "farm",
        status: farmState.mapStatus,
        route: "/farm",
        outputs: ["pet-store-items", "lp"],
        enabled: true,
        summary: {
          status: farmState.mapStatus,
          activePlots: farmState.plots.filter((plot) => plot.status !== "empty").length,
          needsCarePlots: farmState.plots.filter((plot) => plot.status === "needs_water").length,
          maturePlots: farmState.plots.filter((plot) => plot.status === "mature").length,
        },
      },
      ...createPreviewWorkGameMaps(),
    ],
  };
}
