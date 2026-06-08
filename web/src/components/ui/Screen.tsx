import React from "react";

export interface ScreenProps extends React.HTMLAttributes<HTMLDivElement> {
  src?: string;
  alt?: string;
  scale?: number;
  status?: React.ReactNode;
  glow?: boolean;
}

export function Screen({
  src,
  alt = "Game Boy screen",
  scale = 3,
  status,
  glow = true,
  children,
  style,
  ...rest
}: ScreenProps) {
  const W = 160 * scale;
  const H = 144 * scale;
  return (
    <div
      className="rubc-screen"
      style={{
        display: "inline-flex",
        flexDirection: "column",
        gap: "8px",
        ...style,
      }}
      {...rest}
    >
      <div
        style={{
          width: W + "px",
          height: H + "px",
          background: "var(--screen)",
          border: "var(--border-screen-width) solid var(--border-screen)",
          borderRadius: "var(--radius-screen)",
          boxShadow: glow ? "var(--glow-screen)" : "var(--shadow)",
          overflow: "hidden",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          lineHeight: 0,
        }}
      >
        {src ? (
          <img
            src={src}
            alt={alt}
            style={{
              width: "100%",
              height: "100%",
              imageRendering: "pixelated",
              display: "block",
            }}
          />
        ) : (
          children
        )}
      </div>
      {status && (
        <div
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: "12px",
            color: "var(--text-screen)",
            display: "flex",
            alignItems: "center",
            gap: "6px",
          }}
        >
          <span
            aria-hidden={true}
            style={{
              color: "var(--dmg-light)",
            }}
          >
            ●
          </span>
          {status}
        </div>
      )}
    </div>
  );
}
