"use client";

import React, { useCallback, useEffect, useRef, useState } from "react";
import { useEmulator } from "@/lib/store";
import type { EmulatorRom } from "@/lib/store";
import {
  exportSave,
  importSave,
  exportSaveState,
  importSaveState,
  loadSaveRam,
  loadSaveStateRecord,
} from "@/lib/emulator";
import { slotFromSaveStateRecord } from "@/lib/save-state-record";
import type { SaveStateSlotMetadata } from "@/lib/save-state-record";
import { CartIcon } from "@/components/Viewport";

const SLOT_INDICES = [0, 1, 2, 3] as const;

function fmtAgo(ms: number): string {
  const diff = Date.now() - ms;
  const min = Math.floor(diff / 60000);
  if (min < 1) return "just now";
  if (min < 60) return `${min}m ago`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr}h ago`;
  return `${Math.floor(hr / 24)}d ago`;
}

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <div
      style={{
        fontFamily: "var(--font-mono)",
        fontSize: 10,
        color: "var(--text-muted)",
        letterSpacing: "0.12em",
        textTransform: "uppercase",
        padding: "8px 10px 4px",
      }}
    >
      {children}
    </div>
  );
}

function Muted({ children }: { children: React.ReactNode }) {
  return (
    <div
      style={{
        fontFamily: "var(--font-mono)",
        fontSize: 11,
        color: "var(--text-faint)",
        padding: "6px 10px 8px",
      }}
    >
      {children}
    </div>
  );
}

function ActionButton({
  glyph,
  label,
  sub,
  disabled = false,
  big,
  onClick,
}: {
  glyph: string;
  label: string;
  sub?: string;
  disabled?: boolean;
  big: boolean;
  onClick: () => void;
}) {
  const [hover, setHover] = useState(false);
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={(e) => {
        e.stopPropagation();
        if (!disabled) onClick();
      }}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 11,
        width: "100%",
        textAlign: "left",
        padding: big ? "12px 12px" : "8px 10px",
        background: hover && !disabled ? "var(--surface)" : "transparent",
        border: "none",
        borderRadius: "var(--radius)",
        cursor: disabled ? "default" : "pointer",
        opacity: disabled ? 0.4 : 1,
        color: "var(--text)",
        transition: "background 110ms",
      }}
    >
      <span
        aria-hidden
        style={{
          width: big ? 22 : 18,
          flexShrink: 0,
          textAlign: "center",
          color: "var(--accent)",
          fontFamily: "var(--font-mono)",
          fontSize: big ? 17 : 14,
          lineHeight: 1,
        }}
      >
        {glyph}
      </span>
      <span style={{ display: "flex", flexDirection: "column", minWidth: 0, gap: 1 }}>
        <span
          style={{
            fontFamily: "var(--font-sans)",
            fontSize: big ? 14 : 13,
            fontWeight: 600,
            whiteSpace: "nowrap",
          }}
        >
          {label}
        </span>
        {sub && (
          <span
            style={{
              fontFamily: "var(--font-mono)",
              fontSize: big ? 11 : 10,
              color: "var(--text-faint)",
              whiteSpace: "nowrap",
              overflow: "hidden",
              textOverflow: "ellipsis",
            }}
          >
            {sub}
          </span>
        )}
      </span>
    </button>
  );
}

/**
 * Per-ROM save manager: export/import battery (.sav) AND full machine save
 * states (.rubcstate). Rendered as a touch-first bottom sheet on mobile
 * (`compact={false}`) and a compact anchored dropdown on desktop
 * (`compact`). Keys everything by `rom.id`, which equals getRomKey(bytes)
 * (the save key) for user-loaded games.
 */
export function RomActionsMenu({ rom, compact = false }: { rom: EmulatorRom; compact?: boolean }) {
  const emu = useEmulator();
  const [open, setOpen] = useState(false);
  const [loading, setLoading] = useState(false);
  const [hasBattery, setHasBattery] = useState(false);
  const [slots, setSlots] = useState<(SaveStateSlotMetadata | null)[]>([null, null, null, null]);
  const [anchor, setAnchor] = useState<{ top: number; right: number } | null>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const savInputRef = useRef<HTMLInputElement>(null);
  const stateInputRef = useRef<HTMLInputElement>(null);

  const close = useCallback(() => setOpen(false), []);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const [battery, ...records] = await Promise.all([
        loadSaveRam(rom.id),
        ...SLOT_INDICES.map((s) => loadSaveStateRecord(rom.id, s)),
      ]);
      setHasBattery(!!battery && battery.length > 0);
      setSlots(records.map((r) => (r ? slotFromSaveStateRecord(r) : null)));
    } finally {
      setLoading(false);
    }
  }, [rom.id]);

  const openMenu = useCallback(() => {
    if (compact) {
      const rect = triggerRef.current?.getBoundingClientRect();
      if (rect) {
        setAnchor({ top: rect.bottom + 6, right: Math.max(8, window.innerWidth - rect.right) });
      }
    }
    setOpen(true);
    void refresh();
  }, [compact, refresh]);

  // Close on Escape; on desktop also close on scroll/resize (the fixed dropdown
  // would otherwise drift away from its trigger).
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    window.addEventListener("keydown", onKey);
    let cleanupScroll = () => {};
    if (compact) {
      const onMove = () => close();
      window.addEventListener("scroll", onMove, true);
      window.addEventListener("resize", onMove);
      cleanupScroll = () => {
        window.removeEventListener("scroll", onMove, true);
        window.removeEventListener("resize", onMove);
      };
    }
    return () => {
      window.removeEventListener("keydown", onKey);
      cleanupScroll();
    };
  }, [open, compact, close]);

  const onExportSave = useCallback(() => {
    void exportSave(rom.id);
    emu.flash(`Battery save exported · ${rom.title}`);
    close();
  }, [emu, rom.id, rom.title, close]);

  const onImportSaveChange = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      e.target.value = "";
      if (!file) return;
      await importSave(rom.id, file);
      emu.flash(`Battery save imported · reload ${rom.title} to apply`);
      close();
    },
    [emu, rom.id, rom.title, close],
  );

  const onExportState = useCallback(
    async (slot: number) => {
      const ok = await exportSaveState(rom.id, slot);
      emu.flash(ok ? `Save state exported · slot ${slot + 1}` : `No save state · slot ${slot + 1}`);
      close();
    },
    [emu, rom.id, close],
  );

  const onImportStateChange = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      e.target.value = "";
      if (!file) return;
      const res = await importSaveState(file);
      if (!res) {
        emu.flash("Not a valid rubc save state file");
        return;
      }
      emu.importStateSlot(res.slot, res.metadata);
      emu.flash(`Save state imported · slot ${res.slot + 1}`);
      close();
    },
    [emu, close],
  );

  const filledSlots = slots
    .map((meta, i) => ({ meta, i }))
    .filter((x): x is { meta: SaveStateSlotMetadata; i: number } => x.meta !== null);

  const actions = (big: boolean) => (
    <>
      <SectionLabel>Battery save</SectionLabel>
      <ActionButton
        big={big}
        glyph="↓"
        label="Export save (.sav)"
        sub={hasBattery ? undefined : loading ? "Checking…" : "No battery save yet"}
        disabled={!hasBattery}
        onClick={onExportSave}
      />
      <ActionButton big={big} glyph="↑" label="Import save (.sav)" onClick={() => savInputRef.current?.click()} />
      <div style={{ height: 1, background: "var(--border)", margin: "6px 8px" }} />
      <SectionLabel>Save states</SectionLabel>
      {loading ? (
        <Muted>Loading…</Muted>
      ) : filledSlots.length === 0 ? (
        <Muted>No save states yet</Muted>
      ) : (
        filledSlots.map(({ meta, i }) => (
          <ActionButton
            key={i}
            big={big}
            glyph="↓"
            label={`Export state · slot ${i + 1}`}
            sub={`${meta.label} · ${fmtAgo(meta.at)}`}
            onClick={() => onExportState(i)}
          />
        ))
      )}
      <ActionButton big={big} glyph="↑" label="Import save state" onClick={() => stateInputRef.current?.click()} />
    </>
  );

  const triggerStyle: React.CSSProperties = compact
    ? {
        width: 24,
        height: 24,
        borderRadius: "var(--radius)",
        border: "1px solid var(--border-strong)",
        background: "var(--surface-raised)",
        color: "var(--text-muted)",
        fontFamily: "var(--font-mono)",
        fontSize: 14,
        lineHeight: 1,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        cursor: "pointer",
      }
    : {
        width: 40,
        height: 40,
        borderRadius: "var(--radius)",
        border: "1px solid var(--border-strong)",
        background: "var(--surface-raised)",
        color: "var(--text-muted)",
        fontFamily: "var(--font-mono)",
        fontSize: 19,
        lineHeight: 1,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        cursor: "pointer",
        boxShadow: "0 2px 0 0 var(--bg-deep)",
      };

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        aria-label={`Save options for ${rom.title}`}
        aria-haspopup="menu"
        aria-expanded={open}
        title="Save options"
        onClick={(e) => {
          e.stopPropagation();
          if (open) close();
          else openMenu();
        }}
        style={triggerStyle}
      >
        ⋮
      </button>

      {open && !compact && (
        <div style={{ position: "fixed", inset: 0, zIndex: 80 }}>
          <div
            onClick={close}
            style={{ position: "absolute", inset: 0, background: "rgba(0,0,0,0.72)", animation: "rubcFade 160ms ease" }}
          />
          <div
            className="rubc-sheet"
            style={{
              position: "absolute",
              left: 0,
              right: 0,
              bottom: 0,
              background: "var(--bg)",
              borderTop: "2px solid var(--accent)",
              borderTopLeftRadius: 16,
              borderTopRightRadius: 16,
              padding: "10px 16px 30px",
              boxShadow: "0 -8px 32px rgba(0,0,0,0.5)",
              maxHeight: "82%",
              overflowY: "auto",
            }}
          >
            <div
              style={{ width: 38, height: 4, borderRadius: 2, background: "var(--border-strong)", margin: "2px auto 14px" }}
            />
            <div style={{ display: "flex", alignItems: "center", gap: 12, marginBottom: 12 }}>
              <div
                style={{
                  width: 44,
                  height: 44,
                  borderRadius: 5,
                  overflow: "hidden",
                  background: "var(--screen)",
                  border: "1px solid var(--border-screen)",
                  flexShrink: 0,
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                }}
              >
                {rom.thumb ? (
                  <img
                    src={rom.thumb}
                    alt=""
                    style={{ width: "100%", height: "100%", objectFit: "cover", imageRendering: "pixelated" }}
                  />
                ) : (
                  <CartIcon size={32} />
                )}
              </div>
              <div style={{ flex: 1, minWidth: 0 }}>
                <div
                  style={{
                    fontFamily: "var(--font-sans)",
                    fontSize: 15,
                    fontWeight: 600,
                    color: "var(--text-strong)",
                    whiteSpace: "nowrap",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                  }}
                >
                  {rom.title}
                </div>
                <div style={{ fontFamily: "var(--font-mono)", fontSize: 10.5, color: "var(--text-muted)", marginTop: 2 }}>
                  Backup &amp; transfer
                </div>
              </div>
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>{actions(true)}</div>
          </div>
        </div>
      )}

      {open && compact && anchor && (
        <>
          <div onClick={close} style={{ position: "fixed", inset: 0, zIndex: 80 }} />
          <div
            role="menu"
            style={{
              position: "fixed",
              top: anchor.top,
              right: anchor.right,
              zIndex: 81,
              width: 236,
              background: "var(--surface-raised)",
              border: "1px solid var(--border-strong)",
              borderRadius: "var(--radius)",
              boxShadow: "var(--shadow-lg)",
              padding: 6,
              maxHeight: "70vh",
              overflowY: "auto",
            }}
          >
            {actions(false)}
          </div>
        </>
      )}

      <input ref={savInputRef} type="file" accept=".sav" style={{ display: "none" }} onChange={onImportSaveChange} />
      <input
        ref={stateInputRef}
        type="file"
        accept=".rubcstate,.json,application/json"
        style={{ display: "none" }}
        onChange={onImportStateChange}
      />
    </>
  );
}
