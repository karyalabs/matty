// Retires the web app's service worker. Until the app moved to /matty/app/,
// karyalabs.dev/matty/ was the app, and browsers that opened it keep a
// service worker for /matty/ that serves its kept copy of the page. They
// check this address for a new version of it, find this one, and it removes
// itself and that copy, so /matty/ is this website again. The app at
// /matty/app/ registers its own, with its own cache.
self.addEventListener("install", () => self.skipWaiting());
self.addEventListener("activate", (event) => {
  event.waitUntil(
    (async () => {
      await caches.delete("matty-shell-v1");
      await self.registration.unregister();
      for (const client of await self.clients.matchAll({ type: "window" })) client.navigate(client.url);
    })(),
  );
});
