// Keeps a copy of the app shell in this browser, so the app's files never
// wait on the host and Matty still opens when the host can't be reached.
// The page itself comes from the host first, so a reload always shows the
// latest deploy; the kept copy is used only when the host doesn't answer
// within WAIT. A deploy's files are downloaded once, then kept.
//
// Only same-origin GETs pass through here; Mattermost servers never do.
const CACHE = "matty-shell-v1";
const SHELL = new URL("./", self.location).href;
const SHELL_PATH = new URL(SHELL).pathname;
const APP_FILE = /matty-web[^"'\/]*\.(?:js|wasm)/g;
const WAIT = 1500;

self.addEventListener("install", (event) => {
  self.skipWaiting();
  // The page that registered this already has its files, so this costs the
  // browser's own cache, not a download.
  event.waitUntil(
    caches
      .open(CACHE)
      .then(async (cache) => {
        const page = await latest();
        if (page) await keepShell(cache, page);
      })
      .catch(() => {}),
  );
});
self.addEventListener("activate", (event) => event.waitUntil(self.clients.claim()));

self.addEventListener("fetch", (event) => {
  const { request } = event;
  if (request.method !== "GET" || !request.url.startsWith(SHELL)) return;
  const path = new URL(request.url).pathname;
  if (request.mode === "navigate" && (path === SHELL_PATH || path === SHELL_PATH + "index.html")) {
    event.respondWith(shell(event));
  } else if (/\/matty-web[^/]*\.(?:js|wasm)$/.test(path)) {
    event.respondWith(appFile(request.url));
  }
});

// The host's page when it answers within WAIT, else the kept one; the kept
// copy then moves to whatever the host answers.
async function shell(event) {
  const cache = await caches.open(CACHE);
  const fresh = latest();
  event.waitUntil(fresh.then((page) => page && keepShell(cache, page.clone())).catch(() => {}));
  const kept = await cache.match(SHELL);
  if (!kept) return (await fresh)?.clone() || fetch(event.request);
  const page = await Promise.race([fresh, new Promise((done) => setTimeout(done, WAIT, null))]);
  return page ? page.clone() : kept;
}

// The host's page, or null when it can't be had.
function latest() {
  return fetch(SHELL, { cache: "no-cache" })
    .then((response) => (response.ok && !response.redirected ? response : null))
    .catch(() => null);
}

// Keeps a page once the files it names are kept, so a kept page never names
// a file that isn't; then drops whatever no page names any more.
async function keepShell(cache, page) {
  const html = await page.clone().text();
  const files = [...new Set(html.match(APP_FILE) || [])].map((f) => new URL(f, SHELL).href);
  await Promise.all(files.map((file) => keepFile(cache, file)));
  await cache.put(SHELL, page);
  for (const old of await cache.keys()) {
    if (old.url !== SHELL && !files.includes(old.url)) await cache.delete(old);
  }
}

// One download per file, shared by the page loading it and keepShell.
const downloads = new Map();
function keepFile(cache, url) {
  if (!downloads.has(url)) {
    const download = (async () => {
      if (await cache.match(url)) return;
      const response = await fetch(url);
      if (!response.ok) throw new Error(`${url}: ${response.status}`);
      await cache.put(url, response);
    })().finally(() => downloads.delete(url));
    downloads.set(url, download);
  }
  return downloads.get(url);
}

// The app's files are named by their content, so a kept one never changes.
async function appFile(url) {
  const cache = await caches.open(CACHE);
  try {
    await keepFile(cache, url);
    const kept = await cache.match(url);
    if (kept) return kept;
  } catch {}
  return fetch(url);
}
