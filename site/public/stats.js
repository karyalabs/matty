// Sums GitHub's per-file download counters for every Matty release.
(async () => {
  const kinds = [
    ["macOS", (n) => n.endsWith(".dmg")],
    ["Windows", (n) => n.endsWith(".msi") || n.endsWith("-windows-x86_64.zip")],
    ["Linux", (n) => /\.(deb|rpm|AppImage)$/.test(n) || n.endsWith("-linux-x86_64.tar.gz")],
    ["macOS updates", (n) => n.endsWith("-macos-universal.zip")],
  ];
  const total = document.querySelector("[data-total]");
  const table = document.querySelector("[data-table]");
  const fmt = (n) => n.toLocaleString("en");
  try {
    const releases = [];
    for (let page = 1; page <= 5; page++) {
      const r = await fetch(`https://api.github.com/repos/karyalabs/matty/releases?per_page=100&page=${page}`, { headers: { Accept: "application/vnd.github+json" } });
      if (!r.ok) throw new Error(`GitHub answered ${r.status}`);
      const batch = await r.json();
      releases.push(...batch.filter((x) => !x.draft));
      if (batch.length < 100) break;
    }
    const rows = releases.map((rel) => {
      const counts = kinds.map(([, test]) => rel.assets.filter((a) => test(a.name)).reduce((s, a) => s + a.download_count, 0));
      return { tag: rel.tag_name, url: rel.html_url, date: rel.published_at, counts };
    });
    const sums = kinds.map((_, i) => rows.reduce((s, r) => s + r.counts[i], 0));
    const installs = sums[0] + sums[1] + sums[2];
    total.innerHTML = `<b>${fmt(installs)}</b> installer downloads across ${rows.length} releases`;
    const head = `<tr><th>Release</th>${kinds.map(([k]) => `<th class="n">${k}</th>`).join("")}</tr>`;
    const body = rows.map((r) => `<tr><td><a href="${r.url}">${r.tag}</a> <span class="fine">${r.date ? r.date.slice(0, 10) : ""}</span></td>${r.counts.map((c) => `<td class="n">${fmt(c)}</td>`).join("")}</tr>`).join("");
    const foot = `<tr><th>All releases</th>${sums.map((c) => `<th class="n">${fmt(c)}</th>`).join("")}</tr>`;
    table.innerHTML = `<table><thead>${head}</thead><tbody>${body}</tbody><tfoot>${foot}</tfoot></table>`;
  } catch (e) {
    total.textContent = `Could not load the counts (${e.message}). GitHub allows 60 requests an hour from one address; try again later.`;
  }
})();
