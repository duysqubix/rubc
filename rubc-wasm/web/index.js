// rubc WebAssembly demo — dependency-free vanilla JS.
//
// Loads the wasm module, lets you pick a .gb/.gbc ROM, runs a
// requestAnimationFrame loop that steps one frame, draws the RGBA framebuffer
// to a <canvas> via ImageData (zero-copy view over wasm memory), wires keyboard
// events to the joypad, and feeds drained APU samples to a Web Audio context.
//
// Build the `pkg/` directory first (see ../../justfile `wasm-build`), then serve
// this folder over HTTP (ES modules need http://, not file://):
//   python3 -m http.server   # then open http://localhost:8000/

import init, { RubcWasm } from "./pkg/rubc_wasm.js";

// JS button codes — must match rubc_wasm::button_from_code.
const BTN = { A: 0, B: 1, SELECT: 2, START: 3, RIGHT: 4, LEFT: 5, UP: 6, DOWN: 7 };

const KEY_MAP = {
  ArrowRight: BTN.RIGHT,
  ArrowLeft: BTN.LEFT,
  ArrowUp: BTN.UP,
  ArrowDown: BTN.DOWN,
  KeyX: BTN.A,
  KeyZ: BTN.B,
  Enter: BTN.START,
  ShiftRight: BTN.SELECT,
  Backspace: BTN.SELECT,
};

const canvas = document.getElementById("screen");
const ctx = canvas.getContext("2d");
const statusEl = document.getElementById("status");
const romInput = document.getElementById("rom");
const pauseBtn = document.getElementById("pause");
const soundBtn = document.getElementById("sound");

let wasm = null;          // wasm exports (for `.memory`)
let emu = null;           // RubcWasm instance
let imageData = null;     // reused ImageData backing the canvas
let rafId = null;
let paused = false;

// --- audio --------------------------------------------------------------
let audioCtx = null;
let nextAudioTime = 0;    // running schedule cursor (seconds)

function ensureAudio() {
  if (audioCtx) return audioCtx;
  audioCtx = new (window.AudioContext || window.webkitAudioContext)();
  nextAudioTime = audioCtx.currentTime;
  return audioCtx;
}

// Push interleaved-stereo f32 samples into the Web Audio scheduling queue.
function pumpAudio() {
  if (!emu || !audioCtx || audioCtx.state !== "running") return;
  const interleaved = emu.drain_audio(); // Float32Array, L,R,L,R,...
  const frames = interleaved.length >> 1;
  if (frames === 0) return;

  const buf = audioCtx.createBuffer(2, frames, audioCtx.sampleRate);
  const left = buf.getChannelData(0);
  const right = buf.getChannelData(1);
  for (let i = 0; i < frames; i++) {
    left[i] = interleaved[2 * i];
    right[i] = interleaved[2 * i + 1];
  }

  const src = audioCtx.createBufferSource();
  src.buffer = buf;
  src.connect(audioCtx.destination);

  // Keep a small lead so we never schedule in the past (which drops audio).
  const now = audioCtx.currentTime;
  if (nextAudioTime < now + 0.02) nextAudioTime = now + 0.02;
  src.start(nextAudioTime);
  nextAudioTime += frames / audioCtx.sampleRate;
}

// --- main loop ----------------------------------------------------------
function drawFrame() {
  // Recreate the typed-array view every frame: wasm memory can grow and
  // detach the previous ArrayBuffer.
  const ptr = emu.frame_rgba();
  const view = new Uint8ClampedArray(wasm.memory.buffer, ptr, emu.frame_len);
  imageData.data.set(view);
  ctx.putImageData(imageData, 0, 0);
}

function loop() {
  if (!paused && emu) {
    emu.step_frame();
    drawFrame();
    pumpAudio();
  }
  rafId = requestAnimationFrame(loop);
}

// --- ROM loading --------------------------------------------------------
async function loadRom(bytes) {
  const rate = audioCtx ? audioCtx.sampleRate : 48000;
  emu = new RubcWasm(bytes, rate);
  imageData = ctx.createImageData(emu.width, emu.height);
  paused = false;
  pauseBtn.disabled = false;
  soundBtn.disabled = false;
  pauseBtn.textContent = "Pause";
  statusEl.textContent = `Running — ${emu.is_cgb ? "CGB" : "DMG"} mode (${bytes.length} bytes).`;
  if (rafId === null) loop();
}

romInput.addEventListener("change", async (e) => {
  const file = e.target.files[0];
  if (!file) return;
  statusEl.textContent = `Loading ${file.name}…`;
  const bytes = new Uint8Array(await file.arrayBuffer());
  try {
    await loadRom(bytes);
  } catch (err) {
    statusEl.textContent = `Failed to load ROM: ${err}`;
    console.error(err);
  }
});

pauseBtn.addEventListener("click", () => {
  paused = !paused;
  pauseBtn.textContent = paused ? "Resume" : "Pause";
});

soundBtn.addEventListener("click", async () => {
  const ac = ensureAudio();
  await ac.resume();
  soundBtn.textContent = "Sound on";
  soundBtn.disabled = true;
  // If a ROM is already running, re-create the cursor relative to now.
  nextAudioTime = ac.currentTime;
});

// --- input --------------------------------------------------------------
window.addEventListener("keydown", (e) => {
  const code = KEY_MAP[e.code];
  if (code !== undefined && emu) {
    emu.set_button(code, true);
    e.preventDefault();
  }
});
window.addEventListener("keyup", (e) => {
  const code = KEY_MAP[e.code];
  if (code !== undefined && emu) {
    emu.set_button(code, false);
    e.preventDefault();
  }
});

// --- boot ---------------------------------------------------------------
init()
  .then((exports) => {
    wasm = exports;
    statusEl.textContent = "wasm ready — pick a Game Boy ROM to start.";
  })
  .catch((err) => {
    statusEl.textContent = `Failed to initialise wasm: ${err}. Did you run the wasm-build recipe and serve over HTTP?`;
    console.error(err);
  });
