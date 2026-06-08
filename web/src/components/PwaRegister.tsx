"use client";

import { useEffect } from "react";

export function PwaRegister() {
  useEffect(() => {
    // Only run the service worker in production. In dev it would serve stale
    // cached chunks against the live HMR server and cause hydration mismatches.
    if (process.env.NODE_ENV !== "production") {
      if ("serviceWorker" in navigator) {
        navigator.serviceWorker.getRegistrations().then((regs) => {
          regs.forEach((r) => r.unregister());
        });
      }
      return;
    }
    if ("serviceWorker" in navigator) {
      navigator.serviceWorker.register("/sw.js").catch(console.error);
    }
  }, []);
  return null;
}
