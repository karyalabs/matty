// Serves the app shell from this browser's own copy, so a launch doesn't wait
// for a round trip to the host before the app can start: the page and the
// app's files come from the cache, and the page is checked in the background.
// A deploy is picked up on the launch after the one that saw it; its files are
// fetched then too, so that launch is as fast as any other.
//
// Only same-origin GETs pass through here; Mattermost servers never do.
const CACHE = "matty-shell-v1";
const SHELL = new URL("./", self.location).href;
const SHELL_PATH = new URL(SHELL).pathname;
const APP_FILE = /matty-web[^"'\/]*\.(?:js|wasm)/g;

self.addEventListener("install", (event) => {
  self.skipWaiting();
  // The page that registered this already has its files, so this costs the
  // browser's own cache, not a download.
  event.waitUntil(caches.open(CACHE).then(refresh).catch(() => {}));
});
self.addEventListener("activate", (event) => event.waitUntil(self.clients.claim()));

self.addEventListener("fetch", (event) => {
  const { request } = event;
  if (request.method !== "GET" || !request.url.startsWith(SHELL)) return;
  const path = new URL(request.url).pathname;
  if (request.mode === "navigate" && (path === SHELL_PATH || path === SHELL_PATH + "index.html")) {
    event.respondWith(shell(event));
  } else if (/\/matty-web[^/]*\.(?:js|wasm)$/.test(path)) {
    event.respondWith(appFile(request));
  }
});

// The cached page right away, refreshed for the next launch; the network
// while nothing is cached yet.
async function shell(event) {
  const cache = await caches.open(CACHE);
  const cached = await cache.match(SHELL);
  event.waitUntil(refresh(cache).catch(() => {}));
  return cached || fetch(event.request);
}

async function refresh(cache) {
  const response = await fetch(SHELL, { cache: "no-cache" });
  if (!response.ok || response.redirected) return;
  const html = await response.clone().text();
  const files = [...new Set(html.match(APP_FILE) || [])].map((f) => new URL(f, SHELL).href);
  // The new page's files first, then the page, so a cached page never names
  // a file that isn't cached; then whatever no page names any more.
  for (const file of files) {
    if (!(await cache.match(file))) await cache.add(file);
  }
  await cache.put(SHELL, response.clone());
  for (const old of await cache.keys()) {
    if (old.url !== SHELL && !files.includes(old.url)) await cache.delete(old);
  }
}

// The app's files are named by their content, so a cached one never changes.
async function appFile(request) {
  const cache = await caches.open(CACHE);
  const cached = await cache.match(request.url);
  if (cached) return cached;
  const response = await fetch(request);
  if (response.ok) await cache.put(request.url, response.clone());
  return response;
}
