"use client";

import React, { useEffect, useRef, useState } from "react";
import { useEmulator } from "@/lib/store";
import { Button } from "./ui/Button";

import { Viewport } from "./Viewport";
import { Gamepad, OverlayControls } from "./Gamepad";
import { Settings } from "./Settings";
import { Library } from "./Library";
import { QuickMenu } from "./QuickMenu";
import { emulator, BTN } from "@/lib/emulator";

// small square icon button used in chrome
function IconBtn({ children, onClick, active, disabled, label }: { children: React.ReactNode, onClick?: () => void, active?: boolean, disabled?: boolean, label: string }) {
  const [p, setP] = useState(false);
  return (
    <button
      aria-label={label}
      onClick={disabled ? undefined : onClick}
      onPointerDown={() => !disabled && setP(true)}
      onPointerUp={() => setP(false)}
      onPointerLeave={() => setP(false)}
      style={{
        width: 40, height: 40, borderRadius: "var(--radius)", flexShrink: 0,
        display: "flex", alignItems: "center", justifyContent: "center",
        background: active ? "var(--accent-soft)" : "var(--surface-raised)",
        border: `1px solid ${active ? "var(--accent)" : "var(--border-strong)"}`,
        color: active ? "var(--accent)" : "var(--text)",
        fontFamily: "var(--font-mono)", fontSize: 17, cursor: disabled ? "not-allowed" : "pointer",
        opacity: disabled ? 0.4 : 1,
        boxShadow: p ? "0 0 0 0 var(--bg-deep)" : "0 2px 0 0 var(--bg-deep)",
        transform: p ? "translateY(2px)" : "none",
        transition: "transform 90ms cubic-bezier(0.2,0,0.1,1), box-shadow 90ms cubic-bezier(0.2,0,0.1,1)",
      }}
    >{children}</button>
  );
}

function TopBar({ emu }: { emu: ReturnType<typeof useEmulator> }) {
  const { view, rom, phase } = emu;
  
  const formatClock = (ms: number) => {
    const s = Math.floor(ms / 1000);
    const m = Math.floor(s / 60);
    const h = Math.floor(m / 60);
    const pad = (n: number) => n.toString().padStart(2, "0");
    if (h > 0) return `${h}:${pad(m % 60)}:${pad(s % 60)}`;
    return `${m}:${pad(s % 60)}`;
  };

  if (view !== "play") {
    return (
      <div style={barStyle}>
        <IconBtn label="Back" onClick={() => emu.setView("play")}>←</IconBtn>
        <div style={{ flex: 1, textAlign: "center", fontFamily: "var(--font-sans)", fontSize: 16, fontWeight: 600, color: "var(--text-strong)" }}>
          {view === "library" ? "Library" : "Settings"}
        </div>
        <div style={{ width: 40 }} />
      </div>
    );
  }
  return (
    <div style={barStyle}>
      <IconBtn label="Library" onClick={() => emu.setView("library")}>▦</IconBtn>
      <div style={{ flex: 1, minWidth: 0, textAlign: "center" }}>
        {rom ? (
          <>
            <div style={{ fontFamily: "var(--font-sans)", fontSize: 14, fontWeight: 600, color: "var(--text-strong)", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{rom.title}</div>
            <div style={{ fontFamily: "var(--font-mono)", fontSize: 10, color: phase === "running" ? "var(--dmg-light)" : "var(--text-faint)", letterSpacing: "0.06em", marginTop: 1 }}>
              {phase === "running" ? `● running · ${formatClock(emu.elapsed)}` : phase === "booting" ? "○ booting…" : phase === "paused" ? "❚❚ paused" : ""}
            </div>
          </>
        ) : (
          <a href="/" aria-label="rubc home" style={{ fontFamily: "var(--font-pixel)", fontSize: 22, color: "var(--accent)", letterSpacing: "0.02em", textDecoration: "none" }}>rubc</a>
        )}
      </div>
      <IconBtn label="Menu" onClick={() => emu.setMenuOpen(true)} disabled={!rom}>≡</IconBtn>
    </div>
  );
}

const barStyle: React.CSSProperties = {
  display: "flex", alignItems: "center", gap: 10, padding: "8px 14px",
  borderBottom: "1px solid var(--border)", background: "var(--surface-sunken)",
  flexShrink: 0,
};

// quick toggle chip row under the screen (docked mode)
function QuickChip({ on, onClick, children }: { on: boolean, onClick: () => void, children: React.ReactNode }) {
  return (
    <button onClick={onClick} style={{
      display: "inline-flex", alignItems: "center", gap: 6, padding: "7px 11px",
      borderRadius: "var(--radius)", cursor: "pointer",
      fontFamily: "var(--font-mono)", fontSize: 11.5, fontWeight: 600,
      background: on ? "var(--accent-soft)" : "var(--surface-raised)",
      border: `1px solid ${on ? "var(--accent)" : "var(--border-strong)"}`,
      color: on ? "var(--rust-300)" : "var(--text-muted)",
      boxShadow: "0 2px 0 0 var(--bg-deep)",
    }}>{children}</button>
  );
}

function PlayScreen({ emu, pressedDir }: { emu: ReturnType<typeof useEmulator>, pressedDir: string | null }) {
  const { rom, phase, settings: s } = emu;
  const overlay = s.controls === "overlay" && phase !== "empty";
  const glow = phase === "running";

  return (
    <div style={{ flex: 1, display: "flex", flexDirection: "column", minHeight: 0 }}>
      {overlay ? (
        // ── OVERLAY: maximized full-bleed screen, translucent controls over it ──
        <div style={{ flex: 1, position: "relative", display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", background: "#000", minHeight: 0 }}>
          <div style={{ width: "100%", maxHeight: "100%", aspectRatio: "160 / 144", position: "relative" }}>
            <Viewport phase={phase} rom={rom} filter={emu.filter}
              scaling={s.scaling} smoothing={s.smoothing} showFps={s.showFps} turbo={s.turbo}
              pressedDir={pressedDir} glow={glow} fullBleed />
          </div>
          {/* dock toggle chip, top-right over the black */}
          <button onClick={() => emu.set({ controls: "docked" })} style={{
            position: "absolute", top: 26, right: 14, zIndex: 5,
            display: "inline-flex", alignItems: "center", gap: 6, padding: "6px 10px",
            borderRadius: "var(--radius)", cursor: "pointer", fontFamily: "var(--font-mono)", fontSize: 11, fontWeight: 600,
            background: "rgba(42,47,58,0.7)", border: "1px solid var(--border-strong)", color: "var(--text-muted)",
            backdropFilter: "blur(4px)",
          }}>▤ Dock</button>
          <OverlayControls />
        </div>
      ) : (
        // ── DOCKED: screen on top, gamepad below ──
        <>
          <div style={{ padding: "16px 16px 0" }}>
            <Viewport phase={phase} rom={rom} filter={emu.filter}
              scaling={s.scaling} smoothing={s.smoothing} showFps={s.showFps} turbo={s.turbo}
              pressedDir={pressedDir} glow={glow} />
          </div>

          {/* status + quick chips */}
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", padding: "12px 16px 8px", gap: 8 }}>
            {phase === "empty" ? (
              <Button variant="primary" onClick={() => emu.setView("library")}>Load ROM (.gb / .gbc)</Button>
            ) : (
              <div style={{ display: "flex", gap: 7 }}>
                <QuickChip on={s.controls === "overlay"} onClick={() => emu.set({ controls: s.controls === "overlay" ? "docked" : "overlay" })}>⤢ Overlay</QuickChip>
                <QuickChip on={s.turbo} onClick={() => emu.set({ turbo: !s.turbo })}>» Turbo</QuickChip>
                <QuickChip on={s.sound} onClick={() => emu.set({ sound: !s.sound })}>{s.sound ? "♪ Sound" : "✕ Muted"}</QuickChip>
              </div>
            )}
          </div>

          {/* gamepad fills the rest, anchored low */}
          <div style={{ flex: 1, display: "flex", flexDirection: "column", justifyContent: "flex-end", padding: "0 14px 18px", minHeight: 0 }}>
            <div style={{ opacity: phase === "empty" ? 0.4 : 1, pointerEvents: phase === "empty" ? "none" : "auto", transition: "opacity 160ms" }}>
              <Gamepad />
            </div>
          </div>
        </>
      )}
    </div>
  );
}

function Toast({ msg }: { msg: string | null }) {
  if (!msg) return null;
  return (
    <div style={{
      position: "absolute", left: "50%", bottom: 70, transform: "translateX(-50%)", zIndex: 60,
      background: "var(--surface-raised)", border: "1px solid var(--border-strong)",
      borderLeft: "3px solid var(--accent)", borderRadius: "var(--radius)",
      padding: "9px 14px", fontFamily: "var(--font-mono)", fontSize: 12, color: "var(--text)",
      boxShadow: "var(--shadow-lg)", animation: "rubcFade 200ms ease", whiteSpace: "nowrap",
    }}>{msg}</div>
  );
}

export function App() {
  const emu = useEmulator();
  const [pressedDir, setPressedDir] = useState<string | null>(null);
  const dirT = useRef<ReturnType<typeof setTimeout> | null>(null);

  // desktop keyboard support — makes it feel "actually working"
  useEffect(() => {
    const map: Record<string, [string, string?]> = { 
      ArrowLeft: ["dir", "left"], ArrowRight: ["dir", "right"], ArrowUp: ["dir", "up"], ArrowDown: ["dir", "down"], 
      x: ["A"], X: ["A"], z: ["B"], Z: ["B"], Enter: ["start"], Shift: ["select"] 
    };
    
    const btnMap: Record<string, number> = {
      left: BTN.LEFT, right: BTN.RIGHT, up: BTN.UP, down: BTN.DOWN,
      A: BTN.A, B: BTN.B, start: BTN.START, select: BTN.SELECT
    };

    const onKeyDown = (e: KeyboardEvent) => {
      const m = map[e.key];
      if (!m) return;
      e.preventDefault();
      
      if (m[0] === "dir") {
        const dir = m[1]!;
        emulator.setButton(btnMap[dir], true);
        if (emu.phase === "running") {
          setPressedDir(dir);
          if (dirT.current) clearTimeout(dirT.current);
          dirT.current = setTimeout(() => setPressedDir(null), 170);
        }
      } else {
        const btn = m[0];
        emulator.setButton(btnMap[btn], true);
        if (btn === "start" || btn === "A") {
          if (emu.phase === "paused") emu.togglePause();
        }
      }
    };

    const onKeyUp = (e: KeyboardEvent) => {
      const m = map[e.key];
      if (!m) return;
      e.preventDefault();
      
      if (m[0] === "dir") {
        emulator.setButton(btnMap[m[1]!], false);
      } else {
        emulator.setButton(btnMap[m[0]], false);
      }
    };

    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
    };
  }, [emu.phase, emu.togglePause]);

  return (
    <div style={{
      position: "absolute", inset: 0, display: "flex", flexDirection: "column",
      paddingTop: 50, // clear the status bar / dynamic island
      background: "var(--bg)", color: "var(--text)", overflow: "hidden",
    }}>
      <TopBar emu={emu} />
      <div style={{ flex: 1, minHeight: 0, display: "flex", flexDirection: "column", overflowY: emu.view === "play" ? "hidden" : "auto" }}>
        {emu.view === "play" && <PlayScreen emu={emu} pressedDir={pressedDir} />}
        {emu.view === "library" && <Library />}
        {emu.view === "settings" && <div style={{ padding: "16px 16px 0" }}><Settings /></div>}
      </div>

      <QuickMenu open={emu.menuOpen} onClose={() => emu.setMenuOpen(false)} />
      <Toast msg={emu.toast} />
    </div>
  );
}
