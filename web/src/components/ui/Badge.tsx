import React from "react";

export interface BadgeProps extends React.HTMLAttributes<HTMLSpanElement> {
  tone?: "neutral" | "rust" | "dmg" | "cgb" | "pass" | "warn" | "fail";
  variant?: "soft" | "solid";
}

export function Badge({
  children,
  tone = "neutral",
  variant = "soft",
  style,
  ...rest
}: BadgeProps) {
  const tones = {
    neutral: {
      fg: "var(--text-muted)",
      bg: "var(--surface-raised)",
      bd: "var(--border)",
    },
    rust: {
      fg: "var(--rust-300)",
      bg: "var(--accent-soft)",
      bd: "var(--rust-700)",
    },
    dmg: {
      fg: "var(--dmg-light)",
      bg: "rgba(136,192,112,0.12)",
      bd: "var(--dmg-dark)",
    },
    cgb: {
      fg: "var(--cgb-purple)",
      bg: "rgba(139,92,246,0.14)",
      bd: "#5b3aa6",
    },
    pass: {
      fg: "var(--success)",
      bg: "rgba(136,192,112,0.12)",
      bd: "var(--dmg-dark)",
    },
    warn: {
      fg: "var(--warning)",
      bg: "rgba(245,179,66,0.12)",
      bd: "#8a6320",
    },
    fail: {
      fg: "var(--danger)",
      bg: "rgba(239,90,90,0.12)",
      bd: "#8a2f2f",
    },
  };
  const t = tones[tone] || tones.neutral;
  const solid = variant === "solid";
  return (
    <span
      className={`rubc-badge rubc-badge--${tone}`}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "5px",
        fontFamily: "var(--font-mono)",
        fontSize: "11px",
        fontWeight: 600,
        letterSpacing: "0.04em",
        textTransform: "uppercase",
        lineHeight: 1,
        padding: "4px 8px",
        borderRadius: "var(--radius-sm)",
        color: solid ? "var(--white)" : t.fg,
        background: solid ? t.fg : t.bg,
        border: `1px solid ${solid ? "transparent" : t.bd}`,
        ...style,
      }}
      {...rest}
    >
      {children}
    </span>
  );
}
