"use client";

import React from "react";
import { useEmulator } from "../lib/store";
import { Switch, Badge, StatusPill, Card, Kbd } from "./ui";
import type { Scaling } from "../lib/store";

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

interface SliderProps {
  value: number;
  onChange: (value: number) => void;
  min?: number;
  max?: number;
  disabled?: boolean;
}

function Slider({ value, onChange, min = 0, max = 100, disabled }: SliderProps) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 12, opacity: disabled ? 0.45 : 1 }}>
      <input
        type="range"
        min={min}
        max={max}
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(+e.target.value)}
        className="rubc-range"
        style={{ flex: 1 }}
      />
      <span
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: 12,
          color: "var(--text)",
          minWidth: 34,
          textAlign: "right",
        }}
      >
        {value}
      </span>
    </div>
  );
}

interface FieldProps {
  label: string;
  hint?: string;
  children: React.ReactNode;
}

function Field({ label, hint, children }: FieldProps) {
  const isSwitch = React.isValidElement(children) && children.type === Switch;
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 8, padding: "13px 0" }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
        <div>
          <div style={{ fontFamily: "var(--font-sans)", fontSize: 14, fontWeight: 600, color: "var(--text-strong)" }}>
            {label}
          </div>
          {hint && (
            <div style={{ fontFamily: "var(--font-mono)", fontSize: 10.5, color: "var(--text-faint)", marginTop: 2 }}>
              {hint}
            </div>
          )}
        </div>
        {isSwitch ? children : null}
      </div>
      {!isSwitch ? children : null}
    </div>
  );
}

interface GroupProps {
  title: string;
  children: React.ReactNode;
}

function Group({ title, children }: GroupProps) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
      <div
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: 11,
          color: "var(--text-muted)",
          letterSpacing: "0.12em",
          textTransform: "uppercase",
          marginBottom: 6,
        }}
      >
        {title}
      </div>
      <div
        style={{
          background: "var(--surface)",
          border: "1px solid var(--border)",
          borderRadius: "var(--radius-md)",
          padding: "2px 16px",
          boxShadow: "var(--shadow-sm)",
        }}
      >
        {React.Children.toArray(children)
          .filter(Boolean)
          .map((c, i, arr) => (
            <div key={i} style={{ borderBottom: i < arr.length - 1 ? "1px solid var(--border)" : "none" }}>
              {c}
            </div>
          ))}
      </div>
    </div>
  );
}

export function Settings() {
  const emu = useEmulator();
  const { settings: s, set, KEYMAP } = emu;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20, paddingBottom: 28 }}>
      <Group title="Display">
        <Field
          label="Palette"
          hint={
            s.palette === "auto"
              ? "// respect the cartridge"
              : s.palette === "dmg"
                ? "// classic 4-shade green"
                : "// monochrome"
          }
        >
          <Segmented<"auto" | "dmg" | "grayscale">
            value={s.palette}
            onChange={(v) => set({ palette: v })}
            options={[
              { value: "auto", label: "Auto" },
              { value: "dmg", label: "DMG green" },
              { value: "grayscale", label: "Gray" },
            ]}
          />
        </Field>
        <Field
          label="Hardware"
          hint={
            s.bootMode === "auto"
              ? "// auto-detect from cartridge"
              : s.bootMode === "dmg"
                ? "// force classic Game Boy"
                : "// force Game Boy Color (restarts game)"
          }
        >
          <Segmented<"auto" | "dmg" | "cgb">
            value={s.bootMode}
            onChange={(v) => set({ bootMode: v })}
            options={[
              { value: "auto", label: "Auto" },
              { value: "dmg", label: "DMG" },
              { value: "cgb", label: "GBC" },
            ]}
          />
        </Field>
        <Field
          label="Screen scaling"
          hint={s.scaling === "integer" ? "// whole-pixel multiples" : "// fill the frame"}
        >
          <Segmented<Scaling>
            value={s.scaling}
            onChange={(v) => set({ scaling: v })}
            options={[
              { value: "fit", label: "Fit" },
              { value: "integer", label: "Integer" },
            ]}
          />
        </Field>
        <Field label="Smoothing">
          <Switch checked={s.smoothing} onChange={(v) => set({ smoothing: v })} />
        </Field>
        <Field label="Show FPS">
          <Switch checked={s.showFps} onChange={(v) => set({ showFps: v })} />
        </Field>
      </Group>

      <Group title="Audio">
        <Field label="Sound">
          <Switch checked={s.sound} onChange={(v) => set({ sound: v })} />
        </Field>
        <Field label="Volume">
          <Slider value={s.volume} onChange={(v) => set({ volume: v })} disabled={!s.sound} />
        </Field>
      </Group>

      <Group title="Performance">
        <Field label="Turbo (double-speed)" hint="// 59.7275 Hz → ×2">
          <Switch checked={s.turbo} onChange={(v) => set({ turbo: v })} />
        </Field>
      </Group>

      <Group title="Controls">
        <Field label="Haptics">
          <Switch checked={s.haptics} onChange={(v) => set({ haptics: v })} />
        </Field>
      </Group>

      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
        <div
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: 11,
            color: "var(--text-muted)",
            letterSpacing: "0.12em",
            textTransform: "uppercase",
          }}
        >
          Keyboard mapping
        </div>
        <div
          style={{
            background: "var(--surface)",
            border: "1px solid var(--border)",
            borderRadius: "var(--radius-md)",
            padding: 14,
            display: "flex",
            flexDirection: "column",
            gap: 11,
          }}
        >
          {KEYMAP.map((m) => (
            <div key={m.btn} style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <span style={{ fontFamily: "var(--font-sans)", fontSize: 13.5, color: "var(--text)" }}>{m.btn}</span>
              <span style={{ display: "flex", gap: 5 }}>
                {m.keys.map((k, i) => (
                  <Kbd key={i}>{k}</Kbd>
                ))}
              </span>
            </div>
          ))}
          <div style={{ fontFamily: "var(--font-sans)", fontSize: 11, color: "var(--text-faint)", marginTop: 2 }}>
            Physical keyboard bindings, used when rubc runs on desktop.
          </div>
        </div>
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
        <div
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: 11,
            color: "var(--text-muted)",
            letterSpacing: "0.12em",
            textTransform: "uppercase",
          }}
        >
          Accuracy
        </div>
        <div
          style={{
            background: "var(--surface)",
            border: "1px solid var(--border)",
            borderTop: "2px solid var(--accent)",
            borderRadius: "var(--radius-md)",
            padding: 14,
            display: "flex",
            flexDirection: "column",
            gap: 8,
          }}
        >
          <StatusPill status="pass" label="cgb-acid2" detail="pixel-exact" />
          <StatusPill status="pass" label="cpu_instrs" detail="11/11" />
          <StatusPill status="pass" label="mooneye" detail="93/115" />
          <StatusPill status="wip" label="mealybug-tearoom" />
          <div style={{ display: "flex", alignItems: "center", gap: 8, marginTop: 4 }}>
            <Badge tone="rust" variant="solid">
              safe Rust
            </Badge>
            <span style={{ fontFamily: "var(--font-mono)", fontSize: 10.5, color: "var(--text-faint)" }}>
              rubc 0.4.0 · wasm
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}
