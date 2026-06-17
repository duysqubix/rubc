import { GLRenderer } from "./shaders";
import type { ShaderEffect } from "./shaders";
import init, { RubcWasm } from "./wasm/rubc_wasm.js";
import type { InitOutput } from "./wasm/rubc_wasm.js";
import {
  createSaveStateRecord,
  isSaveStateSlotIndex,
  normalizeSaveStateRecord,
  saveStateRecordKey,
  slotFromSaveStateRecord,
} from "./save-state-record";
import type { SaveStateRecord, SaveStateSlotMetadata } from "./save-state-record";

export const BTN = { A: 0, B: 1, SELECT: 2, START: 3, RIGHT: 4, LEFT: 5, UP: 6, DOWN: 7 };

export interface LoadedRom {
  bytes: Uint8Array;
  /** Resolved ROM filename (the inner entry when a .zip was supplied). */
  name: string;
}

export interface SaveStateDetails {
  label?: string;
  elapsed?: number;
}

type AudioWindow = Window & { webkitAudioContext?: typeof AudioContext };

function isRomName(name: string): boolean {
  const n = name.toLowerCase();
  return n.endsWith(".gb") || n.endsWith(".gbc");
}

/**
 * Resolve a dropped/picked file into ROM bytes. Accepts a raw `.gb`/`.gbc`
 * file directly, or a `.zip` from which the first Game Boy ROM is extracted.
 * Throws with a user-facing message when no playable ROM is found.
 */
export async function loadRomFile(file: File): Promise<LoadedRom> {
  const buffer = await file.arrayBuffer();
  const bytes = new Uint8Array(buffer);

  if (isRomName(file.name)) {
    return { bytes, name: file.name };
  }

  if (file.name.toLowerCase().endsWith(".zip")) {
    const { unzipSync } = await import("fflate");
    let entries: Record<string, Uint8Array>;
    try {
      entries = unzipSync(bytes);
    } catch {
      throw new Error("Could not read that .zip file.");
    }
    // Skip macOS resource forks / directory entries; pick the first ROM.
    const romName = Object.keys(entries)
      .filter((n) => !n.startsWith("__MACOSX/") && !n.endsWith("/"))
      .find(isRomName);
    if (!romName) {
      throw new Error("No .gb or .gbc ROM found inside that .zip.");
    }
    return { bytes: entries[romName], name: romName.split("/").pop() || romName };
  }

  throw new Error("Please select a .gb, .gbc, or .zip file.");
}

const SAVE_DB = "rubc-saves";
const SAVE_STORE = "sav";
const ROM_STORE = "rom";
const SAVE_STATE_STORE = "state";

function openSaveDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(SAVE_DB, 3);
    req.onupgradeneeded = () => {
      const db = req.result;
      if (!db.objectStoreNames.contains(SAVE_STORE)) db.createObjectStore(SAVE_STORE);
      if (!db.objectStoreNames.contains(ROM_STORE)) db.createObjectStore(ROM_STORE);
      if (!db.objectStoreNames.contains(SAVE_STATE_STORE)) db.createObjectStore(SAVE_STATE_STORE);
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

export function getRomKey(bytes: Uint8Array): string {
  let title = "";
  for (let i = 0x134; i < 0x144 && i < bytes.length; i++) {
    const c = bytes[i];
    if (c === 0) break;
    if (c >= 0x20 && c < 0x7f) title += String.fromCharCode(c);
  }
  return `${title || "untitled"}:${bytes.length}`;
}

export async function loadSaveRam(key: string): Promise<Uint8Array | null> {
  try {
    const db = await openSaveDb();
    return await new Promise((resolve, reject) => {
      const tx = db.transaction(SAVE_STORE, "readonly");
      const req = tx.objectStore(SAVE_STORE).get(key);
      req.onsuccess = () => resolve(req.result ? new Uint8Array(req.result) : null);
      req.onerror = () => reject(req.error);
    });
  } catch (e) {
    console.warn("save load failed:", e);
    return null;
  }
}

export async function storeSaveRam(key: string, bytes: Uint8Array): Promise<void> {
  try {
    const db = await openSaveDb();
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(SAVE_STORE, "readwrite");
      tx.objectStore(SAVE_STORE).put(bytes.slice(), key);
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  } catch (e) {
    console.warn("save store failed:", e);
  }
}

// Cache the ROM bytes themselves (keyed like saves) so the Recent Games list
// can reopen a game without the user re-picking the file.
export async function storeRomBytes(key: string, bytes: Uint8Array): Promise<void> {
  try {
    const db = await openSaveDb();
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(ROM_STORE, "readwrite");
      tx.objectStore(ROM_STORE).put(bytes.slice(), key);
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  } catch (e) {
    console.warn("rom store failed:", e);
  }
}

export async function loadRomBytes(key: string): Promise<Uint8Array | null> {
  try {
    const db = await openSaveDb();
    return await new Promise((resolve, reject) => {
      const tx = db.transaction(ROM_STORE, "readonly");
      const req = tx.objectStore(ROM_STORE).get(key);
      req.onsuccess = () => resolve(req.result ? new Uint8Array(req.result) : null);
      req.onerror = () => reject(req.error);
    });
  } catch (e) {
    console.warn("rom load failed:", e);
    return null;
  }
}

export async function storeSaveStateRecord(slot: number, record: SaveStateRecord): Promise<boolean> {
  if (!isSaveStateSlotIndex(slot)) return false;
  try {
    const db = await openSaveDb();
    const key = saveStateRecordKey(record.romId, slot);
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(SAVE_STATE_STORE, "readwrite");
      tx.objectStore(SAVE_STATE_STORE).put(record, key);
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
    return true;
  } catch (e) {
    console.warn("state store failed:", e);
    return false;
  }
}

export async function loadSaveStateRecord(romId: string, slot: number): Promise<SaveStateRecord | null> {
  if (!isSaveStateSlotIndex(slot)) return null;
  try {
    const db = await openSaveDb();
    const key = saveStateRecordKey(romId, slot);
    return await new Promise((resolve, reject) => {
      const tx = db.transaction(SAVE_STATE_STORE, "readonly");
      const req = tx.objectStore(SAVE_STATE_STORE).get(key);
      req.onsuccess = () => {
        const result: unknown = req.result;
        resolve(normalizeSaveStateRecord(result));
      };
      req.onerror = () => reject(req.error);
    });
  } catch (e) {
    console.warn("state load failed:", e);
    return null;
  }
}

export async function deleteRomBytes(key: string): Promise<void> {
  try {
    const db = await openSaveDb();
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(ROM_STORE, "readwrite");
      tx.objectStore(ROM_STORE).delete(key);
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  } catch (e) {
    console.warn("rom delete failed:", e);
  }
}

export async function exportSave(key: string): Promise<void> {
  const data = await loadSaveRam(key);
  if (!data) return;
  const blob = new Blob([data.slice().buffer as ArrayBuffer], { type: "application/octet-stream" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `${key.replace(/[^a-z0-9]/gi, '_')}.sav`;
  a.click();
  URL.revokeObjectURL(url);
}

export async function importSave(key: string, file: File): Promise<void> {
  const buffer = await file.arrayBuffer();
  await storeSaveRam(key, new Uint8Array(buffer));
}

export class EmulatorCore {
  glRenderer: GLRenderer | null = null;
  shaderEffect: ShaderEffect = "off";
  wasm: InitOutput | null = null;
  emu: RubcWasm | null = null;
  audioCtx: AudioContext | null = null;
  nextAudioTime = 0;
  canvasCtx: CanvasRenderingContext2D | null = null;
  imageData: ImageData | null = null;
  rafId: number | null = null;
  paused = false;
  currentSaveKey: string | null = null;
  saveTimer: ReturnType<typeof setInterval> | null = null;
  lastFrameTime: number | null = null;
  frameAccumulator = 0;
  // Emulation speed multiplier (1 = normal ~59.7Hz). Turbo sets this to 2+.
  // Speed is purely a frontend-loop concern: step_frame() is one GB frame, so
  // Nx = N step_frame() calls per display refresh. No core change needed.
  speed = 1;
  prevSpeed = 1;
  
  onReady: () => void = () => {};
  onError: (err: Error) => void = () => {};

  constructor() {
    this.loop = this.loop.bind(this);
  }

  async init() {
    try {
      this.wasm = await init({ module_or_path: "/rubc_wasm_bg.wasm?v=" + Date.now() });
      this.onReady();
    } catch (err: unknown) {
      this.onError(err instanceof Error ? err : new Error("Could not initialize emulator core."));
    }
  }

  ensureAudio() {
    if (this.audioCtx) return this.audioCtx;
    const AudioContextCtor = window.AudioContext ?? (window as AudioWindow).webkitAudioContext;
    if (!AudioContextCtor) throw new Error("Web Audio is not available in this browser.");
    this.audioCtx = new AudioContextCtor();
    this.nextAudioTime = this.audioCtx.currentTime;
    return this.audioCtx;
  }

  async resumeAudio() {
    const ac = this.ensureAudio();
    if (ac.state === "suspended") {
      await ac.resume();
      this.nextAudioTime = ac.currentTime + 0.02;
    }
  }

  pumpAudio() {
    if (!this.emu) return;
    const interleaved = this.emu.drain_audio();
    if (!this.audioCtx || this.audioCtx.state !== "running") return;
    const frames = interleaved.length >> 1;
    if (frames === 0) return;

    const now = this.audioCtx.currentTime;
    if (this.nextAudioTime < now + 0.02) this.nextAudioTime = now + 0.02;
    if (this.nextAudioTime > now + 0.25) return;

    const buf = this.audioCtx.createBuffer(2, frames, this.audioCtx.sampleRate);
    buf.getChannelData(0).set(interleaved.filter((_, i) => i % 2 === 0));
    buf.getChannelData(1).set(interleaved.filter((_, i) => i % 2 === 1));

    const src = this.audioCtx.createBufferSource();
    src.buffer = buf;
    src.connect(this.audioCtx.destination);
    src.start(this.nextAudioTime);
    this.nextAudioTime += frames / this.audioCtx.sampleRate;
  }

  setShaderEffect(effect: ShaderEffect) {
    this.shaderEffect = effect;
    if (this.glRenderer) {
      this.glRenderer.setEffect(effect);
    }
  }

  drawFrame() {
    if (!this.emu || !this.wasm || !this.canvasCtx || !this.imageData) return;
    try {
      const ptr = this.emu.frame_rgba();
      // Rebuild the view every frame: wasm memory can grow and detach the buffer.
      const view = new Uint8ClampedArray(this.wasm.memory.buffer, ptr, this.emu.frame_len);
      
      if (this.shaderEffect !== "off") {
        if (!this.glRenderer) {
          this.glRenderer = new GLRenderer();
          this.glRenderer.setEffect(this.shaderEffect);
        }
        const offscreen = this.glRenderer.draw(view);
        if (offscreen) {
          if (this.canvasCtx.canvas.width !== offscreen.width || this.canvasCtx.canvas.height !== offscreen.height) {
            this.canvasCtx.canvas.width = offscreen.width;
            this.canvasCtx.canvas.height = offscreen.height;
          }
          this.canvasCtx.drawImage(offscreen, 0, 0);
          return;
        }
      }

      if (this.canvasCtx.canvas.width !== 160 || this.canvasCtx.canvas.height !== 144) {
        this.canvasCtx.canvas.width = 160;
        this.canvasCtx.canvas.height = 144;
      }
      this.imageData.data.set(view);
      this.canvasCtx.putImageData(this.imageData, 0, 0);
    } catch {
      // Detached buffer / torn-down emu mid-frame: skip this frame quietly.
    }
  }

  loop(now: number) {
    // If the game was torn down, stop the loop entirely (do NOT reschedule).
    if (!this.emu) {
      this.rafId = null;
      return;
    }
    this.rafId = requestAnimationFrame(this.loop);
    if (this.paused) {
      this.lastFrameTime = now;
      return;
    }
    if (this.lastFrameTime === null) this.lastFrameTime = now;
    this.frameAccumulator += now - this.lastFrameTime;
    this.lastFrameTime = now;

    const FRAME_MS = 1000 / 59.7275;
    const speed = this.speed >= 1 ? this.speed : 1;
    // Cap catch-up at 4 display-frames worth of work, scaled by speed.
    const maxSteps = 4 * speed;
    let steps = 0;
    while (this.frameAccumulator >= FRAME_MS && steps < maxSteps) {
      // Advance `speed` emulated frames per consumed display-frame budget.
      for (let i = 0; i < speed; i++) this.emu.step_frame();
      this.frameAccumulator -= FRAME_MS;
      steps++;
    }
    if (steps >= maxSteps && this.frameAccumulator >= FRAME_MS) {
      this.frameAccumulator = 0;
    }

    if (steps > 0) {
      this.drawFrame();
      // Resync audio scheduling on any speed change: nextAudioTime can be stale
      // (in the past) after a transition, so buffers would be scheduled in the
      // past and silently dropped -> sound never recovers.
      if (speed !== this.prevSpeed && this.audioCtx) {
        this.nextAudioTime = this.audioCtx.currentTime + 0.02;
      }
      // Always pump audio, including at turbo. At >1x the core produces audio
      // faster than realtime, so it plays back faster/pitched-up -- that's the
      // expected turbo sound (and it keeps draining the wasm buffer so the APU
      // never stalls). pumpAudio caps how far ahead it schedules.
      this.pumpAudio();
    }
    this.prevSpeed = speed;
    
    this.pollGamepad();
  }

  pollGamepad() {
    if (!this.emu) return;
    const gamepads = navigator.getGamepads ? navigator.getGamepads() : [];
    for (let i = 0; i < gamepads.length; i++) {
      const gp = gamepads[i];
      if (!gp) continue;
      // Standard gamepad mapping
      this.emu.set_button(BTN.A, gp.buttons[0]?.pressed || gp.buttons[1]?.pressed); // A or B (cross/circle)
      this.emu.set_button(BTN.B, gp.buttons[2]?.pressed || gp.buttons[3]?.pressed); // X or Y (square/triangle)
      this.emu.set_button(BTN.SELECT, gp.buttons[8]?.pressed);
      this.emu.set_button(BTN.START, gp.buttons[9]?.pressed);
      this.emu.set_button(BTN.UP, gp.buttons[12]?.pressed || gp.axes[1] < -0.5);
      this.emu.set_button(BTN.DOWN, gp.buttons[13]?.pressed || gp.axes[1] > 0.5);
      this.emu.set_button(BTN.LEFT, gp.buttons[14]?.pressed || gp.axes[0] < -0.5);
      this.emu.set_button(BTN.RIGHT, gp.buttons[15]?.pressed || gp.axes[0] > 0.5);
      break; // Only use first connected gamepad
    }
  }

  async flushSave() {
    if (!this.emu || !this.currentSaveKey || !this.emu.has_battery) return;
    const ram = this.emu.save_ram();
    if (ram.length === 0) return;
    await storeSaveRam(this.currentSaveKey, ram);
  }

  async saveState(slot: number, details: SaveStateDetails = {}): Promise<SaveStateSlotMetadata | null> {
    if (!isSaveStateSlotIndex(slot) || !this.emu || !this.currentSaveKey) return null;
    try {
      const record = createSaveStateRecord({
        romId: this.currentSaveKey,
        thumb: this.captureThumbnail(),
        label: details.label ?? this.currentSaveKey.split(":")[0],
        elapsed: details.elapsed ?? 0,
        data: this.emu.save_state(),
      });
      const stored = await storeSaveStateRecord(slot, record);
      return stored ? slotFromSaveStateRecord(record) : null;
    } catch (e) {
      console.warn("state save failed:", e);
      return null;
    }
  }

  async loadState(slot: number): Promise<SaveStateSlotMetadata | null> {
    if (!isSaveStateSlotIndex(slot) || !this.emu || !this.currentSaveKey) return null;
    const record = await loadSaveStateRecord(this.currentSaveKey, slot);
    if (!record) return null;
    if (record.romId !== this.currentSaveKey) {
      console.warn("state load rejected: ROM mismatch");
      return null;
    }
    try {
      if (!this.emu.load_state(record.data)) {
        console.warn("state load rejected: incompatible snapshot");
        return null;
      }
      this.drawFrame();
      return slotFromSaveStateRecord(record);
    } catch (e) {
      console.warn("state load failed:", e);
      return null;
    }
  }

  private captureThumbnail(): string | null {
    if (!this.canvasCtx) return null;
    this.drawFrame();
    try {
      return this.canvasCtx.canvas.toDataURL("image/png");
    } catch (e) {
      console.warn("state thumbnail failed:", e);
      return null;
    }
  }

  async loadRom(bytes: Uint8Array, canvas: HTMLCanvasElement, bootMode: "auto" | "dmg" | "cgb" = "auto") {
    // Tear down any previous game first so reopening is a clean restart.
    await this.flushSave();
    if (this.saveTimer !== null) {
      clearInterval(this.saveTimer);
      this.saveTimer = null;
    }
    if (this.rafId !== null) {
      cancelAnimationFrame(this.rafId);
      this.rafId = null;
    }
    if (this.emu) {
      this.emu.free();
      this.emu = null;
    }
    this.lastFrameTime = null;
    this.frameAccumulator = 0;

    const rate = this.ensureAudio().sampleRate;
    this.emu = new RubcWasm(bytes, rate, bootMode === "auto" ? undefined : bootMode);
    this.currentSaveKey = getRomKey(bytes);

    if (this.emu.has_battery) {
      const saved = await loadSaveRam(this.currentSaveKey);
      if (saved) this.emu.load_ram(saved);
      this.saveTimer = setInterval(() => this.flushSave(), 10000);
    }

    this.canvasCtx = canvas.getContext("2d");
    if (this.canvasCtx) {
      this.imageData = this.canvasCtx.createImageData(this.emu.width, this.emu.height);
    }

    this.paused = false;
    this.rafId = requestAnimationFrame(this.loop);
  }

  // Retarget the draw surface to a new canvas WITHOUT reloading the ROM. Needed
  // when the Viewport remounts (e.g. switching to Library and back): the running
  // emulator keeps drawing, but to a now-detached canvas, leaving the new one
  // black. Rebind the 2D context + imageData so drawFrame paints the live canvas.
  setCanvas(canvas: HTMLCanvasElement) {
    if (!this.emu) return;
    this.canvasCtx = canvas.getContext("2d");
    if (this.canvasCtx) {
      this.imageData = this.canvasCtx.createImageData(this.emu.width, this.emu.height);
      this.drawFrame();
    }
  }

  setButton(btn: number, pressed: boolean) {
    if (this.emu) this.emu.set_button(btn, pressed);
  }

  async destroy() {
    // Stop the loop BEFORE freeing wasm so loop() can never touch freed memory.
    if (this.rafId !== null) {
      cancelAnimationFrame(this.rafId);
      this.rafId = null;
    }
    if (this.saveTimer !== null) {
      clearInterval(this.saveTimer);
      this.saveTimer = null;
    }
    // Await the save so we never free() wasm while save_ram() is still reading it.
    await this.flushSave();
    if (this.emu) {
      this.emu.free();
      this.emu = null;
    }
    if (this.glRenderer) {
      this.glRenderer.dispose();
      this.glRenderer = null;
    }
    this.currentSaveKey = null;
    this.canvasCtx = null;
    this.imageData = null;
    this.lastFrameTime = null;
    this.frameAccumulator = 0;
  }
}

export const emulator = new EmulatorCore();
