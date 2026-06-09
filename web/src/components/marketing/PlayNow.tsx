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
  // Navigate purely via JS at tap time. We deliberately do NOT render a real
  // <a href>: on a static export the href is prerendered to one fixed route, and
  // on iOS Safari that anchor navigation can win over an onClick redirect -- which
  // sent real iPhones to the desktop player. Resolving the device on tap fixes it.
  const go = () => {
    window.location.assign(playHref());
  };

  return (
    <Button variant="primary" size={size} onClick={go}>
      {children || "Play Now ▸"}
    </Button>
  );
}
