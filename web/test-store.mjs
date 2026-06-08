import assert from "assert/strict";
import fs from "fs";
import path from "path";
import ts from "typescript";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const logicPath = path.join(__dirname, "src/lib/store-logic.ts");

if (!fs.existsSync(logicPath)) {
  console.error("❌ src/lib/store-logic.ts not found");
  process.exit(1);
}

const source = fs.readFileSync(logicPath, "utf8");
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
    isolatedModules: true,
  },
});
const moduleUrl = `data:text/javascript;base64,${Buffer.from(compiled.outputText).toString("base64")}`;
const store = await import(moduleUrl);

const {
  DEFAULT_SETTINGS,
  STORAGE_KEY,
  createEmptySlots,
  createInitialState,
  emulatorReducer,
  hydrateState,
  paletteFilter,
  parsePersistence,
  serializePersistence,
} = store;

assert.equal(STORAGE_KEY, "rubc.mobile.v1");
assert.deepEqual(DEFAULT_SETTINGS, {
  sound: false,
  volume: 70,
  palette: "auto",
  turbo: false,
  scaling: "fit",
  smoothing: false,
  haptics: true,
  showFps: true,
  controls: "docked",
});

assert.equal(paletteFilter("dmg"), "grayscale(1) brightness(.95) sepia(1) hue-rotate(58deg) saturate(2.4) contrast(1.05)");
assert.equal(paletteFilter("grayscale"), "grayscale(1) contrast(1.05)");
assert.equal(paletteFilter("auto"), "none");

const initial = createInitialState();
assert.deepEqual(initial, {
  settings: DEFAULT_SETTINGS,
  romId: null,
  phase: "empty",
  view: "play",
  menuOpen: false,
  slots: createEmptySlots(),
  elapsed: 0,
  toast: null,
});

const booting = emulatorReducer(initial, { type: "boot", romId: "PKMN:2097152" });
assert.equal(booting.romId, "PKMN:2097152");
assert.equal(booting.phase, "booting");
assert.equal(booting.view, "play");
assert.equal(booting.menuOpen, false);

const running = emulatorReducer(booting, { type: "bootFinished" });
assert.equal(running.phase, "running");
assert.equal(emulatorReducer(running, { type: "togglePause" }).phase, "paused");
assert.equal(emulatorReducer({ ...running, phase: "paused" }, { type: "togglePause" }).phase, "running");
assert.equal(emulatorReducer(initial, { type: "togglePause" }).phase, "empty");

const reset = emulatorReducer({ ...running, menuOpen: true }, { type: "reset" });
assert.equal(reset.phase, "booting");
assert.equal(reset.menuOpen, false);
assert.equal(emulatorReducer(initial, { type: "reset" }), initial);

const patched = emulatorReducer(initial, { type: "setSettings", patch: { sound: true, volume: 25, controls: "overlay" } });
assert.equal(patched.settings.sound, true);
assert.equal(patched.settings.volume, 25);
assert.equal(patched.settings.controls, "overlay");
assert.equal(patched.settings.palette, "auto");

const withElapsed = { ...running, elapsed: 42 };
const saved = emulatorReducer(withElapsed, {
  type: "saveTo",
  index: 1,
  now: 1234,
  rom: { id: "PKMN:2097152", title: "Pokémon Crystal", thumb: null, live: null },
});
assert.deepEqual(saved.slots[1], {
  at: 1234,
  romId: "PKMN:2097152",
  thumb: null,
  label: "Pokémon Crystal",
  elapsed: 42,
});
assert.equal(saved.slots[0], null);

const loaded = emulatorReducer({ ...saved, romId: "other", menuOpen: true, phase: "paused" }, { type: "loadFrom", index: 1 });
assert.equal(loaded.romId, "PKMN:2097152");
assert.equal(loaded.elapsed, 42);
assert.equal(loaded.phase, "running");
assert.equal(loaded.menuOpen, false);
assert.equal(emulatorReducer(initial, { type: "loadFrom", index: 2 }), initial);

const flashed = emulatorReducer(initial, { type: "flash", message: "State saved · slot 2" });
assert.equal(flashed.toast, "State saved · slot 2");
assert.equal(emulatorReducer(flashed, { type: "clearToast" }).toast, null);

const poweredOff = emulatorReducer({ ...saved, phase: "running", menuOpen: true }, { type: "powerOff" });
assert.equal(poweredOff.phase, "empty");
assert.equal(poweredOff.romId, null);
assert.equal(poweredOff.menuOpen, false);
assert.equal(poweredOff.elapsed, 0);

const persisted = serializePersistence(saved);
assert.deepEqual(Object.keys(persisted).sort(), ["elapsed", "romId", "settings", "slots"]);
assert.equal(persisted.romId, "PKMN:2097152");
assert.equal(persisted.elapsed, 42);
assert.deepEqual(persisted.slots, saved.slots);

assert.equal(parsePersistence("not-json"), null);
const parsed = parsePersistence(JSON.stringify({
  settings: { sound: true, palette: "dmg", invalid: true },
  romId: "PKMN:2097152",
  slots: [saved.slots[1]],
  elapsed: 99,
  phase: "running",
}));
assert.deepEqual(parsed.settings, { ...DEFAULT_SETTINGS, sound: true, palette: "dmg" });
assert.equal(parsed.romId, "PKMN:2097152");
assert.equal(parsed.slots.length, 4);
assert.deepEqual(parsed.slots[0], saved.slots[1]);
assert.equal(parsed.slots[1], null);
assert.equal(parsed.elapsed, 99);
assert.equal(parsed.phase, undefined);

const hydrated = hydrateState(initial, parsed);
assert.equal(hydrated.phase, "paused");
assert.equal(hydrated.romId, "PKMN:2097152");
assert.equal(hydrated.elapsed, 99);

console.log("✅ store logic tests passed");
