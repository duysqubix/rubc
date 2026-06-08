"use client";

import React from "react";

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "ghost" | "screen";
  size?: "sm" | "md" | "lg";
  block?: boolean;
}

export function Button({
  children,
  variant = "primary",
  size = "md",
  disabled = false,
  block = false,
  type = "button",
  onClick,
  style,
  ...rest
}: ButtonProps) {
  const sizes = {
    sm: {
      padding: "6px 12px",
      fontSize: "13px",
    },
    md: {
      padding: "9px 16px",
      fontSize: "14px",
    },
    lg: {
      padding: "12px 22px",
      fontSize: "16px",
    },
  };
  const variants = {
    primary: {
      background: "var(--accent)",
      color: "var(--text-on-accent)",
      borderColor: "var(--accent-press)",
    },
    secondary: {
      background: "var(--surface-raised)",
      color: "var(--text)",
      borderColor: "var(--border-strong)",
    },
    ghost: {
      background: "transparent",
      color: "var(--text)",
      borderColor: "var(--border)",
    },
    screen: {
      background: "var(--dmg-darkest)",
      color: "var(--dmg-light)",
      borderColor: "var(--dmg-dark)",
    },
  };
  const v = variants[variant] || variants.primary;
  const s = sizes[size] || sizes.md;
  return (
    <button
      type={type}
      disabled={disabled}
      onClick={onClick}
      className={`rubc-btn rubc-btn--${variant}`}
      style={{
        fontFamily: "var(--font-mono)",
        fontWeight: 600,
        letterSpacing: "0.01em",
        lineHeight: 1,
        display: block ? "flex" : "inline-flex",
        width: block ? "100%" : "auto",
        alignItems: "center",
        justifyContent: "center",
        gap: "8px",
        border: "var(--border-width-2) solid",
        borderRadius: "var(--radius)",
        cursor: disabled ? "not-allowed" : "pointer",
        opacity: disabled ? 0.45 : 1,
        boxShadow: "0 var(--press-offset) 0 0 var(--bg-deep)",
        transition:
          "transform var(--dur-fast) var(--ease), box-shadow var(--dur-fast) var(--ease), background var(--dur-fast) var(--ease)",
        ...s,
        ...v,
        ...style,
      }}
      onMouseDown={(e) => {
        if (disabled) return;
        e.currentTarget.style.transform = "translateY(var(--press-offset))";
        e.currentTarget.style.boxShadow = "0 0 0 0 var(--bg-deep)";
      }}
      onMouseUp={(e) => {
        if (disabled) return;
        e.currentTarget.style.transform = "translateY(0)";
        e.currentTarget.style.boxShadow = "0 var(--press-offset) 0 0 var(--bg-deep)";
      }}
      onMouseLeave={(e) => {
        if (disabled) return;
        e.currentTarget.style.transform = "translateY(0)";
        e.currentTarget.style.boxShadow = "0 var(--press-offset) 0 0 var(--bg-deep)";
      }}
      {...rest}
    >
      {children}
    </button>
  );
}
