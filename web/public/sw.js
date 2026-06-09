// Self-destructing service worker.
//
// Earlier builds registered a caching SW that served stale JS chunks cache-first
// (so HTML updated but the app code did not). Browsers re-fetch this script on
// navigation/SW-update checks; this version unregisters itself and deletes every
// cache, breaking out of the stale-cache deadlock. The app only re-registers a
// SW in production (see PwaRegister), so in dev this stays gone.
self.addEventListener('install', () => {
  self.skipWaiting();
});

self.addEventListener('activate', (e) => {
  e.waitUntil(
    (async () => {
      // Clear caches + unregister SILENTLY. Do NOT call client.navigate(): in an
      // installed standalone PWA (no address bar) a forced reload here wedges the
      // app mid-teardown and leaves every button dead. The next normal navigation
      // loads cleanly with no SW.
      const keys = await caches.keys();
      await Promise.all(keys.map((k) => caches.delete(k)));
      await self.registration.unregister();
    })()
  );
});

// No fetch handler at all -> the browser goes straight to the network. (A
// passthrough fetch handler still routes every request through the SW, which is
// pointless overhead and risk; omitting it lets the SW truly get out of the way.)
