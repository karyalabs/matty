// Requests no file in dist/ matches (wrangler.jsonc). /download/<kind> sends
// you to that file of the newest release on GitHub; the build wrote their
// addresses into release.json. GitHub counts the download, and the Worker
// keeps nothing.
export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const match = url.pathname.match(/^\/download\/([a-z0-9-]+)\/?$/);
    if (match) {
      const release = await (await env.ASSETS.fetch(new URL("/release.json", url))).json();
      const file = release.files[match[1]];
      if (file) {
        return new Response(null, {
          status: 302,
          headers: { Location: file.url, "Cache-Control": "no-store" },
        });
      }
      return Response.redirect(new URL("/#download", url), 302);
    }
    if (url.pathname === "/download" || url.pathname === "/download/") {
      return Response.redirect(new URL("/#download", url), 301);
    }
    return env.ASSETS.fetch(new URL("/404.html", url)).then(
      (page) => new Response(page.body, { status: 404, headers: page.headers }),
    );
  },
};
