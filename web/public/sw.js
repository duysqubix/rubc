const CACHE_NAME = 'rubc-v4';
// Precache the app shell. Hash-named build assets (JS chunks, the wasm under
// /_next/static/media/) are cached on first fetch by the runtime handler below,
// so the app works offline after the first online visit.
const ASSETS = [
  '/',
  '/play/desktop',
  '/play/mobile',
  '/manifest.json',
  '/icon-192.png',
  '/icon-512.png'
];

self.addEventListener('install', (e) => {
  self.skipWaiting();
  e.waitUntil(
    caches.open(CACHE_NAME).then((cache) => cache.addAll(ASSETS))
  );
});

// On activate, drop old caches so a deploy never leaves a stale app shell
// pointing at hash-named chunks that no longer exist.
self.addEventListener('activate', (e) => {
  e.waitUntil(
    caches.keys().then((keys) =>
      Promise.all(keys.filter((k) => k !== CACHE_NAME).map((k) => caches.delete(k)))
    ).then(() => self.clients.claim())
  );
});

self.addEventListener('fetch', (e) => {
  const req = e.request;
  if (req.method !== 'GET') return;
  const url = new URL(req.url);
  if (url.origin !== self.location.origin) return;

  // Navigations (the HTML shell): network-first, so a new deploy is picked up
  // immediately and we never serve an index.html that references stale chunks.
  // Fall back to the cached shell only when offline.
  if (req.mode === 'navigate') {
    e.respondWith(
      fetch(req)
        .then((res) => {
          caches.open(CACHE_NAME).then((c) => c.put('/', res.clone()));
          return res;
        })
        .catch(() => caches.match('/').then((r) => r || caches.match(req)))
    );
    return;
  }

  // Hash-named immutable assets (and everything else): cache-first, populate on
  // first fetch. Safe because these URLs are content-hashed per build.
  e.respondWith(
    caches.match(req).then((cached) => {
      return (
        cached ||
        fetch(req).then((res) => {
          const copy = res.clone();
          caches.open(CACHE_NAME).then((c) => c.put(req, copy));
          return res;
        })
      );
    })
  );
});
