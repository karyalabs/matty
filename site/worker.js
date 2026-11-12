// Requests no file in dist/ matches (wrangler.jsonc). /download/<kind> sends
// you to that file of the newest release on GitHub; the build wrote their
// addresses into release.json. Each download adds one to a counter for that
// day, file kind, version and whether the request looked automated. Nothing
// about the person (address, browser, referrer) is read or kept.
import { DurableObject } from "cloudflare:workers";

// Link previews, crawlers and scripts. They still get the file; they are
// counted apart so the numbers show people.
const AUTOMATED = /bot|crawl|spider|slurp|preview|scan|fetch|curl|wget|python|go-http|java|okhttp|headless|httpclient|axios|node|ruby|perl|libwww/i;

export class DownloadCounter extends DurableObject {
  constructor(ctx, env) {
    super(ctx, env);
    ctx.blockConcurrencyWhile(async () => {
      this.ctx.storage.sql.exec(`CREATE TABLE IF NOT EXISTS downloads (
        day TEXT NOT NULL, kind TEXT NOT NULL, version TEXT NOT NULL, automated INTEGER NOT NULL,
        count INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (day, kind, version, automated))`);
    });
  }

  add(day, kind, version, automated) {
    this.ctx.storage.sql.exec(
      `INSERT INTO downloads (day, kind, version, automated, count) VALUES (?, ?, ?, ?, 1)
       ON CONFLICT (day, kind, version, automated) DO UPDATE SET count = count + 1`,
      day, kind, version, automated ? 1 : 0,
    );
  }

  rows() {
    return this.ctx.storage.sql
      .exec("SELECT day, kind, version, automated, count FROM downloads ORDER BY day")
      .toArray();
  }
}

const counter = (env) => env.DOWNLOADS.get(env.DOWNLOADS.idFromName("all"));

export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);
    const match = url.pathname.match(/^\/download\/([a-z0-9-]+)\/?$/);
    if (match) {
      const release = await (await env.ASSETS.fetch(new URL("/release.json", url))).json();
      const kind = match[1];
      const file = release.files[kind];
      if (!file) return Response.redirect(new URL("/#download", url), 302);
      if (request.method === "GET") {
        const automated = AUTOMATED.test(request.headers.get("User-Agent") || "bot");
        const day = new Date().toISOString().slice(0, 10);
        ctx.waitUntil(counter(env).add(day, kind, release.version, automated));
      }
      return new Response(null, {
        status: 302,
        headers: { Location: file.url, "Cache-Control": "no-store" },
      });
    }
    if (url.pathname === "/download" || url.pathname === "/download/") {
      return Response.redirect(new URL("/#download", url), 301);
    }
    if (url.pathname === "/api/downloads") {
      return Response.json(await counter(env).rows(), {
        headers: { "Cache-Control": "public, max-age=60" },
      });
    }
    return env.ASSETS.fetch(new URL("/404.html", url)).then(
      (page) => new Response(page.body, { status: 404, headers: page.headers }),
    );
  },
};
