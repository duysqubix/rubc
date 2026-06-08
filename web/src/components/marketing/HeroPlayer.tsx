"use client";

import React, { useRef, useState } from "react";
import { useEmulator } from "@/lib/store";
import { Viewport } from "@/components/Viewport";
import { Screen } from "@/components/ui";

const SCALE = 3;

export function HeroPlayer() {
  const { phase, loadFile, flash } = useEmulator();
  const inputRef = useRef<HTMLInputElement | null>(null);
  const [dragging, setDragging] = useState(false);
  const [loading, setLoading] = useState(false);

  const live = phase !== "empty";

  const pick = () => inputRef.current?.click();

  const handleFile = async (file: File) => {
    setLoading(true);
    try {
      await loadFile(file);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        gap: 14,
      }}
    >
      {live ? (
        <div style={{ width: 160 * SCALE, height: 144 * SCALE }}>
          <Viewport glow />
        </div>
      ) : (
        <button
          type="button"
          onClick={pick}
          onDragOver={(e) => {
            e.preventDefault();
            setDragging(true);
          }}
          onDragLeave={() => setDragging(false)}
          onDrop={(e) => {
            e.preventDefault();
            setDragging(false);
            const file = e.dataTransfer.files?.[0];
            if (file) handleFile(file);
          }}
          aria-label="Load a Game Boy ROM and play it here"
          style={{
            position: "relative",
            padding: 0,
            border: "none",
            background: "transparent",
            cursor: "pointer",
            borderRadius: "var(--radius-screen)",
            outline: dragging ? "2px solid var(--accent)" : "none",
            outlineOffset: 6,
          }}
        >
          <Screen
            src="/crystal-intro.gif"
            scale={SCALE}
            glow
            status="Pokémon Crystal — CGB mode · 59.7275 Hz"
          />
          <div
            style={{
              position: "absolute",
              inset: 0,
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              gap: 8,
              borderRadius: "var(--radius-screen)",
              background:
                "color-mix(in srgb, var(--bg-deep) 62%, transparent)",
              opacity: 0,
              transition: "opacity var(--dur) var(--ease)",
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.opacity = "1";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.opacity = "0";
            }}
          >
            <span
              style={{
                fontFamily: "var(--font-pixel)",
                fontSize: 22,
                color: "var(--text-strong)",
              }}
            >
              {loading ? "loading…" : "▸ play a rom"}
            </span>
            <span
              style={{
                fontFamily: "var(--font-mono)",
                fontSize: 11,
                color: "var(--text-muted)",
                letterSpacing: "0.04em",
              }}
            >
              drop or pick a .gb / .gbc / .zip
            </span>
          </div>
        </button>
      )}

      <input
        ref={inputRef}
        type="file"
        accept=".gb,.gbc,.zip"
        style={{ display: "none" }}
        onChange={(e) => {
          const file = e.target.files?.[0];
          if (file) handleFile(file);
          e.target.value = "";
        }}
      />

      <div
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: 11,
          color: "var(--text-faint)",
          letterSpacing: "0.04em",
          textAlign: "center",
        }}
      >
        {live
          ? "▸ playing in your browser · ⌨ arrows · X=A · Z=B · enter=start"
          : "▸ live WebAssembly build · your ROM never leaves your machine"}
      </div>
    </div>
  );
}
