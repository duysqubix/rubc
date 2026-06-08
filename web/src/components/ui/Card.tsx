import React from "react";

export interface CardProps extends Omit<React.HTMLAttributes<HTMLDivElement>, "title"> {
  title?: React.ReactNode;
  eyebrow?: React.ReactNode;
  accent?: boolean;
  padding?: string | number;
}

export function Card({
  children,
  title,
  eyebrow,
  accent = false,
  padding = "20px",
  style,
  ...rest
}: CardProps) {
  return (
    <div
      className="rubc-card"
      style={{
        background: "var(--surface)",
        border: "1px solid var(--border)",
        borderRadius: "var(--radius-md)",
        boxShadow: "var(--shadow)",
        borderTop: accent
          ? "var(--border-width-2) solid var(--accent)"
          : "1px solid var(--border)",
        overflow: "hidden",
        ...style,
      }}
      {...rest}
    >
      {(title || eyebrow) && (
        <div
          style={{
            padding: "14px 20px",
            borderBottom: "1px solid var(--border)",
            background: "var(--surface-sunken)",
          }}
        >
          {eyebrow && (
            <div
              style={{
                fontFamily: "var(--font-mono)",
                fontSize: "10px",
                letterSpacing: "0.12em",
                textTransform: "uppercase",
                color: "var(--accent)",
                marginBottom: title ? "4px" : 0,
              }}
            >
              {eyebrow}
            </div>
          )}
          {title && (
            <div
              style={{
                fontFamily: "var(--font-sans)",
                fontSize: "15px",
                fontWeight: 600,
                color: "var(--text-strong)",
              }}
            >
              {title}
            </div>
          )}
        </div>
      )}
      <div
        style={{
          padding,
        }}
      >
        {children}
      </div>
    </div>
  );
}
