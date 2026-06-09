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
  // A real <a> is the most reliable navigation primitive on iOS Safari (a JS-only
  // window.location call inside onClick can get dropped). The static export
  // prerenders one fixed href, so we correct it on mount to the device route, and
  // also re-resolve at click time as a belt-and-suspenders safety net.
  const [href, setHref] = useState("/play/mobile");

  useEffect(() => {
    setHref(playHref());
  }, []);

  const onClick = (e: React.MouseEvent<HTMLAnchorElement>) => {
    const target = playHref();
    if (target !== href) {
      e.preventDefault();
      window.location.assign(target);
    }
  };

  return (
    <a
      href={href}
      onClick={onClick}
      style={{ textDecoration: "none", display: "inline-flex" }}
    >
      <Button variant="primary" size={size}>
        {children || "Play Now ▸"}
      </Button>
    </a>
  );
}
