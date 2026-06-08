import init, { RubcWasm } from "./wasm/rubc_wasm.js";

export const BTN = { A: 0, B: 1, SELECT: 2, START: 3, RIGHT: 4, LEFT: 5, UP: 6, DOWN: 7 };

const SAVE_DB = "rubc-saves";
const SAVE_STORE = "sav";

function openSaveDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(SAVE_DB, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(SAVE_STORE);
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
  wasm: any = null;
  emu: RubcWasm | null = null;
  audioCtx: AudioContext | null = null;
  nextAudioTime = 0;
  canvasCtx: CanvasRenderingContext2D | null = null;
  imageData: ImageData | null = null;
  rafId: number | null = null;
  paused = false;
  currentSaveKey: string | null = null;
  saveTimer: any = null;
  lastFrameTime: number | null = null;
  frameAccumulator = 0;
  
  onReady: () => void = () => {};
  onError: (err: Error) => void = () => {};

  constructor() {
    this.loop = this.loop.bind(this);
  }

  async init() {
    try {
      this.wasm = await init();
      this.onReady();
    } catch (err: any) {
      this.onError(err);
    }
  }

  ensureAudio() {
    if (this.audioCtx) return this.audioCtx;
    this.audioCtx = new (window.AudioContext || (window as any).webkitAudioContext)();
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

  drawFrame() {
    if (!this.emu || !this.canvasCtx || !this.imageData) return;
    const ptr = this.emu.frame_rgba();
    const view = new Uint8ClampedArray(this.wasm.memory.buffer, ptr, this.emu.frame_len);
    this.imageData.data.set(view);
    this.canvasCtx.putImageData(this.imageData, 0, 0);
  }

  loop(now: number) {
    this.rafId = requestAnimationFrame(this.loop);
    if (this.paused || !this.emu) {
      this.lastFrameTime = now;
      return;
    }
    if (this.lastFrameTime === null) this.lastFrameTime = now;
    this.frameAccumulator += now - this.lastFrameTime;
    this.lastFrameTime = now;

    const FRAME_MS = 1000 / 59.7275;
    let steps = 0;
    while (this.frameAccumulator >= FRAME_MS && steps < 4) {
      this.emu.step_frame();
      this.frameAccumulator -= FRAME_MS;
      steps++;
    }
    if (steps === 4 && this.frameAccumulator >= FRAME_MS) {
      this.frameAccumulator = 0;
    }

    if (steps > 0) {
      this.drawFrame();
      this.pumpAudio();
    }
    
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

  async loadRom(bytes: Uint8Array, canvas: HTMLCanvasElement) {
    await this.flushSave();
    if (this.saveTimer !== null) {
      clearInterval(this.saveTimer);
      this.saveTimer = null;
    }

    const rate = this.ensureAudio().sampleRate;
    this.emu = new RubcWasm(bytes, rate);
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
    if (this.rafId === null) this.rafId = requestAnimationFrame(this.loop);
  }

  setButton(btn: number, pressed: boolean) {
    if (this.emu) this.emu.set_button(btn, pressed);
  }

  destroy() {
    this.flushSave();
    if (this.rafId !== null) cancelAnimationFrame(this.rafId);
    if (this.saveTimer !== null) clearInterval(this.saveTimer);
    if (this.emu) this.emu.free();
  }
}

export const emulator = new EmulatorCore();
