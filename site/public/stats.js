// The site's own counter (/api/downloads, see worker.js), then GitHub's
// per-file download counters for every Matty release.
const fmt = (n) => n.toLocaleString("en");
const PLATFORM = {
  mac: "macOS", "mac-zip": "macOS", windows: "Windows", "windows-zip": "Windows",
  deb: "Linux", rpm: "Linux", appimage: "Linux", "linux-tar": "Linux", checksums: "Checksums",
};
const table = (head, rows, foot) =>
  `<table><thead><tr>${head.map((h, i) => `<th${i ? ' class="n"' : ""}>${h}</th>`).join("")}</tr></thead>` +
  `<tbody>${rows.map((r) => `<tr>${r.map((c, i) => `<td${i ? ' class="n"' : ""}>${c}</td>`).join("")}</tr>`).join("")}</tbody>` +
  (foot ? `<tfoot><tr>${foot.map((c, i) => `<th${i ? ' class="n"' : ""}>${c}</th>`).join("")}</tr></tfoot>` : "") +
  `</table>`;

(async () => {
  const total = document.querySelector("[data-site-total]");
  try {
    const res = await fetch("/api/downloads");
    if (!res.ok) throw new Error(`the counter answered ${res.status}`);
    const rows = await res.json();
    const people = rows.filter((r) => !r.automated && r.kind !== "checksums");
    const automated = rows.filter((r) => r.automated).reduce((s, r) => s + r.count, 0);
    const sum = people.reduce((s, r) => s + r.count, 0);
    const since = rows.length ? rows[0].day : null;
    total.innerHTML = `<b>${fmt(sum)}</b> downloads by people${since ? ` since ${since}` : ""}` +
      ` <span class="fine">(${fmt(automated)} automated, not included)</span>`;
    const platforms = ["macOS", "Windows", "Linux"];
    const byVersion = new Map();
    for (const r of people) {
      const row = byVersion.get(r.version) || Object.fromEntries(platforms.map((p) => [p, 0]));
      row[PLATFORM[r.kind]] += r.count;
      byVersion.set(r.version, row);
    }
    const versions = [...byVersion.keys()].sort((a, b) => b.localeCompare(a, "en", { numeric: true }));
    if (versions.length) {
      document.querySelector("[data-site-platforms]").innerHTML = table(
        ["Version", ...platforms],
        versions.map((v) => [v, ...platforms.map((p) => fmt(byVersion.get(v)[p]))]),
        ["All versions", ...platforms.map((p) => fmt(versions.reduce((s, v) => s + byVersion.get(v)[p], 0)))],
      );
      const byDay = new Map();
      for (const r of people) byDay.set(r.day, (byDay.get(r.day) || 0) + r.count);
      const days = [...byDay.keys()].sort().reverse().slice(0, 30);
      document.querySelector("[data-site-days]").innerHTML = table(
        ["Day", "Downloads"],
        days.map((d) => [d, fmt(byDay.get(d))]),
      );
    }
  } catch (e) {
    total.textContent = `Could not load the site's counts (${e.message}).`;
  }
})();

(async () => {
  const kinds = [
    ["macOS", (n) => n.endsWith(".dmg")],
    ["Windows", (n) => n.endsWith(".msi") || n.endsWith("-windows-x86_64.zip")],
    ["Linux", (n) => /\.(deb|rpm|AppImage)$/.test(n) || n.endsWith("-linux-x86_64.tar.gz")],
    ["macOS updates", (n) => n.endsWith("-macos-universal.zip")],
  ];
  const total = document.querySelector("[data-total]");
  const holder = document.querySelector("[data-table]");
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
    holder.innerHTML = `<table><thead>${head}</thead><tbody>${body}</tbody><tfoot>${foot}</tfoot></table>`;
  } catch (e) {
    total.textContent = `Could not load the counts (${e.message}). GitHub allows 60 requests an hour from one address; try again later.`;
  }
})();
