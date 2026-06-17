export const SAVE_STATE_SLOT_COUNT = 4;

export type SaveStateSlotIndex = 0 | 1 | 2 | 3;

export interface SaveStateSlotMetadata {
  at: number;
  romId: string | null;
  thumb: string | null;
  label: string;
  elapsed: number;
}

export interface SaveStateRecord extends SaveStateSlotMetadata {
  romId: string;
  data: Uint8Array;
}

export interface SaveStateRecordInput {
  at?: number;
  romId: string;
  thumb?: string | null;
  label?: string | null;
  elapsed: number;
  data: Uint8Array;
}

export function isSaveStateSlotIndex(index: number): index is SaveStateSlotIndex {
  return Number.isInteger(index) && index >= 0 && index < SAVE_STATE_SLOT_COUNT;
}

export function saveStateRecordKey(romId: string, slot: SaveStateSlotIndex): string {
  return `${romId}::state:${slot}`;
}

export function createSaveStateRecord(input: SaveStateRecordInput): SaveStateRecord {
  return {
    at: input.at ?? Date.now(),
    romId: input.romId,
    thumb: typeof input.thumb === "string" ? input.thumb : null,
    label: normalizeLabel(input.label),
    elapsed: normalizeElapsed(input.elapsed),
    data: input.data.slice(),
  };
}

export function slotFromSaveStateRecord(record: SaveStateRecord): SaveStateSlotMetadata {
  return {
    at: record.at,
    romId: record.romId,
    thumb: record.thumb,
    label: record.label,
    elapsed: record.elapsed,
  };
}

export function normalizeSaveStateRecord(value: unknown): SaveStateRecord | null {
  if (!isRecord(value) || !isFiniteNumber(value.at) || typeof value.romId !== "string") return null;
  const data = bytesFromUnknown(value.data);
  if (!data) return null;
  return createSaveStateRecord({
    at: value.at,
    romId: value.romId,
    thumb: typeof value.thumb === "string" ? value.thumb : null,
    label: typeof value.label === "string" ? value.label : null,
    elapsed: isFiniteNumber(value.elapsed) ? value.elapsed : 0,
    data,
  });
}

function bytesFromUnknown(value: unknown): Uint8Array | null {
  if (value instanceof Uint8Array) return value.slice();
  if (value instanceof ArrayBuffer) return new Uint8Array(value).slice();
  return null;
}

function normalizeLabel(value: string | null | undefined): string {
  const trimmed = value?.trim();
  return trimmed && trimmed.length > 0 ? trimmed : "—";
}

function normalizeElapsed(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.floor(value));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

// --- Save-state file (download/upload) ---------------------------------------
// A self-contained, portable snapshot file: the full machine SaveStateRecord
// (so it restores the EXACT frame) plus the slot it belongs to. The binary
// `data` is base64-encoded so the file is a plain JSON document that round-trips
// byte-for-byte through JSON.stringify/parse (a number array would balloon ~6x
// and risk precision/ío issues; base64 is compact and lossless).

export const SAVE_STATE_FILE_FORMAT = "rubc-savestate";
export const SAVE_STATE_FILE_VERSION = 1;

export interface SaveStateFile {
  format: typeof SAVE_STATE_FILE_FORMAT;
  version: typeof SAVE_STATE_FILE_VERSION;
  slot: SaveStateSlotIndex;
  record: {
    at: number;
    romId: string;
    thumb: string | null;
    label: string;
    elapsed: number;
    data: string;
  };
}

/** Encode bytes as base64. Chunked so large snapshots never overflow the
  * argument stack via spread into String.fromCharCode. */
export function bytesToBase64(bytes: Uint8Array): string {
  let binary = "";
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(binary);
}

/** Decode a base64 string back to the exact original bytes. */
export function base64ToBytes(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

/** Serialize a slot + record into a portable JSON save-state file string. */
export function serializeSaveStateFile(slot: SaveStateSlotIndex, record: SaveStateRecord): string {
  const file: SaveStateFile = {
    format: SAVE_STATE_FILE_FORMAT,
    version: SAVE_STATE_FILE_VERSION,
    slot,
    record: {
      at: record.at,
      romId: record.romId,
      thumb: record.thumb,
      label: record.label,
      elapsed: record.elapsed,
      data: bytesToBase64(record.data),
    },
  };
  return JSON.stringify(file);
}

/** Parse a save-state file string back into a slot + validated record.
  * Returns null for anything malformed, hostile, or version-mismatched — never
  * throws. Reuses normalizeSaveStateRecord so the record is validated by the
  * SAME path as records loaded from IndexedDB. */
export function parseSaveStateFile(text: string): { slot: SaveStateSlotIndex; record: SaveStateRecord } | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    return null;
  }
  if (!isRecord(parsed)) return null;
  if (parsed.format !== SAVE_STATE_FILE_FORMAT) return null;
  if (parsed.version !== SAVE_STATE_FILE_VERSION) return null;
  const slot = parsed.slot;
  if (typeof slot !== "number" || !isSaveStateSlotIndex(slot)) return null;
  const rawRecord = parsed.record;
  if (!isRecord(rawRecord)) return null;
  if (typeof rawRecord.data !== "string") return null;
  let data: Uint8Array;
  try {
    data = base64ToBytes(rawRecord.data);
  } catch {
    return null;
  }
  const record = normalizeSaveStateRecord({
    at: rawRecord.at,
    romId: rawRecord.romId,
    thumb: rawRecord.thumb,
    label: rawRecord.label,
    elapsed: rawRecord.elapsed,
    data,
  });
  if (!record) return null;
  return { slot, record };
}
