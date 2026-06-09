"use client";

import React, { useState, useCallback } from "react";
import { emulator, BTN } from "@/lib/emulator";
import { useEmulator } from "@/lib/store";

export function usePress(
  onDown?: () => void,
  onUp?: () => void
) {
  const [down, setDown] = useState(false);

  const start = useCallback((e: React.PointerEvent<HTMLElement>) => {
    e.preventDefault();
    if (down) return;
    setDown(true);
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
    onDown?.();
  }, [down, onDown]);

  const end = useCallback((e: React.PointerEvent<HTMLElement>) => {
    if (!down) return;
    setDown(false);
    (e.target as HTMLElement).releasePointerCapture(e.pointerId);
    onUp?.();
  }, [down, onUp]);

  return {
    down,
    handlers: {
      onPointerDown: start,
      onPointerUp: end,
      onPointerLeave: end,
      onPointerCancel: end,
    },
  };
}

export function DPad({ size = 150, dim = false }: { size?: number; dim?: boolean }) {
  const { buzz } = useEmulator();
  const arm = size / 3;

  const Cell = ({ dir, area, chev }: { dir: "up" | "down" | "left" | "right"; area: string; chev?: boolean }) => {
    const btnMap = { up: BTN.UP, down: BTN.DOWN, left: BTN.LEFT, right: BTN.RIGHT };
    const btn = btnMap[dir];
    
    const { down, handlers } = usePress(
      () => {
        emulator.setButton(btn, true);
        buzz(10);
      },
      () => {
        emulator.setButton(btn, false);
      }
    );

    const rot = { up: 0, right: 90, down: 180, left: 270 }[dir];

    return (
      <button
        aria-label={dir}
        style={{
          gridArea: area,
          background: down ? "var(--bg-deep)" : "var(--ink-800)",
          boxShadow: down
            ? "inset 0 2px 4px rgba(0,0,0,0.7)"
            : "inset 0 1px 0 rgba(255,255,255,0.05)",
          transform: down ? "scale(0.97)" : "none",
          borderRadius:
            area === "u" ? "7px 7px 0 0" :
            area === "d" ? "0 0 7px 7px" :
            area === "l" ? "7px 0 0 7px" :
            area === "r" ? "0 7px 7px 0" : "0",
          touchAction: "none",
          border: "none",
          outline: "none",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          cursor: "pointer",
        }}
        {...handlers}
      >
        {chev && (
          <span style={{
            display: "block", width: 0, height: 0,
            borderLeft: `${arm * 0.16}px solid transparent`,
            borderRight: `${arm * 0.16}px solid transparent`,
            borderBottom: `${arm * 0.2}px solid ${down ? "var(--ink-400)" : "var(--slate-300)"}`,
            transform: `rotate(${rot}deg)`,
          }} />
        )}
      </button>
    );
  };

  return (
    <div
      style={{
        width: size, height: size,
        display: "grid",
        gridTemplateColumns: `${arm}px ${arm}px ${arm}px`,
        gridTemplateRows: `${arm}px ${arm}px ${arm}px`,
        gridTemplateAreas: '". u ." "l c r" ". d ."',
        filter: "drop-shadow(0 var(--press-offset) 0 var(--bg-deep))",
        opacity: dim ? 0.92 : 1,
        touchAction: "none",
      }}
    >
      <Cell dir="up" area="u" chev />
      <Cell dir="left" area="l" chev />
      <div style={{
        gridArea: "c", background: "var(--ink-800)",
        display: "flex", alignItems: "center", justifyContent: "center",
      }}>
        <span style={{
          width: arm * 0.34, height: arm * 0.34, borderRadius: "50%",
          background: "radial-gradient(circle at 38% 32%, var(--ink-600), var(--ink-950))",
          boxShadow: "inset 0 1px 1px rgba(0,0,0,0.8)",
        }} />
      </div>
      <Cell dir="right" area="r" chev />
      <Cell dir="down" area="d" chev />
    </div>
  );
}

export function ActionBtn({ label, sub, size = 74, btn, dim = false }: { label: string; sub?: string; size?: number; btn: number; dim?: boolean }) {
  const { buzz, phase, togglePause } = useEmulator();
  const { down, handlers } = usePress(
    () => {
      emulator.setButton(btn, true);
      buzz(10);
      // Pressing A resumes a paused game (mirrors the desktop keyboard).
      if (btn === BTN.A && phase === "paused") togglePause();
    },
    () => {
      emulator.setButton(btn, false);
    }
  );

  return (
    <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 6, touchAction: "none" }}>
      <button
        aria-label={`${label} button`}
        style={{
          width: size, height: size, borderRadius: "50%",
          border: "2px solid var(--rust-700)",
          background: down
            ? "radial-gradient(circle at 50% 60%, var(--rust-600), var(--rust-700))"
            : "radial-gradient(circle at 42% 34%, var(--rust-400), var(--rust-600))",
          color: "var(--white)",
          fontFamily: "var(--font-pixel)",
          fontSize: size * 0.42,
          fontWeight: 600,
          display: "flex", alignItems: "center", justifyContent: "center",
          cursor: "pointer", userSelect: "none",
          transform: down ? "translateY(var(--press-offset))" : "none",
          boxShadow: down
            ? "0 0 0 0 var(--bg-deep), inset 0 2px 6px rgba(0,0,0,0.45)"
            : "0 var(--press-offset) 0 0 var(--bg-deep), inset 0 1px 0 rgba(255,255,255,0.25)",
          transition: "transform var(--dur-fast) var(--ease), box-shadow var(--dur-fast) var(--ease)",
          opacity: dim ? 0.95 : 1,
          paddingBottom: size * 0.04,
          outline: "none",
        }}
        {...handlers}
      >
        {label}
      </button>
      {sub && (
        <span style={{
          fontFamily: "var(--font-mono)", fontSize: 11, fontWeight: 600,
          color: "var(--text-faint)", letterSpacing: "0.08em",
        }}>{sub}</span>
      )}
    </div>
  );
}

export function PillBtn({ label, btn }: { label: string; btn: number }) {
  const { buzz, phase, togglePause } = useEmulator();
  const { down, handlers } = usePress(
    () => {
      emulator.setButton(btn, true);
      buzz(10);
      // Pressing Start resumes a paused game (mirrors the desktop keyboard).
      if (btn === BTN.START && phase === "paused") togglePause();
    },
    () => {
      emulator.setButton(btn, false);
    }
  );

  return (
    <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 6, touchAction: "none" }}>
      <button
        aria-label={label}
        style={{
          width: 56, height: 16, borderRadius: 9,
          border: "1px solid var(--ink-600)",
          background: down ? "var(--ink-950)" : "linear-gradient(180deg, var(--ink-500), var(--ink-700))",
          transform: down ? "translateY(2px) rotate(-20deg)" : "rotate(-20deg)",
          boxShadow: down ? "0 0 0 0 var(--bg-deep)" : "0 var(--press-offset) 0 0 var(--bg-deep)",
          transition: "transform var(--dur-fast) var(--ease), box-shadow var(--dur-fast) var(--ease)",
          cursor: "pointer",
          outline: "none",
        }}
        {...handlers}
      />
      <span style={{
        fontFamily: "var(--font-mono)", fontSize: 10, fontWeight: 600,
        color: "var(--text-faint)", letterSpacing: "0.1em", textTransform: "uppercase",
      }}>{label}</span>
    </div>
  );
}

export function Gamepad({ compact = false }: { compact?: boolean }) {
  const padSize = compact ? 132 : 150;
  const abSize = compact ? 66 : 74;
  return (
    <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: compact ? 14 : 20, width: "100%" }}>
      <div style={{
        display: "flex", alignItems: "center", justifyContent: "space-between",
        width: "100%", padding: "0 10px", boxSizing: "border-box",
      }}>
        <DPad size={padSize} />
        <div style={{ display: "flex", alignItems: "flex-end", gap: 14, paddingBottom: 6 }}>
          <ActionBtn label="B" sub="Z" size={abSize} btn={BTN.B} />
          <div style={{ paddingBottom: abSize * 0.5 }}>
            <ActionBtn label="A" sub="X" size={abSize} btn={BTN.A} />
          </div>
        </div>
      </div>
      <div style={{ display: "flex", gap: 30 }}>
        <PillBtn label="Select" btn={BTN.SELECT} />
        <PillBtn label="Start" btn={BTN.START} />
      </div>
    </div>
  );
}

export function OverlayControls() {
  const wrap: React.CSSProperties = {
    position: "absolute", top: 0, left: 0, right: 0, bottom: 0, pointerEvents: "none",
    display: "flex", flexDirection: "column", justifyContent: "flex-end",
    paddingTop: 60, paddingBottom: 8,
    background: "linear-gradient(transparent, rgba(6,7,9,0.55) 36%, rgba(6,7,9,0.88) 100%)",
    zIndex: 50,
  };
  const glassCol: React.CSSProperties = {
    pointerEvents: "auto",
    display: "flex", alignItems: "flex-end", justifyContent: "space-between",
    padding: "0 18px 18px",
  };
  return (
    <div style={wrap}>
      <div style={{ pointerEvents: "auto", display: "flex", justifyContent: "center", gap: 24, paddingBottom: 14, opacity: 0.92 }}>
        <PillBtn label="Select" btn={BTN.SELECT} />
        <PillBtn label="Start" btn={BTN.START} />
      </div>
      <div style={{ ...glassCol, opacity: 0.95 }}>
        <DPad size={128} dim />
        <div style={{ display: "flex", alignItems: "flex-end", gap: 12 }}>
          <ActionBtn label="B" size={60} btn={BTN.B} dim />
          <div style={{ paddingBottom: 30 }}>
            <ActionBtn label="A" size={60} btn={BTN.A} dim />
          </div>
        </div>
      </div>
    </div>
  );
}
