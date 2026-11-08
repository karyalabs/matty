#!/usr/bin/env python3
"""Make the side-by-side video of a bench/run.py run (after bench/analyze.py).

    bench/compose.py bench/out/runs/<timestamp>          # side-by-side.mp4
    bench/compose.py RUN --speed 1                       # real time throughout

Each scenario becomes one 1920x1080 segment: Matty on the left, the official
app on the right, both cut from the same input marker so they start on the
same frame, with a running clock and that scenario's numbers underneath. A
summary card closes the video. Real time unless --speed is given, which only
speeds up the long idle and live-traffic segments.
"""

from __future__ import annotations

import argparse
import json
import platform
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ORDER = [("startup-1", "Cold start", "Launch to ready, warm OS cache, signed in"),
         ("switch", "Channel switching", "Quick switcher (Cmd+K) to 10 channels, then the same 10 again"),
         ("sidebar", "Next / previous channel", "Alt+Down 15 times, then Alt+Up 15 times"),
         ("scroll", "Huge history", "Firehose: 100,000 posts. Scroll up 15s, down 7.5s, then a fast fling"),
         ("emoji", "Emoji and reactions", "Emoji Storm: emoji-only posts, 3 to 14 reactions each"),
         ("gallery", "Images", "Gallery: 2,500 posts with 1 to 4 photos each, first visit"),
         ("thread", "Large thread", "Megathread: open a 3,000-reply thread (Shift+Up), then scroll it"),
         ("live", "Live traffic", "About 30 posts/s from other users, 4/s into the open channel"),
         ("typing", "Typing", "One key every 120ms into the composer")]
W, H = 1920, 1080
SIDE_W = 944


def font() -> str:
    for f in ["/System/Library/Fonts/Supplemental/Arial.ttf", "/System/Library/Fonts/Helvetica.ttc",
              "/Library/Fonts/Arial.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
              "/usr/share/fonts/dejavu/DejaVuSans.ttf"]:
        if Path(f).exists():
            return f
    out = subprocess.run(["fc-match", "-f", "%{file}", "sans"], capture_output=True, text=True).stdout
    return out or sys.exit("no font found for drawtext")


def encoder() -> list[str]:
    if platform.system() == "Darwin":
        return ["-c:v", "h264_videotoolbox", "-b:v", "16M"]
    return ["-c:v", "libx264", "-preset", "veryfast", "-crf", "20"]


def lines_for(name: str, r: dict) -> list[str]:
    def g(*path, unit="ms"):
        d = r
        for p in path:
            if not isinstance(d, dict) or p not in d:
                return "–"
            d = d[p]
        return "–" if d is None else f"{d:,} {unit}".strip()

    if name.startswith("startup"):
        s = r.get("startup", {})
        return [f"First paint {s.get('first_paint_ms_median', '–')} ms   Ready {s.get('ready_ms_median', '–')} ms (median of {s.get('runs', '?')})",
                f"Memory after start {s.get('footprint_mb_end_median') or s.get('rss_mb_end_median') or '–'} MB   Processes {s.get('procs', '–')}"]
    if name == "switch":
        return [f"First visit ready {g('switch', 'nav_ready_ms_median_first')} median, {g('switch', 'nav_ready_ms_p95_first')} p95",
                f"Revisit ready {g('switch', 'nav_ready_ms_median_again')} median   CPU {g('switch', 'cpu_pct', unit='%')}"]
    if name == "sidebar":
        return [f"Ready {g('sidebar', 'nav_ready_ms_median')} median, {g('sidebar', 'nav_ready_ms_p95')} p95",
                f"CPU {g('sidebar', 'cpu_pct', unit='%')}"]
    if name in ("scroll", "emoji", "gallery", "thread"):
        out = [f"Scroll up {g(name, 'scroll_up', 'shown_fps', unit='fps')}   longest freeze {g(name, 'scroll_up', 'longest_freeze_ms')}   hitches {g(name, 'scroll_up', 'hitches', unit='')}",
               f"CPU {g(name, 'scroll_up', 'cpu_pct', unit='%')}   memory {g(name, 'footprint_mb_end', unit='MB') if r.get(name, {}).get('footprint_mb_end') else g(name, 'rss_mb_end', unit='MB')}"]
        if name in ("gallery", "thread"):
            out[1] = f"Open {g(name, 'nav_ready_ms_median')}   " + out[1]
        return out
    if name == "live":
        return [f"CPU {g('live', 'traffic', 'cpu_pct', unit='%')}   frames shown {g('live', 'traffic', 'shown_fps', unit='fps')}",
                f"Memory {g('live', 'footprint_mb_end', unit='MB') if r.get('live', {}).get('footprint_mb_end') else g('live', 'rss_mb_end', unit='MB')}"]
    if name == "typing":
        return [f"Keypress to glyph {g('typing', 'key_ms_median')} median, {g('typing', 'key_ms_p95')} p95", ""]
    return ["", ""]


def load_font(path: str, size: int):
    from PIL import ImageFont
    return ImageFont.truetype(path, size)


def card(path: Path, fontfile: str, texts: list[tuple[int, int, str, int, str]], holes=()) -> None:
    """A 1920x1080 frame: dark background, text, and transparent holes where the videos go."""
    from PIL import Image, ImageDraw
    img = Image.new("RGBA", (W, H), (22, 24, 29, 255))
    draw = ImageDraw.Draw(img)
    for x, y, w, h in holes:
        draw.rectangle([x, y, x + w - 1, y + h - 1], fill=(0, 0, 0, 0))
    for x, y, text, size, color in texts:
        draw.text((x, y), text, font=load_font(fontfile, size), fill=color)
    img.save(path)


def clock_frames(folder: Path, fontfile: str, dur: float, xs: list[int], lead: float) -> None:
    """A transparent strip per frame (30 fps) with a running ms clock over each side."""
    from PIL import Image, ImageDraw
    folder.mkdir(parents=True, exist_ok=True)
    f = load_font(fontfile, 30)
    for i in range(int(dur * 30) + 1):
        t = max(0.0, i / 30 - lead)
        img = Image.new("RGBA", (W, 48), (0, 0, 0, 0))
        d = ImageDraw.Draw(img)
        for x in xs:
            d.text((x + SIDE_W - 190, 6), f"{int(t * 1000):>6} ms", font=f, fill=(255, 215, 94, 255))
        img.save(folder / f"{i:05d}.png")


def segment(tmp: Path, idx: int, run: Path, keys: list[str], labels: dict, results: dict, name: str, title: str,
            subtitle: str, fontfile: str, speed: float) -> Path | None:
    clips = []
    for k in keys:
        video = run / k / f"{name}.mp4"
        t = _clip_from_results(results, k, name)
        if not video.exists() or t is None:
            return None
        meta = json.loads((run / k / f"{name}.json").read_text())
        clips.append((video, max(0.0, t[0] - 0.3), t[1] - t[0] + 0.6, meta))
    dur = max(c[2] for c in clips)
    xs = [16 + i * (SIDE_W + 16) for i in range(len(clips))]
    vid_h = []
    for _, _, _, meta in clips:
        _, _, aw, ah = meta["rect"]
        vid_h.append(int(SIDE_W * ah / aw) // 2 * 2)
    top = 128
    label_y = top + max(vid_h) + 16
    texts = [(24, 18, title, 44, (255, 255, 255, 255)), (24, 76, subtitle, 24, (184, 188, 198, 255))]
    for x, k in zip(xs, keys):
        texts.append((x + 8, label_y, labels[k], 30, (255, 255, 255, 255)))
        for j, line in enumerate(lines_for(name, results.get(k, {}))):
            texts.append((x + 8, label_y + 50 + j * 40, line, 26, (223, 227, 234, 255)))
    bg = tmp / f"bg{idx:02d}.png"
    card(bg, fontfile, texts, holes=[(x, top, SIDE_W, h) for x, h in zip(xs, vid_h)])
    clock_dir = tmp / f"clock{idx:02d}"
    clock_frames(clock_dir, fontfile, dur, xs, 0.3)
    out = tmp / f"seg{idx:02d}.mp4"
    inputs = ["-loop", "1", "-framerate", "60", "-t", f"{dur:.3f}", "-i", str(bg)]
    filters = []
    for i, (video, start, _, meta) in enumerate(clips):
        inputs += ["-ss", f"{start:.3f}", "-t", f"{dur:.3f}", "-i", str(video)]
        _, _, aw, ah = meta["rect"]
        sc = meta["scale"]
        # Crop off the marker strip; it is for the analysis, not the viewer.
        filters.append(f"[{i + 1}:v]crop={int(aw * sc) // 2 * 2}:{int(ah * sc) // 2 * 2}:0:0,scale={SIDE_W}:{vid_h[i]},"
                       f"setpts=PTS-STARTPTS,tpad=stop_mode=clone:stop_duration={dur:.2f},trim=duration={dur:.3f}[v{i}]")
    n = len(clips) + 1
    inputs += ["-framerate", "30", "-i", str(clock_dir / "%05d.png")]
    filters.append(f"color=c=0x16181d:s={W}x{H}:d={dur:.3f}:r=60[base]")
    chain = "[base]"
    for i, x in enumerate(xs):
        filters.append(f"{chain}[v{i}]overlay=x={x}:y={top}:eof_action=repeat[o{i}]")
        chain = f"[o{i}]"
    filters.append(f"{chain}[0:v]overlay=0:0[card]")
    filters.append(f"[card][{n}:v]overlay=x=0:y={label_y - 6}:eof_action=repeat[clk]")
    sp = speed if name == "live" else 1.0
    filters.append(f"[clk]{'setpts=PTS/' + str(sp) + ',' if sp != 1 else ''}trim=duration={dur / sp:.3f},fps=60,format=yuv420p[out]")
    cmd = ["ffmpeg", "-v", "error", "-y", *inputs, "-filter_complex", ";".join(filters), "-map", "[out]",
           *encoder(), str(out)]
    subprocess.run(cmd, check=True)
    return out


def _clip_from_results(results: dict, key: str, name: str):
    """Where the scenario starts and ends in its video (analyze.py's "clip")."""
    res = results.get(key, {})
    if name.startswith("startup"):
        return res.get("startup", {}).get("clips", {}).get(name)
    return res.get(name, {}).get("clip")


def summary(tmp: Path, run: Path, keys, labels, fontfile: str) -> Path:
    report = (run / "report.md").read_text().splitlines()
    rows = [l for l in report if l.startswith("| ") and not l.startswith("| Metric")]
    section, picked = "", []
    for l in report:
        if l.startswith("## "):
            section = l[3:]
        elif l.startswith("| ") and not l.startswith("| Metric") and l in rows:
            cells = [c.strip() for c in l.strip("|").split("|")]
            picked.append((section, cells))
    keep = ["Ready: screen", "Memory after start (footprint)", "CPU, idle", "first visit: ready (median)",
            "revisit: ready (median)", "Alt+Down/Up: ready (median)", "Scroll up: frames shown",
            "Scroll up: longest freeze", "Open gallery: ready", "Open thread: ready", "CPU while ~30",
            "Keypress to glyph (median)", "Memory (footprint)"]
    chosen = [(sec, c) for sec, c in picked if any(k in c[0] for k in keep)
              and not (sec.startswith("Idle") and c[0].startswith("Memory"))][:18]
    cols = [60, 860, 1230, 1560]
    texts = [(60, 30, "Summary", 52, (255, 255, 255, 255))]
    head = ["", labels[keys[0]], labels[keys[1]] if len(keys) > 1 else "", "Better" if len(keys) > 1 else ""]
    for c, h in zip(cols, head):
        texts.append((c, 112, h, 26, (255, 215, 94, 255)))
    for i, (sec, cells) in enumerate(chosen):
        y = 160 + i * 48
        label = f"{sec}: {cells[0]}"
        for j, (c, v) in enumerate(zip(cols, [label, *cells[1:]])):
            texts.append((c, y, v, 24 if j == 0 else 26, (223, 227, 234, 255)))
    png = tmp / "summary.png"
    card(png, fontfile, texts)
    out = tmp / "summary.mp4"
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-loop", "1", "-framerate", "60", "-t", "12", "-i", str(png),
                    "-vf", "format=yuv420p", *encoder(), str(out)], check=True)
    return out


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("run", type=Path)
    ap.add_argument("--speed", type=float, default=2.0, help="speed-up for the live-traffic segment (default 2)")
    ap.add_argument("--out", type=Path)
    args = ap.parse_args()
    run = args.run
    if not shutil.which("ffmpeg"):
        sys.exit("ffmpeg is needed")
    results = json.loads((run / "results.json").read_text())
    info = json.loads((run / "run.json").read_text())
    keys = [k for k in ("matty", "mattermost") if k in results] + [k for k in results if k not in ("matty", "mattermost")]
    labels = {k: f"{info['apps'].get(k, {}).get('label', k)} {info['apps'].get(k, {}).get('version', '')}".strip() for k in keys}
    fontfile = font()
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        segs = []
        for i, (name, title, sub) in enumerate(ORDER):
            print(f"  segment {name}", flush=True)
            try:
                seg = segment(tmp, i, run, keys, labels, results, name, title, sub, fontfile, args.speed)
            except subprocess.CalledProcessError as err:
                print(f"    skipped: ffmpeg failed ({err})")
                seg = None
            if seg:
                segs.append(seg)
        segs.append(summary(tmp, run, keys, labels, fontfile))
        lst = tmp / "list.txt"
        lst.write_text("".join(f"file '{s}'\n" for s in segs))
        out = args.out or run / "side-by-side.mp4"
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-f", "concat", "-safe", "0", "-i", str(lst), "-c", "copy",
                        "-movflags", "+faststart", str(out)], check=True)
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
