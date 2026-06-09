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
  const [iosSheet, setIosSheet] = useState(false);

  useEffect(() => {
    const onBip = (e: Event) => {
      e.preventDefault();
      installEvent.current = e as BeforeInstallPromptEvent;
    };
    window.addEventListener("beforeinstallprompt", onBip);
    return () => window.removeEventListener("beforeinstallprompt", onBip);
  }, []);

  const onClick = async (e: React.MouseEvent) => {
    e.preventDefault();

    // Desktop -> straight to the desktop player.
    if (!isMobile()) {
      window.location.assign("/play/desktop");
      return;
    }

    // Already installed / running standalone -> just open the mobile player.
    if (isStandalone()) {
      window.location.assign("/play/mobile");
      return;
    }

    // Android / Chrome: fire the native install prompt if we captured it.
    if (installEvent.current) {
      try {
        await installEvent.current.prompt();
        const choice = await installEvent.current.userChoice;
        installEvent.current = null;
        if (choice.outcome === "accepted") return;
      } catch {
        // fall through to the instruction sheet / player
      }
      window.location.assign("/play/mobile");
      return;
    }

    // iOS (any browser) has no install API -> show the Add-to-Home-Screen modal.
    if (isIos()) {
      setIosSheet(true);
      return;
    }

    // Other mobile with no install prompt available -> just open the player.
    setIosSheet(true);
  };

  return (
    <>
      <Button variant="primary" size={size} onClick={onClick}>
        {children || "Play Now ▸"}
      </Button>
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
