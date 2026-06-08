import React from "react";
import { Button, Badge, Card, StatusPill, Screen } from "@/components/ui";
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
      style={{
        position: "sticky",
        top: 0,
        zIndex: 20,
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        padding: "16px 48px",
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
        <div style={{ display: "flex", gap: 26 }}>
          <a href="#accuracy" style={linkStyle}>
            Accuracy
          </a>
          <a href="#features" style={linkStyle}>
            Features
          </a>
          <a href="#run" style={linkStyle}>
            Run it
          </a>
          <a
            href="https://github.com/duysqubix/rubc"
            target="_blank"
            rel="noreferrer"
            style={linkStyle}
          >
            GitHub ↗
          </a>
        </div>
      </div>
      <PlayNow size="sm" />
    </nav>
  );
}

function Hero() {
  return (
    <section
      style={{
        display: "grid",
        gridTemplateColumns: "1fr 520px",
        gap: 56,
        alignItems: "center",
        padding: "72px 48px 64px",
      }}
    >
      <div>
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
            fontSize: 60,
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
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          gap: 14,
        }}
      >
        <Screen
          src="/crystal-intro.gif"
          scale={3}
          status="Pokémon Crystal — CGB mode · 59.7275 Hz · save persisted."
        />
        <div
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: 11,
            color: "var(--text-faint)",
            letterSpacing: "0.04em",
          }}
        >
          ▸ live WebAssembly build · your ROM never leaves your machine
        </div>
      </div>
    </section>
  );
}

function Stat({ n, l }: { n: string; l: string }) {
  return (
    <div>
      <div
        style={{
          fontFamily: "var(--font-pixel)",
          fontSize: 30,
          color: "var(--dmg-light)",
        }}
      >
        {n}
      </div>
      <div
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: 11,
          color: "var(--text-faint)",
          marginTop: 4,
          letterSpacing: "0.03em",
        }}
      >
        {l}
      </div>
    </div>
  );
}

function Accuracy() {
  const pills = [
    { status: "pass" as const, label: "dmg-acid2", detail: "pixel-exact" },
    { status: "pass" as const, label: "cgb-acid2", detail: "pixel-exact" },
    { status: "pass" as const, label: "cgb-acid-hell", detail: "pixel-exact" },
    { status: "pass" as const, label: "Blargg cpu_instrs", detail: "11/11" },
    { status: "pass" as const, label: "Blargg dmg_sound", detail: "12/12" },
    { status: "pass" as const, label: "Mooneye", detail: "93/115" },
    { status: "wip" as const, label: "mealybug-tearoom", detail: "in progress" },
  ];
  return (
    <section
      id="accuracy"
      style={{
        padding: "64px 48px",
        background: "var(--surface-sunken)",
        borderTop: "1px solid var(--border)",
        borderBottom: "1px solid var(--border)",
      }}
    >
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "1fr 1.1fr",
          gap: 56,
          alignItems: "center",
        }}
      >
        <div>
          <div
            style={{
              fontFamily: "var(--font-mono)",
              fontSize: 12,
              color: "var(--accent)",
              letterSpacing: "0.12em",
              textTransform: "uppercase",
              marginBottom: 14,
            }}
          >
            // the headline feature
          </div>
          <h2
            style={{
              fontFamily: "var(--font-pixel)",
              fontSize: 38,
              color: "var(--text-strong)",
              margin: "0 0 18px",
              lineHeight: 1.1,
            }}
          >
            We don't claim<br />accuracy. We prove it.
          </h2>
          <p
            style={{
              fontFamily: "var(--font-sans)",
              fontSize: 16,
              lineHeight: 1.65,
              color: "var(--text-muted)",
              maxWidth: 440,
            }}
          >
            Every build runs the industry-standard hardware test ROMs — the same
            suites used to validate real silicon. Results are reported as exact
            figures, never rounded. The numbers{" "}
            <em style={{ color: "var(--text)", fontStyle: "normal" }}>are</em> the
            brand.
          </p>
          <div style={{ display: "flex", gap: 28, marginTop: 28 }}>
            <Stat n="0/23040" l="pixel diffs vs. SameBoy" />
            <Stat n="59.7275" l="Hz refresh, to the dot" />
          </div>
        </div>
        <div style={{ display: "flex", flexWrap: "wrap", gap: 10 }}>
          {pills.map((p, i) => (
            <StatusPill
              key={i}
              status={p.status}
              label={p.label}
              detail={p.detail}
            />
          ))}
        </div>
      </div>
    </section>
  );
}

function SectionHead({ eyebrow, title }: { eyebrow: string; title: string }) {
  return (
    <div style={{ textAlign: "center" }}>
      <div
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: 12,
          color: "var(--accent)",
          letterSpacing: "0.12em",
          textTransform: "uppercase",
          marginBottom: 12,
        }}
      >
        {eyebrow}
      </div>
      <h2
        style={{
          fontFamily: "var(--font-pixel)",
          fontSize: 36,
          color: "var(--text-strong)",
          margin: 0,
          lineHeight: 1.1,
        }}
      >
        {title}
      </h2>
    </div>
  );
}

function Features() {
  const items = [
    {
      eyebrow: "// memory-safe",
      title: "100% safe Rust",
      body: "The emulation core is #![forbid(unsafe_code)] — no C bindings, no FFI, no unsafe blocks.",
    },
    {
      eyebrow: "// rendering",
      title: "Pixel-exact PPU",
      body: "Dot-based PPU passes dmg-acid2, cgb-acid2 and the brutal cgb-acid-hell torture test.",
    },
    {
      eyebrow: "// audio",
      title: "Real APU sound",
      body: "All four channels emulated with correct timing — Blargg dmg_sound 12/12.",
    },
    {
      eyebrow: "// persistence",
      title: "Battery saves + RTC",
      body: "MBC1/2/3/5 with battery-backed RAM and a real-time clock. Your save files just work.",
    },
    {
      eyebrow: "// portable",
      title: "Runs everywhere",
      body: "Native desktop, in the browser via WebAssembly, or Docker + nginx. Same core, three surfaces.",
    },
    {
      eyebrow: "// lean",
      title: "Dependency-light",
      body: "A small, fast binary with a minimal dependency tree. Boots instantly, sips memory.",
    },
  ];
  return (
    <section id="features" style={{ padding: "72px 48px" }}>
      <SectionHead
        eyebrow="// what's inside"
        title="Built to get the hardware right."
      />
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(3, 1fr)",
          gap: 18,
          marginTop: 36,
        }}
      >
        {items.map((it, i) => (
          <Card key={i} eyebrow={it.eyebrow} title={it.title} accent={i === 0}>
            <p
              style={{
                fontFamily: "var(--font-sans)",
                fontSize: 14,
                lineHeight: 1.6,
                color: "var(--text-muted)",
                margin: 0,
              }}
            >
              {it.body}
            </p>
          </Card>
        ))}
      </div>
    </section>
  );
}

function RunIt() {
  const ways = [
    {
      tag: "browser",
      title: "In your browser",
      body: "WebAssembly build. The ROM never leaves your machine.",
      cmd: "▸ just press Play Now",
      primary: true,
    },
    {
      tag: "desktop",
      title: "Native desktop",
      body: "A single fast Rust binary for macOS, Linux & Windows.",
      cmd: "$ cargo install rubc",
    },
    {
      tag: "docker",
      title: "Docker + nginx",
      body: "Self-host the player anywhere in one command.",
      cmd: "$ docker run -p 8080:80 rubc",
    },
  ];
  return (
    <section
      id="run"
      style={{
        padding: "64px 48px",
        background: "var(--surface-sunken)",
        borderTop: "1px solid var(--border)",
        borderBottom: "1px solid var(--border)",
      }}
    >
      <SectionHead eyebrow="// three ways to run" title="Pick your surface." />
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(3, 1fr)",
          gap: 18,
          marginTop: 36,
        }}
      >
        {ways.map((w, i) => (
          <div
            key={i}
            style={{
              background: "var(--surface)",
              border: `1px solid ${
                w.primary ? "var(--accent)" : "var(--border)"
              }`,
              borderRadius: "var(--radius-md)",
              padding: 22,
              display: "flex",
              flexDirection: "column",
              gap: 12,
              boxShadow: "var(--shadow)",
            }}
          >
            <Badge tone={w.primary ? "rust" : "neutral"}>
              {w.tag}
            </Badge>
            <div
              style={{
                fontFamily: "var(--font-sans)",
                fontSize: 18,
                fontWeight: 600,
                color: "var(--text-strong)",
              }}
            >
              {w.title}
            </div>
            <p
              style={{
                fontFamily: "var(--font-sans)",
                fontSize: 14,
                lineHeight: 1.55,
                color: "var(--text-muted)",
                margin: 0,
                flex: 1,
              }}
            >
              {w.body}
            </p>
            <code
              style={{
                fontFamily: "var(--font-mono)",
                fontSize: 12.5,
                color: w.primary ? "var(--accent)" : "var(--dmg-light)",
                background: "var(--bg-deep)",
                border: "1px solid var(--border)",
                borderRadius: "var(--radius-sm)",
                padding: "9px 11px",
              }}
            >
              {w.cmd}
            </code>
          </div>
        ))}
      </div>
    </section>
  );
}

function Showcase() {
  const shots = [
    { src: "/crystal-title.png", label: "Pokémon Crystal", tag: "CGB" as const },
    { src: "/dmg-acid2.png", label: "dmg-acid2 · pixel-exact", tag: "DMG" as const },
    { src: "/cgb-acid2.png", label: "cgb-acid2 · pixel-exact", tag: "CGB" as const },
  ];
  return (
    <section style={{ padding: "72px 48px" }}>
      <SectionHead
        eyebrow="// nearest-neighbour, never filtered"
        title="Every pixel where it belongs."
      />
      <div
        style={{
          display: "flex",
          gap: 28,
          justifyContent: "center",
          marginTop: 40,
          flexWrap: "wrap",
        }}
      >
        {shots.map((s, i) => (
          <div
            key={i}
            style={{
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              gap: 12,
            }}
          >
            <Screen src={s.src} scale={2} glow={false} />
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <Badge tone={s.tag === "CGB" ? "cgb" : "dmg"}>
                {s.tag}
              </Badge>
              <span
                style={{
                  fontFamily: "var(--font-mono)",
                  fontSize: 12,
                  color: "var(--text-muted)",
                }}
              >
                {s.label}
              </span>
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

function CTA() {
  return (
    <section
      style={{
        padding: "80px 48px",
        textAlign: "center",
        background:
          "radial-gradient(120% 120% at 50% 0%, color-mix(in srgb, var(--accent) 10%, var(--bg)), var(--bg) 60%)",
      }}
    >
      <h2
        style={{
          fontFamily: "var(--font-pixel)",
          fontSize: 44,
          color: "var(--text-strong)",
          margin: "0 0 16px",
        }}
      >
        Play it now. No install.
      </h2>
      <p
        style={{
          fontFamily: "var(--font-sans)",
          fontSize: 17,
          color: "var(--text-muted)",
          margin: "0 auto 30px",
          maxWidth: 460,
          lineHeight: 1.6,
        }}
      >
        Drop in a{" "}
        <span style={{ fontFamily: "var(--font-mono)", color: "var(--text)" }}>
          .gb
        </span>{" "}
        /{" "}
        <span style={{ fontFamily: "var(--font-mono)", color: "var(--text)" }}>
          .gbc
        </span>{" "}
        file and you're playing in under a second — entirely in your browser.
      </p>
      <div style={{ display: "flex", gap: 12, justifyContent: "center" }}>
        <PlayNow size="lg">▸ Play Now</PlayNow>
        <a href="rubc Mobile.html" style={{ textDecoration: "none" }}>
          <Button variant="secondary" size="lg">
            On mobile ▸
          </Button>
        </a>
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
      style={{
        padding: "48px 48px 40px",
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
            { t: "Browser player", href: "rubc Desktop.html" },
            { t: "Mobile PWA", href: "rubc Mobile.html" },
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
      <Accuracy />
      <Features />
      <RunIt />
      <Showcase />
      <CTA />
      <Footer />
    </div>
  );
}
