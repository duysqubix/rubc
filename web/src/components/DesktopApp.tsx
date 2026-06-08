"use client";

import React, { useEffect, useRef, useState } from "react";
import { useEmulator } from "@/lib/store";
import { emulator, BTN } from "@/lib/emulator";
import type { EmulatorRom, EmulatorContextValue } from "@/lib/store";
import { Button, Badge, StatusPill, Kbd, Switch } from "@/components/ui";
import { Viewport, CartIcon } from "@/components/Viewport";
import { Settings } from "@/components/Settings";

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

function Logo({ h = 30 }: { h?: number }) {
  return (
    <a href="/" aria-label="rubc home" style={{ display: "inline-flex", lineHeight: 0 }}>
      <img
        src="/logo.png"
        alt="rubc"
        style={{ height: h, width: "auto", imageRendering: "pixelated", objectFit: "contain" }}
      />
    </a>
  );
}

function DeskRomItem({ rom, active, onPlay }: { rom: EmulatorRom; active: boolean; onPlay: (id: string) => void }) {
  const [hover, setHover] = useState(false);
  return (
    <button
      onClick={() => onPlay(rom.id)}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 10,
        width: "100%",
        textAlign: "left",
        background: active ? "var(--surface-raised)" : hover ? "var(--surface)" : "transparent",
        border: active ? "1px solid var(--accent)" : "1px solid transparent",
        borderRadius: "var(--radius)",
        padding: "7px 8px",
        cursor: "pointer",
        transition: "background 120ms",
      }}
    >
      <div
        style={{
          width: 40,
          height: 40,
          flexShrink: 0,
          borderRadius: 4,
          overflow: "hidden",
          background: "var(--screen)",
          border: "1px solid var(--border-screen)",
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
          <CartIcon accent="var(--dmg-light)" size={30} />
        )}
      </div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div
          style={{
            fontFamily: "var(--font-sans)",
            fontSize: 13,
            fontWeight: 600,
            color: "var(--text-strong)",
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
        >
          {rom.title}
        </div>
        <div style={{ fontFamily: "var(--font-mono)", fontSize: 10, color: "var(--text-muted)", marginTop: 1 }}>
          {rom.mode === "CGB" ? "MBC5" : "MBC1"} · {rom.size ? `${(rom.size / 1024).toFixed(0)}KB` : "Unknown"}
        </div>
      </div>
      <Badge tone={rom.mode === "CGB" ? "cgb" : "dmg"}>{rom.mode || "DMG"}</Badge>
    </button>
  );
}

function RailGroup({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div style={{ marginBottom: 14 }}>
      <div
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: 10,
          color: "var(--text-muted)",
          letterSpacing: "0.12em",
          textTransform: "uppercase",
          padding: "4px 8px 7px",
        }}
      >
        {label}
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>{children}</div>
    </div>
  );
}

function LibraryRail({ emu }: { emu: EmulatorContextValue }) {
  const games = emu.ROMS.filter((r) => !r.id.includes("test"));
  const tests = emu.ROMS.filter((r) => r.id.includes("test"));
  const inputRef = useRef<HTMLInputElement>(null);
  return (
    <aside
      style={{
        width: 264,
        flexShrink: 0,
        borderRight: "1px solid var(--border)",
        background: "var(--surface-sunken)",
        display: "flex",
        flexDirection: "column",
        minHeight: 0,
      }}
    >
      <div style={{ padding: "14px 14px 10px", borderBottom: "1px solid var(--border)" }}>
        <input
          ref={inputRef}
          type="file"
          accept=".gb,.gbc,.zip"
          style={{ display: "none" }}
          onChange={(e) => {
            const file = e.target.files?.[0];
            if (file) emu.loadFile(file);
            e.target.value = "";
          }}
        />
        <button
          onClick={() => inputRef.current?.click()}
          style={{
            width: "100%",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            gap: 8,
            padding: "10px",
            borderRadius: "var(--radius)",
            cursor: "pointer",
            border: "2px dashed var(--border-strong)",
            background: "var(--surface-sunken)",
            color: "var(--text)",
            fontFamily: "var(--font-mono)",
            fontSize: 12.5,
            fontWeight: 600,
          }}
        >
          <span style={{ color: "var(--accent)", fontSize: 16 }}>＋</span> Load ROM
        </button>
      </div>
      <div style={{ flex: 1, overflowY: "auto", padding: "10px 10px 16px", minHeight: 0 }}>
        <RailGroup label={`Games · ${games.length}`}>
          {games.map((r) => (
            <DeskRomItem key={r.id} rom={r} active={r.id === emu.romId} onPlay={emu.boot} />
          ))}
        </RailGroup>
        <RailGroup label={`Test ROMs · ${tests.length}`}>
          {tests.map((r) => (
            <DeskRomItem key={r.id} rom={r} active={r.id === emu.romId} onPlay={emu.boot} />
          ))}
        </RailGroup>
      </div>
    </aside>
  );
}

function MapRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        padding: "9px 0",
        borderBottom: "1px solid var(--border)",
      }}
    >
      <span style={{ fontFamily: "var(--font-sans)", fontSize: 13.5, color: "var(--text)" }}>{label}</span>
      <span style={{ display: "flex", gap: 5 }}>{children}</span>
    </div>
  );
}

function ControlsTab() {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
      <div>
        <div
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: 10.5,
            color: "var(--accent)",
            letterSpacing: "0.12em",
            textTransform: "uppercase",
            marginBottom: 6,
          }}
        >
          // keyboard
        </div>
        <MapRow label="D-pad">
          <Kbd>←</Kbd>
          <Kbd>↑</Kbd>
          <Kbd>↓</Kbd>
          <Kbd>→</Kbd>
        </MapRow>
        <MapRow label="A button">
          <Kbd>X</Kbd>
        </MapRow>
        <MapRow label="B button">
          <Kbd>Z</Kbd>
        </MapRow>
        <MapRow label="Start">
          <Kbd>↵</Kbd>
        </MapRow>
        <MapRow label="Select">
          <Kbd>⇧</Kbd>
        </MapRow>
        <MapRow label="Turbo (hold)">
          <Kbd>Tab</Kbd>
        </MapRow>
        <MapRow label="Pause">
          <Kbd>Space</Kbd>
        </MapRow>
        <MapRow label="Save / Load state">
          <Kbd>F5</Kbd>
          <span
            style={{
              color: "var(--text-faint)",
              fontFamily: "var(--font-mono)",
              fontSize: 11,
              alignSelf: "center",
            }}
          >
            /
          </span>
          <Kbd>F8</Kbd>
        </MapRow>
      </div>
      <div style={{ fontFamily: "var(--font-sans)", fontSize: 11.5, color: "var(--text-faint)", lineHeight: 1.6 }}>
        Controls are rebindable in the native build. Gamepads (XInput / DualSense) are auto-detected.
      </div>
    </div>
  );
}

function AccuracyTab() {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 9 }}>
      <div
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: 10.5,
          color: "var(--accent)",
          letterSpacing: "0.12em",
          textTransform: "uppercase",
          marginBottom: 2,
        }}
      >
        // verified on real silicon
      </div>
      <StatusPill status="pass" label="dmg-acid2" detail="pixel-exact" />
      <StatusPill status="pass" label="cgb-acid2" detail="pixel-exact" />
      <StatusPill status="pass" label="cgb-acid-hell" detail="pixel-exact" />
      <StatusPill status="pass" label="Blargg cpu_instrs" detail="11/11" />
      <StatusPill status="pass" label="Blargg dmg_sound" detail="12/12" />
      <StatusPill status="pass" label="Mooneye" detail="94/115" />
      <StatusPill status="wip" label="mealybug-tearoom" />
      <div style={{ display: "flex", alignItems: "center", gap: 8, marginTop: 6 }}>
        <Badge tone="rust" variant="solid">
          safe Rust
        </Badge>
        <span style={{ fontFamily: "var(--font-mono)", fontSize: 10.5, color: "var(--text-faint)" }}>
          #![forbid(unsafe_code)]
        </span>
      </div>
    </div>
  );
}

function SavesTab({ emu }: { emu: EmulatorContextValue }) {
  const [mode, setMode] = useState<"load" | "save">("load");
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <span
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: 10.5,
            color: "var(--text-muted)",
            letterSpacing: "0.12em",
            textTransform: "uppercase",
          }}
        >
          Save states
        </span>
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "1fr 1fr",
            gap: 4,
            background: "var(--surface-sunken)",
            border: "1px solid var(--border)",
            borderRadius: "var(--radius)",
            padding: 4,
          }}
        >
          {(["load", "save"] as const).map((m) => {
            const on = m === mode;
            return (
              <button
                key={m}
                onClick={() => setMode(m)}
                style={{
                  fontFamily: "var(--font-mono)",
                  fontSize: 12,
                  fontWeight: 600,
                  letterSpacing: "0.02em",
                  padding: "4px 8px",
                  borderRadius: "var(--radius-sm)",
                  cursor: "pointer",
                  border: on ? "1px solid var(--accent-press)" : "1px solid transparent",
                  background: on ? "var(--accent)" : "transparent",
                  color: on ? "#fff" : "var(--text-muted)",
                  boxShadow: on ? "0 2px 0 0 var(--bg-deep)" : "none",
                  transition: "background 120ms, color 120ms",
                }}
              >
                {m === "load" ? "Load" : "Save"}
              </button>
            );
          })}
        </div>
      </div>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 10 }}>
        {emu.slots.map((slot, i) => {
          const filled = !!slot;
          const disabled = (mode === "load" && !filled) || (!emu.rom && mode === "save");
          return (
            <button
              key={i}
              disabled={disabled}
              onClick={() => (mode === "save" ? emu.saveTo(i) : filled && emu.loadFrom(i))}
              style={{
                padding: 0,
                border: `1px solid ${
                  mode === "save" ? "var(--accent)" : filled ? "var(--border-strong)" : "var(--border)"
                }`,
                borderRadius: "var(--radius)",
                overflow: "hidden",
                background: "var(--surface-sunken)",
                cursor: disabled ? "default" : "pointer",
                opacity: disabled ? 0.4 : 1,
                textAlign: "left",
              }}
            >
              <div
                style={{
                  aspectRatio: "160/144",
                  background: "var(--screen)",
                  position: "relative",
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                }}
              >
                {filled && slot.thumb ? (
                  <img
                    src={slot.thumb}
                    alt=""
                    style={{ width: "100%", height: "100%", objectFit: "cover", imageRendering: "pixelated" }}
                  />
                ) : (
                  <span style={{ fontFamily: "var(--font-mono)", fontSize: 10, color: "var(--text-faint)" }}>
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
                  {i + 1}
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
                    }}
                  >
                    {filled ? "OVERWRITE" : "SAVE"}
                  </span>
                )}
              </div>
              <div style={{ padding: "5px 7px", display: "flex", justifyContent: "space-between" }}>
                <span style={{ fontFamily: "var(--font-mono)", fontSize: 9.5, color: "var(--text)" }}>
                  {filled ? fmtClock(slot.elapsed || 0) : "—:—"}
                </span>
                <span style={{ fontFamily: "var(--font-mono)", fontSize: 9, color: "var(--text-faint)" }}>
                  {filled ? fmtAgo(slot.at) : "—"}
                </span>
              </div>
            </button>
          );
        })}
      </div>
    </div>
  );
}

function RightPanel({ emu }: { emu: EmulatorContextValue }) {
  const [tab, setTab] = useState<"controls" | "accuracy" | "settings" | "saves">("controls");
  const tabs = [
    { id: "controls", label: "Controls" },
    { id: "accuracy", label: "Accuracy" },
    { id: "settings", label: "Settings" },
    { id: "saves", label: "Saves" },
  ] as const;
  return (
    <aside
      style={{
        width: 320,
        flexShrink: 0,
        borderLeft: "1px solid var(--border)",
        background: "var(--surface-sunken)",
        display: "flex",
        flexDirection: "column",
        minHeight: 0,
      }}
    >
      <div style={{ display: "flex", borderBottom: "1px solid var(--border)" }}>
        {tabs.map((t) => (
          <button
            key={t.id}
            onClick={() => setTab(t.id)}
            style={{
              flex: 1,
              padding: "12px 4px",
              cursor: "pointer",
              background: "transparent",
              border: "none",
              borderBottom: tab === t.id ? "2px solid var(--accent)" : "2px solid transparent",
              color: tab === t.id ? "var(--text-strong)" : "var(--text-muted)",
              fontFamily: "var(--font-mono)",
              fontSize: 12,
              fontWeight: 600,
            }}
          >
            {t.label}
          </button>
        ))}
      </div>
      <div style={{ flex: 1, overflowY: "auto", padding: 16, minHeight: 0 }}>
        {tab === "controls" && <ControlsTab />}
        {tab === "accuracy" && <AccuracyTab />}
        {tab === "settings" && <Settings />}
        {tab === "saves" && <SavesTab emu={emu} />}
      </div>
    </aside>
  );
}

function DeskToolbar({ emu }: { emu: EmulatorContextValue }) {
  const { phase, settings: s, rom } = emu;
  const inputRef = useRef<HTMLInputElement>(null);
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap", justifyContent: "center" }}>
      <input
        ref={inputRef}
        type="file"
        accept=".gb,.gbc,.zip"
        style={{ display: "none" }}
        onChange={(e) => {
          const file = e.target.files?.[0];
          if (file) emu.loadFile(file);
          e.target.value = "";
        }}
      />
      <Button variant="primary" onClick={() => (rom ? emu.reset() : inputRef.current?.click())}>
        {rom ? "↻ Reset" : "Load ROM"}
      </Button>
      <Button variant="secondary" disabled={phase === "empty"} onClick={emu.togglePause}>
        {phase === "running" ? "❚❚ Pause" : "▶ Resume"}
      </Button>
      <Button variant="ghost" disabled={!rom} onClick={() => emu.saveTo(0)}>
        ⤓ Save state
      </Button>
      <div style={{ width: 1, height: 24, background: "var(--border)" }} />
      <Switch checked={s.sound} onChange={(v) => emu.set({ sound: v })} label="Sound" />
      <Switch checked={s.turbo} onChange={(v) => emu.set({ turbo: v })} label="Turbo" />
    </div>
  );
}

export function DesktopApp() {
  const emu = useEmulator();
  const [pressedDir, setPressedDir] = useState<string | null>(null);
  const dirT = useRef<ReturnType<typeof setTimeout> | null>(null);
  const { phase, rom, settings: s } = emu;
  const [dragging, setDragging] = useState(false);

  const onDir = (d: string) => {
    if (phase !== "running") return;
    setPressedDir(d);
    if (dirT.current) clearTimeout(dirT.current);
    dirT.current = setTimeout(() => setPressedDir(null), 170);
  };
  const resumeIf = () => {
    if (phase === "paused") emu.togglePause();
  };

  useEffect(() => {
    // Map keyboard -> GameBoy buttons. Arrows = d-pad, X=A, Z=B, Enter=Start,
    // Shift/Backspace=Select. Space toggles pause. These actually drive the core
    // via emulator.setButton (the d-pad pulse + resume are just UX on top).
    const dirMap: Record<string, [string, number]> = {
      ArrowLeft: ["left", BTN.LEFT],
      ArrowRight: ["right", BTN.RIGHT],
      ArrowUp: ["up", BTN.UP],
      ArrowDown: ["down", BTN.DOWN],
    };
    const btnMap: Record<string, number> = {
      x: BTN.A, X: BTN.A,
      z: BTN.B, Z: BTN.B,
      Enter: BTN.START,
      Shift: BTN.SELECT, Backspace: BTN.SELECT,
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.repeat) return;
      const dir = dirMap[e.key];
      if (dir) {
        e.preventDefault();
        emulator.setButton(dir[1], true);
        onDir(dir[0]);
        return;
      }
      if (e.key in btnMap) {
        e.preventDefault();
        emulator.setButton(btnMap[e.key], true);
        if (e.key === "x" || e.key === "X" || e.key === "Enter") resumeIf();
        return;
      }
      if (e.key === " ") {
        e.preventDefault();
        if (phase !== "empty") emu.togglePause();
      }
    };
    const onKeyUp = (e: KeyboardEvent) => {
      const dir = dirMap[e.key];
      if (dir) {
        e.preventDefault();
        emulator.setButton(dir[1], false);
        return;
      }
      if (e.key in btnMap) {
        e.preventDefault();
        emulator.setButton(btnMap[e.key], false);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
    };
  }, [phase, emu]);

  return (
    <div
      onDragOver={(e) => {
        e.preventDefault();
        if (!dragging) setDragging(true);
      }}
      onDragLeave={(e) => {
        // only clear when leaving the window, not when moving between children
        if (e.relatedTarget === null) setDragging(false);
      }}
      onDrop={(e) => {
        e.preventDefault();
        setDragging(false);
        const file = e.dataTransfer.files?.[0];
        if (file) void emu.loadFile(file);
      }}
      style={{
        position: "absolute",
        inset: 0,
        display: "flex",
        flexDirection: "column",
        background: "var(--bg)",
        color: "var(--text)",
        fontFamily: "var(--font-sans)",
      }}
    >
      {/* top bar */}
      <header
        style={{
          display: "flex",
          alignItems: "center",
          gap: 16,
          padding: "10px 18px",
          borderBottom: "1px solid var(--border)",
          background: "var(--surface-sunken)",
          flexShrink: 0,
        }}
      >
        <Logo h={28} />
        <div style={{ width: 1, height: 24, background: "var(--border)" }} />
        <div style={{ flex: 1, minWidth: 0 }}>
          {rom ? (
            <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
              <span
                style={{
                  fontFamily: "var(--font-sans)",
                  fontSize: 14,
                  fontWeight: 600,
                  color: "var(--text-strong)",
                }}
              >
                {rom.title}
              </span>
              <span
                style={{
                  fontFamily: "var(--font-mono)",
                  fontSize: 11,
                  color: phase === "running" ? "var(--dmg-light)" : "var(--text-faint)",
                }}
              >
                {phase === "running"
                  ? `● running · ${fmtClock(emu.elapsed)}`
                  : phase === "booting"
                    ? "○ booting…"
                    : "❚❚ paused"}
              </span>
            </div>
          ) : (
            <span style={{ fontFamily: "var(--font-mono)", fontSize: 12.5, color: "var(--text-muted)" }}>
              wasm ready — pick a ROM from the library →
            </span>
          )}
        </div>
        <div style={{ display: "flex", gap: 7, alignItems: "center" }}>
          {rom && <Badge tone={rom.mode === "CGB" ? "cgb" : "dmg"}>{rom.mode || "DMG"}</Badge>}
          <Badge tone="neutral">WebAssembly</Badge>
          <a href="https://github.com/duysqubix/rubc" target="_blank" rel="noreferrer" style={{ textDecoration: "none" }}>
            <Button variant="ghost" size="sm">
              GitHub ↗
            </Button>
          </a>
        </div>
      </header>

      {/* body */}
      <div style={{ flex: 1, display: "flex", minHeight: 0 }}>
        <LibraryRail emu={emu} />
        {/* center stage */}
        <main
          style={{
            flex: 1,
            minWidth: 0,
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            gap: 22,
            padding: 28,
            background: "radial-gradient(120% 80% at 50% 0%, #14161c, var(--bg) 70%)",
          }}
        >
          <div style={{ width: "100%", maxWidth: 540, position: "relative" }}>
            <Viewport
              phase={phase}
              rom={rom}
              filter={emu.filter}
              scaling={s.scaling}
              smoothing={s.smoothing}
              showFps={s.showFps}
              turbo={s.turbo}
              pressedDir={pressedDir}
              glow={phase === "running"}
            />
          </div>
          <DeskToolbar emu={emu} />
          <div
            style={{
              display: "flex",
              gap: 14,
              alignItems: "center",
              flexWrap: "wrap",
              justifyContent: "center",
              fontFamily: "var(--font-mono)",
              fontSize: 11.5,
              color: "var(--text-faint)",
            }}
          >
            <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
              <Kbd>←</Kbd>
              <Kbd>↑</Kbd>
              <Kbd>↓</Kbd>
              <Kbd>→</Kbd> move
            </span>
            <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
              <Kbd>X</Kbd> A
            </span>
            <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
              <Kbd>Z</Kbd> B
            </span>
            <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
              <Kbd>Space</Kbd> pause
            </span>
          </div>
        </main>
        <RightPanel emu={emu} />
      </div>

      {emu.toast && (
        <div
          style={{
            position: "absolute",
            left: "50%",
            bottom: 24,
            transform: "translateX(-50%)",
            background: "var(--surface-raised)",
            border: "1px solid var(--border-strong)",
            borderLeft: "3px solid var(--accent)",
            borderRadius: "var(--radius)",
            padding: "9px 14px",
            fontFamily: "var(--font-mono)",
            fontSize: 12,
            color: "var(--text)",
            boxShadow: "var(--shadow-lg)",
          }}
        >
          {emu.toast}
        </div>
      )}
      {dragging && (
        <div
          style={{
            position: "absolute",
            inset: 0,
            zIndex: 100,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            pointerEvents: "none",
            background: "color-mix(in srgb, var(--bg-deep) 72%, transparent)",
            border: "3px dashed var(--accent)",
            borderRadius: 0,
          }}
        >
          <span
            style={{
              fontFamily: "var(--font-pixel)",
              fontSize: 32,
              color: "var(--text-strong)",
            }}
          >
            ▸ drop rom to play
          </span>
        </div>
      )}
    </div>
  );
}
