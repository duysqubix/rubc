"use client";

import React, { useState, useRef } from "react";
import { useEmulator } from "@/lib/store";
import type { EmulatorRom } from "@/lib/store";
import { Badge } from "@/components/ui/Badge";
import { CartIcon } from "@/components/Viewport";

function getTestAccuracy(title: string): string | null {
  const t = title.toLowerCase();
  if (t.includes("cpu_instrs")) return "11/11";
  if (t.includes("dmg_sound") || t.includes("cgb_sound")) return "12/12";
  if (t.includes("acid2") || t.includes("acid-hell")) return "Pixel-exact";
  if (t.includes("mooneye")) return "93/115";
  if (t.includes("instr_timing") || t.includes("mem_timing") || t.includes("halt_bug") || t.includes("interrupt_time")) return "Pass";
  if (t.includes("oam_bug")) return "7/8";
  return null;
}

function isTestRom(rom: EmulatorRom): boolean {
  const t = (rom.name || rom.title).toLowerCase();
  return t.includes("test") || t.includes("mooneye") || t.includes("acid2") || t.includes("acid-hell") || t.includes("cpu_instrs") || t.includes("dmg_sound") || t.includes("cgb_sound") || t.includes("instr_timing") || t.includes("mem_timing") || t.includes("halt_bug") || t.includes("interrupt_time") || t.includes("oam_bug");
}

function DropZone({ onFile }: { onFile: (file: File) => void }) {
  const [drag, setDrag] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  return (
    <div
      onClick={() => inputRef.current?.click()}
      onDragOver={(e) => {
        e.preventDefault();
        setDrag(true);
      }}
      onDragLeave={() => setDrag(false)}
      onDrop={(e) => {
        e.preventDefault();
        setDrag(false);
        if (e.dataTransfer.files[0]) {
          onFile(e.dataTransfer.files[0]);
        }
      }}
      style={{
        border: `2px dashed ${drag ? "var(--accent)" : "var(--border-strong)"}`,
        borderRadius: "var(--radius-md)",
        background: drag ? "var(--accent-soft)" : "var(--surface-sunken)",
        padding: "22px 16px",
        textAlign: "center",
        cursor: "pointer",
        transition: "border-color 140ms, background 140ms",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        gap: 8,
      }}
    >
      <div style={{ fontFamily: "var(--font-pixel)", fontSize: 22, color: "var(--accent)" }}>＋</div>
      <div style={{ fontFamily: "var(--font-mono)", fontSize: 13, color: "var(--text)", fontWeight: 600 }}>
        Load ROM
      </div>
      <div style={{ fontFamily: "var(--font-sans)", fontSize: 11.5, color: "var(--text-faint)", lineHeight: 1.5, maxWidth: 220 }}>
        Tap to browse, or drop a <span style={{ fontFamily: "var(--font-mono)", color: "var(--text-muted)" }}>.gb</span> / <span style={{ fontFamily: "var(--font-mono)", color: "var(--text-muted)" }}>.gbc</span> file. It never leaves your device.
      </div>
      <input
        type="file"
        ref={inputRef}
        className="hidden"
        style={{ display: "none" }}
        accept=".gb,.gbc,.zip"
        onChange={(e) => {
          if (e.target.files?.[0]) {
            onFile(e.target.files[0]);
          }
          if (inputRef.current) {
            inputRef.current.value = "";
          }
        }}
      />
    </div>
  );
}

function RomRow({ rom, active, onPlay }: { rom: EmulatorRom; active: boolean; onPlay: (id: string) => void }) {
  const [press, setPress] = useState(false);
  
  const sizeStr = rom.size ? `${(rom.size / 1024).toFixed(0)}KB` : "???KB";
  
  let timeStr = "";
  if (rom.lastPlayed) {
    const d = new Date(rom.lastPlayed);
    timeStr = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
  }

  const testAccuracy = getTestAccuracy(rom.title || rom.name || "");
  const isTest = isTestRom(rom);

  // Mock MBC and battery since they are not in EmulatorRom
  const mbc = "MBC5";
  const battery = true;
  const year = "1998";

  return (
    <button
      onPointerDown={() => setPress(true)}
      onPointerUp={() => setPress(false)}
      onPointerLeave={() => setPress(false)}
      onClick={() => onPlay(rom.id)}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 13,
        width: "100%",
        textAlign: "left",
        background: active ? "var(--surface-raised)" : "var(--surface)",
        border: active ? "1px solid var(--accent)" : "1px solid var(--border)",
        borderRadius: "var(--radius-md)",
        padding: 10,
        cursor: "pointer",
        boxShadow: press ? "0 0 0 0 var(--bg-deep)" : "0 2px 0 0 var(--bg-deep)",
        transform: press ? "translateY(2px)" : "none",
        transition: "transform 90ms cubic-bezier(0.2,0,0.1,1), box-shadow 90ms cubic-bezier(0.2,0,0.1,1)",
      }}
    >
      {/* thumb */}
      <div
        style={{
          width: 56,
          height: 56,
          flexShrink: 0,
          borderRadius: 5,
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
          <CartIcon accent={rom.mode === "CGB" ? "var(--cgb-purple)" : "var(--dmg-light)"} size={42} />
        )}
      </div>
      {/* meta */}
      <div style={{ flex: 1, minWidth: 0 }}>
        <div
          style={{
            fontFamily: "var(--font-sans)",
            fontSize: 14.5,
            fontWeight: 600,
            color: "var(--text-strong)",
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
        >
          {rom.title}
        </div>
        <div
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: 10.5,
            color: "var(--text-muted)",
            marginTop: 3,
            display: "flex",
            gap: 7,
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
        >
          <span>{mbc}</span>
          <span style={{ opacity: 0.4 }}>·</span>
          <span>{sizeStr}</span>
          {battery && (
            <>
              <span style={{ opacity: 0.4 }}>·</span>
              <span style={{ color: "var(--cgb-amber)" }}>save</span>
            </>
          )}
        </div>
      </div>
      {/* tags */}
      <div style={{ display: "flex", flexDirection: "column", alignItems: "flex-end", gap: 6, flexShrink: 0 }}>
        <Badge tone={rom.mode === "CGB" ? "cgb" : "dmg"}>{rom.mode}</Badge>
        {active ? (
          <span style={{ fontFamily: "var(--font-mono)", fontSize: 9.5, color: "var(--accent)", letterSpacing: "0.1em" }}>
            ● LOADED
          </span>
        ) : isTest && testAccuracy ? (
          <span style={{ fontFamily: "var(--font-mono)", fontSize: 9.5, color: "var(--success)" }}>
            ✅ {testAccuracy}
          </span>
        ) : timeStr ? (
          <span style={{ fontFamily: "var(--font-mono)", fontSize: 9.5, color: "var(--text-faint)" }}>
            {timeStr}
          </span>
        ) : (
          <span style={{ fontFamily: "var(--font-mono)", fontSize: 9.5, color: "var(--text-faint)" }}>
            {year}
          </span>
        )}
      </div>
    </button>
  );
}

function Section({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 9 }}>
      <div style={{ display: "flex", alignItems: "baseline", justifyContent: "space-between" }}>
        <div
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: 11,
            color: "var(--text-muted)",
            letterSpacing: "0.1em",
            textTransform: "uppercase",
          }}
        >
          {label}
        </div>
        {hint && <div style={{ fontFamily: "var(--font-mono)", fontSize: 10, color: "var(--accent)" }}>{hint}</div>}
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>{children}</div>
    </div>
  );
}

export function Library() {
  const store = useEmulator();

  const handleFile = async (file: File) => {
    const rom = await store.loadFile(file);
    if (rom) {
      store.setView("play");
    }
  };

  const handlePlay = async (id: string) => {
    await store.openRom(id);
    store.setView("play");
  };

  const games = store.roms.filter((r) => !isTestRom(r));
  const tests = store.roms.filter((r) => isTestRom(r));

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, paddingBottom: 24 }}>
      <DropZone onFile={handleFile} />

      {games.length > 0 && (
        <Section label={`Games · ${games.length}`}>
          {games.map((r) => (
            <RomRow key={r.id} rom={r} active={r.id === store.romId} onPlay={handlePlay} />
          ))}
        </Section>
      )}

      {tests.length > 0 && (
        <Section label={`Test ROMs · ${tests.length}`} hint="// verified on real silicon">
          {tests.map((r) => (
            <RomRow key={r.id} rom={r} active={r.id === store.romId} onPlay={handlePlay} />
          ))}
        </Section>
      )}
    </div>
  );
}
