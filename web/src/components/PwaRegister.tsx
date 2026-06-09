"use client";

import { useEffect } from "react";

export function PwaRegister() {
  useEffect(() => {
    // Only run the service worker in production. In dev a leftover SW would serve
    // stale cached chunks against the live HMR server (hydration mismatches +
    // "my changes aren't showing"). In dev we actively self-heal: unregister any
    // service worker AND delete all caches on every load, so a browser that was
    // poisoned by an older build cleans itself without manual cache clearing.
    if (process.env.NODE_ENV !== "production") {
      if ("serviceWorker" in navigator) {
        navigator.serviceWorker.getRegistrations().then((regs) => {
          regs.forEach((r) => r.unregister());
        });
      }
      if (typeof caches !== "undefined") {
        caches.keys().then((keys) => {
          keys.forEach((k) => caches.delete(k));
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
