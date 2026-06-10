export const STORAGE_KEY = "rubc.mobile.v1";
export const BOOT_DELAY_MS = 1500;
export const TOAST_DELAY_MS = 1700;

export type Palette = "auto" | "dmg" | "grayscale";
export type Scaling = "fit" | "integer";
export type ControlsLayout = "docked" | "overlay";
export type Phase = "empty" | "booting" | "running" | "paused";
export type View = "play" | "library" | "settings";

export interface EmulatorSettings {
  sound: boolean;
  volume: number;
  palette: Palette;
  turbo: boolean;
  scaling: Scaling;
  smoothing: boolean;
  haptics: boolean;
  showFps: boolean;
  controls: ControlsLayout;
  bootMode: "auto" | "dmg" | "cgb";
}

export interface SaveSlot {
  at: number;
  romId: string | null;
  thumb: string | null;
  label: string;
  elapsed: number;
}

export type SaveSlots = [SaveSlot | null, SaveSlot | null, SaveSlot | null, SaveSlot | null];

export interface EmulatorRom {
  id: string;
  title: string;
  name?: string;
  size?: number;
  mode?: "DMG" | "CGB";
  thumb: string | null;
  live: string | null;
  lastPlayed?: number;
}

export interface EmulatorState {
  settings: EmulatorSettings;
  romId: string | null;
  phase: Phase;
  view: View;
  menuOpen: boolean;
  slots: SaveSlots;
  elapsed: number;
  toast: string | null;
}

export interface EmulatorPersistence {
  settings: EmulatorSettings;
  romId: string | null;
  slots: SaveSlots;
  elapsed: number;
}

export type EmulatorAction =
  | { type: "hydrate"; persisted: EmulatorPersistence | null }
  | { type: "setSettings"; patch: Partial<EmulatorSettings> }
  | { type: "setView"; view: View }
  | { type: "setMenuOpen"; menuOpen: boolean }
  | { type: "boot"; romId: string }
  | { type: "bootFinished" }
  | { type: "togglePause" }
  | { type: "reset" }
  | { type: "powerOff" }
  | { type: "saveTo"; index: number; slot: SaveSlot }
  | { type: "loadFrom"; index: number; slot?: SaveSlot }
  | { type: "tick"; seconds?: number }
  | { type: "flash"; message: string }
  | { type: "clearToast" };

export const DEFAULT_SETTINGS: EmulatorSettings = {
  sound: false,
  volume: 70,
  palette: "auto",
  turbo: false,
  scaling: "fit",
  smoothing: false,
  haptics: true,
  showFps: true,
  controls: "docked",
  bootMode: "auto",
};

export function createEmptySlots(): SaveSlots {
  return [null, null, null, null];
}

export function createInitialState(): EmulatorState {
  return {
    settings: { ...DEFAULT_SETTINGS },
    romId: null,
    phase: "empty",
    view: "play",
    menuOpen: false,
    slots: createEmptySlots(),
    elapsed: 0,
    toast: null,
  };
}

export function paletteFilter(palette: Palette): string {
  if (palette === "dmg") {
    return "grayscale(1) brightness(.95) sepia(1) hue-rotate(58deg) saturate(2.4) contrast(1.05)";
  }
  if (palette === "grayscale") {
    return "grayscale(1) contrast(1.05)";
  }
  return "none";
}

export function emulatorReducer(state: EmulatorState, action: EmulatorAction): EmulatorState {
  switch (action.type) {
    case "hydrate":
      return hydrateState(state, action.persisted);
    case "setSettings":
      return { ...state, settings: normalizeSettings({ ...state.settings, ...action.patch }) };
    case "setView":
      return { ...state, view: action.view };
    case "setMenuOpen":
      return { ...state, menuOpen: action.menuOpen };
    case "boot":
      return { ...state, romId: action.romId, phase: "booting", view: "play", menuOpen: false };
    case "bootFinished":
      if (state.phase !== "booting" || !state.romId) return state;
      return { ...state, phase: "running" };
    case "togglePause":
      if (state.phase === "running") return { ...state, phase: "paused" };
      if (state.phase === "paused") return { ...state, phase: "running" };
      return state;
    case "reset":
      if (!state.romId) return state;
      return { ...state, phase: "booting", menuOpen: false };
    case "powerOff":
      return { ...state, phase: "empty", romId: null, menuOpen: false, elapsed: 0 };
    case "saveTo":
      return saveToSlot(state, action.index, action.slot);
    case "loadFrom":
      return loadFromSlot(state, action.index, action.slot);
    case "tick":
      return { ...state, elapsed: state.elapsed + (action.seconds ?? 1) };
    case "flash":
      return { ...state, toast: action.message };
    case "clearToast":
      return { ...state, toast: null };
  }
}

export function serializePersistence(state: EmulatorState): EmulatorPersistence {
  return {
    settings: state.settings,
    romId: state.romId,
    slots: state.slots,
    elapsed: state.elapsed,
  };
}

export function parsePersistence(raw: string | null): EmulatorPersistence | null {
  if (!raw) return null;
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return null;
  }
  if (!isRecord(parsed)) return null;

  return {
    settings: normalizeSettings(parsed.settings),
    romId: typeof parsed.romId === "string" ? parsed.romId : null,
    slots: normalizeSlots(parsed.slots),
    elapsed: normalizeElapsed(parsed.elapsed),
  };
}

export function hydrateState(state: EmulatorState, persisted: EmulatorPersistence | null): EmulatorState {
  if (!persisted) return state;
  return {
    ...state,
    settings: persisted.settings,
    romId: persisted.romId,
    slots: persisted.slots,
    elapsed: persisted.elapsed,
    phase: persisted.romId ? "paused" : state.phase,
  };
}

function saveToSlot(state: EmulatorState, index: number, slot: SaveSlot): EmulatorState {
  if (!isSlotIndex(index)) return state;
  const slots = state.slots.slice() as SaveSlots;
  slots[index] = slot;
  return { ...state, slots };
}

function loadFromSlot(state: EmulatorState, index: number, loadedSlot?: SaveSlot): EmulatorState {
  if (!isSlotIndex(index)) return state;
  const slot = loadedSlot ?? state.slots[index];
  if (!slot) return state;
  const slots = state.slots.slice() as SaveSlots;
  slots[index] = slot;
  return {
    ...state,
    slots,
    romId: slot.romId || state.romId,
    elapsed: slot.elapsed || 0,
    phase: "running",
    menuOpen: false,
  };
}

function isSlotIndex(index: number): index is 0 | 1 | 2 | 3 {
  return Number.isInteger(index) && index >= 0 && index < 4;
}

function normalizeSettings(value: unknown): EmulatorSettings {
  const settings: EmulatorSettings = { ...DEFAULT_SETTINGS };
  if (!isRecord(value)) return settings;
  if (typeof value.sound === "boolean") settings.sound = value.sound;
  if (isFiniteNumber(value.volume)) settings.volume = clamp(Math.round(value.volume), 0, 100);
  if (isPalette(value.palette)) settings.palette = value.palette;
  if (typeof value.turbo === "boolean") settings.turbo = value.turbo;
  if (isScaling(value.scaling)) settings.scaling = value.scaling;
  if (typeof value.smoothing === "boolean") settings.smoothing = value.smoothing;
  if (typeof value.haptics === "boolean") settings.haptics = value.haptics;
  if (typeof value.showFps === "boolean") settings.showFps = value.showFps;
  if (isControlsLayout(value.controls)) settings.controls = value.controls;
  if (isBootMode(value.bootMode)) settings.bootMode = value.bootMode;
  return settings;
}

function normalizeSlots(value: unknown): SaveSlots {
  const slots = createEmptySlots();
  if (!Array.isArray(value)) return slots;
  for (let i = 0; i < slots.length; i++) {
    slots[i] = normalizeSlot(value[i]);
  }
  return slots;
}

function normalizeSlot(value: unknown): SaveSlot | null {
  if (!isRecord(value) || !isFiniteNumber(value.at)) return null;
  return {
    at: value.at,
    romId: typeof value.romId === "string" ? value.romId : null,
    thumb: typeof value.thumb === "string" ? value.thumb : null,
    label: typeof value.label === "string" ? value.label : "—",
    elapsed: normalizeElapsed(value.elapsed),
  };
}

function normalizeElapsed(value: unknown): number {
  if (!isFiniteNumber(value)) return 0;
  return Math.max(0, Math.floor(value));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function isPalette(value: unknown): value is Palette {
  return value === "auto" || value === "dmg" || value === "grayscale";
}

function isScaling(value: unknown): value is Scaling {
  return value === "fit" || value === "integer";
}

function isControlsLayout(value: unknown): value is ControlsLayout {
  return value === "docked" || value === "overlay";
}

function isBootMode(value: unknown): value is "auto" | "dmg" | "cgb" {
  return value === "auto" || value === "dmg" || value === "cgb";
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}
