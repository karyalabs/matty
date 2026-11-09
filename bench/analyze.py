#!/usr/bin/env python3
"""Turn a bench/run.py run into numbers: report.md and results.json.

    bench/analyze.py bench/out/runs/<timestamp>

Every latency comes from the screen recording. The input driver drew a
barcode under the app window at the instant it sent each input; this script
finds the frame where each barcode first appears and then looks at what the
app region did after it:

- first change: the first frame after the input in which the app region
  changed noticeably (more than 0.15% of its pixels). For a quick-switcher
  jump this is usually the switcher closing.
- ready: the end of the last noticeable change before the app region stayed
  still for 600ms (1s for launches), capped at the next input. This is when
  the screen stopped changing: the channel, thread or images are drawn.
- scrolling: during the scroll input, the share of captured frames in which
  the picture moved (frames per second actually shown), the longest freeze
  and the number of hitches (freezes of 50ms or more).
- typing: keypress to the first change in the composer band.

CPU and memory come from the process samples (the app and every helper
process it started; Electron runs several). CPU% is per core, so 100% is one
core busy. Memory is the summed resident size and, on macOS, the physical
footprint that Activity Monitor shows.

Needs numpy (bench/setup installs it in bench/.venv) and ffmpeg/ffprobe.
"""

from __future__ import annotations

import json
import statistics
import subprocess
import sys
from pathlib import Path

import numpy as np

BIG = 0.0015     # share of the app region that counts as a noticeable change
SMALL_PX = 150   # changed pixels that count as "the picture moved" while scrolling
FPS = 60
DIFF = 24        # gray-level difference that counts as a changed pixel


def frames(video: Path, scale: float):
    """Yields (pts seconds, gray frame at 1 px per point)."""
    probe = subprocess.run(
        ["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height",
         "-of", "json", str(video)], capture_output=True, text=True, check=True)
    st = json.loads(probe.stdout)["streams"][0]
    w, h = int(st["width"] / scale) // 2 * 2, int(st["height"] / scale) // 2 * 2
    # Resample to a constant 60 fps (the fps filter repeats or drops frames to
    # the nearest slot), so frame i is at first_pts + i/60 with no timestamp
    # bookkeeping; the error is at most half a frame.
    first = subprocess.run(
        ["ffprobe", "-v", "error", "-select_streams", "v:0", "-read_intervals", "%+#1",
         "-show_entries", "frame=best_effort_timestamp_time", "-of", "csv=p=0", str(video)],
        capture_output=True, text=True, check=True).stdout.split()
    t0 = float(first[0].strip(",")) if first and first[0].strip(",") not in ("", "N/A") else 0.0
    proc = subprocess.Popen(
        ["ffmpeg", "-v", "error", "-i", str(video),
         "-vf", f"fps={FPS}:start_time={t0},scale={w}:{h}:flags=area,format=gray", "-f", "rawvideo", "-"],
        stdout=subprocess.PIPE)
    size = w * h
    i = 0
    while True:
        buf = proc.stdout.read(size)
        if len(buf) < size:
            break
        yield t0 + i / FPS, np.frombuffer(buf, np.uint8).reshape(h, w)
        i += 1
    proc.wait()


def decode_marker(frame: np.ndarray, rect) -> int | None:
    x, y, w, h = rect
    fh, fw = frame.shape
    if y + h > fh or x + w > fw:
        return None
    row = frame[y + h // 4: y + h - h // 4, x: x + w].astype(np.int16)
    cw = w / 18
    vals = [row[:, int(i * cw + cw * 0.3): int(i * cw + cw * 0.7)].mean() for i in range(18)]
    if vals[0] < 160 or vals[17] > 96:
        return None
    if any(64 < v < 192 for v in vals):
        return None  # mid-redraw or blended
    n = 0
    for v in vals[1:17]:
        n = (n << 1) | (1 if v > 128 else 0)
    return n


def scan(video: Path, meta: dict):
    """Per frame: time, marker value, changed pixels in the app, the composer band and the right pane."""
    scale = meta["scale"]
    _, _, aw, ah = meta["rect"]
    marker = meta["marker"]
    rows = []
    prev = None
    for t, f in frames(video, scale):
        app = f[:ah, :aw]
        m = decode_marker(f, marker)
        if prev is None:
            rows.append((t, m, 0, 0, 0))
        else:
            d = np.abs(app.astype(np.int16) - prev.astype(np.int16)) > DIFF
            comp = d[int(ah * 0.80):, int(aw * 0.18):]
            rhs = d[:, int(aw * 0.66):]
            rows.append((t, m, int(d.sum()), int(comp.sum()), int(rhs.sum())))
        prev = app
    return rows


def locate(rows, events):
    """Video time of each event: the first frame showing its barcode, else the clock offset."""
    first = {}
    for t, m, *_ in rows:
        if m is not None and m not in first:
            first[m] = t
    offsets = [first[e["seq"]] - e["t"] for e in events if e["seq"] in first]
    off = statistics.median(offsets) if offsets else None
    out = {}
    for e in events:
        if e["seq"] in first:
            out[e["seq"]] = (first[e["seq"]], True)
        elif off is not None:
            out[e["seq"]] = (e["t"] + off, False)
    return out


def response(rows, t0, t_cap, area, quiet=0.6):
    thresh = area * BIG
    first = last = None
    for t, _, d, *_ in rows:
        if t <= t0 + 1e-4:
            continue
        if t > t_cap:
            break
        if d > thresh:
            if first is None:
                first = t
            last = t
        elif last is not None and t - last >= quiet:
            break
    if first is None:
        return None, None
    return (first - t0) * 1000, (last - t0) * 1000 + 1000 / 60


def scroll_stats(rows, t0, t1):
    seg = [(t, d) for t, _, d, *_ in rows if t0 < t <= t1]
    if len(seg) < 2:
        return None
    moved = [t for t, d in seg if d > SMALL_PX]
    dur = t1 - t0
    gaps = [b - a for a, b in zip([t0] + moved, moved + [t1])]
    return {
        "seconds": round(dur, 2),
        "captured_fps": round(len(seg) / dur, 1),
        "shown_fps": round(len(moved) / dur, 1),
        "smoothness_pct": round(100 * len(moved) / len(seg), 1),
        "longest_freeze_ms": round(max(gaps) * 1000),
        "hitches": sum(1 for g in gaps if g >= 0.05),
    }


def cpu_mem(samples, t0, t1):
    seg = [s for s in samples if t0 <= s["t"] <= t1]
    if len(seg) < 2:
        return {}
    cpu = (seg[-1]["cpu_s"] - seg[0]["cpu_s"]) / (seg[-1]["t"] - seg[0]["t"]) * 100
    return {"cpu_pct": round(cpu, 1), "rss_mb_avg": round(sum(s["rss_mb"] for s in seg) / len(seg)),
            "rss_mb_max": round(max(s["rss_mb"] for s in seg)), "procs": seg[-1]["procs"]}


def med(xs):
    xs = [x for x in xs if x is not None]
    return round(statistics.median(xs)) if xs else None


def p95(xs):
    xs = sorted(x for x in xs if x is not None)
    return round(xs[min(len(xs) - 1, int(len(xs) * 0.95))]) if xs else None


def analyze_app(d: Path) -> dict:
    events = [json.loads(l) for l in (d / "events.jsonl").read_text().splitlines() if l.strip()]
    samples = [json.loads(l) for l in (d / "samples.jsonl").read_text().splitlines() if l.strip()] \
        if (d / "samples.jsonl").exists() else []
    memory = [json.loads(l) for l in (d / "memory.jsonl").read_text().splitlines() if l.strip()] \
        if (d / "memory.jsonl").exists() else []
    out = {}
    starts = []
    for meta_path in sorted(d.glob("*.json")):
        if meta_path.name in ("results.json",):
            continue
        meta = json.loads(meta_path.read_text())
        if "scenario" not in meta:
            continue
        name = meta["scenario"]
        evs = [e for e in events if e["scenario"] == name]
        video = d / f"{name}.mp4"
        res: dict = {}
        mono = {e["seq"]: e["t"] for e in evs}
        t_begin = next((e["t"] for e in evs if e["name"] == "begin"), None)
        t_end = next((e["t"] for e in evs if e["name"] == "end"), None)
        if t_begin and t_end:
            res.update(cpu_mem(samples, t_begin, t_end))
        mem = next((m for m in memory if m["scenario"] == name), None)
        if mem:
            res["rss_mb_end"] = round(mem["rss_mb"])
            if mem.get("footprint_mb"):
                res["footprint_mb_end"] = round(mem["footprint_mb"])
        if video.exists() and video.stat().st_size > 0:
            print(f"  {d.name}/{name}", flush=True)
            rows = scan(video, meta)
            at = locate(rows, evs)
            area = meta["rect"][2] * meta["rect"][3]
            ordered = [e for e in evs if e["seq"] in at]
            vt = {e["seq"]: at[e["seq"]][0] for e in ordered}
            b = next((e for e in ordered if e["name"] == "begin"), None)
            en = next((e for e in ordered if e["name"] == "end"), None)
            if b and en:
                res["clip"] = [round(vt[b["seq"]], 3), round(vt[en["seq"]], 3)]
            res["markers_seen"] = f"{sum(1 for e in ordered if at[e['seq']][1])}/{len(evs)}"
            nexts = {e["seq"]: (vt[ordered[i + 1]["seq"]] if i + 1 < len(ordered) else rows[-1][0])
                     for i, e in enumerate(ordered)}
            navs, keys = [], []
            for e in ordered:
                t0 = vt[e["seq"]]
                if e["name"] == "launch":
                    first, ready = response(rows, t0, t0 + 30, area, quiet=1.0)
                    res["first_paint_ms"], res["ready_ms"] = (round(first) if first else None), (round(ready) if ready else None)
                elif e["name"] == "nav":
                    first, ready = response(rows, t0, min(nexts[e["seq"]], t0 + 8), area)
                    navs.append({"target": e.get("target"), "round": e.get("round"), "first_ms": first, "ready_ms": ready})
                elif e["name"] == "key":
                    lat = None
                    for t, _, _, comp, _ in rows:
                        if t > t0 + 1e-4 and t < t0 + 0.6 and comp > 6:
                            lat = (t - t0) * 1000
                            break
                        if t >= t0 + 0.6:
                            break
                    keys.append(lat)
                elif e["name"].endswith("-start"):
                    tag = e["name"][:-6]
                    end = next((x for x in ordered if x["name"] == f"{tag}-end" and x["seq"] > e["seq"]), None)
                    if end:
                        st = scroll_stats(rows, t0, vt[end["seq"]])
                        if st:
                            st.update(cpu_mem(samples, mono[e["seq"]], mono[end["seq"]]))
                            _, catch = response(rows, vt[end["seq"]], vt[end["seq"]] + 5, area, quiet=0.4)
                            st["catch_up_ms"] = round(catch) if catch else 0
                            res[f"scroll_{tag}"] = st
                elif e["name"] == "traffic-start":
                    stop = next((x for x in ordered if x["name"] == "traffic-end"), None)
                    if stop:
                        st = scroll_stats(rows, t0, vt[stop["seq"]])
                        res["traffic"] = {**(st or {}), **cpu_mem(samples, mono[e["seq"]], mono[stop["seq"]]),
                                          "posts_sent": stop.get("sent"), "errors": stop.get("errors")}
            if navs:
                res["navs"] = [{k: (round(v) if isinstance(v, float) else v) for k, v in n.items()} for n in navs]
                for rnd in sorted({n["round"] for n in navs}, key=str):
                    sub = [n for n in navs if n["round"] == rnd]
                    suffix = f"_{rnd}" if rnd else ""
                    res[f"nav_first_ms_median{suffix}"] = med([n["first_ms"] for n in sub])
                    res[f"nav_ready_ms_median{suffix}"] = med([n["ready_ms"] for n in sub])
                    res[f"nav_ready_ms_p95{suffix}"] = p95([n["ready_ms"] for n in sub])
                    res[f"nav_missed{suffix}"] = sum(1 for n in sub if n["ready_ms"] is None)
            if keys:
                res["key_ms_median"] = med(keys)
                res["key_ms_p95"] = p95(keys)
                res["keys_missed"] = sum(1 for k in keys if k is None)
            if name in ("idle", "settle") and t_begin and t_end and t_begin in mono.values():
                b = next(e for e in ordered if e["name"] == "begin")
                en = next((e for e in ordered if e["name"] == "end"), None)
                if en:
                    seg = [r for r in rows if vt[b["seq"]] + 0.5 < r[0] <= vt[en["seq"]]]
                    res["idle_repaints"] = sum(1 for r in seg if r[2] > SMALL_PX)
                    res["idle_seconds"] = round(vt[en["seq"]] - vt[b["seq"]] - 0.5, 1)
        if name.startswith("startup-"):
            starts.append(res)
        else:
            out[name] = res
    if starts:
        out["startup"] = {
            "runs": len(starts),
            "first_paint_ms_median": med([s.get("first_paint_ms") for s in starts]),
            "ready_ms_median": med([s.get("ready_ms") for s in starts]),
            "ready_ms_each": [s.get("ready_ms") for s in starts],
            "first_paint_ms_each": [s.get("first_paint_ms") for s in starts],
            "rss_mb_end_median": med([s.get("rss_mb_end") for s in starts]),
            "footprint_mb_end_median": med([s.get("footprint_mb_end") for s in starts]),
            "cpu_pct_median": med([s.get("cpu_pct") for s in starts]),
            "procs": starts[-1].get("procs"),
            "clips": {f"startup-{i + 1}": s.get("clip") for i, s in enumerate(starts)},
        }
    return out


# Rows of the report: (section, label, path in results, unit, lower is better)
ROWS = [
    ("Startup", "First window paint (median)", "startup.first_paint_ms_median", "ms", True),
    ("Startup", "Ready: screen stops changing (median)", "startup.ready_ms_median", "ms", True),
    ("Startup", "Memory after start (footprint)", "startup.footprint_mb_end_median", "MB", True),
    ("Startup", "Memory after start (RSS, all processes)", "startup.rss_mb_end_median", "MB", True),
    ("Startup", "Processes", "startup.procs", "", True),
    ("Idle", "CPU, idle 20s", "idle.cpu_pct", "%", True),
    ("Idle", "Repainted frames while idle", "idle.idle_repaints", "", True),
    ("Idle", "Memory (footprint)", "idle.footprint_mb_end", "MB", True),
    ("Channel switch", "Quick switcher, first visit: ready (median)", "switch.nav_ready_ms_median_first", "ms", True),
    ("Channel switch", "Quick switcher, first visit: ready (p95)", "switch.nav_ready_ms_p95_first", "ms", True),
    ("Channel switch", "Quick switcher, revisit: ready (median)", "switch.nav_ready_ms_median_again", "ms", True),
    ("Channel switch", "Quick switcher, revisit: first change (median)", "switch.nav_first_ms_median_again", "ms", True),
    ("Channel switch", "Alt+Down/Up: ready (median)", "sidebar.nav_ready_ms_median", "ms", True),
    ("Channel switch", "Alt+Down/Up: ready (p95)", "sidebar.nav_ready_ms_p95", "ms", True),
    ("Channel switch", "CPU while switching", "switch.cpu_pct", "%", True),
    ("Huge history (100k posts)", "Scroll up: frames shown per second", "scroll.scroll_up.shown_fps", "fps", False),
    ("Huge history (100k posts)", "Scroll up: longest freeze", "scroll.scroll_up.longest_freeze_ms", "ms", True),
    ("Huge history (100k posts)", "Scroll up: hitches ≥50ms", "scroll.scroll_up.hitches", "", True),
    ("Huge history (100k posts)", "Scroll up: CPU", "scroll.scroll_up.cpu_pct", "%", True),
    ("Huge history (100k posts)", "Scroll down: frames shown per second", "scroll.scroll_down.shown_fps", "fps", False),
    ("Huge history (100k posts)", "Fling: frames shown per second", "scroll.scroll_fling.shown_fps", "fps", False),
    ("Huge history (100k posts)", "Fling: catch-up after input stops", "scroll.scroll_fling.catch_up_ms", "ms", True),
    ("Huge history (100k posts)", "Memory after scrolling (footprint)", "scroll.footprint_mb_end", "MB", True),
    ("Huge history (100k posts)", "Memory after scrolling (RSS)", "scroll.rss_mb_end", "MB", True),
    ("Emoji and reactions", "Scroll up: frames shown per second", "emoji.scroll_up.shown_fps", "fps", False),
    ("Emoji and reactions", "Scroll up: longest freeze", "emoji.scroll_up.longest_freeze_ms", "ms", True),
    ("Emoji and reactions", "Scroll up: CPU", "emoji.scroll_up.cpu_pct", "%", True),
    ("Images", "Open gallery: ready", "gallery.nav_ready_ms_median", "ms", True),
    ("Images", "Scroll up: frames shown per second", "gallery.scroll_up.shown_fps", "fps", False),
    ("Images", "Scroll up: longest freeze", "gallery.scroll_up.longest_freeze_ms", "ms", True),
    ("Images", "Scroll up: CPU", "gallery.scroll_up.cpu_pct", "%", True),
    ("Images", "Memory after gallery (footprint)", "gallery.footprint_mb_end", "MB", True),
    ("Large thread (3,000 replies)", "Open thread: ready", "thread.nav_ready_ms_median", "ms", True),
    ("Large thread (3,000 replies)", "Scroll up: frames shown per second", "thread.scroll_up.shown_fps", "fps", False),
    ("Large thread (3,000 replies)", "Scroll up: longest freeze", "thread.scroll_up.longest_freeze_ms", "ms", True),
    ("Large thread (3,000 replies)", "Scroll up: CPU", "thread.scroll_up.cpu_pct", "%", True),
    ("Live traffic", "CPU while ~30 posts/s arrive", "live.traffic.cpu_pct", "%", True),
    ("Live traffic", "Memory after live traffic (footprint)", "live.footprint_mb_end", "MB", True),
    ("Typing", "Keypress to glyph (median)", "typing.key_ms_median", "ms", True),
    ("Typing", "Keypress to glyph (p95)", "typing.key_ms_p95", "ms", True),
    ("End of run", "Memory (footprint)", "settle.footprint_mb_end", "MB", True),
    ("End of run", "Memory (RSS, all processes)", "settle.rss_mb_end", "MB", True),
    ("End of run", "Idle CPU after the stress", "settle.cpu_pct", "%", True),
]


def get(d, path):
    for part in path.split("."):
        if not isinstance(d, dict) or part not in d:
            return None
        d = d[part]
    return d


def fmt(v, unit):
    if v is None:
        return "–"
    if isinstance(v, float):
        v = round(v, 1)
    return f"{v:,} {unit}".strip() if not isinstance(v, str) else v


def report(run: Path, results: dict, info: dict) -> str:
    keys = [k for k in ("matty", "mattermost") if k in results] + [k for k in results if k not in ("matty", "mattermost")]
    labels = {k: f"{info['apps'].get(k, {}).get('label', k)} {info['apps'].get(k, {}).get('version', '')}".strip() for k in keys}
    lines = [f"# Matty benchmark: {info.get('started', run.name)}", ""]
    corpus = info.get("corpus", {})
    lines += [f"Machine: {info.get('cpu', info.get('machine'))}, {info.get('memory_gb', '?')} GB, {info.get('os')}.",
              f"Server: {info.get('server')} with {corpus.get('posts', 0):,} posts, {corpus.get('users', 0)} users, "
              f"{len(corpus.get('channels', {}))} channels in the bench team.", ""]
    head = "| Metric | " + " | ".join(labels[k] for k in keys) + (" | Better |" if len(keys) == 2 else " |")
    sep = "|---|" + "---:|" * len(keys) + ("---|" if len(keys) == 2 else "")
    section = None
    wins = {k: 0 for k in keys}
    for sec, label, path, unit, lower in ROWS:
        vals = [get(results[k], path) for k in keys]
        if all(v is None for v in vals):
            continue
        if sec != section:
            lines += ["", f"## {sec}", "", head, sep]
            section = sec
        row = f"| {label} | " + " | ".join(fmt(v, unit) for v in vals) + " |"
        if len(keys) == 2:
            a, b = vals
            better = ""
            if isinstance(a, (int, float)) and isinstance(b, (int, float)) and a != b:
                win = 0 if (a < b) == lower else 1
                lo, hi = sorted([a, b])
                ratio = (hi / lo) if lo else None
                better = f"{labels[keys[win]].split()[0]}" + (f" ({ratio:.1f}×)" if ratio and ratio >= 1.1 else " (≈)")
                if not ratio or ratio >= 1.1:
                    wins[keys[win]] += 1
            row += f" {better} |"
        lines.append(row)
    if len(keys) == 2:
        lines += ["", f"Clear wins (10% or more apart): " + ", ".join(f"{labels[k]} {wins[k]}" for k in keys) + "."]
    lines += ["", "How each number is measured: bench/README.md. Raw numbers per input: results.json.", ""]
    return "\n".join(lines)


def main() -> None:
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    run = Path(sys.argv[1])
    info = json.loads((run / "run.json").read_text()) if (run / "run.json").exists() else {"apps": {}}
    results = {}
    for d in sorted(p for p in run.iterdir() if p.is_dir()):
        if (d / "events.jsonl").exists():
            results[d.name] = analyze_app(d)
    (run / "results.json").write_text(json.dumps(results, indent=2))
    text = report(run, results, info)
    (run / "report.md").write_text(text)
    print(text)


if __name__ == "__main__":
    main()
