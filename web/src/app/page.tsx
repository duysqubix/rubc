import React from "react";
import { Button, Badge } from "@/components/ui";
import { PlayNow } from "@/components/marketing/PlayNow";

function Nav() {
  const linkStyle: React.CSSProperties = {
    fontFamily: "var(--font-mono)",
    fontSize: 13,
    color: "var(--text-muted)",
    textDecoration: "none",
    letterSpacing: "0.02em",
  };
  return (
    <nav
      className="rubc-nav"
      style={{
        position: "sticky",
        top: 0,
        zIndex: 20,
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        borderBottom: "1px solid var(--border)",
        background: "color-mix(in srgb, var(--bg) 88%, transparent)",
        backdropFilter: "blur(8px)",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 36 }}>
        <img
          src="/logo.png"
          alt="rubc"
          style={{ height: 30, imageRendering: "pixelated" }}
        />
        <a
          href="https://github.com/duysqubix/rubc"
          target="_blank"
          rel="noreferrer"
          style={linkStyle}
        >
          GitHub ↗
        </a>
      </div>
      <PlayNow size="sm" />
    </nav>
  );
}

function Hero() {
  return (
    <section className="rubc-hero">
      <div>
        <img
          src="/logo.png"
          alt="rubc"
          style={{ height: 120, width: "auto", imageRendering: "pixelated", marginBottom: 28 }}
        />
        <div
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: 13,
            color: "var(--accent)",
            letterSpacing: "0.14em",
            textTransform: "uppercase",
            marginBottom: 20,
          }}
        >
          // cycle-accurate · verified on real silicon
        </div>
        <h1
          style={{
            fontFamily: "var(--font-pixel)",
            lineHeight: 1.05,
            color: "var(--text-strong)",
            margin: 0,
            letterSpacing: "0.01em",
            textWrap: "balance",
          }}
        >
          Game Boy,<br />exactly as the<br />
          <span style={{ color: "var(--accent)" }}>silicon</span> meant it.
        </h1>
        <p
          style={{
            fontFamily: "var(--font-sans)",
            fontSize: 18,
            lineHeight: 1.6,
            color: "var(--text-muted)",
            maxWidth: 480,
            margin: "24px 0 32px",
          }}
        >
          rubc is a cycle-accurate Game Boy &amp; Game Boy Color emulator written
          in 100% safe Rust. Pixel-exact rendering, real sound, battery saves —
          cross-checked dot-for-dot against the hardware test ROMs.
        </p>
        <div
          style={{
            display: "flex",
            gap: 14,
            alignItems: "center",
            marginBottom: 28,
          }}
        >
          <PlayNow size="lg" />
          <a
            href="https://github.com/duysqubix/rubc"
            target="_blank"
            rel="noreferrer"
            style={{ textDecoration: "none" }}
          >
            <Button variant="ghost" size="lg">
              ★ Star on GitHub
            </Button>
          </a>
        </div>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
          <Badge tone="dmg">DMG</Badge>
          <Badge tone="cgb">CGB</Badge>
          <Badge tone="neutral">WebAssembly</Badge>
          <Badge tone="rust" variant="solid">
            safe Rust
          </Badge>
          <Badge tone="pass">0 unsafe</Badge>
        </div>
      </div>
    </section>
  );
}


function Footer() {
  const col = (title: string, links: { t: string; href: string }[]) => (
    <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
      <div
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: 11,
          color: "var(--text-faint)",
          letterSpacing: "0.12em",
          textTransform: "uppercase",
        }}
      >
        {title}
      </div>
      {links.map((l, i) => (
        <a
          key={i}
          href={l.href}
          target="_blank"
          rel="noreferrer"
          style={{
            fontFamily: "var(--font-sans)",
            fontSize: 13.5,
            color: "var(--text-muted)",
            textDecoration: "none",
          }}
        >
          {l.t}
        </a>
      ))}
    </div>
  );
  return (
    <footer
      className="rubc-section"
      style={{
        borderTop: "1px solid var(--border)",
        background: "var(--surface-sunken)",
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", gap: 40 }}>
        <div style={{ maxWidth: 300 }}>
          <img
            src="/logo.png"
            alt="rubc"
            style={{ height: 30, imageRendering: "pixelated", marginBottom: 14 }}
          />
          <p
            style={{
              fontFamily: "var(--font-sans)",
              fontSize: 13,
              color: "var(--text-faint)",
              lineHeight: 1.6,
              margin: 0,
            }}
          >
            A cycle-accurate Game Boy / Game Boy Color emulator in safe Rust.{" "}
            <span style={{ fontFamily: "var(--font-mono)" }}>rubc</span> is always
            lowercase.
          </p>
        </div>
        <div style={{ display: "flex", gap: 64 }}>
          {col("Project", [
            { t: "GitHub", href: "https://github.com/duysqubix/rubc" },
            { t: "USAGE.md", href: "https://github.com/duysqubix/rubc" },
            { t: "ACCURACY.md", href: "https://github.com/duysqubix/rubc" },
          ])}
          {col("Play", [
            { t: "Browser player", href: "/play/desktop" },
            { t: "Mobile PWA", href: "/play/mobile" },
          ])}
        </div>
      </div>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          marginTop: 36,
          paddingTop: 20,
          borderTop: "1px solid var(--border)",
        }}
      >
        <span
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: 11.5,
            color: "var(--text-faint)",
          }}
        >
          MIT licensed · built with safe Rust 🦀
        </span>
        <span
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: 11.5,
            color: "var(--text-faint)",
          }}
        >
          #![forbid(unsafe_code)]
        </span>
      </div>
    </footer>
  );
}

export default function Homepage() {
  return (
    <div
      style={{
        background: "var(--bg)",
        color: "var(--text)",
        fontFamily: "var(--font-sans)",
        minHeight: "100%",
      }}
    >
      <Nav />
      <Hero />
      <Footer />
    </div>
  );
}
