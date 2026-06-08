"use client";

import React, { useEffect, useState } from "react";
import { Button } from "@/components/ui";

export function playHref() {
  try {
    const ua = navigator.userAgent || "";
    const mobile =
      /Android|iPhone|iPod|Mobile/i.test(ua) ||
      (window.matchMedia && window.matchMedia("(max-width: 820px)").matches);
    return mobile ? "/play/mobile" : "/play/desktop";
  } catch (e) {
    return "/play/desktop";
  }
}

export function PlayNow({
  size = "md",
  children,
}: {
  size?: "sm" | "md" | "lg";
  children?: React.ReactNode;
}) {
  const [href, setHref] = useState("/play/desktop");

  useEffect(() => {
    setHref(playHref());
  }, []);

  // Resolve the device at CLICK time too, so the static-export-prerendered
  // href (always /play/desktop) never sends a real phone to the desktop player
  // if it taps before hydration finishes.
  const onClick = (e: React.MouseEvent<HTMLAnchorElement>) => {
    const target = playHref();
    if (target !== href) {
      e.preventDefault();
      window.location.href = target;
    }
  };

  return (
    <a href={href} onClick={onClick} style={{ textDecoration: "none" }}>
      <Button variant="primary" size={size}>
        {children || "Play Now ▸"}
      </Button>
    </a>
  );
}
