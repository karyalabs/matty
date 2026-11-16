# karyalabs.dev/matty

The Matty website: a static page with the download buttons, served by a
Cloudflare Worker on the `karyalabs.dev` zone at <https://karyalabs.dev/matty/>.
The web app is at `/matty/app/`, from its own Worker (`crates/matty-web`,
[docs/web.md](../docs/web.md)); that route is more specific, so it wins. The
old address, `matty.karyalabs.dev`, redirects every path to the same one under
`karyalabs.dev/matty`.

| Path | What |
| --- | --- |
| `public/` | The pages, styles, script and fonts. `{{NAME}}` placeholders are filled by the build. |
| `build.mjs` | Writes `dist/`: copies `public/` into `dist/matty/`, fills in the newest release, and fetches the screenshots. |
| `worker.js` | Answers what no file matches: `/matty/download/<kind>`, `/matty/api/downloads`, the 404 page and the old host's redirect, plus the download counter. |
| `wrangler.jsonc` | The Worker and its routes. |

Every link in `public/` is absolute under `/matty/`.

## Downloads and privacy

`/matty/download/<kind>` redirects to that file of the newest release on GitHub
(`mac`, `mac-zip`, `windows`, `windows-zip`, `deb`, `rpm`, `appimage`,
`linux-tar`, `checksums`). The build writes the addresses into
`release.json`, so the Worker never calls GitHub at request time.

Each GET of `/matty/download/<kind>` adds one to a row in the `DownloadCounter`
Durable Object (SQLite): day, kind, version, and whether the User-Agent
looked automated. Nothing else about the request is read or stored.
`/matty/api/downloads` returns the rows, and `/matty/stats` shows them next to GitHub's
per-file counters. GitHub's counters alone overstate real use: every file of
every release gets about one automated download soon after publishing.

The site has no cookies, no analytics and no third-party requests (`/matty/stats`
also reads the public GitHub releases API from the browser).

## Screenshots

Images live in [karyalabs/assets](https://github.com/karyalabs/assets) under
`matty/site/` (never in this repository): WebP at 1200 and 2400 px wide for
each feature, light and dark, plus `og.png` (1200 × 630). They come from the
preview fixtures, so every name and message in them is made up. To change
one, push the new file to assets and move `ASSETS_REF` in `build.mjs`.

## Build and run locally

```sh
gh api 'repos/karyalabs/matty/releases?per_page=10' > /tmp/releases.json
node site/build.mjs --releases /tmp/releases.json [--assets ../assets]
cd site && npx wrangler@4 dev   # http://localhost:8787/matty/
```

Without `--releases` the build calls the GitHub API (with `GITHUB_TOKEN` when
set); without `--assets` it downloads the images.

## Deploy

`.github/workflows/site.yml` builds and deploys from master when `site/`
changes, after every release (so the version and links move), and by hand
(Actions → Site → Run workflow). It uses `CLOUDFLARE_API_TOKEN` and
`CLOUDFLARE_ACCOUNT_ID` from the `release` environment.

The route `karyalabs.dev/matty*` uses the zone's proxied record for the bare
domain. Deploy the web app first when its routes change, since Cloudflare
gives a route to one Worker at a time. The old `matty.karyalabs.dev/*` route
keeps its proxied DNS record (`AAAA matty 100::`) so the redirect answers.
