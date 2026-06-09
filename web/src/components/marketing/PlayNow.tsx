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

function isIos(): boolean {
  try {
    return /iPhone|iPad|iPod/i.test(navigator.userAgent || "");
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
      await installEvent.current.prompt();
      const choice = await installEvent.current.userChoice;
      installEvent.current = null;
      // Whether they install or dismiss, let them play either way.
      if (choice.outcome === "accepted") return;
      window.location.assign("/play/mobile");
      return;
    }

    // iOS Safari has no install API -> show Add-to-Home-Screen instructions.
    if (isIos()) {
      setIosSheet(true);
      return;
    }

    // Fallback (mobile, no install support detected): open the player.
    window.location.assign("/play/mobile");
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
      onClick={onClose}
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 200,
        display: "flex",
        alignItems: "flex-end",
        justifyContent: "center",
        background: "color-mix(in srgb, var(--bg-deep) 70%, transparent)",
        cursor: "pointer",
      }}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          width: "100%",
          maxWidth: 480,
          background: "var(--surface-raised)",
          borderTop: "2px solid var(--accent)",
          borderTopLeftRadius: "var(--radius-lg)",
          borderTopRightRadius: "var(--radius-lg)",
          padding: "20px 20px max(20px, env(safe-area-inset-bottom))",
          display: "flex",
          flexDirection: "column",
          gap: 14,
        }}
      >
        <div
          style={{
            fontFamily: "var(--font-pixel)",
            fontSize: 22,
            color: "var(--text-strong)",
          }}
        >
          install rubc
        </div>
        <ol
          style={{
            margin: 0,
            paddingLeft: 18,
            fontFamily: "var(--font-sans)",
            fontSize: 15,
            lineHeight: 1.7,
            color: "var(--text-muted)",
          }}
        >
          <li>
            Tap the <strong>Share</strong> button{" "}
            <span aria-hidden style={{ color: "var(--accent)" }}>
              ⬆
            </span>{" "}
            in Safari&apos;s toolbar.
          </li>
          <li>
            Choose <strong>Add to Home Screen</strong>.
          </li>
          <li>Open rubc from your home screen to play full-screen.</li>
        </ol>
        <div style={{ display: "flex", gap: 10 }}>
          <Button variant="ghost" size="md" onClick={onClose}>
            Close
          </Button>
          <Button
            variant="secondary"
            size="md"
            block
            onClick={() => window.location.assign("/play/mobile")}
          >
            Play in browser instead
          </Button>
        </div>
      </div>
    </div>
  );
}
