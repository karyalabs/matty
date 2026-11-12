# matty.karyalabs.dev

The Matty website: a static page with the download buttons, served by a
Cloudflare Worker on the `karyalabs.dev` zone.

| Path | What |
| --- | --- |
| `public/` | The pages, styles, script and fonts. `{{NAME}}` placeholders are filled by the build. |
| `build.mjs` | Writes `dist/`: copies `public/`, fills in the newest release, and fetches the screenshots. |
| `worker.js` | Answers what no file matches: `/download/<kind>`, `/api/downloads` and the 404 page, plus the download counter. |
| `wrangler.jsonc` | The Worker and its route. |

## Downloads and privacy

`/download/<kind>` redirects to that file of the newest release on GitHub
(`mac`, `mac-zip`, `windows`, `windows-zip`, `deb`, `rpm`, `appimage`,
`linux-tar`, `checksums`). The build writes the addresses into
`release.json`, so the Worker never calls GitHub at request time.

Each GET of `/download/<kind>` adds one to a row in the `DownloadCounter`
Durable Object (SQLite): day, kind, version, and whether the User-Agent
looked automated. Nothing else about the request is read or stored.
`/api/downloads` returns the rows, and `/stats` shows them next to GitHub's
per-file counters. GitHub's counters alone overstate real use: every file of
every release gets about one automated download soon after publishing.

The site has no cookies, no analytics and no third-party requests (`/stats`
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
cd site && npx wrangler@4 dev
```

Without `--releases` the build calls the GitHub API (with `GITHUB_TOKEN` when
set); without `--assets` it downloads the images.

## Deploy

`.github/workflows/site.yml` builds and deploys from master when `site/`
changes, after every release (so the version and links move), and by hand
(Actions → Site → Run workflow). It uses `CLOUDFLARE_API_TOKEN` and
`CLOUDFLARE_ACCOUNT_ID` from the `release` environment.

The route `matty.karyalabs.dev/*` needs a proxied DNS record for
`matty.karyalabs.dev` in the karyalabs.dev zone (for example `AAAA matty 100::`,
proxied), like `demo`.
