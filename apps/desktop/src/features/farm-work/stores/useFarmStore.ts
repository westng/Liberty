import { useSyncExternalStore } from "react";
import { createLocalFarmService } from "@/shared/services/tauri/farm";
import type { FarmHarvestLedgerEntry, FarmState, WorkMarketState } from "@/shared/types/meeting";

type FarmStoreState = {
  farmState: FarmState | null;
  workMarketState: WorkMarketState | null;
  lastHarvest: FarmHarvestLedgerEntry | null;
  loaded: boolean;
  loading: boolean;
};

let state: FarmStoreState = {
  farmState: null,
  workMarketState: null,
  lastHarvest: null,
  loaded: false,
  loading: false,
};

const farmService = createLocalFarmService();
const listeners = new Set<() => void>();
let requestSequence = 0;
let loadPromise: Promise<FarmState | null> | null = null;
let loadToken: object | null = null;
let writesInFlight = 0;

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function getSnapshot() {
  return state;
}

function setState(patch: Partial<FarmStoreState>) {
  state = { ...state, ...patch };
  for (const listener of listeners) {
    listener();
  }
}

async function loadFarmState(force = false) {
  if (state.loaded && !force) {
    return state.farmState;
  }
  if (writesInFlight > 0) {
    return state.farmState;
  }
  if (loadPromise) {
    return loadPromise;
  }
  const requestId = ++requestSequence;
  const requestToken = {};
  loadToken = requestToken;
  setState({ loading: true });
  const request = (async () => {
    try {
      const [farmState, workMarketState] = await Promise.all([
        farmService.getFarmState(),
        farmService.getWorkMarketState(),
      ]);
      if (requestId === requestSequence) {
        setState({ farmState, workMarketState, loaded: true });
      }
      return farmState;
    } finally {
      if (loadToken === requestToken) {
        loadPromise = null;
        loadToken = null;
        setState({ loading: false });
      }
    }
  })();
  loadPromise = request;
  return request;
}

async function refresh() {
  await loadFarmState(true);
}

async function plantCrop(plotId: string, cropKey: string) {
  const requestId = ++requestSequence;
  writesInFlight += 1;
  try {
    const farmState = await farmService.plantCrop(plotId, cropKey);
    const workMarketState = await farmService.getWorkMarketState();
    if (requestId === requestSequence) {
      setState({ farmState, workMarketState, loaded: true });
    }
    return farmState;
  } finally {
    writesInFlight -= 1;
  }
}

async function waterPlot(plotId: string) {
  const requestId = ++requestSequence;
  writesInFlight += 1;
  try {
    const farmState = await farmService.waterPlot(plotId);
    const workMarketState = await farmService.getWorkMarketState();
    if (requestId === requestSequence) {
      setState({ farmState, workMarketState, loaded: true });
    }
    return farmState;
  } finally {
    writesInFlight -= 1;
  }
}

async function harvestPlot(plotId: string) {
  const requestId = ++requestSequence;
  writesInFlight += 1;
  try {
    const result = await farmService.harvestPlot(plotId);
    const workMarketState = await farmService.getWorkMarketState();
    if (requestId === requestSequence) {
      setState({
        farmState: result.state,
        workMarketState,
        lastHarvest: result.harvest,
        loaded: true,
      });
    }
    return result;
  } finally {
    writesInFlight -= 1;
  }
}

const actions = {
  loadFarmState,
  refresh,
  plantCrop,
  waterPlot,
  harvestPlot,
};

export function useFarmStore() {
  const snapshot = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
  return {
    ...snapshot,
    ...actions,
  };
}

export type FarmStore = ReturnType<typeof useFarmStore>;
