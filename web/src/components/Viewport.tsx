"use client";

import React, { useEffect, useRef, useState } from "react";
import { useEmulator } from "@/lib/store";
import type { EmulatorRom, Phase, Scaling } from "@/lib/store";

export interface ViewportProps {
  phase?: Phase;
  rom?: EmulatorRom | null;
  filter?: string;
  scaling?: Scaling;
  smoothing?: boolean;
  showFps?: boolean;
  turbo?: boolean;
  pressedDir?: string | null;
  glow?: boolean;
  fullBleed?: boolean;
}

function EdgeFlash({ dir }: { dir: string | null }) {
  if (!dir) return null;
  const base: React.CSSProperties = { position: "absolute", background: "rgba(136,192,112,0.22)", pointerEvents: "none" };
  const map: Record<string, React.CSSProperties> = {
    up:    { ...base, top: 0, left: 0, right: 0, height: "22%", background: "linear-gradient(rgba(136,192,112,0.3), transparent)" },
    down:  { ...base, bottom: 0, left: 0, right: 0, height: "22%", background: "linear-gradient(transparent, rgba(136,192,112,0.3))" },
    left:  { ...base, top: 0, bottom: 0, left: 0, width: "16%", background: "linear-gradient(90deg, rgba(136,192,112,0.3), transparent)" },
    right: { ...base, top: 0, bottom: 0, right: 0, width: "16%", background: "linear-gradient(270deg, rgba(136,192,112,0.3), transparent)" },
  };
  return <div style={map[dir]} key={dir + Math.random()} />;
}

function FpsHud({ turbo }: { turbo: boolean }) {
  const [fps, setFps] = useState(59.7);
  useEffect(() => {
    const t = setInterval(() => {
      const base = turbo ? 119.4 : 59.7;
      setFps(+(base + (Math.random() - 0.5) * 0.6).toFixed(1));
    }, 700);
    return () => clearInterval(t);
  }, [turbo]);
  return (
    <div style={{
      position: "absolute", top: 6, right: 7, display: "flex", gap: 5, alignItems: "center",
      fontFamily: "var(--font-mono)", fontSize: 9, lineHeight: 1,
      color: "var(--dmg-lightest)", background: "rgba(8,24,32,0.66)",
      padding: "3px 5px", borderRadius: 3, letterSpacing: "0.02em",
      textShadow: "0 1px 0 #000",
    }}>
      <span style={{ color: turbo ? "var(--cgb-amber)" : "var(--dmg-light)" }}>●</span>
      {fps.toFixed(1)} fps
    </div>
  );
}

export function BootSplash() {
  return (
    <div style={{ textAlign: "center", lineHeight: 1.6, padding: 12 }}>
      <div style={{ fontFamily: "var(--font-pixel)", color: "var(--dmg-light)", fontSize: 34, textShadow: "0 0 14px rgba(136,192,112,0.4)" }}>rubc</div>
      <div style={{ fontFamily: "var(--font-mono)", color: "var(--dmg-dark)", fontSize: 10, marginTop: 10, letterSpacing: "0.2em" }}>► NO CARTRIDGE</div>
      <div style={{ fontFamily: "var(--font-mono)", color: "var(--dmg-dark)", fontSize: 9, marginTop: 14, letterSpacing: "0.1em", opacity: 0.7 }}>load a .gb / .gbc rom</div>
    </div>
  );
}

export function PlaceholderRender({ rom }: { rom: EmulatorRom }) {
  return (
    <div style={{
      width: "100%", height: "100%",
      background: "linear-gradient(160deg, #11131a, #0b0c10)",
      display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", gap: 10,
    }}>
      <div style={{ fontFamily: "var(--font-pixel)", fontSize: 22, color: "var(--dmg-light)" }}>{rom.title}</div>
      <div style={{ fontFamily: "var(--font-mono)", fontSize: 9, color: "var(--text-faint)", letterSpacing: "0.12em" }}>► RUNNING · NO CAPTURE</div>
    </div>
  );
}

export function CartIcon({ accent = "var(--dmg-light)", size = 56, label }: { accent?: string; size?: number; label?: string }) {
  return (
    <div style={{
      width: size, height: size, position: "relative",
      background: "#1d2129", border: "2px solid #2a2f3a", borderRadius: 4,
      display: "flex", alignItems: "center", justifyContent: "center", overflow: "hidden",
    }}>
      <div style={{ position: "absolute", top: 0, left: 0, right: 0, height: "30%", background: "#11131a", borderBottom: "1px solid #2a2f3a" }} />
      <div style={{ position: "absolute", top: "12%", right: "14%", width: size * 0.16, height: size * 0.06, background: accent, opacity: 0.8 }} />
      <div style={{ width: "56%", height: "40%", marginTop: "12%", background: "#0b0c10", border: `1px solid ${accent}`, opacity: 0.85 }} />
      {label && <span style={{ position: "absolute", bottom: 3, fontFamily: "var(--font-pixel)", fontSize: 9, color: accent }}>{label}</span>}
    </div>
  );
}

export function Viewport(props: ViewportProps) {
  const store = useEmulator();
  
  const phase = props.phase ?? store.phase;
  const rom = props.rom !== undefined ? props.rom : store.rom;
  const filter = props.filter ?? store.filter;
  const scaling = props.scaling ?? store.settings.scaling;
  const smoothing = props.smoothing ?? store.settings.smoothing;
  const showFps = props.showFps ?? store.settings.showFps;
  const turbo = props.turbo ?? store.settings.turbo;
  
  const pressedDir = props.pressedDir ?? null;
  const glow = props.glow ?? true;
  const fullBleed = props.fullBleed ?? false;

  const ref = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);

  const showCanvas = phase === "booting" || phase === "running" || phase === "paused";

  // Re-attach whenever the canvas becomes shown (phase leaves "empty") so the
  // store can bind it to the emulator core and start rendering. The player
  // routes mount Viewport at phase="empty" with a hidden canvas, so a
  // mount-only attach would hand the store a canvas it never wires up.
  useEffect(() => {
    store.attachCanvas(canvasRef.current);
  }, [store.attachCanvas, showCanvas]);





  return (
    <div
      ref={ref}
      style={{
        position: "relative", width: "100%", aspectRatio: "160 / 144",
        background: "var(--screen)",
        border: fullBleed ? "none" : "var(--border-screen-width) solid var(--border-screen)",
        borderRadius: fullBleed ? 0 : "var(--radius-screen)",
        boxShadow: fullBleed ? "none" : (glow ? "var(--glow-screen)" : "var(--shadow)"),
        overflow: "hidden", display: "flex", alignItems: "center", justifyContent: "center",
      }}
    >
      {/* render layer */}
      <canvas
        ref={canvasRef}
        width={160}
        height={144}
        style={{
          // Fill the aspect-ratio'd parent wrapper absolutely. Using height:"100%"
          // relative sizing collapsed to 0 on mobile when a caller wrapper had no
          // bounded height (audio played but nothing rendered).
          position: "absolute", inset: 0,
          width: "100%", height: "100%",
          objectFit: scaling === "integer" ? "contain" : "fill",
          imageRendering: smoothing ? "auto" : "pixelated",
          display: showCanvas ? "block" : "none",
          filter: phase === "booting" ? `${filter === "none" ? "" : filter} brightness(1.05)` : (filter === "none" ? "none" : filter),
          transition: "filter 160ms cubic-bezier(0.2,0,0.1,1)",
        }}
      />
      
      {!showCanvas && (
        rom ? <PlaceholderRender rom={rom} /> : <BootSplash />
      )}

      {/* directional feedback */}
      {phase === "running" && <EdgeFlash dir={pressedDir} />}

      {/* boot scanline sweep */}
      {phase === "booting" && (
        <div style={{
          position: "absolute", inset: 0, pointerEvents: "none",
          background: "repeating-linear-gradient(0deg, rgba(0,0,0,0.18) 0 2px, transparent 2px 4px)",
        }}>
          <div className="rubc-scan" />
        </div>
      )}

      {/* paused veil */}
      {phase === "paused" && (
        <div style={{
          position: "absolute", inset: 0, background: "rgba(8,24,32,0.72)",
          display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", gap: 6,
        }}>
          <div style={{ fontFamily: "var(--font-pixel)", fontSize: 26, color: "var(--dmg-lightest)", letterSpacing: "0.06em" }}>PAUSED</div>
          <div style={{ fontFamily: "var(--font-mono)", fontSize: 10, color: "var(--dmg-light)", letterSpacing: "0.12em" }}>▶ tap A or Start</div>
        </div>
      )}

      {/* CGB/DMG mode tag + FPS while alive */}
      {(phase === "running" || phase === "booting") && (
        <>
          {showFps && <FpsHud turbo={turbo} />}
          <div style={{
            position: "absolute", top: 6, left: 7,
            fontFamily: "var(--font-mono)", fontSize: 9, lineHeight: 1, letterSpacing: "0.08em",
            color: "var(--dmg-lightest)", background: "rgba(8,24,32,0.66)",
            padding: "3px 5px", borderRadius: 3, textShadow: "0 1px 0 #000",
          }}>{rom ? rom.mode : ""}{turbo ? " · ×2" : ""}</div>
        </>
      )}
    </div>
  );
}
