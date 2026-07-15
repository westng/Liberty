import { useSyncExternalStore } from "react";
import { createLocalWorkGameService } from "@/shared/services/tauri/workGame";
import type { WorkGameRewardLedgerEntry, WorkGameState } from "@/shared/types/meeting";

type WorkGameStoreState = {
  states: Record<string, WorkGameState>;
  lastReward: WorkGameRewardLedgerEntry | null;
  loadedKeys: Record<string, boolean>;
  loadingKeys: Record<string, boolean>;
};

let state: WorkGameStoreState = {
  states: {},
  lastReward: null,
  loadedKeys: {},
  loadingKeys: {},
};

const workGameService = createLocalWorkGameService();
const listeners = new Set<() => void>();
const requestSequences = new Map<string, number>();
const loadPromises = new Map<string, Promise<WorkGameState | null>>();
const loadTokens = new Map<string, object>();
const writesInFlight = new Map<string, number>();

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function getSnapshot() {
  return state;
}

function setState(patch: Partial<WorkGameStoreState>) {
  state = { ...state, ...patch };
  for (const listener of listeners) {
    listener();
  }
}

function setGameState(gameState: WorkGameState) {
  setState({
    states: { ...state.states, [gameState.gameKey]: gameState },
    loadedKeys: { ...state.loadedKeys, [gameState.gameKey]: true },
  });
}

function setLoading(gameKey: string, loading: boolean) {
  setState({
    loadingKeys: { ...state.loadingKeys, [gameKey]: loading },
  });
}

function nextRequestSequence(gameKey: string) {
  const sequence = (requestSequences.get(gameKey) ?? 0) + 1;
  requestSequences.set(gameKey, sequence);
  return sequence;
}

function isLatestRequest(gameKey: string, sequence: number) {
  return requestSequences.get(gameKey) === sequence;
}

function beginWrite(gameKey: string) {
  writesInFlight.set(gameKey, (writesInFlight.get(gameKey) ?? 0) + 1);
}

function endWrite(gameKey: string) {
  const remaining = (writesInFlight.get(gameKey) ?? 1) - 1;
  if (remaining <= 0) {
    writesInFlight.delete(gameKey);
  } else {
    writesInFlight.set(gameKey, remaining);
  }
}

async function loadGameState(gameKey: string, force = false) {
  if (!gameKey) {
    return null;
  }
  if (state.loadedKeys[gameKey] && !force) {
    return state.states[gameKey] ?? null;
  }
  if ((writesInFlight.get(gameKey) ?? 0) > 0) {
    return state.states[gameKey] ?? null;
  }
  const existingRequest = loadPromises.get(gameKey);
  if (existingRequest) {
    return existingRequest;
  }
  const requestSequence = nextRequestSequence(gameKey);
  const requestToken = {};
  loadTokens.set(gameKey, requestToken);
  setLoading(gameKey, true);
  const request = (async () => {
    try {
      const gameState = await workGameService.getState(gameKey);
      if (isLatestRequest(gameKey, requestSequence)) {
        setGameState(gameState);
      }
      return gameState;
    } finally {
      if (loadTokens.get(gameKey) === requestToken) {
        loadPromises.delete(gameKey);
        loadTokens.delete(gameKey);
        setLoading(gameKey, false);
      }
    }
  })();
  loadPromises.set(gameKey, request);
  return request;
}

async function refresh(gameKey: string) {
  return loadGameState(gameKey, true);
}

async function startTask(gameKey: string, taskId: string, jobKey: string) {
  const requestSequence = nextRequestSequence(gameKey);
  beginWrite(gameKey);
  try {
    const gameState = await workGameService.startTask(gameKey, taskId, jobKey);
    if (isLatestRequest(gameKey, requestSequence)) {
      setGameState(gameState);
    }
    return gameState;
  } finally {
    endWrite(gameKey);
  }
}

async function careTask(gameKey: string, taskId: string) {
  const requestSequence = nextRequestSequence(gameKey);
  beginWrite(gameKey);
  try {
    const gameState = await workGameService.careTask(gameKey, taskId);
    if (isLatestRequest(gameKey, requestSequence)) {
      setGameState(gameState);
    }
    return gameState;
  } finally {
    endWrite(gameKey);
  }
}

async function claimTask(gameKey: string, taskId: string) {
  const requestSequence = nextRequestSequence(gameKey);
  beginWrite(gameKey);
  try {
    const result = await workGameService.claimTask(gameKey, taskId);
    if (isLatestRequest(gameKey, requestSequence)) {
      setState({ lastReward: result.reward });
      setGameState(result.state);
    }
    return result;
  } finally {
    endWrite(gameKey);
  }
}

const actions = {
  loadGameState,
  refresh,
  startTask,
  careTask,
  claimTask,
};

export function useWorkGameStore(gameKey?: string) {
  const snapshot = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
  return {
    state: gameKey ? snapshot.states[gameKey] ?? null : null,
    lastReward: snapshot.lastReward,
    loaded: gameKey ? Boolean(snapshot.loadedKeys[gameKey]) : false,
    loading: gameKey ? Boolean(snapshot.loadingKeys[gameKey]) : false,
    ...actions,
  };
}

export type WorkGameStore = ReturnType<typeof useWorkGameStore>;
