# Matty vs the official client: the benchmark

Local tools that run the same scripted stress journey in Matty and in the
official Mattermost desktop app against the same big local server, record
both screens, and turn the recordings into numbers and a side-by-side video.
They are not a CI job, on purpose: the numbers only mean something on a real
machine with a real GPU, with nothing else running.

```sh
bench/setup                       # checks tools, makes bench/.venv (numpy, Pillow)
bench/corpus.py                   # ~480,000 posts + 320 photos → bench/out/corpus/corpus.zip (1 min)
bench/server up                   # Mattermost 10.5 + Postgres on http://localhost:8075
bench/server import               # loads the corpus (20–40 min at full size)
bench/.venv/bin/python bench/run.py check   # input + screen-recording permissions
bench/.venv/bin/python bench/run.py setup   # once: sign both apps in as bench / Password1!
bench/.venv/bin/python bench/run.py         # the run (~12 min), then report + video
```

A run lands in `bench/out/runs/<timestamp>/`:

| File | What |
| --- | --- |
| `report.md` | The comparison table, per scenario, with which app is better and by how much. |
| `results.json` | Every number, including each individual channel switch and keypress. |
| `side-by-side.mp4` | 1920×1080: Matty left, official app right, cut from the same input frame, a running clock and the scenario's numbers under each side, a summary card at the end. |
| `<app>/<scenario>.mp4` | The raw screen recordings (window plus the marker strip). |
| `<app>/events.jsonl`, `samples.jsonl`, `memory.jsonl` | Inputs sent, CPU/RSS every 0.5s, memory at the end of each scenario. |
| `run.json` | Machine, OS, app versions, corpus size, options. |

`bench/analyze.py RUN` and `bench/compose.py RUN` redo the report and the
video from the raw files, so the measuring can be improved later and old runs
re-measured.

Before a run: quit your everyday Matty and Mattermost (the benchmark uses its
own profiles in `bench/out/profiles/`, but two copies of an app fight over the
keychain and single-instance locks), close other heavy apps, plug in power,
and leave the machine alone for the ~12 minutes it drives the screen.

## The corpus

`bench/corpus.py` writes a Mattermost bulk-import archive, deterministic for
a given `--seed` and `--scale`, so runs months apart look at the same data.
At `--scale 1`:

| Channel | Content | Used by |
| --- | --- | --- |
| `firehose` | 100,000 posts over two years plus ~25,000 thread replies: chat lines, paragraphs, Markdown, code, mentions, emoji | huge history scroll, switching |
| `megathread` | Some ordinary threads, then one root with 3,000 replies | large thread |
| `emoji-storm` | 6,000 posts of mostly emoji (Unicode, shortcodes, 120 custom), 3–14 reactions each | emoji and reactions |
| `gallery` | 2,500 posts with 1–4 JPEGs each, from 320 distinct images between 640×400 and 4000×2500 | images |
| `markdown-zoo` | 4,000 posts of tables, long code blocks, lists, quotes, headings, math | switching |
| `long-reads` | 1,200 posts of 2,000–8,000 characters | switching |
| 70 public + 15 private | 300–3,000 posts each | switching, sidebar |
| 40 `quiet-*` | Up to 12 posts | sidebar length, typing |
| DMs and GMs | 60 direct and 10 group conversations, 100–1,500 posts each | sidebar |

Plus 500 users with avatars, teams `ops` and `labs`, and `benchadmin`
(system admin) whose token posts the live traffic. Text is generated from
templates (not lorem ipsum), with realistic lengths: half are one-liners, a
few are walls of text or code.

The server (`bench/compose.yml`) is its own Compose project on port 8075,
apart from the dev server, with link previews, the GIF picker, onboarding
tours and the desktop landing page off, so neither app shows something the
other does not, and nothing reaches the internet.

## The journey

Each app runs the same script, one app at a time, with fixed timings, so
both recordings of a scenario are the same length:

| Scenario | What happens |
| --- | --- |
| `startup` | Five launches (app not running, OS file cache warm, already signed in, opens on Town Square), 10s each. |
| `idle` | 20s with nothing happening. |
| `switch` | Quick switcher (Cmd+K, type a name, Enter) through 10 channels, including the heavy ones, then the same 10 again (cached). |
| `sidebar` | Alt+Down 15 times, then Alt+Up 15 times, 1.2s apart. |
| `scroll` | In `firehose`: wheel up 15s at ~3,100 px/s, down 7.5s, then a 5s fling at ~10,000 px/s. |
| `emoji` | In `emoji-storm`: up 10s, down 5s. |
| `gallery` | Open `gallery` (first visit), wait 5s, up 12s, down 6s. |
| `thread` | In `megathread`: Shift+Up opens the 3,000-reply thread, then scroll the thread pane up 10s and down 5s. |
| `live` | 30s of traffic from `benchadmin`: 4 posts/s into the open channel, 25/s into 40 others, reactions on a third. |
| `typing` | 66 characters into the composer, one every 120ms, then cleared. |
| `settle` | 20s idle at the end: what is left of CPU and memory after the stress. |

`--scenarios scroll,thread` runs a subset, `--starts 10` more launches,
`--apps matty` one app, `--app matty=~/src/matty/target/release/matty` a local
build.

## How it is measured

Fair means the same instrument for both apps, so nothing relies on either
app's own logging.

**Latency from the video.** `bench/mac/driver.swift` sends every input with
CGEvent and, at the same instant, redraws a small barcode (18 cells: a white
anchor, 16 bits of a sequence number, a black anchor) in an always-on-top
panel just under the app window. ffmpeg records the window and the barcode
together at 60 fps. `bench/analyze.py` decodes the barcode in every frame, so
it knows the frame where each input happened, and compares each frame of the
app region with the one before it:

- *first change*: the first frame after the input where more than 0.15% of the
  window changed (for the quick switcher, usually the switcher closing);
- *ready*: the end of the last such change before the window stayed still for
  600ms (1s for launches). Spinners, placeholders and images popping in all
  push "ready" later, as they should;
- *frames shown per second* while scrolling: frames in which the picture moved,
  per second of input. 60 is perfect at this capture rate. *Longest freeze* is
  the longest stretch without a new picture while input kept coming;
  *hitches* count freezes of 50ms or more;
- *catch-up*: how long the picture keeps changing after the input stopped;
- *keypress to glyph*: the first change in the composer band after a key.

The resolution is one captured frame (16.7ms). A caret blink is too small to
count as a change.

**CPU and memory from the OS.** Every 0.5s, `ps` over the app's whole process
tree (Electron runs a main process, a GPU process, renderers and utilities;
Matty runs one): summed CPU time, so 100% is one core busy, and summed
resident memory. At the end of each scenario the macOS `footprint` tool gives
the physical footprint, which is what Activity Monitor shows; summed RSS
over-counts shared memory, which flatters neither app consistently, so the
report shows both.

**Caveats.** The screen recorder runs during the whole journey and costs both
apps the same. ProMotion displays draw at up to 120 Hz but the capture is 60
fps, so "60 fps" means at least 60. Wheel events are continuous pixel deltas,
like a trackpad without momentum. The server runs in Docker on the same
machine (on macOS, colima or Docker Desktop), which takes the same share from
both runs. A run is one sample; run it twice before believing a small
difference.

## Linux

The same scripts run Matty under Xvfb (xdotool for input, an X window for the
barcode, x11grab for video), which is how they were tested in the cloud. It
proves the harness, not the performance: Xvfb renders in software, and the
official app is not installed there.

```sh
Xvfb :99 -screen 0 1600x1000x24 & export DISPLAY=:99
bench/.venv/bin/python bench/run.py --app matty=target/release/matty
```
