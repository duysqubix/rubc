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
  bytesToBase64,
  base64ToBytes,
  serializeSaveStateFile,
  parseSaveStateFile,
  SAVE_STATE_FILE_FORMAT,
  SAVE_STATE_FILE_VERSION,
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

// --- base64 helpers: exact raw-byte round-trip across the full byte range ---
const rawBytes = new Uint8Array([0, 1, 127, 128, 255, 42, 7, 200]);
const b64 = bytesToBase64(rawBytes);
assert.equal(typeof b64, "string");
assert.deepEqual(Array.from(base64ToBytes(b64)), Array.from(rawBytes));
assert.equal(bytesToBase64(new Uint8Array([])), "");
assert.deepEqual(Array.from(base64ToBytes("")), []);

// --- save-state file: export -> import round-trip ---
assert.equal(SAVE_STATE_FILE_FORMAT, "rubc-savestate");
assert.equal(SAVE_STATE_FILE_VERSION, 1);
const fileRecord = createSaveStateRecord({
  at: 5678,
  romId: "PKMN_CRYSTAL:2097152",
  thumb: "data:image/png;base64,zzz",
  label: "Pokémon Crystal",
  elapsed: 360,
  data: rawBytes,
});
const fileText = serializeSaveStateFile(2, fileRecord);
assert.equal(typeof fileText, "string");
const envelope = JSON.parse(fileText);
assert.equal(envelope.format, "rubc-savestate");
assert.equal(envelope.version, 1);
assert.equal(envelope.slot, 2);
assert.equal(envelope.record.romId, "PKMN_CRYSTAL:2097152");
assert.equal(typeof envelope.record.data, "string"); // base64, never a JSON number array

const round = parseSaveStateFile(fileText);
assert.ok(round);
assert.equal(round.slot, 2);
assert.equal(round.record.romId, "PKMN_CRYSTAL:2097152");
assert.equal(round.record.at, 5678);
assert.equal(round.record.label, "Pokémon Crystal");
assert.equal(round.record.elapsed, 360);
assert.equal(round.record.thumb, "data:image/png;base64,zzz");
assert.deepEqual(Array.from(round.record.data), Array.from(rawBytes)); // EXACT byte fidelity

// --- rejects malformed / hostile input gracefully (never throws) ---
assert.equal(parseSaveStateFile("not json"), null);
assert.equal(parseSaveStateFile(JSON.stringify({ format: "nope", version: 1, slot: 0, record: envelope.record })), null);
assert.equal(parseSaveStateFile(JSON.stringify({ format: "rubc-savestate", version: 2, slot: 0, record: envelope.record })), null);
assert.equal(parseSaveStateFile(JSON.stringify({ format: "rubc-savestate", version: 1, slot: 9, record: envelope.record })), null);
assert.equal(parseSaveStateFile(JSON.stringify({ format: "rubc-savestate", version: 1, slot: 0, record: { ...envelope.record, romId: null } })), null);
assert.equal(parseSaveStateFile(JSON.stringify({ format: "rubc-savestate", version: 1, slot: 0, record: { ...envelope.record, data: 123 } })), null);

console.log("✅ save-state file round-trip tests passed");

console.log("✅ save-state record tests passed");
