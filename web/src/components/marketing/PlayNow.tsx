"use client";

import React, { useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui";

// Minimal type for the (non-standard) beforeinstallprompt event (Chrome/Android).
interface BeforeInstallPromptEvent extends Event {
  prompt: () => Promise<void>;
  userChoice: Promise<{ outcome: "accepted" | "dismissed" }>;
}

function isMobile(): boolean {
  try {
    const ua = navigator.userAgent || "";
    return (
      /Android|iPhone|iPod|Mobile/i.test(ua) ||
      (typeof window !== "undefined" &&
        window.matchMedia &&
        window.matchMedia("(max-width: 820px)").matches)
    );
  } catch {
    return false;
  }
}

// iOS has NO programmatic install API on ANY browser (Safari, Chrome, Vivaldi,
// Firefox on iOS are all WebKit and use the same Share -> Add to Home Screen
// flow). So we detect iOS broadly, not Safari-only.
function isIos(): boolean {
  try {
    const ua = navigator.userAgent || "";
    return (
      /iPad|iPhone|iPod/.test(ua) ||
      // iPadOS 13+ reports as Mac; disambiguate via touch points.
      (navigator.platform === "MacIntel" && navigator.maxTouchPoints > 1)
    );
  } catch {
    return false;
  }
}
function isStandalone(): boolean {
  try {
    return (
      window.matchMedia("(display-mode: standalone)").matches ||
      // iOS Safari
      (window.navigator as unknown as { standalone?: boolean }).standalone === true
    );
  } catch {
    return false;
  }
}

export function playHref() {
  return isMobile() ? "/play/mobile" : "/play/desktop";
}

export function PlayNow({
  size = "md",
  children,
}: {
  size?: "sm" | "md" | "lg";
  children?: React.ReactNode;
}) {
  const installEvent = useRef<BeforeInstallPromptEvent | null>(null);
  const [href, setHref] = useState("/play/mobile");
  const [iosSheet, setIosSheet] = useState(false);

  useEffect(() => {
    // Resolve the real destination on mount (the static export prerenders one).
    setHref(playHref());
    const onBip = (e: Event) => {
      e.preventDefault();
      installEvent.current = e as BeforeInstallPromptEvent;
    };
    window.addEventListener("beforeinstallprompt", onBip);
    return () => window.removeEventListener("beforeinstallprompt", onBip);
  }, []);

  // Render a REAL <a href> -- identical to the footer 'Mobile PWA' link that works
  // reliably on iOS. We only intercept the click for the two non-navigation cases
  // (Android native install prompt, iOS Add-to-Home-Screen modal); otherwise we let
  // the browser follow the anchor natively (no JS navigation that iOS can drop).
  const onClick = (e: React.MouseEvent<HTMLAnchorElement>) => {
    // Already installed -> let the anchor navigate to /play/mobile natively.
    if (isStandalone()) return;

    // Android/Chrome with a captured prompt -> intercept and show native install.
    if (isMobile() && installEvent.current) {
      e.preventDefault();
      const ev = installEvent.current;
      void (async () => {
        try {
          await ev.prompt();
          await ev.userChoice;
        } catch {
          /* ignore */
        }
        installEvent.current = null;
      })();
      return;
    }

    // iOS (any browser) -> intercept and show the Add-to-Home-Screen modal.
    if (isMobile() && isIos()) {
      e.preventDefault();
      setIosSheet(true);
      return;
    }

    // Everything else (desktop, mobile-no-install) -> native anchor navigation.
  };

  return (
    <>
      <a href={href} onClick={onClick} style={{ textDecoration: "none", display: "inline-flex" }}>
        <Button variant="primary" size={size}>
          {children || "Play Now ▸"}
        </Button>
      </a>
      {iosSheet && <IosInstallSheet onClose={() => setIosSheet(false)} />}
    </>
  );
}

function IosInstallSheet({ onClose }: { onClose: () => void }) {
  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Add rubc to your Home Screen"
      onClick={onClose}
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 200,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        padding: 20,
        background: "color-mix(in srgb, var(--bg-deep) 78%, transparent)",
        cursor: "pointer",
      }}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          width: "100%",
          maxWidth: 360,
          background: "var(--surface-raised)",
          border: "2px solid var(--accent)",
          borderRadius: "var(--radius-lg)",
          padding: "24px 22px",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          gap: 16,
          textAlign: "center",
          cursor: "auto",
        }}
      >
        <div
          aria-hidden
          style={{
            fontSize: 44,
            lineHeight: 1,
            color: "var(--accent)",
          }}
        >
          ↑
        </div>
        <div
          style={{
            fontFamily: "var(--font-pixel)",
            fontSize: 20,
            color: "var(--text-strong)",
            lineHeight: 1.3,
          }}
        >
          Add to Home Screen to install as PWA
        </div>
        <div
          style={{
            fontFamily: "var(--font-sans)",
            fontSize: 14,
            lineHeight: 1.6,
            color: "var(--text-muted)",
          }}
        >
          Tap the <strong>Share</strong> button in Safari, then choose{" "}
          <strong>Add to Home Screen</strong>.
        </div>
        <div style={{ display: "flex", gap: 10, width: "100%" }}>
          <Button variant="ghost" size="md" block onClick={onClose}>
            Close
          </Button>
          <Button
            variant="secondary"
            size="md"
            block
            onClick={() => window.location.assign("/play/mobile")}
          >
            Play in browser
          </Button>
        </div>
      </div>
    </div>
  );
}
