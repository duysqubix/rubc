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
      const keys = await caches.keys();
      await Promise.all(keys.map((k) => caches.delete(k)));
      await self.registration.unregister();
      const clients = await self.clients.matchAll({ type: 'window' });
      clients.forEach((c) => c.navigate(c.url));
    })()
  );
});

// Pass everything straight through to the network — never serve from cache.
self.addEventListener('fetch', (e) => {
  e.respondWith(fetch(e.request));
});
