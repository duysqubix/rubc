"use client";

import React, { useState, useCallback, useRef, useEffect } from "react";
import { emulator, BTN } from "@/lib/emulator";
import { useEmulator } from "@/lib/store";

export function usePress(onDown?: () => void, onUp?: () => void) {
  // Visual press state only. The AUTHORITATIVE held-state lives in refs so it
  // survives re-renders/remounts -- the on-screen pad sits inside components that
  // subscribe to the store, which re-renders ~1x/sec (the elapsed-time tick) and
  // on every buzz()/state change while a game runs. If a button unmounts mid-press
  // the release path must still run, or the wasm button stays HELD (stuck input).
  const [down, setDown] = useState(false);
  const pressedRef = useRef(false);
  const activePointers = useRef<Set<number>>(new Set());
  const onDownRef = useRef(onDown);
  const onUpRef = useRef(onUp);
  useEffect(() => {
    onDownRef.current = onDown;
    onUpRef.current = onUp;
  }, [onDown, onUp]);

  const press = useCallback(() => {
    if (pressedRef.current) return;
    pressedRef.current = true;
    setDown(true);
    onDownRef.current?.();
  }, []);

  const release = useCallback((el: HTMLElement, pointerId: number) => {
    activePointers.current.delete(pointerId);
    try {
      if (el.hasPointerCapture(pointerId)) el.releasePointerCapture(pointerId);
    } catch {
      // capture may already be gone (element unmounting / pointer lost)
    }
    // Release only once the LAST pointer on this button lifts (multi-touch safe).
    if (activePointers.current.size === 0 && pressedRef.current) {
      pressedRef.current = false;
      setDown(false);
      onUpRef.current?.();
    }
  }, []);

  const isInside = (el: HTMLElement, e: React.PointerEvent<HTMLElement>) => {
    const r = el.getBoundingClientRect();
    return e.clientX >= r.left && e.clientX <= r.right && e.clientY >= r.top && e.clientY <= r.bottom;
  };

  // Safety net: if this button unmounts while still held (e.g. a store-driven
  // re-render swaps the subtree mid-press), force the release so nothing sticks.
  useEffect(() => {
    return () => {
      activePointers.current.clear();
      if (pressedRef.current) {
        pressedRef.current = false;
        onUpRef.current?.();
      }
    };
  }, []);

  return {
    down,
    handlers: {
      onPointerDown: (e: React.PointerEvent<HTMLElement>) => {
        e.preventDefault();
        const el = e.currentTarget;
        activePointers.current.add(e.pointerId);
        try {
          el.setPointerCapture(e.pointerId);
        } catch {
          // capture is best-effort; release paths don't depend on it
        }
        press();
      },
      // Capture keeps events flowing even off-element, so pointerleave is
      // unreliable for drag-off; bounds-check pointermove instead.
      onPointerMove: (e: React.PointerEvent<HTMLElement>) => {
        if (!activePointers.current.has(e.pointerId)) return;
        if (!isInside(e.currentTarget, e)) release(e.currentTarget, e.pointerId);
      },
      onPointerUp: (e: React.PointerEvent<HTMLElement>) => release(e.currentTarget, e.pointerId),
      onPointerCancel: (e: React.PointerEvent<HTMLElement>) => release(e.currentTarget, e.pointerId),
      onLostPointerCapture: (e: React.PointerEvent<HTMLElement>) => release(e.currentTarget, e.pointerId),
    },
  };
}

// Hoisted to module scope so its component identity is STABLE across DPad
// re-renders. When this lived inline inside DPad, every store-driven re-render
// (the ~1s elapsed tick, buzz(), any state change) gave it a fresh identity and
// React unmounted+remounted it -- dropping the pointer-release of a held button
// and leaving the input stuck/rapid-firing.
function DPadCell({
  dir,
  area,
  arm,
  chev,
}: {
  dir: "up" | "down" | "left" | "right";
  area: string;
  arm: number;
  chev?: boolean;
}) {
  const { buzz } = useEmulator();
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
      data-down={down}
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
}

export function DPad({ size = 150, dim = false }: { size?: number; dim?: boolean }) {
  const arm = size / 3;

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
      <DPadCell dir="up" area="u" arm={arm} chev />
      <DPadCell dir="left" area="l" arm={arm} chev />
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
      <DPadCell dir="right" area="r" arm={arm} chev />
      <DPadCell dir="down" area="d" arm={arm} chev />
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
