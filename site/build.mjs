#!/usr/bin/env node
// Builds the Matty website into site/dist (site/README.md).
//
//   node site/build.mjs [--assets <karyalabs/assets checkout>] [--releases <file>]
//
// Copies public/, fills in the newest release (version, date, sizes, and the
// file addresses /download/<kind> redirects to), and fetches the screenshots
// from karyalabs/assets at ASSETS_REF, or from a local checkout.
import { cp, mkdir, readdir, readFile, rm, writeFile } from "node:fs/promises";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const dist = join(here, "dist");
const REPO = "karyalabs/matty";
// The karyalabs/assets commit the screenshots come from. Move it when the
// images under matty/site/ change.
const ASSETS_REF = "main";

// Which release file each /download/<kind> serves.
const KINDS = {
  mac: (n) => n.endsWith(".dmg"),
  "mac-zip": (n) => n.endsWith("-macos-universal.zip"),
  windows: (n) => n.endsWith(".msi"),
  "windows-zip": (n) => n.endsWith("-windows-x86_64.zip"),
  deb: (n) => n.endsWith("_amd64.deb"),
  rpm: (n) => n.endsWith(".x86_64.rpm"),
  appimage: (n) => n.endsWith(".AppImage"),
  "linux-tar": (n) => n.endsWith("-linux-x86_64.tar.gz"),
  checksums: (n) => n === "SHA256SUMS",
};

const args = process.argv.slice(2);
const flag = (name) => (args.includes(name) ? args[args.indexOf(name) + 1] : null);
// A local copy of the releases API answer, for building without network:
// gh api 'repos/karyalabs/matty/releases?per_page=10' > releases.json
const localReleases = flag("--releases");
const localAssets = flag("--assets");

async function latestRelease() {
  const list = localReleases ? JSON.parse(await readFile(localReleases, "utf8")) : await fetchReleases();
  const release = list.find((r) => !r.draft);
  if (!release) throw new Error("no published release");
  const files = {};
  for (const [kind, test] of Object.entries(KINDS)) {
    const asset = release.assets.find((a) => test(a.name));
    if (!asset) throw new Error(`${release.tag_name} has no file for ${kind}`);
    files[kind] = { name: asset.name, url: asset.browser_download_url, size: asset.size };
  }
  return {
    tag: release.tag_name,
    version: release.tag_name.replace(/^v/, ""),
    published: release.published_at,
    files,
  };
}

async function fetchReleases() {
  const headers = { Accept: "application/vnd.github+json", "User-Agent": "matty-site-build" };
  if (process.env.GITHUB_TOKEN) headers.Authorization = `Bearer ${process.env.GITHUB_TOKEN}`;
  const res = await fetch(`https://api.github.com/repos/${REPO}/releases?per_page=10`, { headers });
  if (!res.ok) throw new Error(`GitHub releases: ${res.status} ${await res.text()}`);
  return res.json();
}

async function image(name) {
  if (localAssets) return readFile(join(localAssets, "matty/site", name));
  const url = `https://raw.githubusercontent.com/karyalabs/assets/${ASSETS_REF}/matty/site/${name}`;
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${url}: ${res.status}`);
  return Buffer.from(await res.arrayBuffer());
}

const mb = (bytes) => `${Math.round(bytes / 1e6)} MB`;
const date = (iso) =>
  new Date(iso).toLocaleDateString("en-GB", { day: "numeric", month: "long", year: "numeric", timeZone: "UTC" });

await rm(dist, { recursive: true, force: true });
await cp(join(here, "public"), dist, { recursive: true });

const release = await latestRelease();
await writeFile(join(dist, "release.json"), JSON.stringify(release, null, 2) + "\n");

const lockup = (await readFile(join(here, "public/karya-lockup.svg"), "utf8"))
  .replace(/^<svg /, '<svg role="img" aria-hidden="true" ');
const values = {
  VERSION: release.version,
  TAG: release.tag,
  DATE: date(release.published),
  BUILD: release.version + "-" + Date.now().toString(36),
  KARYA_LOCKUP: lockup.trim(),
};
for (const [kind, file] of Object.entries(release.files)) values[`SIZE_${kind}`] = mb(file.size);

const images = new Set(["og.png"]);
for (const name of await readdir(dist)) {
  if (!name.endsWith(".html")) continue;
  let html = await readFile(join(dist, name), "utf8");
  html = html.replace(/\{\{(\w+)\}\}/g, (all, key) => {
    if (!(key in values)) throw new Error(`${name}: nothing fills ${all}`);
    return values[key];
  });
  for (const m of html.matchAll(/\/img\/([\w.-]+\.(?:webp|png))/g)) images.add(m[1]);
  await writeFile(join(dist, name), html);
}

await mkdir(join(dist, "img"), { recursive: true });
for (const name of images) {
  const target = name === "og.png" ? join(dist, name) : join(dist, "img", name);
  await writeFile(target, await image(name));
}
console.log(`Built site/dist for ${release.tag} with ${images.size} images`);
