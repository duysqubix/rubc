"use client";

import React from "react";

export interface SwitchProps extends Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, "onChange"> {
  checked?: boolean;
  onChange?: (checked: boolean) => void;
  label?: string;
}

export function Switch({
  checked = false,
  onChange,
  label,
  disabled = false,
  id,
  style,
  ...rest
}: SwitchProps) {
  const switchId =
    id ||
    (label ? `sw-${label.replace(/\s+/g, "-").toLowerCase()}` : undefined);
  return (
    <label
      htmlFor={switchId}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "10px",
        cursor: disabled ? "not-allowed" : "pointer",
        opacity: disabled ? 0.45 : 1,
        fontFamily: "var(--font-sans)",
        fontSize: "14px",
        color: "var(--text)",
        ...style,
      }}
    >
      <button
        id={switchId}
        type="button"
        role="switch"
        aria-checked={checked}
        disabled={disabled}
        onClick={() => !disabled && onChange && onChange(!checked)}
        style={{
          position: "relative",
          width: "42px",
          height: "24px",
          flexShrink: 0,
          padding: 0,
          border: `1px solid ${
            checked ? "var(--dmg-dark)" : "var(--border-strong)"
          }`,
          borderRadius: "var(--radius-sm)",
          background: checked ? "var(--dmg-dark)" : "var(--surface-sunken)",
          cursor: disabled ? "not-allowed" : "pointer",
          transition:
            "background var(--dur-fast) var(--ease), border-color var(--dur-fast) var(--ease)",
        }}
        {...rest}
      >
        <span
          style={{
            position: "absolute",
            top: "2px",
            left: checked ? "20px" : "2px",
            width: "18px",
            height: "18px",
            borderRadius: "2px",
            background: checked ? "var(--dmg-light)" : "var(--slate-300)",
            transition:
              "left var(--dur-fast) var(--ease), background var(--dur-fast) var(--ease)",
          }}
        />
      </button>
      {label && <span>{label}</span>}
    </label>
  );
}
