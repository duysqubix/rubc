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
