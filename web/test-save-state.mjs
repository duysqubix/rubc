import assert from "assert/strict";
import fs from "fs";
import path from "path";
import ts from "typescript";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const logicPath = path.join(__dirname, "src/lib/save-state-record.ts");

if (!fs.existsSync(logicPath)) {
  console.error("❌ src/lib/save-state-record.ts not found");
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
const slots = await import(moduleUrl);

const {
  SAVE_STATE_SLOT_COUNT,
  createSaveStateRecord,
  isSaveStateSlotIndex,
  normalizeSaveStateRecord,
  saveStateRecordKey,
  slotFromSaveStateRecord,
} = slots;

assert.equal(SAVE_STATE_SLOT_COUNT, 4);
assert.equal(isSaveStateSlotIndex(0), true);
assert.equal(isSaveStateSlotIndex(3), true);
assert.equal(isSaveStateSlotIndex(4), false);
assert.equal(isSaveStateSlotIndex(1.5), false);
assert.equal(saveStateRecordKey("PKMN:2097152", 2), "PKMN:2097152::state:2");

const bytes = new Uint8Array([1, 2, 3, 4]);
const record = createSaveStateRecord({
  at: 1234,
  romId: "PKMN:2097152",
  thumb: "data:image/png;base64,thumb",
  label: "Pokémon Crystal",
  elapsed: 42.9,
  data: bytes,
});

assert.deepEqual(Object.keys(record).sort(), ["at", "data", "elapsed", "label", "romId", "thumb"]);
assert.equal(record.at, 1234);
assert.equal(record.romId, "PKMN:2097152");
assert.equal(record.thumb, "data:image/png;base64,thumb");
assert.equal(record.label, "Pokémon Crystal");
assert.equal(record.elapsed, 42);
assert.deepEqual(Array.from(record.data), [1, 2, 3, 4]);

bytes[0] = 99;
assert.deepEqual(Array.from(record.data), [1, 2, 3, 4]);

const slot = slotFromSaveStateRecord(record);
assert.deepEqual(slot, {
  at: 1234,
  romId: "PKMN:2097152",
  thumb: "data:image/png;base64,thumb",
  label: "Pokémon Crystal",
  elapsed: 42,
});

const normalized = normalizeSaveStateRecord({ ...record, data: new Uint8Array([9, 8]) });
assert.ok(normalized);
assert.deepEqual(Array.from(normalized.data), [9, 8]);
assert.deepEqual(slotFromSaveStateRecord(normalized), { ...slot, elapsed: 42 });
assert.equal(normalizeSaveStateRecord({ ...record, romId: null }), null);
assert.equal(normalizeSaveStateRecord({ ...record, data: [] }), null);

console.log("✅ save-state record tests passed");
