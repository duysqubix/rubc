import React from "react";

export interface StatusPillProps extends React.HTMLAttributes<HTMLSpanElement> {
  status?: "pass" | "fail" | "wip" | "info";
  label?: string;
  detail?: string;
}

export function StatusPill({
  status = "pass",
  label,
  detail,
  style,
  ...rest
}: StatusPillProps) {
  const map = {
    pass: {
      glyph: "✅",
      color: "var(--success)",
      text: "PASS",
    },
    fail: {
      glyph: "✕",
      color: "var(--danger)",
      text: "FAIL",
    },
    wip: {
      glyph: "🚧",
      color: "var(--warning)",
      text: "WIP",
    },
    info: {
      glyph: "●",
      color: "var(--info)",
      text: "INFO",
    },
  };
  const s = map[status] || map.pass;
  return (
    <span
      className={`rubc-status rubc-status--${status}`}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "8px",
        fontFamily: "var(--font-mono)",
        fontSize: "12px",
        padding: "5px 10px",
        borderRadius: "var(--radius-sm)",
        background: "var(--surface-sunken)",
        border: `1px solid ${s.color}`,
        boxShadow: `inset 3px 0 0 0 ${s.color}`,
        ...style,
      }}
      {...rest}
    >
      <span
        aria-hidden={true}
        style={{
          color: s.color,
          fontWeight: 700,
        }}
      >
        {s.glyph}
      </span>
      <span
        style={{
          color: "var(--text)",
          fontWeight: 600,
        }}
      >
        {label || s.text}
      </span>
      {detail && (
        <span
          style={{
            color: "var(--text-muted)",
            marginLeft: "2px",
          }}
        >
          {detail}
        </span>
      )}
    </span>
  );
}
