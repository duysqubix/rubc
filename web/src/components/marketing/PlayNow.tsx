"use client";

import React, { useEffect, useState } from "react";
import { Button } from "@/components/ui";

export function playHref() {
  try {
    const ua = navigator.userAgent || "";
    const mobile =
      /Android|iPhone|iPod|Mobile/i.test(ua) ||
      (window.matchMedia && window.matchMedia("(max-width: 820px)").matches);
    return mobile ? "rubc Mobile.html" : "rubc Desktop.html";
  } catch (e) {
    return "rubc Desktop.html";
  }
}

export function PlayNow({
  size = "md",
  children,
}: {
  size?: "sm" | "md" | "lg";
  children?: React.ReactNode;
}) {
  const [href, setHref] = useState("rubc Desktop.html");

  useEffect(() => {
    setHref(playHref());
  }, []);

  return (
    <a href={href} style={{ textDecoration: "none" }}>
      <Button variant="primary" size={size}>
        {children || "Play Now ▸"}
      </Button>
    </a>
  );
}
