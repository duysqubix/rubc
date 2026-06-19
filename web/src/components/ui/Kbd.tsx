import React from "react";

export type KbdProps = React.HTMLAttributes<HTMLElement>;

export function Kbd({ children, style, ...rest }: KbdProps) {
  return (
    <kbd
      className="rubc-kbd"
      style={{
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        minWidth: "22px",
        height: "22px",
        padding: "0 6px",
        fontFamily: "var(--font-mono)",
        fontSize: "12px",
        fontWeight: 600,
        color: "var(--text)",
        background: "var(--surface-raised)",
        border: "1px solid var(--border-strong)",
        borderRadius: "var(--radius-sm)",
        boxShadow: "0 2px 0 0 var(--bg-deep)",
        lineHeight: 1,
        ...style,
      }}
      {...rest}
    >
      {children}
    </kbd>
  );
}
