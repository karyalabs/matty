#!/usr/bin/env python3
"""Run the same scripted stress journey in Matty and the official Mattermost
desktop app, recording the screen and sampling CPU and memory.

    bench/run.py setup                 # once: fresh profiles, sign both apps in
    bench/run.py                       # the benchmark; then analyze + compose
    bench/run.py --apps matty          # one app only
    bench/run.py --scenarios startup,scroll

Everything lands in bench/out/runs/<timestamp>/: one screen recording per app
and scenario, the input events, CPU/memory samples, then (via
bench/analyze.py and bench/compose.py, which this calls at the end)
report.md, results.json and side-by-side.mp4.

See bench/README.md for what each scenario does and how it is measured. In
short: the input driver (bench/mac/driver.swift, or xdotool on Linux) draws a
small barcode under the app window at the instant it sends each input, so the
recording itself says when each input happened, and every latency comes from
the video, measured identically for both apps.

macOS is the real target. Linux under Xvfb works for trying the harness
(Matty only; the official app is not installed there).
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import platform
import queue
import random
import re
import shutil
import signal
import subprocess
import sys
import threading
import time
import urllib.request
from dataclasses import dataclass, field
from pathlib import Path

HERE = Path(__file__).resolve().parent
OUT = HERE / "out"
MAC = platform.system() == "Darwin"
WIN_X, WIN_Y, WIN_W, WIN_H = 40, 60, 1280, 800
MARK_H = 16
MARK_W = 18 * 20

SCENARIOS = ["startup", "idle", "switch", "sidebar", "scroll", "emoji", "gallery",
             "thread", "live", "typing", "settle"]


# -- Drivers ---------------------------------------------------------------


class MacDriver:
    """bench/mac/driver.swift, compiled on first use."""

    def __init__(self):
        binary = OUT / "driver"
        src = HERE / "mac" / "driver.swift"
        if not binary.exists() or binary.stat().st_mtime < src.stat().st_mtime:
            OUT.mkdir(parents=True, exist_ok=True)
            print("building the input driver (swiftc)…")
            subprocess.run(["swiftc", "-O", "-o", str(binary), str(src)], check=True)
        self.proc = subprocess.Popen([str(binary)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1)
        self.lock = threading.Lock()

    def cmd(self, line: str) -> str:
        with self.lock:
            self.proc.stdin.write(line + "\n")
            self.proc.stdin.flush()
            reply = self.proc.stdout.readline().strip()
        if not reply.startswith("ok"):
            raise RuntimeError(f"driver: {line!r} -> {reply!r}")
        return reply[2:].strip()

    def close(self):
        try:
            self.cmd("quit")
        except Exception:
            pass
        self.proc.kill()


class X11Driver:
    """The same commands on Linux: xdotool for input, an Xlib window for the marker."""

    def __init__(self):
        from Xlib import X, display  # pip install python-xlib
        self.X = X
        self.d = display.Display()
        s = self.d.screen()
        self.screen = s
        self.win = s.root.create_window(0, 0, MARK_W, MARK_H, 0, s.root_depth, X.InputOutput, X.CopyFromParent,
                                        background_pixel=s.black_pixel, override_redirect=True)
        self.gc_w = self.win.create_gc(foreground=s.white_pixel)
        self.gc_b = self.win.create_gc(foreground=s.black_pixel)
        self.rect = (0, 0, MARK_W, MARK_H)
        self.value = 0
        self.lock = threading.Lock()

    def _draw(self):
        _, _, w, h = self.rect
        cw = w / 18
        for i in range(18):
            on = True if i == 0 else False if i == 17 else bool((self.value >> (16 - i)) & 1)
            self.win.fill_rectangle(self.gc_w if on else self.gc_b, int(i * cw), 0, int(cw) + 1, h)
        self.d.sync()

    def _refocus(self):
        """With no window manager, focus falls back to PointerRoot when a window
        closes; put it back on the app's window (its popups are inside it)."""
        focus = self.d.get_input_focus().focus
        if getattr(self, "focus_wid", None) and (isinstance(focus, int) or focus.id in (0, 1)):
            self.xdo("windowfocus", self.focus_wid)

    def xdo(self, *args: str) -> str:
        # `search` exits 1 when nothing matches; that is an empty answer, not an error.
        out = subprocess.run(["xdotool", *args], capture_output=True, text=True)
        if out.returncode and args[0] != "search":
            raise RuntimeError(f"xdotool {' '.join(args)}: {out.stderr.strip()}")
        return out.stdout.strip()

    def cmd(self, line: str) -> str:
        with self.lock:
            words = line.split()
            if words and words[0].startswith("@"):
                self.value = int(words[0][1:])
                self._draw()
                words = words[1:]
                if not words:
                    return str(time.monotonic_ns())
            op, args = words[0], words[1:]
            if op in ("key", "char", "type"):
                self._refocus()
                # Under Xvfb, Matty sometimes holds keyboard input until the
                # pointer moves; nudge it after each key (Linux dry runs only).
                self._nudge = True
            keymap = {"return": "Return", "escape": "Escape", "down": "Down", "up": "Up", "delete": "BackSpace",
                      "pageup": "Prior", "pagedown": "Next", "space": "space", "a": "a", "k": "k", "q": "q"}
            if op == "check":
                return "ax=1 screen=1"
            if op == "screen":
                return f"{self.screen.width_in_pixels} {self.screen.height_in_pixels} 1.0"
            if op == "markrect":
                x, y, w, h = map(int, args)
                self.rect = (x, y, w, h)
                self.win.configure(x=x, y=y, width=w, height=h, stack_mode=self.X.Above)
                self.win.map()
                self.d.sync()
                time.sleep(0.05)
                self._draw()
            elif op == "key":
                mods = [{"cmd": "ctrl", "alt": "alt", "shift": "shift", "ctrl": "ctrl"}[m]
                        for m in (args[1].split(",") if len(args) > 1 else [])]
                self.xdo("key", "+".join([*mods, keymap.get(args[0], args[0])]))
            elif op == "char":
                self.xdo("type", " " if args[0] == "space" else args[0])
            elif op == "type":
                self.xdo("type", "--delay", args[0], " ".join(args[1:]))
            elif op == "move":
                self.xdo("mousemove", args[0], args[1])
            elif op == "click":
                self.xdo("mousemove", args[0], args[1], "click", "1")
            elif op == "scroll":
                # X has no pixel scrolling: one wheel click per event (about 50px in GPUI).
                dy, count, interval = int(args[0]), int(args[1]), float(args[2])
                button = "4" if dy > 0 else "5"
                start = time.monotonic()
                for i in range(count):
                    subprocess.run(["xdotool", "click", button], check=True)
                    delay = start + (i + 1) * interval / 1000 - time.monotonic()
                    if delay > 0:
                        time.sleep(delay)
            elif op in ("place", "windows"):
                pid = args[0]
                ids = self.xdo("search", "--pid", pid).split()
                if op == "place" and ids:
                    x, y, w, h = args[1:5]
                    self.xdo("windowsize", ids[-1], w, h)
                    self.xdo("windowmove", ids[-1], x, y)
                    time.sleep(0.2)
                rects = []
                for wid in ids:
                    geo = self.xdo("getwindowgeometry", "--shell", wid)
                    g = dict(line.split("=") for line in geo.splitlines() if "=" in line)
                    if int(g["WIDTH"]) > 100:
                        rects.append(f"{g['X']},{g['Y']},{g['WIDTH']},{g['HEIGHT']}")
                if op == "place":
                    return rects[-1].replace(",", " ") if rects else "0 0 0 0"
                return " ".join(rects)
            elif op == "front":
                ids = self.xdo("search", "--pid", args[0]).split()
                if ids:
                    self.focus_wid = ids[-1]
                    self.xdo("windowfocus", ids[-1])
            elif op == "sleep":
                time.sleep(float(args[0]) / 1000)
            if getattr(self, "_nudge", False):
                self._nudge = False
                self.xdo("mousemove_relative", "--", "1", "0")
                self.xdo("mousemove_relative", "--", "-1", "0")
            return str(time.monotonic_ns())

    def close(self):
        self.win.destroy()
        self.d.sync()


# -- Apps --------------------------------------------------------------------


@dataclass
class App:
    key: str
    label: str
    path: Path
    profile: Path
    version: str = ""
    env: dict = field(default_factory=dict)

    def __post_init__(self):
        pass

    def command(self) -> list[str]:
        raise NotImplementedError

    def prepare(self, server: str) -> None:
        pass

    def spawn(self) -> subprocess.Popen:
        env = {**os.environ, **self.env}
        log = open(self.profile / "app.log", "ab")
        cmd = self.command()
        if sys.platform == "darwin":
            # The driver execs the app as its own responsible process (see
            # "disclaim" in bench/mac/driver.swift); the pid stays the same.
            cmd = [str(OUT / "driver"), "disclaim", *cmd]
        return subprocess.Popen(cmd, env=env, stdout=log, stderr=log, start_new_session=True)


class Matty(App):
    def __post_init__(self):  # dataclass calls this only if App defines it
        # A fresh HOME keeps settings, cache and the single-instance socket
        # apart from your everyday Matty.
        self.home = self.profile / "home"
        self.home.mkdir(parents=True, exist_ok=True)
        self.env = {"HOME": str(self.home)}
        if os.environ.get("MATTY_PERF_LOG_BENCH"):
            self.env["MATTY_PERF_LOG"] = str(self.profile / "perf.log")

    def prepare(self, server: str):
        if not MAC:
            return
        # macOS finds the default keychain through HOME, so with the bench's
        # own HOME, Matty could not keep its session ("A default keychain could
        # not be found") and opened signed out on every launch. Give that HOME
        # a keychain of its own, apart from your login keychain, never locked.
        kc = self.home / "Library" / "Keychains" / "bench.keychain-db"
        env = {**os.environ, "HOME": str(self.home)}

        def security(*a: str, env=env) -> str:
            return subprocess.run(["security", *a], env=env, check=True, capture_output=True, text=True).stdout

        # Your own default keychain and search list, to prove they are untouched.
        mine = security("default-keychain", "-d", "user", env=None), security("list-keychains", "-d", "user", env=None)
        prefs = self.home / "Library" / "Preferences"
        prefs.mkdir(parents=True, exist_ok=True)
        if not kc.exists():
            kc.parent.mkdir(parents=True, exist_ok=True)
            security("create-keychain", "-p", "bench", str(kc))
        security("list-keychains", "-d", "user", "-s", str(kc))
        security("default-keychain", "-d", "user", "-s", str(kc))
        if "bench.keychain-db" not in self._default_keychain(env):
            # `security … -d user -s` may store the setting through cfprefsd,
            # which ignores HOME; the Security framework reads this file.
            self._write_security_plist(prefs / "com.apple.security.plist", kc)
        if "bench.keychain-db" not in self._default_keychain(env):
            sys.exit(f"could not give the bench HOME ({self.home}) a default keychain; "
                     "Matty would open signed out on every launch")
        security("unlock-keychain", "-p", "bench", str(kc))
        security("set-keychain-settings", str(kc))  # no auto-lock
        if (security("default-keychain", "-d", "user", env=None),
                security("list-keychains", "-d", "user", env=None)) != mine:
            paths = [p.strip().strip('"') for p in mine[1].splitlines() if p.strip()]
            security("list-keychains", "-d", "user", "-s", *paths, env=None)
            security("default-keychain", "-d", "user", "-s", mine[0].strip().strip('"'), env=None)
            sys.exit("macOS applied the bench keychain to your own account instead of the bench HOME; "
                     f"restored your settings:\n{mine[0]}{mine[1]}")

    @staticmethod
    def _default_keychain(env) -> str:
        out = subprocess.run(["security", "default-keychain", "-d", "user"], env=env, capture_output=True, text=True)
        return out.stdout + out.stderr

    @staticmethod
    def _write_security_plist(path: Path, kc: Path) -> None:
        """The Security framework's own preferences, modelled on yours when you
        have the file, pointing the search list and the default at `kc`."""
        import plistlib

        entry = {"DbName": str(kc), "DbSubserviceType": 6, "SubserviceType": 6,
                 "GUID": {"Data1": 2269428384, "Data2": 63696, "Data3": 4567,
                          "Data4": bytes.fromhex("bf5f000a95ab0a92")}}  # the Apple file DL
        mine = Path.home() / "Library" / "Preferences" / "com.apple.security.plist"
        try:
            ref = plistlib.loads(mine.read_bytes())
            entry = {**ref["DefaultKeychain"], "DbName": str(kc)}
        except Exception:
            pass
        path.write_bytes(plistlib.dumps({"DLDBSearchList": [entry], "DefaultKeychain": entry}))

    def command(self):
        if self.path.suffix == ".app":
            exe = self.path / "Contents" / "MacOS"
            names = [p for p in exe.iterdir() if p.is_file()]
            return [str(names[0])]
        return [str(self.path)]


class Mattermost(App):
    def command(self):
        exe = self.path / "Contents" / "MacOS" / "Mattermost" if self.path.suffix == ".app" else self.path
        return [str(exe), f"--dataDir={self.profile / 'data'}"]

    def prepare(self, server: str):
        data = self.profile / "data"
        data.mkdir(parents=True, exist_ok=True)
        cfg = data / "config.json"
        config = json.loads(cfg.read_text()) if cfg.exists() else {}
        config.update({
            "version": 4,
            "servers": [{"name": "bench", "url": server, "order": 0}],
            "autostart": False, "hideOnStart": False, "startInFullscreen": False,
            "showTrayIcon": False, "minimizeToTray": False, "enableMetrics": False,
            "enableSentry": False, "lastActiveServer": 0, "useSpellChecker": False,
            "autoCheckForUpdates": False, "darkMode": False,
        })
        cfg.write_text(json.dumps(config, indent=2))
        (data / "bounds-info.json").write_text(json.dumps(
            {"x": WIN_X, "y": WIN_Y, "width": WIN_W, "height": WIN_H, "maximized": False, "fullscreen": False}))


def app_version(app: App) -> str:
    plist = app.path / "Contents" / "Info.plist"
    if MAC and plist.exists():
        out = subprocess.run(["defaults", "read", str(plist), "CFBundleShortVersionString"],
                             capture_output=True, text=True)
        return out.stdout.strip()
    return ""


# -- Sampling --------------------------------------------------------------


def parse_cputime(s: str) -> float:
    days = 0
    if "-" in s:
        d, s = s.split("-", 1)
        days = int(d)
    parts = [float(p) for p in s.split(":")]
    secs = 0.0
    for p in parts:
        secs = secs * 60 + p
    return days * 86400 + secs


def _linux_cpu(pid: int) -> float | None:
    """ps on Linux prints whole seconds only; /proc has clock ticks."""
    try:
        stat = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
        return (int(stat[11]) + int(stat[12])) / os.sysconf("SC_CLK_TCK")
    except (OSError, IndexError, ValueError):
        return None


def process_tree(root: int) -> dict[int, tuple[int, float, str]]:
    """pid -> (rss KB, cpu seconds, command) for root and all its descendants."""
    out = subprocess.run(["ps", "-A", "-o", "pid=,ppid=,rss=,time=,comm="], capture_output=True, text=True).stdout
    rows = {}
    children: dict[int, list[int]] = {}
    for line in out.splitlines():
        parts = line.split(None, 4)
        if len(parts) < 5:
            continue
        pid, ppid = int(parts[0]), int(parts[1])
        rows[pid] = (int(parts[2]), parse_cputime(parts[3]), parts[4])
        children.setdefault(ppid, []).append(pid)
    tree, stack = {}, [root]
    while stack:
        pid = stack.pop()
        if pid in rows:
            tree[pid] = rows[pid]
            if not MAC:
                cpu = _linux_cpu(pid)
                if cpu is not None:
                    tree[pid] = (rows[pid][0], cpu, rows[pid][2])
            stack.extend(children.get(pid, []))
    return tree


def footprint(pids: list[int]) -> float | None:
    """Physical footprint in MB, the number Activity Monitor shows as Memory."""
    if not MAC or not shutil.which("footprint") or not pids:
        return None
    args = ["footprint"]
    for pid in pids:
        args += ["-p", str(pid)]
    out = subprocess.run(args, capture_output=True, text=True).stdout
    total = 0.0
    found = False
    for m in re.finditer(r"^\S.*?\[\d+\].*?:\s*([\d.]+)\s*([KMG]B)", out, re.M):
        found = True
        total += float(m.group(1)) * {"KB": 1 / 1024, "MB": 1, "GB": 1024}[m.group(2)]
    if not found:
        m = re.search(r"phys_footprint:\s*([\d.]+)\s*([KMG]B)", out)
        if m:
            return float(m.group(1)) * {"KB": 1 / 1024, "MB": 1, "GB": 1024}[m.group(2)]
        return None
    return total


class Sampler(threading.Thread):
    def __init__(self, pid: int, path: Path, interval: float = 0.5):
        super().__init__(daemon=True)
        self.pid, self.interval = pid, interval
        self.file = open(path, "a")
        self.stop_flag = threading.Event()
        self.scenario = ""

    def run(self):
        while not self.stop_flag.is_set():
            t = time.monotonic()
            tree = process_tree(self.pid)
            if tree:
                self.file.write(json.dumps({
                    "t": t, "scenario": self.scenario, "procs": len(tree),
                    "rss_mb": sum(v[0] for v in tree.values()) / 1024,
                    "cpu_s": sum(v[1] for v in tree.values()),
                    "per_proc": {str(pid): [round(v[0] / 1024, 1), v[1]] for pid, v in tree.items()},
                }) + "\n")
                self.file.flush()
            self.stop_flag.wait(max(0.05, self.interval - (time.monotonic() - t)))

    def stop(self):
        self.stop_flag.set()
        self.join(timeout=3)
        self.file.close()


# -- Recording ---------------------------------------------------------------


def avfoundation_screen() -> str:
    out = subprocess.run(["ffmpeg", "-hide_banner", "-f", "avfoundation", "-list_devices", "true", "-i", ""],
                         capture_output=True, text=True).stderr
    m = re.search(r"\[(\d+)\] Capture screen 0", out)
    if not m:
        sys.exit("ffmpeg sees no screen to capture; give your terminal Screen Recording permission")
    return m.group(1)


class Recorder:
    def __init__(self, path: Path, rect: tuple[int, int, int, int], scale: float, fps: int):
        x, y, w, h = rect
        px = [int(round(v * scale)) // 2 * 2 for v in (x, y, w, h)]
        self.path = path
        if MAC:
            src = ["-f", "avfoundation", "-capture_cursor", "0", "-framerate", str(fps),
                   "-pixel_format", "uyvy422", "-i", f"{avfoundation_screen()}:none"]
            enc = ["-c:v", "h264_videotoolbox", "-b:v", "40M", "-realtime", "1"]
            vf = f"crop={px[2]}:{px[3]}:{px[0]}:{px[1]}"
        else:
            src = ["-f", "x11grab", "-framerate", str(fps), "-video_size", f"{px[2]}x{px[3]}",
                   "-i", f"{os.environ.get('DISPLAY', ':99')}+{px[0]},{px[1]}"]
            enc = ["-c:v", "libx264", "-preset", "ultrafast", "-crf", "16"]
            vf = "null"
        self.proc = subprocess.Popen(
            ["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", *src, "-vf", vf, "-fps_mode", "passthrough",
             *enc, "-pix_fmt", "yuv420p", str(path)],
            stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=open(str(path) + ".log", "w"))
        time.sleep(1.5)  # the capture session takes a moment to deliver frames

    def stop(self):
        try:
            self.proc.stdin.write(b"q")
            self.proc.stdin.flush()
            self.proc.stdin.close()
        except Exception:
            pass
        try:
            self.proc.wait(timeout=20)
        except subprocess.TimeoutExpired:
            self.proc.send_signal(signal.SIGINT)
            self.proc.wait(timeout=10)


# -- Live traffic ------------------------------------------------------------


class Traffic(threading.Thread):
    """Posts and reactions from other users while the client sits in a channel."""

    def __init__(self, server: dict, focus_channel: str, rate_focus: float, rate_other: float, seconds: float):
        super().__init__(daemon=True)
        self.url, self.token = server["url"], server["admin_token"]
        self.focus, self.rate_focus, self.rate_other, self.seconds = focus_channel, rate_focus, rate_other, seconds
        self.sent = 0
        self.errors = 0

    def api(self, method, path, body=None):
        req = urllib.request.Request(f"{self.url}/api/v4{path}", method=method,
                                     data=json.dumps(body).encode() if body is not None else None,
                                     headers={"Authorization": f"Bearer {self.token}", "Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=30) as resp:
            return json.loads(resp.read() or b"null")

    def run(self):
        team = self.api("GET", "/teams/name/bench")
        chans = self.api("GET", f"/users/me/teams/{team['id']}/channels")
        focus = next(c for c in chans if c["name"] == self.focus)
        others = [c for c in chans if c["type"] in "OP" and c["name"] != self.focus][:40]
        r = random.Random(7)
        jobs: queue.Queue = queue.Queue()
        end = time.monotonic() + self.seconds
        lines = ["Deploy finished, all green :tada:", "Anyone around to review a tiny PR?",
                 "```rust\nfn main() { println!(\"live\"); }\n```", "| a | b |\n|---|---|\n| 1 | 2 |",
                 "Heads up: the build queue is long right now 🚧", "lol 😂😂😂", "+1", "Looking now."]

        def worker():
            while True:
                item = jobs.get()
                if item is None:
                    return
                try:
                    post = self.api("POST", "/posts", {"channel_id": item, "message": r.choice(lines) + f" #{self.sent}"})
                    self.sent += 1
                    if r.random() < 0.3:
                        self.api("POST", "/reactions", {"user_id": post["user_id"], "post_id": post["id"],
                                                        "emoji_name": r.choice(["+1", "tada", "eyes", "rocket"])})
                except Exception:
                    self.errors += 1

        threads = [threading.Thread(target=worker, daemon=True) for _ in range(8)]
        for t in threads:
            t.start()
        next_focus = next_other = time.monotonic()
        while time.monotonic() < end:
            now = time.monotonic()
            if now >= next_focus:
                jobs.put(focus["id"])
                next_focus += 1 / self.rate_focus
            if now >= next_other:
                jobs.put(r.choice(others)["id"])
                next_other += 1 / self.rate_other
            time.sleep(0.005)
        for _ in threads:
            jobs.put(None)
        for t in threads:
            t.join(timeout=30)


# -- The journey -------------------------------------------------------------


class Session:
    def __init__(self, app: App, driver, run_dir: Path, server: dict, args):
        self.app, self.driver, self.args, self.server = app, driver, args, server
        self.dir = run_dir / app.key
        self.dir.mkdir(parents=True, exist_ok=True)
        self.events = open(self.dir / "events.jsonl", "a")
        self.seq = 0
        self.proc: subprocess.Popen | None = None
        self.sampler: Sampler | None = None
        self.scenario = ""
        self.rect = (WIN_X, WIN_Y, WIN_W, WIN_H)
        w, h, scale = driver.cmd("screen").split()
        self.screen = (int(w), int(h))
        self.scale = float(scale)
        self.recorder: Recorder | None = None

    # Input with a marker.
    def mark(self, name: str, op: str = "", **extra) -> int:
        self.seq += 1
        t = time.monotonic()
        self.driver.cmd(f"@{self.seq} {op}".strip())
        self.events.write(json.dumps({"seq": self.seq, "t": t, "name": name, "scenario": self.scenario, **extra}) + "\n")
        self.events.flush()
        return self.seq

    def do(self, op: str) -> None:
        self.driver.cmd(op)

    def wait(self, s: float) -> None:
        time.sleep(s)

    def cx(self, fx: float) -> int:
        return int(self.rect[0] + self.rect[2] * fx)

    def cy(self, fy: float) -> int:
        return int(self.rect[1] + self.rect[3] * fy)

    # Process control.
    def launch(self) -> None:
        self.proc = self.app.spawn()
        self.driver.cmd(f"target {self.proc.pid}")
        self.sampler = Sampler(self.proc.pid, self.dir / "samples.jsonl")
        self.sampler.scenario = self.scenario
        self.sampler.start()

    def wait_window(self, limit: float = 60) -> tuple[int, int, int, int]:
        deadline = time.monotonic() + limit
        while time.monotonic() < deadline:
            rects = self.driver.cmd(f"windows {self.proc.pid}").split()
            if rects:
                return tuple(int(v) for v in rects[0].split(","))
            time.sleep(0.05)
        raise RuntimeError(f"{self.app.label} showed no window within {limit}s")

    def after_launch(self) -> None:
        """Linux has no keychain under Xvfb, so Matty signs in on every launch there."""
        if MAC or self.app.key != "matty":
            return
        self.wait(4)
        m = self.server.get("manifest", {})
        self.do(f"type 20 {self.server['url']}")
        self.do("key return")
        self.wait(2.5)
        self.do(f"type 20 {m.get('server_user', 'bench')}")
        # Tab does not reach the password field there; click it.
        got = self.driver.cmd(f"windows {self.proc.pid}").split()
        x, y, w, h = (int(v) for v in got[0].split(",")) if got else self.rect
        self.do(f"click {x + w // 2} {y + int(h * 0.613)}")
        self.do(f"type 20 {m.get('password', 'Password1!')}")
        self.do("key return")

    def place(self) -> None:
        got = self.driver.cmd(f"place {self.proc.pid} {WIN_X} {WIN_Y} {WIN_W} {WIN_H}").split()
        self.rect = tuple(int(v) for v in got) if got and got[2] != "0" else (WIN_X, WIN_Y, WIN_W, WIN_H)
        self.show_marker()

    def show_marker(self) -> None:
        x, y, w, h = self.rect
        self.driver.cmd(f"markrect {x} {y + h + 4} {MARK_W} {MARK_H}")

    def quit(self) -> None:
        if not self.proc:
            return
        try:
            self.driver.cmd(f"front {self.proc.pid}")
            time.sleep(0.3)
            self.driver.cmd("key q cmd")
            self.proc.wait(timeout=15)
        except Exception:
            try:
                os.killpg(self.proc.pid, signal.SIGTERM)
                self.proc.wait(timeout=10)
            except Exception:
                os.killpg(self.proc.pid, signal.SIGKILL)
        # Electron's helpers can outlive the main process for a moment.
        deadline = time.monotonic() + 10
        while process_tree(self.proc.pid) and time.monotonic() < deadline:
            time.sleep(0.2)
        if self.sampler:
            self.sampler.stop()
            self.sampler = None
        self.proc = None

    # Recording per scenario.
    def begin(self, scenario: str) -> None:
        self.scenario = scenario
        if self.sampler:
            self.sampler.scenario = scenario
        print(f"  [{self.app.key}] {scenario}", flush=True)
        x, y, w, h = self.rect
        region = (x, y, max(w, MARK_W), h + 4 + MARK_H)
        meta = {"scenario": scenario, "rect": self.rect, "region": region, "scale": self.scale,
                "marker": [0, h + 4, MARK_W, MARK_H], "fps": self.args.fps}
        (self.dir / f"{scenario}.json").write_text(json.dumps(meta))
        self.recorder = Recorder(self.dir / f"{scenario}.mp4", region, self.scale, self.args.fps) if not self.args.no_video else None
        self.mark("begin")

    def end(self) -> None:
        self.mark("end")
        self.wait(0.4)
        if MAC:
            stolen = int(self.driver.cmd("steals") or 0)
            if stolen:
                # The driver gave the focus back each time, but the recording
                # shows the other app in between: rerun this scenario.
                print(f"  WARNING: another app took the focus {stolen}x during {self.scenario}")
                self.events.write(json.dumps({"name": "focus-stolen", "scenario": self.scenario,
                                              "count": stolen}) + "\n")
                self.events.flush()
        if self.recorder:
            self.recorder.stop()
            self.recorder = None
        if self.proc:
            fp = footprint(list(process_tree(self.proc.pid)))
            tree = process_tree(self.proc.pid)
            with open(self.dir / "memory.jsonl", "a") as f:
                f.write(json.dumps({"scenario": self.scenario, "t": time.monotonic(), "footprint_mb": fp,
                                    "rss_mb": sum(v[0] for v in tree.values()) / 1024, "procs": len(tree)}) + "\n")

    def switch_to(self, display: str, mark: bool = True, settle: float = 2.5) -> None:
        """Quick switcher (Cmd+K), the channel's name, Enter; the Enter is marked."""
        self.do("key k cmd")
        self.wait(0.7)
        self.do(f"type 15 {display}")
        self.wait(1.2)  # let the switcher's list catch up, or Enter searches instead
        if mark:
            self.mark("nav", "key return", target=display)
        else:
            self.do("key return")
        self.wait(settle)

    def scroll(self, name: str, dy: int, seconds: float, interval_ms: float = 8) -> None:
        count = int(seconds * 1000 / interval_ms)
        self.mark(f"{name}-start", f"scroll {dy} {count} {interval_ms}", dy=dy, seconds=seconds)
        self.mark(f"{name}-end")


def run_app(app: App, driver, run_dir: Path, server: dict, args) -> None:
    s = Session(app, driver, run_dir, server, args)
    scen = args.scenarios
    app.prepare(server["url"])
    print(f"{app.label} {app.version}")

    # Warm-up launch: place the window where every later launch will open it,
    # land on Town Square, quit cleanly so the app saves its state.
    s.scenario = "warmup"
    log_mark = matty_log_size(app)
    s.launch()
    s.wait_window()
    s.after_launch()
    s.wait(8)
    s.place()
    s.do(f"front {s.proc.pid}")
    s.wait(1)
    s.switch_to("Town Square", mark=False, settle=4)
    if MAC:
        try:
            check_signed_in(app, s.rect, s.dir / "warmup.png", log_mark)
        except SystemExit:
            s.quit()  # a leftover copy would take over the next run's launches
            raise
    s.quit()
    s.wait(3)

    if "startup" in scen:
        for i in range(args.starts):
            s.begin(f"startup-{i + 1}")
            s.mark("launch")
            s.launch()
            try:
                s.wait_window()
                s.after_launch()
            finally:
                s.wait(args.startup_wait)
            s.end()
            if i < args.starts - 1:
                s.quit()
                s.wait(3)
    else:
        s.launch()
        s.wait_window()
        s.after_launch()
        s.wait(args.startup_wait)
    got = s.driver.cmd(f"windows {s.proc.pid}").split()
    if got:
        s.rect = tuple(int(v) for v in got[0].split(","))
    if s.rect[2:] != (WIN_W, WIN_H):
        s.place()
    s.show_marker()
    s.do(f"front {s.proc.pid}")
    s.wait(0.5)

    if "idle" in scen:
        s.begin("idle")
        s.wait(args.idle)
        s.end()

    if "switch" in scen:
        targets = ["Firehose", "Markdown Zoo", "Long Reads", "Emoji Storm", "Gallery", "Megathread",
                   "Eng General 00", "Design General 01", "Infra General 03", "Off-Topic"]
        s.switch_to("Town Square", mark=False, settle=1.5)
        s.begin("switch")
        for rnd in ("first", "again"):
            for t in targets:
                s.do("key k cmd")
                s.wait(0.6)
                s.do(f"type 15 {t}")
                s.wait(1.2)
                s.mark("nav", "key return", target=t, round=rnd)
                s.wait(2.0)
        s.end()

    if "sidebar" in scen:
        s.switch_to("Town Square", mark=False, settle=1.5)
        s.begin("sidebar")
        for _ in range(15):
            s.mark("nav", "key down alt")
            s.wait(1.2)
        for _ in range(15):
            s.mark("nav", "key up alt")
            s.wait(1.2)
        s.end()

    if "scroll" in scen:
        s.switch_to("Firehose", mark=False, settle=3)
        s.do(f"move {s.cx(0.6)} {s.cy(0.45)}")
        s.begin("scroll")
        s.scroll("up", 25, args.scroll_seconds)
        s.wait(1.5)
        s.scroll("down", -25, args.scroll_seconds / 2)
        s.wait(1.5)
        s.scroll("fling", 80, 5, interval_ms=8)
        s.wait(1.5)
        s.end()

    if "emoji" in scen:
        s.switch_to("Emoji Storm", mark=False, settle=3)
        s.do(f"move {s.cx(0.6)} {s.cy(0.45)}")
        s.begin("emoji")
        s.scroll("up", 25, 10)
        s.wait(1.5)
        s.scroll("down", -25, 5)
        s.wait(1.5)
        s.end()

    if "gallery" in scen:
        s.switch_to("Town Square", mark=False, settle=1.5)
        s.begin("gallery")
        s.do("key k cmd")
        s.wait(0.6)
        s.do("type 15 Gallery")
        s.wait(1.2)
        s.mark("nav", "key return", target="Gallery")
        s.wait(5)
        s.do(f"move {s.cx(0.6)} {s.cy(0.45)}")
        s.scroll("up", 25, 12)
        s.wait(2)
        s.scroll("down", -25, 6)
        s.wait(2)
        s.end()

    if "thread" in scen:
        s.switch_to("Megathread", mark=False, settle=3)
        s.begin("thread")
        s.mark("nav", "key up shift", target="megathread")
        s.wait(6)
        s.do(f"move {s.cx(0.86)} {s.cy(0.5)}")
        s.scroll("up", 25, 10)
        s.wait(1.5)
        s.scroll("down", -25, 5)
        s.wait(1.5)
        s.end()
        s.do("key escape")
        s.wait(0.5)

    if "live" in scen:
        s.switch_to("Town Square", mark=False, settle=2)
        s.begin("live")
        s.mark("traffic-start")
        traffic = Traffic(server, "town-square", args.live_focus_rate, args.live_other_rate, args.live)
        traffic.start()
        traffic.join(args.live + 30)
        s.mark("traffic-end", sent=traffic.sent, errors=traffic.errors)
        s.wait(3)
        s.end()

    if "typing" in scen:
        s.switch_to("Quiet 00", mark=False, settle=2)
        s.begin("typing")
        text = "The quick brown fox jumps over the lazy dog, then types some more."
        for ch in text:
            s.mark("key", f"char {'space' if ch == ' ' else ch}", ch=ch)
            s.wait(0.12)
        s.wait(0.8)
        s.do("key a cmd")
        s.wait(0.2)
        s.do("key delete")
        s.wait(1)
        s.end()

    if "settle" in scen:
        s.switch_to("Town Square", mark=False, settle=2)
        s.begin("settle")
        s.wait(args.idle)
        s.end()

    s.quit()
    s.events.close()


# -- Entry -------------------------------------------------------------------


def default_apps() -> dict[str, Path]:
    if MAC:
        return {"matty": Path("/Applications/Matty.app"), "mattermost": Path("/Applications/Mattermost.app")}
    return {"matty": HERE.parent / "target" / "release" / "matty"}


def make_apps(args) -> list[App]:
    paths = default_apps()
    for spec in args.app or []:
        k, _, v = spec.partition("=")
        paths[k] = Path(v).expanduser()
    apps = []
    profiles = OUT / "profiles"
    for key in args.apps.split(","):
        path = paths.get(key)
        if not path or not path.exists():
            sys.exit(f"{key}: {path} not found; pass --app {key}=/path/to/App.app")
        cls, label = {"matty": (Matty, "Matty"), "mattermost": (Mattermost, "Mattermost Desktop")}[key]
        app = cls(key, label, path, profiles / key)
        app.profile.mkdir(parents=True, exist_ok=True)
        app.version = app_version(app)
        apps.append(app)
    return apps


def load_server(args) -> dict:
    state = OUT / "server.json"
    if not state.exists():
        sys.exit("bench/out/server.json is missing: run bench/server up && bench/server import first")
    server = json.loads(state.read_text())
    try:
        urllib.request.urlopen(server["url"] + "/api/v4/system/ping", timeout=5)
    except Exception:
        sys.exit(f"{server['url']} does not answer: bench/server up")
    return server


def matty_log(app: App) -> Path | None:
    logs = sorted((getattr(app, "home", app.profile) / "Library" / "Logs" / "Matty").glob("*.log"))
    return logs[-1] if logs else None


def matty_log_size(app: App) -> tuple[Path | None, int]:
    log = matty_log(app)
    return log, (log.stat().st_size if log else 0)


def check_signed_in(app: App, rect, shot: Path, log_mark=(None, 0)) -> None:
    """Stops the run when the app is not where setup left it (signed in, on
    Town Square): the warm-up screenshot is compared, coarsely, with setup's
    signed-in.png. A sign-in screen differs from Town Square by far more than
    new posts do."""
    ref = app.profile / "signed-in.png"
    x, y, w, h = rect
    subprocess.run(["screencapture", "-x", "-R", f"{x},{y},{w},{h}", str(shot)], check=False)
    if not ref.exists() or not shot.exists():
        print(f"  (no {ref.name} to compare with; check {shot} yourself)")
        return
    from PIL import Image, ImageChops, ImageStat

    def small(p: Path):
        return Image.open(p).convert("L").resize((64, 40))

    diff = ImageStat.Stat(ImageChops.difference(small(ref), small(shot))).mean[0]
    print(f"  {app.label}: warm-up screen vs signed-in.png differ by {diff:.0f}/255")
    # Only what this launch logged: older lines may predate a fixed setup.
    log = matty_log(app)
    text = ""
    if log:
        with open(log, "rb") as f:
            f.seek(log_mark[1] if log == log_mark[0] else 0)
            text = f.read().decode(errors="replace")
    lost = [l for l in text.splitlines() if "could not save the session" in l or "could not read the saved session" in l]
    if lost:
        sys.exit(f"{app.label} cannot keep its session, so later launches open signed out:\n  {lost[-1]}")
    if diff > 35:
        sys.exit(f"{app.label} does not look signed in on Town Square (see {shot} and {ref}). "
                 f"Run `bench/run.py setup` again, then the run.")


def refuse_running(apps) -> None:
    """A copy of an app under test that is already running (left by an
    interrupted run) would take over the launches: stop instead."""
    for app in apps:
        cmd = app.command()
        found = subprocess.run(["pgrep", "-f", cmd[0]], capture_output=True, text=True).stdout.split()
        if found:
            sys.exit(f"{app.label} ({cmd[0]}) is already running as pid {', '.join(found)}; quit it first")


def setup(args, driver) -> None:
    """Fresh profiles, signed in as `bench`, windows where the run expects them.

    By default it signs in by itself (typing into each app's sign-in screen)
    and saves a screenshot of the result in bench/out/profiles/<app>/signed-in.png
    to check; --manual waits for you to sign in instead.
    """
    server = load_server(args)
    m = server.get("manifest", {})
    user, password = m.get("server_user", "bench"), m.get("password", "Password1!")
    apps = make_apps(args)
    refuse_running(apps)
    for app in apps:
        app.prepare(server["url"])
        proc = app.spawn()
        deadline = time.monotonic() + 60
        rect = None
        while time.monotonic() < deadline and not rect:
            got = driver.cmd(f"windows {proc.pid}").split()
            rect = tuple(int(v) for v in got[0].split(",")) if got else None
            time.sleep(0.2)
        time.sleep(3)
        got = driver.cmd(f"place {proc.pid} {WIN_X} {WIN_Y} {WIN_W} {WIN_H}").split()
        x, y, w, h = (int(v) for v in got) if got and got[2] != "0" else (WIN_X, WIN_Y, WIN_W, WIN_H)
        driver.cmd(f"front {proc.pid}")
        time.sleep(0.5)
        if args.manual:
            print(f"\n{app.label}: sign in to {server['url']} as `{user}`, password `{password}`.")
            print("Dismiss any welcome tips, open Town Square, then press Enter here.")
            input()
        elif app.key == "matty":
            driver.cmd(f"type 20 {server['url']}")
            driver.cmd("key return")
            time.sleep(3)
            driver.cmd(f"type 20 {user}")
            driver.cmd(f"click {x + w // 2} {y + int(h * 0.613)}")
            driver.cmd(f"type 20 {password}")
            driver.cmd("key return")
            time.sleep(10)
        else:
            # The webapp's sign-in page focuses the username field itself. No
            # click: the middle of the window is the page's hero, whose
            # "create an account" link would leave the sign-in page.
            time.sleep(12)
            driver.cmd(f"type 20 {user}")
            driver.cmd("key tab")
            driver.cmd(f"type 20 {password}")
            driver.cmd("key return")
            time.sleep(12)
        shot = app.profile / "signed-in.png"
        if MAC:
            subprocess.run(["screencapture", "-x", "-R", f"{x},{y},{w},{h}", str(shot)], check=False)
            print(f"{app.label}: check {shot}")
        try:
            driver.cmd("key q cmd")
            proc.wait(timeout=15)
        except Exception:
            os.killpg(proc.pid, signal.SIGTERM)
    print("\nProfiles ready in bench/out/profiles. Check the signed-in screenshots, then run bench/run.py.")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("command", nargs="?", default="run", choices=["run", "setup", "check"])
    ap.add_argument("--apps", default="matty,mattermost" if MAC else "matty")
    ap.add_argument("--app", action="append", help="KEY=PATH, e.g. matty=~/src/matty/target/release/matty")
    ap.add_argument("--scenarios", default=",".join(SCENARIOS))
    ap.add_argument("--starts", type=int, default=5, help="cold starts per app (default 5)")
    ap.add_argument("--startup-wait", type=float, default=10)
    ap.add_argument("--idle", type=float, default=20)
    ap.add_argument("--scroll-seconds", type=float, default=15)
    ap.add_argument("--live", type=float, default=30, help="seconds of live traffic")
    ap.add_argument("--live-focus-rate", type=float, default=4, help="posts/s into the open channel")
    ap.add_argument("--live-other-rate", type=float, default=25, help="posts/s into other channels")
    ap.add_argument("--fps", type=int, default=60)
    ap.add_argument("--no-video", action="store_true")
    ap.add_argument("--out", type=Path)
    ap.add_argument("--no-analyze", action="store_true")
    ap.add_argument("--manual", action="store_true", help="setup: sign in by hand instead of typing")
    args = ap.parse_args()
    args.scenarios = set(args.scenarios.split(","))

    driver = MacDriver() if MAC else X11Driver()
    try:
        if args.command == "check":
            state = driver.cmd("check")
            print("permissions:", state, "| screen (points, scale):", driver.cmd("screen"))
            if MAC and ("ax=0" in state or "screen=0" in state):
                print("asking for Accessibility and Screen Recording:", driver.cmd("request"))
                print("Grant both in System Settings > Privacy & Security to the app this runs in, then run check again.")
            return
        if args.command == "setup":
            setup(args, driver)
            return
        server = load_server(args)
        refuse_running(make_apps(args))
        run_dir = args.out or OUT / "runs" / dt.datetime.now().strftime("%Y%m%d-%H%M%S")
        run_dir.mkdir(parents=True, exist_ok=True)
        apps = make_apps(args)
        info = {"started": dt.datetime.now().isoformat(timespec="seconds"), "host": platform.node(),
                "machine": platform.machine(), "os": platform.platform(), "server": server["url"],
                "corpus": server.get("manifest", {}), "apps": {a.key: {"label": a.label, "version": a.version,
                                                                        "path": str(a.path)} for a in apps},
                "args": {k: (sorted(v) if isinstance(v, set) else str(v) if isinstance(v, Path) else v)
                         for k, v in vars(args).items()}}
        if MAC:
            hw = subprocess.run(["sysctl", "-n", "machdep.cpu.brand_string", "hw.memsize"], capture_output=True, text=True).stdout.split("\n")
            info["cpu"], info["memory_gb"] = hw[0], round(int(hw[1]) / 2**30) if len(hw) > 1 and hw[1] else None
        (run_dir / "run.json").write_text(json.dumps(info, indent=2))
        print(f"run → {run_dir}")
        for app in apps:
            run_app(app, driver, run_dir, server, args)
            time.sleep(5)
    finally:
        driver.close()
    if not args.no_analyze:
        subprocess.run([sys.executable, str(HERE / "analyze.py"), str(run_dir)], check=False)
        if not args.no_video:
            subprocess.run([sys.executable, str(HERE / "compose.py"), str(run_dir)], check=False)


if __name__ == "__main__":
    main()
