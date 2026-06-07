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

// Bound on how far the audio schedule cursor may run ahead of real playback.
// Mirrors the native ring buffer's MAX_BUFFERED_FRAMES latency cap: once more
// than this many seconds are queued we drop fresh chunks instead of letting
// latency grow without bound.
const MAX_AUDIO_LEAD = 0.25;

// Create the AudioContext eagerly so its real device `sampleRate` is known at
// ROM-load time. RubcWasm fixes the APU sample rate in its constructor and
// exposes no setter, so the emulator MUST be built with the context's rate up
// front (see loadRom). The context starts "suspended" under the browser autoplay
// policy and only produces sound after the user clicks "Enable sound"
// (resume()); its sampleRate is readable while suspended.
function ensureAudio() {
  if (audioCtx) return audioCtx;
  audioCtx = new (window.AudioContext || window.webkitAudioContext)();
  nextAudioTime = audioCtx.currentTime;
  return audioCtx;
}

// Push interleaved-stereo f32 samples into the Web Audio scheduling queue.
//
// drain_audio() is called EVERY tick, even while muted, so the APU's internal
// sample buffer can never grow unbounded (the native frontend likewise consumes
// audio every step). While the context is suspended the drained samples are
// simply discarded.
function pumpAudio() {
  if (!emu) return;
  const interleaved = emu.drain_audio(); // Float32Array, L,R,L,R,... — always drain
  if (!audioCtx || audioCtx.state !== "running") return; // muted: discard samples
  const frames = interleaved.length >> 1;
  if (frames === 0) return;

  const now = audioCtx.currentTime;
  // Underrun: the cursor fell behind real playback (after a stall, a resume, or
  // first start). Snap it just ahead of `now` so we never schedule in the past
  // (which drops the buffer) and so a fresh resume starts cleanly instead of
  // trying to flush a huge backlog.
  if (nextAudioTime < now + 0.02) nextAudioTime = now + 0.02;
  // Overrun: more than MAX_AUDIO_LEAD seconds are already queued ahead of the
  // clock. Drop this chunk to bound latency. The browser can't un-schedule
  // already-started buffers, so this drop-newest is the practical analogue of
  // the native ring buffer's drop-oldest policy. With correct frame pacing
  // (below) emulation no longer outruns realtime, so this rarely triggers.
  if (nextAudioTime > now + MAX_AUDIO_LEAD) return;

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
  src.start(nextAudioTime);
  nextAudioTime += frames / audioCtx.sampleRate;
}

// --- main loop ----------------------------------------------------------
// The Game Boy refreshes at 59.7275 Hz — one frame every ~16.7427 ms. The
// browser's requestAnimationFrame fires at the DISPLAY refresh rate (60 Hz, but
// 120/144 Hz on high-refresh panels), which is NOT the GB rate. Stepping once
// per rAF would run the emulator at the monitor's rate (up to 2.4x too fast on
// a 144 Hz display) AND make the APU produce samples faster than the audio
// device drains them, so audio drifts ever further behind the picture.
//
// Instead we pace against the wall clock with a time accumulator: measure the
// real elapsed time each rAF and step exactly as many emulator frames as fit
// into it. This decouples emulation speed from refresh rate — 60/120/144 Hz all
// run the GB at the correct 59.7275 Hz.
const GB_FRAME_RATE = 59.7275;            // Game Boy frames per second
const FRAME_MS = 1000 / GB_FRAME_RATE;    // ~16.7427 ms per emulated frame
const MAX_CATCHUP_STEPS = 4;              // cap steps/rAF to avoid a spiral of death

let lastFrameTime = null;  // rAF timestamp of the previous tick (ms)
let frameAccumulator = 0;  // unspent real time carried between ticks (ms)

function drawFrame() {
  // Recreate the typed-array view every frame: wasm memory can grow and
  // detach the previous ArrayBuffer.
  const ptr = emu.frame_rgba();
  const view = new Uint8ClampedArray(wasm.memory.buffer, ptr, emu.frame_len);
  imageData.data.set(view);
  ctx.putImageData(imageData, 0, 0);
}

function loop(now) {
  rafId = requestAnimationFrame(loop);
  if (paused || !emu) {
    // Don't accumulate time while paused/idle, or we'd fast-forward on resume.
    lastFrameTime = now;
    return;
  }
  if (lastFrameTime === null) lastFrameTime = now;
  frameAccumulator += now - lastFrameTime;
  lastFrameTime = now;

  let steps = 0;
  while (frameAccumulator >= FRAME_MS && steps < MAX_CATCHUP_STEPS) {
    emu.step_frame();
    frameAccumulator -= FRAME_MS;
    steps++;
  }
  // Hit the catch-up cap (e.g. the tab was backgrounded for a while): drop the
  // remaining backlog instead of permanently running fast to "catch up".
  if (steps === MAX_CATCHUP_STEPS && frameAccumulator >= FRAME_MS) {
    frameAccumulator = 0;
  }

  // Draw the framebuffer once per rAF (only when we advanced), and feed audio.
  if (steps > 0) {
    drawFrame();
    pumpAudio();
  }
}

// --- ROM loading --------------------------------------------------------
async function loadRom(bytes) {
  // Create the AudioContext now (if not already) so the emulator is built with
  // the real device sample rate — RubcWasm has no post-construction rate setter.
  const rate = ensureAudio().sampleRate;
  emu = new RubcWasm(bytes, rate);
  imageData = ctx.createImageData(emu.width, emu.height);
  paused = false;
  pauseBtn.disabled = false;
  soundBtn.disabled = false;
  pauseBtn.textContent = "Pause";
  statusEl.textContent = `Running — ${emu.is_cgb ? "CGB" : "DMG"} mode (${bytes.length} bytes).`;
  // Kick the loop via rAF so the first `now` is a real timestamp (never undefined).
  if (rafId === null) rafId = requestAnimationFrame(loop);
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
  // Reset the schedule cursor relative to now so the first post-resume buffer
  // starts cleanly with a small lead instead of flushing a stale backlog.
  nextAudioTime = ac.currentTime + 0.02;
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
