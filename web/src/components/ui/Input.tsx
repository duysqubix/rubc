"use client";

import React, { useState } from "react";

export interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  hint?: string;
  prefix?: string;
  invalid?: boolean;
}

export function Input({
  label,
  hint,
  prefix,
  invalid = false,
  style,
  id,
  ...rest
}: InputProps) {
  const inputId =
    id ||
    (label ? `in-${label.replace(/\s+/g, "-").toLowerCase()}` : undefined);
  const [focus, setFocus] = useState(false);
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: "6px",
        ...style,
      }}
    >
      {label && (
        <label
          htmlFor={inputId}
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: "11px",
            letterSpacing: "0.06em",
            textTransform: "uppercase",
            color: "var(--text-muted)",
          }}
        >
          {label}
        </label>
      )}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          background: "var(--surface-sunken)",
          border: `1px solid ${
            invalid
              ? "var(--danger)"
              : focus
              ? "var(--accent)"
              : "var(--border)"
          }`,
          borderRadius: "var(--radius)",
          boxShadow: focus ? "var(--shadow-focus)" : "none",
          transition:
            "border-color var(--dur-fast) var(--ease), box-shadow var(--dur-fast) var(--ease)",
        }}
      >
        {prefix && (
          <span
            style={{
              padding: "0 0 0 12px",
              color: "var(--text-faint)",
              fontFamily: "var(--font-mono)",
              fontSize: "14px",
            }}
          >
            {prefix}
          </span>
        )}
        <input
          id={inputId}
          onFocus={() => setFocus(true)}
          onBlur={() => setFocus(false)}
          style={{
            flex: 1,
            background: "transparent",
            border: "none",
            outline: "none",
            color: "var(--text)",
            fontFamily: "var(--font-mono)",
            fontSize: "14px",
            padding: "10px 12px",
          }}
          {...rest}
        />
      </div>
      {hint && (
        <span
          style={{
            fontFamily: "var(--font-sans)",
            fontSize: "12px",
            color: invalid ? "var(--danger)" : "var(--text-faint)",
          }}
        >
          {hint}
        </span>
      )}
    </div>
  );
}
