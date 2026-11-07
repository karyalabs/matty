// Requests that match no file in dist/ (wrangler.jsonc). The old address,
// matty.jha.sh, showed the demo, so every path there goes to the demo's home;
// its own files moved under /matty/, so nothing there still matches.
export default {
  fetch(request) {
    const url = new URL(request.url);
    if (url.hostname === "matty.jha.sh") {
      return Response.redirect(`https://demo.karyalabs.dev/matty/${url.search}`, 301);
    }
    return new Response("Not found", { status: 404 });
  },
};
