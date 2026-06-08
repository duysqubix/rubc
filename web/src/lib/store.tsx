"use client";

import type { ReactNode } from "react";
import { createContext, useCallback, useContext, useEffect, useMemo, useReducer, useRef, useState } from "react";
import {
  deleteRomBytes,
  emulator,
  getRomKey,
  loadRomBytes,
  loadRomFile as resolveRomFile,
  storeRomBytes,
} from "./emulator";
import {
  BOOT_DELAY_MS,
  DEFAULT_SETTINGS,
  STORAGE_KEY,
  TOAST_DELAY_MS,
  createInitialState,
  emulatorReducer,
  paletteFilter,
  parsePersistence,
  serializePersistence,
} from "./store-logic";
import type { EmulatorRom, EmulatorSettings, EmulatorState, SaveSlot, View } from "./store-logic";

export { DEFAULT_SETTINGS, STORAGE_KEY, paletteFilter } from "./store-logic";
export type { ControlsLayout, EmulatorRom, EmulatorSettings, Phase, SaveSlot, SaveSlots, Scaling, View } from "./store-logic";

export const RECENT_ROMS_KEY = "rubc-recent-roms";

export interface KeyMapItem {
  btn: string;
  keys: string[];
}

export const KEYMAP: KeyMapItem[] = [
  { btn: "D-pad", keys: ["←", "↑", "↓", "→"] },
  { btn: "A", keys: ["X"] },
  { btn: "B", keys: ["Z"] },
  { btn: "Start", keys: ["↵"] },
  { btn: "Select", keys: ["⇧"] },
  { btn: "Turbo", keys: ["Tab"] },
];

interface StoredRecentRom {
  key: string;
  name: string;
  size: number;
  lastPlayed: number;
}

export interface EmulatorContextValue extends EmulatorState {
  ROMS: EmulatorRom[];
  KEYMAP: KeyMapItem[];
  ready: boolean;
  error: string | null;
  filter: string;
  rom: EmulatorRom | null;
  roms: EmulatorRom[];
  attachCanvas: (canvas: HTMLCanvasElement | null) => void;
  boot: (id: string) => void;
  buzz: (ms: number, enabled?: boolean) => void;
  flash: (message: string) => void;
  loadFile: (file: File) => Promise<EmulatorRom | null>;
  loadFrom: (index: number) => void;
  openRom: (id: string) => Promise<void>;
  powerOff: () => void;
  removeRom: (id: string) => Promise<void>;
  reset: () => void;
  saveTo: (index: number) => void;
  set: (patch: Partial<EmulatorSettings>) => void;
  setMenuOpen: (menuOpen: boolean) => void;
  setView: (view: View) => void;
  togglePause: () => void;
}

const EmulatorContext = createContext<EmulatorContextValue | null>(null);

export function buzz(ms: number, enabled: boolean): void {
  if (!enabled || typeof navigator === "undefined" || typeof navigator.vibrate !== "function") return;
  try {
    navigator.vibrate(ms);
  } catch {
  }
}

export function EmulatorProvider({ children }: { children: ReactNode }) {
  const [state, dispatch] = useReducer(emulatorReducer, undefined, createInitialState);
  const [roms, setRoms] = useState<EmulatorRom[]>([]);
  const [ready, setReady] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [hydrated, setHydrated] = useState(false);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const bootTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const toastTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastLoadedRomRef = useRef<string | null>(null);
  const loadingRomRef = useRef<string | null>(null);
  const stateRef = useRef(state);

  useEffect(() => {
    stateRef.current = state;
  }, [state]);

  const rom = useMemo(() => findRom(roms, state.romId), [roms, state.romId]);

  const writeRoms = useCallback((next: EmulatorRom[]) => {
    setRoms(next);
    writeRecentRoms(next);
  }, []);

  const rememberRom = useCallback((nextRom: EmulatorRom) => {
    setRoms((current) => {
      const updated = [nextRom, ...current.filter((item) => item.id !== nextRom.id)].slice(0, 5);
      writeRecentRoms(updated);
      return updated;
    });
  }, []);

  const clearBootTimer = useCallback(() => {
    if (bootTimerRef.current !== null) {
      clearTimeout(bootTimerRef.current);
      bootTimerRef.current = null;
    }
  }, []);

  const finishBootSoon = useCallback(() => {
    clearBootTimer();
    bootTimerRef.current = setTimeout(() => {
      dispatch({ type: "bootFinished" });
      bootTimerRef.current = null;
    }, BOOT_DELAY_MS);
  }, [clearBootTimer]);

  const showToast = useCallback((message: string) => {
    dispatch({ type: "flash", message });
    if (toastTimerRef.current !== null) clearTimeout(toastTimerRef.current);
    toastTimerRef.current = setTimeout(() => {
      dispatch({ type: "clearToast" });
      toastTimerRef.current = null;
    }, TOAST_DELAY_MS);
  }, []);

  const loadCore = useCallback(
    async (id: string, force: boolean) => {
      if (!ready || !canvasRef.current) return;
      if (!force && loadingRomRef.current === id) return;
      if (!force && lastLoadedRomRef.current === id && emulator.emu) return;

      const bytes = await loadRomBytes(id);
      if (!bytes) {
        setError("That game's data is no longer cached. Please load the file again.");
        showToast("ROM cache missing · load the file again");
        return;
      }

      try {
        loadingRomRef.current = id;
        await emulator.loadRom(bytes, canvasRef.current);
        emulator.paused = stateRef.current.phase === "paused";
        lastLoadedRomRef.current = id;
      } catch (err) {
        const message = err instanceof Error ? err.message : "Could not start emulator core.";
        setError(message);
        showToast(message);
      } finally {
        if (loadingRomRef.current === id) loadingRomRef.current = null;
      }
    },
    [ready, showToast],
  );

  const boot = useCallback(
    (id: string) => {
      dispatch({ type: "boot", romId: id });
      buzz(18, stateRef.current.settings.haptics);
      void loadCore(id, false);
      finishBootSoon();
    },
    [finishBootSoon, loadCore],
  );

  const openRom = useCallback(
    async (id: string) => {
      const bytes = await loadRomBytes(id);
      if (!bytes) {
        setError("That game's data is no longer cached. Please load the file again.");
        showToast("ROM cache missing · load the file again");
        return;
      }

      const current = findRom(roms, id) ?? romFromBytes(id, id, bytes.length, Date.now());
      rememberRom({ ...current, lastPlayed: Date.now() });
      boot(id);
    },
    [boot, rememberRom, roms, showToast],
  );

  const loadFile = useCallback(
    async (file: File) => {
      try {
        const loaded = await resolveRomFile(file);
        const id = getRomKey(loaded.bytes);
        await storeRomBytes(id, loaded.bytes);
        const nextRom = romFromBytes(id, loaded.name, loaded.bytes.length, Date.now());
        rememberRom(nextRom);
        boot(id);
        return nextRom;
      } catch (err) {
        const message = err instanceof Error ? err.message : "Could not load that file.";
        setError(message);
        showToast(message);
        return null;
      }
    },
    [boot, rememberRom, showToast],
  );

  const attachCanvas = useCallback(
    (canvas: HTMLCanvasElement | null) => {
      canvasRef.current = canvas;
      if (canvas && stateRef.current.romId && stateRef.current.phase !== "empty") {
        void loadCore(stateRef.current.romId, false);
      }
    },
    [loadCore],
  );

  const set = useCallback((patch: Partial<EmulatorSettings>) => {
    dispatch({ type: "setSettings", patch });
  }, []);

  const setView = useCallback((view: View) => {
    dispatch({ type: "setView", view });
  }, []);

  const setMenuOpen = useCallback((menuOpen: boolean) => {
    dispatch({ type: "setMenuOpen", menuOpen });
  }, []);

  const togglePause = useCallback(() => {
    const phase = stateRef.current.phase;
    if (phase !== "running" && phase !== "paused") return;
    dispatch({ type: "togglePause" });
    buzz(10, stateRef.current.settings.haptics);
  }, []);

  const reset = useCallback(() => {
    const id = stateRef.current.romId;
    if (!id) return;
    dispatch({ type: "reset" });
    buzz(24, stateRef.current.settings.haptics);
    void loadCore(id, true);
    finishBootSoon();
  }, [finishBootSoon, loadCore]);

  const powerOff = useCallback(() => {
    clearBootTimer();
    dispatch({ type: "powerOff" });
    buzz(24, stateRef.current.settings.haptics);
    lastLoadedRomRef.current = null;
    loadingRomRef.current = null;
    void emulator.destroy();
  }, [clearBootTimer]);

  const saveTo = useCallback(
    (index: number) => {
      const currentRom = findRom(roms, stateRef.current.romId);
      if (!currentRom) return;
      void (async () => {
        const slot = await emulator.saveState(index, { label: currentRom.title, elapsed: stateRef.current.elapsed });
        if (!slot) {
          showToast(`Could not save state · slot ${index + 1}`);
          return;
        }
        dispatch({ type: "saveTo", index, slot });
        buzz(14, stateRef.current.settings.haptics);
        showToast(`State saved · slot ${index + 1}`);
      })();
    },
    [roms, showToast],
  );

  const loadFrom = useCallback(
    (index: number) => {
      const slot = stateRef.current.slots[index] as SaveSlot | null | undefined;
      if (!slot?.romId) return;
      const romId = slot.romId;
      void (async () => {
        if (romId !== lastLoadedRomRef.current || !emulator.emu) {
          await loadCore(romId, romId !== lastLoadedRomRef.current);
        }
        const loadedSlot = await emulator.loadState(index);
        if (!loadedSlot) {
          showToast(`Could not load state · slot ${index + 1}`);
          return;
        }
        dispatch({ type: "loadFrom", index, slot: loadedSlot });
        buzz(14, stateRef.current.settings.haptics);
        showToast(`State loaded · slot ${index + 1}`);
      })();
    },
    [loadCore, showToast],
  );

  const removeRom = useCallback(
    async (id: string) => {
      const next = roms.filter((item) => item.id !== id);
      writeRoms(next);
      await deleteRomBytes(id);
      if (stateRef.current.romId === id) powerOff();
    },
    [powerOff, roms, writeRoms],
  );

  useEffect(() => {
    const saved = parsePersistence(readLocalStorage(STORAGE_KEY));
    dispatch({ type: "hydrate", persisted: saved });
    setRoms(readRecentRoms());
    setHydrated(true);
  }, []);

  useEffect(() => {
    emulator.onReady = () => {
      setReady(true);
      setError(null);
    };
    emulator.onError = (err) => setError(err.message);
    void emulator.init();

    return () => {
      clearBootTimer();
      if (toastTimerRef.current !== null) clearTimeout(toastTimerRef.current);
      void emulator.destroy();
    };
  }, [clearBootTimer]);

  useEffect(() => {
    if (!hydrated) return;
    writeLocalStorage(STORAGE_KEY, JSON.stringify(serializePersistence(state)));
  }, [hydrated, state]);

  useEffect(() => {
    if (state.phase !== "running") return;
    const timer = setInterval(() => dispatch({ type: "tick" }), 1000);
    return () => clearInterval(timer);
  }, [state.phase]);

  useEffect(() => {
    emulator.paused = state.phase !== "running";
  }, [state.phase]);

  useEffect(() => {
    if (ready && canvasRef.current && state.romId && state.phase !== "empty") {
      void loadCore(state.romId, false);
    }
  }, [loadCore, ready, state.romId, state.phase]);

  useEffect(() => {
    if (!state.settings.sound && emulator.audioCtx) {
      void emulator.audioCtx.suspend();
    }
  }, [state.settings.sound]);

  const value = useMemo<EmulatorContextValue>(
    () => ({
      ...state,
      ROMS: roms,
      KEYMAP,
      ready,
      error,
      filter: paletteFilter(state.settings.palette),
      rom,
      roms,
      attachCanvas,
      boot,
      buzz: (ms: number, enabled = state.settings.haptics) => buzz(ms, enabled),
      flash: showToast,
      loadFile,
      loadFrom,
      openRom,
      powerOff,
      removeRom,
      reset,
      saveTo,
      set,
      setMenuOpen,
      setView,
      togglePause,
    }),
    [
      attachCanvas,
      boot,
      error,
      loadFile,
      loadFrom,
      openRom,
      powerOff,
      ready,
      removeRom,
      reset,
      rom,
      roms,
      saveTo,
      set,
      setMenuOpen,
      setView,
      showToast,
      state,
      togglePause,
    ],
  );

  return <EmulatorContext.Provider value={value}>{children}</EmulatorContext.Provider>;
}

export function useEmulator(): EmulatorContextValue {
  const context = useContext(EmulatorContext);
  if (!context) throw new Error("useEmulator must be used inside EmulatorProvider");
  return context;
}

function findRom(roms: EmulatorRom[], id: string | null): EmulatorRom | null {
  if (!id) return null;
  return roms.find((item) => item.id === id) ?? fallbackRom(id);
}

function fallbackRom(id: string): EmulatorRom {
  const title = id.split(":")[0] || id;
  return { id, title, name: title, thumb: null, live: null };
}

function romFromBytes(id: string, name: string, size: number, lastPlayed: number): EmulatorRom {
  return {
    id,
    title: displayName(name),
    name,
    size,
    mode: name.toLowerCase().endsWith(".gbc") ? "CGB" : "DMG",
    thumb: null,
    live: null,
    lastPlayed,
  };
}

function displayName(name: string): string {
  return name.replace(/\.(gbc?|zip)$/i, "") || name;
}

function readRecentRoms(): EmulatorRom[] {
  const raw = readLocalStorage(RECENT_ROMS_KEY);
  if (!raw) return [];
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return [];
  }
  if (!Array.isArray(parsed)) return [];
  return parsed.filter(isStoredRecentRom).map(storedToRom).slice(0, 5);
}

function writeRecentRoms(roms: EmulatorRom[]): void {
  const stored = roms.map(romToStored);
  writeLocalStorage(RECENT_ROMS_KEY, JSON.stringify(stored));
}

function storedToRom(stored: StoredRecentRom): EmulatorRom {
  return romFromBytes(stored.key, stored.name, stored.size, stored.lastPlayed);
}

function romToStored(rom: EmulatorRom): StoredRecentRom {
  return {
    key: rom.id,
    name: rom.name ?? rom.title,
    size: rom.size ?? 0,
    lastPlayed: rom.lastPlayed ?? Date.now(),
  };
}

function isStoredRecentRom(value: unknown): value is StoredRecentRom {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const item = value as Record<string, unknown>;
  return (
    typeof item.key === "string" &&
    typeof item.name === "string" &&
    typeof item.size === "number" &&
    Number.isFinite(item.size) &&
    typeof item.lastPlayed === "number" &&
    Number.isFinite(item.lastPlayed)
  );
}

function readLocalStorage(key: string): string | null {
  if (typeof window === "undefined") return null;
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

function writeLocalStorage(key: string, value: string): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(key, value);
  } catch {
  }
}
