"use client";

import React, { useState } from "react";
import { useEmulator } from "../lib/store";
import type { SaveSlot } from "../lib/store";
import { Button, Badge } from "./ui";
import { CartIcon } from "./Viewport";

function fmtClock(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

function fmtAgo(ms: number): string {
  const diff = Date.now() - ms;
  const min = Math.floor(diff / 60000);
  if (min < 1) return "just now";
  if (min < 60) return `${min}m ago`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr}h ago`;
  return `${Math.floor(hr / 24)}d ago`;
}

interface SegmentedOption<T extends string> {
  value: T;
  label: string;
}

interface SegmentedProps<T extends string> {
  value: T;
  options: SegmentedOption<T>[];
  onChange: (value: T) => void;
}

function Segmented<T extends string>({ value, options, onChange }: SegmentedProps<T>) {
  return (
    <div
      style={{
        display: "grid",
        gridTemplateColumns: `repeat(${options.length}, 1fr)`,
        gap: 4,
        background: "var(--surface-sunken)",
        border: "1px solid var(--border)",
        borderRadius: "var(--radius)",
        padding: 4,
      }}
    >
      {options.map((o) => {
        const on = o.value === value;
        return (
          <button
            key={o.value}
            onClick={() => onChange(o.value)}
            style={{
              fontFamily: "var(--font-mono)",
              fontSize: 12,
              fontWeight: 600,
              letterSpacing: "0.02em",
              padding: "8px 4px",
              borderRadius: "var(--radius-sm)",
              cursor: "pointer",
              border: on ? "1px solid var(--accent-press)" : "1px solid transparent",
              background: on ? "var(--accent)" : "transparent",
              color: on ? "#fff" : "var(--text-muted)",
              boxShadow: on ? "0 2px 0 0 var(--bg-deep)" : "none",
              transition: "background 120ms, color 120ms",
            }}
          >
            {o.label}
          </button>
        );
      })}
    </div>
  );
}

interface SlotCardProps {
  slot: SaveSlot | null;
  index: number;
  mode: "load" | "save";
  onSave: (index: number) => void;
  onLoad: (index: number) => void;
}

function SlotCard({ slot, index, mode, onSave, onLoad }: SlotCardProps) {
  const filled = !!slot;
  const tap = () => {
    if (mode === "save") onSave(index);
    else if (filled) onLoad(index);
  };
  const disabled = mode === "load" && !filled;
  return (
    <button
      onClick={tap}
      disabled={disabled}
      style={{
        position: "relative",
        display: "flex",
        flexDirection: "column",
        gap: 0,
        background: "var(--surface-sunken)",
        border: `1px solid ${mode === "save" ? "var(--accent)" : filled ? "var(--border-strong)" : "var(--border)"}`,
        borderRadius: "var(--radius)",
        overflow: "hidden",
        cursor: disabled ? "default" : "pointer",
        opacity: disabled ? 0.4 : 1,
        padding: 0,
        textAlign: "left",
      }}
    >
      <div
        style={{
          aspectRatio: "160 / 144",
          background: "var(--screen)",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          position: "relative",
          width: "100%",
        }}
      >
        {filled ? (
          slot.thumb ? (
            <img
              src={slot.thumb}
              alt=""
              style={{ width: "100%", height: "100%", objectFit: "cover", imageRendering: "pixelated" }}
            />
          ) : (
            <span style={{ fontFamily: "var(--font-mono)", fontSize: 10, color: "var(--text-faint)", letterSpacing: "0.1em" }}>
              NO THUMB
            </span>
          )
        ) : (
          <span style={{ fontFamily: "var(--font-mono)", fontSize: 10, color: "var(--text-faint)", letterSpacing: "0.1em" }}>
            EMPTY
          </span>
        )}
        <span
          style={{
            position: "absolute",
            top: 4,
            left: 4,
            fontFamily: "var(--font-mono)",
            fontSize: 9,
            color: "var(--dmg-lightest)",
            background: "rgba(8,24,32,0.7)",
            padding: "2px 4px",
            borderRadius: 2,
          }}
        >
          {index + 1}
        </span>
        {mode === "save" && (
          <span
            style={{
              position: "absolute",
              inset: 0,
              background: "rgba(226,87,30,0.16)",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              fontFamily: "var(--font-mono)",
              fontSize: 10,
              color: "#fff",
              fontWeight: 600,
              letterSpacing: "0.08em",
            }}
          >
            {filled ? "OVERWRITE" : "SAVE"}
          </span>
        )}
      </div>
      <div style={{ padding: "6px 7px", display: "flex", flexDirection: "column", gap: 1, width: "100%" }}>
        <span
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: 10,
            color: "var(--text)",
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
        >
          {filled ? fmtClock(slot.elapsed || 0) : "—:—"}
        </span>
        <span style={{ fontFamily: "var(--font-mono)", fontSize: 9, color: "var(--text-faint)" }}>
          {filled ? fmtAgo(slot.at) : "no state"}
        </span>
      </div>
    </button>
  );
}

export interface QuickMenuProps {
  open: boolean;
  onClose: () => void;
}

export function QuickMenu({ open, onClose }: QuickMenuProps) {
  const emu = useEmulator();
  const [mode, setMode] = useState<"load" | "save">("load");
  const { rom, phase } = emu;

  if (!open) return null;

  return (
    <div style={{ position: "absolute", inset: 0, zIndex: 40 }}>
      {/* scrim */}
      <div
        onClick={onClose}
        style={{
          position: "absolute",
          inset: 0,
          background: "rgba(0,0,0,0.72)",
          animation: "rubcFade 160ms ease",
        }}
      />
      {/* sheet */}
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
          style={{
            width: 38,
            height: 4,
            borderRadius: 2,
            background: "var(--border-strong)",
            margin: "2px auto 14px",
          }}
        />

        {/* cart header */}
        <div style={{ display: "flex", alignItems: "center", gap: 12, marginBottom: 16 }}>
          <div
            style={{
              width: 46,
              height: 46,
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
            {rom && rom.thumb ? (
              <img
                src={rom.thumb}
                alt=""
                style={{ width: "100%", height: "100%", objectFit: "cover", imageRendering: "pixelated" }}
              />
            ) : rom ? (
              <CartIcon size={34} />
            ) : null}
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
              {rom ? rom.title : "No cartridge"}
            </div>
            <div style={{ fontFamily: "var(--font-mono)", fontSize: 10.5, color: "var(--text-muted)", marginTop: 2 }}>
              {rom ? `${rom.mode} · ` : ""}
              <span style={{ color: "var(--dmg-light)" }}>{fmtClock(emu.elapsed)}</span>
            </div>
          </div>
          {rom && (
            <Badge tone={rom.mode === "CGB" ? "cgb" : "dmg"}>
              {rom.mode}
            </Badge>
          )}
        </div>

        {/* primary actions */}
        <div style={{ display: "grid", gridTemplateColumns: "1.4fr 1fr 1fr", gap: 8, marginBottom: 18 }}>
          <Button variant="primary" block onClick={emu.togglePause}>
            {phase === "running" ? "❚❚ Pause" : "▶ Resume"}
          </Button>
          <Button variant="secondary" block onClick={emu.reset}>
            ↻ Reset
          </Button>
          <Button variant="ghost" block onClick={emu.powerOff}>
            ⏻ Eject
          </Button>
        </div>

        {/* save states */}
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 9 }}>
          <span
            style={{
              fontFamily: "var(--font-mono)",
              fontSize: 11,
              color: "var(--text-muted)",
              letterSpacing: "0.12em",
              textTransform: "uppercase",
            }}
          >
            Save states
          </span>
          <Segmented<"load" | "save">
            value={mode}
            onChange={setMode}
            options={[
              { value: "load", label: "Load" },
              { value: "save", label: "Save" },
            ]}
          />
        </div>
        <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 8, marginBottom: 18 }}>
          {emu.slots.map((slot, i) => (
            <SlotCard
              key={i}
              slot={slot}
              index={i}
              mode={mode}
              
              onSave={emu.saveTo}
              onLoad={emu.loadFrom}
            />
          ))}
        </div>

        {/* nav */}
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8 }}>
          <Button
            variant="secondary"
            block
            onClick={() => {
              onClose();
              emu.setView("library");
            }}
          >
            ⇆ Swap ROM
          </Button>
          <Button
            variant="secondary"
            block
            onClick={() => {
              onClose();
              emu.setView("settings");
            }}
          >
            ⚙ Settings
          </Button>
        </div>
      </div>
    </div>
  );
}
