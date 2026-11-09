#!/usr/bin/env python3
"""Generate the benchmark corpus as a Mattermost bulk-import archive.

    bench/corpus.py                    # full size, about 300,000 posts
    bench/corpus.py --scale 0.05       # a quick one for trying the tools
    bench/corpus.py --out /tmp/corpus  # somewhere else

The corpus is deterministic: the same --seed and --scale always give the same
archive, so two benchmark runs months apart look at the same data. It writes
`<out>/corpus.zip` (import.jsonl plus a data/ folder of images) and
`<out>/manifest.json`, which names the channels and posts the benchmark
scenarios open. `bench/server import` loads the archive.

What is in it (at --scale 1):

- Team `bench` with 500 users, plus teams `ops` and `labs` so the team
  switcher has something in it. The benchmark signs in as `bench`
  (password `Password1!`), who is in every channel listed here.
- `firehose`: 100,000 posts over two years, the huge-history channel.
  Mixed lengths, Markdown, code, mentions, emoji and threads.
- `megathread`: a few ordinary threads and, last, one root with 3,000
  replies, the large-thread scenario.
- `emoji-storm`: 6,000 posts made mostly of emoji, each carrying 3 to 14
  reactions, half of them custom emoji.
- `gallery`: 2,500 posts with one to four JPEG images each, from a pool of
  320 distinct images between 640x400 and 4000x2500.
- `markdown-zoo`: 4,000 posts of tables, long code blocks, lists, quotes,
  headings and math.
- `long-reads`: 1,200 posts of 2,000 to 8,000 characters.
- 70 public and 15 private channels with 300 to 3,000 posts each, and 40
  quiet channels, so the sidebar is about 135 channels long.
- 60 direct and 10 group conversations, 100 to 1,500 posts each.
- 120 custom emoji.

Needs ffmpeg (for the images); otherwise only the Python standard library.
"""

from __future__ import annotations

import argparse
import json
import random
import shutil
import subprocess
import sys
import time
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
PASSWORD = "Password1!"
BENCH_USER = "bench"
ADMIN_USER = "benchadmin"
TEAM = "bench"
DAY = 86_400_000

FIRST = ["ada", "alan", "barbara", "brian", "dennis", "edsger", "frances",
         "grace", "guido", "hedy", "ivan", "jean", "ken", "linus", "margaret",
         "niklaus", "radia", "rob", "sophie", "tim", "ursula", "vint",
         "yukihiro", "anita", "bjarne", "carol", "donald", "elena", "fran",
         "gilbert", "hal", "ingrid", "james", "karen", "leslie", "mary",
         "noam", "olga", "peter", "quinn", "rosa", "steve", "tara", "uma",
         "victor", "wendy", "xavier", "yara", "zane", "priya", "arjun",
         "mei", "kenji", "amara", "lars", "sofia", "mateo", "aisha", "omar"]
LAST = ["lovelace", "turing", "liskov", "kernighan", "shannon", "ritchie",
        "dijkstra", "allen", "hopper", "rossum", "lamarr", "sutherland",
        "sammet", "thompson", "torvalds", "hamilton", "wirth", "perlman",
        "pike", "wilson", "cerf", "matsumoto", "borg", "stroustrup", "shaw",
        "knuth", "nakamura", "okafor", "fischer", "rossi", "garcia", "khan",
        "patel", "sharma", "iyer", "chen", "tanaka", "silva", "novak"]
POSITIONS = ["Engineer", "Senior Engineer", "Staff Engineer", "Designer",
             "Product Manager", "SRE", "Data Scientist", "Support Lead",
             "Engineering Manager", "QA", "Technical Writer", ""]

# System emoji names the server and both clients know.
REACTIONS = ["+1", "-1", "heart", "tada", "smile", "laughing", "joy", "rocket",
             "fire", "eyes", "100", "white_check_mark", "clap", "pray",
             "thinking", "sweat_smile", "sob", "wave", "star", "sparkles",
             "muscle", "ok_hand", "raised_hands", "bulb", "warning", "x",
             "heavy_check_mark", "coffee", "beers", "zap", "boom", "bug",
             "construction", "hourglass", "lock", "memo", "pushpin", "sunny",
             "rainbow", "pizza", "cake", "trophy", "gift"]
UNICODE = ["😀", "😂", "🥲", "😍", "🤔", "😎", "🙌", "👏", "🔥", "🚀", "🎉", "✅",
           "❌", "⚠️", "💡", "🐛", "🛠️", "📦", "📈", "📉", "🧪", "🧵", "🍕", "☕",
           "🌈", "⭐", "💯", "👀", "🙏", "💪", "🤝", "🫠", "🥳", "😅", "😭", "🤯",
           "🏳️‍🌈", "👩🏽‍💻", "🧑🏿‍🚀", "🇮🇳", "🇯🇵", "🇧🇷", "❤️", "💙", "💚", "💜"]

NOUNS = ["deploy", "build", "migration", "release", "test suite", "pipeline",
         "dashboard", "query", "index", "cache", "worker", "queue", "API",
         "schema", "config", "rollout", "feature flag", "load balancer",
         "cluster", "node pool", "certificate", "token", "webhook", "cron job",
         "benchmark", "profile", "flame graph", "memory leak", "race",
         "regression", "hotfix", "design doc", "RFC", "roadmap", "sprint",
         "retro", "on-call rotation", "incident", "postmortem", "runbook",
         "sidebar", "composer", "thread view", "search", "emoji picker",
         "notification", "websocket", "reconnect", "keychain", "installer"]
VERBS = ["shipped", "rolled back", "fixed", "broke", "refactored", "profiled",
         "merged", "reviewed", "benchmarked", "rewrote", "deleted", "renamed",
         "paged on", "documented", "pinned", "bumped", "split", "cached",
         "parallelised", "throttled", "retried", "deprecated", "measured"]
ADJ = ["flaky", "slow", "fast", "noisy", "stale", "huge", "tiny", "broken",
       "green", "red", "blocked", "urgent", "optional", "legacy", "new",
       "experimental", "stable", "weird", "surprising", "expensive", "cheap"]
OPENERS = ["Heads up:", "FYI", "Quick one:", "Hmm,", "Okay so", "Update:",
           "PSA:", "Question:", "Thanks!", "Nice.", "Ugh,", "Good news:",
           "Bad news:", "Small thing:", "Reminder:", "+1,", "lol", "Agreed,"]
TEMPLATES = [
    "{op} the {n} is {a} again.",
    "I {v} the {n} this morning, can someone sanity check?",
    "Who owns the {n}? It looks {a} since yesterday.",
    "{op} we {v} the {n}; numbers in the doc.",
    "The {a} {n} keeps timing out after the {n2} change.",
    "Can we pair on the {n} after standup?",
    "Just {v} the {n2}. Next up is the {n}.",
    "Is the {n} supposed to be this {a}?",
    "{op} {user} {v} the {n} and it's {a} now.",
    "Looks like the {n} and the {n2} disagree about the timeout.",
    "I'll take the {n}. Can you look at the {n2}?",
    "Pushed a fix for the {a} {n}. PR incoming.",
    "Not sure the {n} is worth it, the {n2} is {a} enough.",
    "Retro note: the {n} was {a}, we should have {v} it earlier.",
    "{op} reminder that the {n} freeze starts Friday.",
    "Anyone else seeing {a} {n} numbers on staging?",
]
LANGS = {
    "rust": ["fn {f}(input: &[u8]) -> Result<usize, Error> {{",
             "    let mut total = 0;", "    for chunk in input.chunks(64) {{",
             "        total += chunk.iter().filter(|b| **b == b'\\n').count();",
             "    }}", "    Ok(total)", "}}"],
    "python": ["def {f}(rows):", "    seen = set()", "    for row in rows:",
               "        if row.id in seen:", "            continue",
               "        seen.add(row.id)", "        yield transform(row)"],
    "go": ["func {f}(ctx context.Context, ids []string) error {{",
           "\tfor _, id := range ids {{",
           "\t\tif err := store.Touch(ctx, id); err != nil {{",
           "\t\t\treturn fmt.Errorf(\"touch %s: %w\", id, err)", "\t\t}}",
           "\t}}", "\treturn nil", "}}"],
    "typescript": ["export async function {f}(client: Client, id: string) {{",
                   "  const post = await client.getPost(id);",
                   "  if (!post) throw new Error(`missing ${{id}}`);",
                   "  return {{...post, seen: Date.now()}};", "}}"],
    "sql": ["SELECT c.name, count(*) AS posts", "FROM posts p",
            "JOIN channels c ON c.id = p.channel_id",
            "WHERE p.create_at > now() - interval '7 days'",
            "GROUP BY c.name ORDER BY posts DESC LIMIT 20;"],
    "bash": ["set -euo pipefail", "for host in $(cat hosts.txt); do",
             "  ssh \"$host\" 'sudo systemctl restart {f}' &", "done", "wait"],
}
LOREM = ("Performance work is mostly measurement. Before changing anything we "
         "write down what we expect to see, run the same journey three times, "
         "and keep the raw numbers next to the summary so anyone can check the "
         "arithmetic. When a result surprises us we assume the harness is wrong "
         "first, then the environment, and only then the code. A regression "
         "that only shows up on one machine is still a regression, but it is "
         "also a clue about where the time goes. Scrolling a long history "
         "touches layout, text shaping, image decoding and the cache all at "
         "once, so a single frame graph rarely tells the whole story. We "
         "profile, form a hypothesis, change one thing, and measure again. ").split()


class Gen:
    def __init__(self, seed: int, users: list[str], channels: list[str]):
        self.r = random.Random(seed)
        self.users = users
        self.channels = channels

    def pick(self, seq):
        return self.r.choice(seq)

    def sentence(self) -> str:
        r = self.r
        return r.choice(TEMPLATES).format(
            op=r.choice(OPENERS), n=r.choice(NOUNS), n2=r.choice(NOUNS),
            v=r.choice(VERBS), a=r.choice(ADJ), user="@" + r.choice(self.users))

    def emoji_bits(self, n: int, custom: list[str]) -> str:
        out = []
        for _ in range(n):
            x = self.r.random()
            if x < 0.45:
                out.append(self.r.choice(UNICODE))
            elif x < 0.8:
                out.append(f":{self.r.choice(REACTIONS)}:")
            else:
                out.append(f":{self.r.choice(custom)}:")
        return " ".join(out)

    def code(self) -> str:
        lang = self.r.choice(list(LANGS))
        f = self.r.choice(["flush", "reconcile", "load_page", "dedupe",
                           "warm_cache", "measure", "compact"])
        lines = [line.format(f=f) for line in LANGS[lang]]
        reps = self.r.choice([1, 1, 1, 2, 4, 8])
        body = "\n".join(lines * reps)
        return f"```{lang}\n{body}\n```"

    def table(self) -> str:
        cols = self.r.randint(3, 6)
        rows = self.r.randint(3, 14)
        head = "| " + " | ".join(self.r.choice(NOUNS).title() for _ in range(cols)) + " |"
        sep = "|" + "|".join([" --- "] * cols) + "|"
        body = ["| " + " | ".join(
            self.r.choice([f"{self.r.uniform(0, 999):.1f} ms", self.r.choice(ADJ),
                           f"`{self.r.choice(NOUNS)}`", f"**{self.r.randint(1, 99)}%**"])
            for _ in range(cols)) + " |" for _ in range(rows)]
        return "\n".join([head, sep, *body])

    def markdown(self, custom: list[str]) -> str:
        r = self.r
        kind = r.randrange(9)
        if kind == 0:
            return self.code()
        if kind == 1:
            return self.table()
        if kind == 2:
            return "\n".join(f"{'  ' * r.randint(0, 2)}- {self.sentence()}"
                             for _ in range(r.randint(3, 12)))
        if kind == 3:
            return "\n".join(f"{i + 1}. {self.sentence()}" for i in range(r.randint(3, 9)))
        if kind == 4:
            return "\n".join("> " + self.sentence() for _ in range(r.randint(1, 5)))
        if kind == 5:
            return (f"{'#' * r.randint(1, 4)} {r.choice(NOUNS).title()} {r.choice(['notes', 'plan', 'status'])}\n\n"
                    + "\n".join(f"- [{r.choice([' ', 'x'])}] {self.sentence()}" for _ in range(r.randint(2, 8))))
        if kind == 6:
            return f"{self.sentence()}\n\n```latex\n\\sum_{{i=1}}^{{n}} x_i^2 = \\frac{{n(n+1)(2n+1)}}{{6}}\n```"
        if kind == 7:
            return (f"**{r.choice(NOUNS).title()}** is *{r.choice(ADJ)}* and ~~{r.choice(ADJ)}~~ "
                    f"`{r.choice(NOUNS)}`; see [the doc](https://example.com/{r.randrange(10**6)}) "
                    f"and ~{r.choice(self.channels)}. {self.emoji_bits(2, custom)}")
        return self.sentence() + "\n\n---\n\n" + self.sentence()

    def long(self, lo: int, hi: int) -> str:
        target = self.r.randint(lo, hi)
        paras, n = [], 0
        while n < target:
            start = self.r.randrange(len(LOREM))
            words = [LOREM[(start + i) % len(LOREM)] for i in range(self.r.randint(40, 140))]
            para = " ".join(words).capitalize()
            paras.append(para)
            n += len(para)
        return "\n\n".join(paras)

    def message(self, custom: list[str]) -> str:
        r = self.r
        x = r.random()
        if x < 0.42:
            return self.sentence()
        if x < 0.62:
            return " ".join(self.sentence() for _ in range(r.randint(2, 4)))
        if x < 0.70:
            return f"{self.sentence()} {self.emoji_bits(r.randint(1, 3), custom)}"
        if x < 0.78:
            return self.markdown(custom)
        if x < 0.84:
            return self.long(400, 1600)
        if x < 0.90:
            return f"{self.sentence()}\n\n{self.code()}"
        if x < 0.95:
            return r.choice(["ok", "👍", "lgtm", "on it", "same", "ty!", "brb", "😂😂", "+1",
                             "done", "hmm", "nice", "🎉", "yep", "nope", "ack"])
        return (f"@{r.choice(self.users)} @{r.choice(self.users)} {self.sentence()} "
                f"https://example.com/issues/{r.randrange(10**5)}")


# -- Images (ffmpeg) -------------------------------------------------------

def ffmpeg(args: list[str]) -> None:
    subprocess.run(["ffmpeg", "-loglevel", "error", "-y", *args], check=True)


def make_images(data: Path, count: int, r: random.Random) -> list[str]:
    """`count` distinct JPEGs of photo-like sizes from ffmpeg's generators."""
    out = data / "gallery"
    out.mkdir(parents=True, exist_ok=True)
    sizes = [(640, 400), (800, 600), (1024, 768), (1280, 720), (1600, 1000),
             (1920, 1080), (1080, 1350), (2400, 1600), (4000, 2500)]
    weights = [6, 8, 8, 10, 10, 8, 5, 3, 1]
    sources = ["mandelbrot=s={w}x{h}:start_scale={s}:start_x={x}:start_y={y}:maxiter={it}",
               "gradients=s={w}x{h}:n={n}:seed={seed}:type={gt}",
               "cellauto=s={w}x{h}:rule={rule}:random_seed={seed}:scroll=0",
               "life=s={w}x{h}:mold=10:random_seed={seed}:ratio=0.{ratio}:death_color=#{c1}:life_color=#{c2}",
               "testsrc2=s={w}x{h}"]
    names = []
    for i in range(count):
        w, h = r.choices(sizes, weights)[0]
        src = sources[i % len(sources)].format(
            w=w, h=h, s=r.uniform(0.0005, 2.5), x=r.uniform(-1.8, 0.3), y=r.uniform(-0.6, 0.6),
            it=r.choice([128, 256, 512]), n=r.randint(2, 8), seed=r.randrange(2**31),
            gt=r.choice(["linear", "radial", "circular", "spiral"]), rule=r.randint(1, 255),
            ratio=r.randint(1, 9), c1="%06x" % r.randrange(2**24), c2="%06x" % r.randrange(2**24))
        name = f"gallery/img-{i:04d}.jpg"
        # A little noise and a soft blur so JPEG sizes look like photos.
        ffmpeg(["-f", "lavfi", "-i", src, "-frames:v", "1",
                "-vf", f"noise=alls={r.randint(4, 22)}:allf=t,gblur=sigma=0.6,eq=saturation={r.uniform(0.6, 1.6):.2f}",
                "-q:v", str(r.randint(2, 6)), str(data / name)])
        names.append(name)
        if i % 40 == 0:
            print(f"  images {i}/{count}", flush=True)
    return names


def make_squares(data: Path, folder: str, count: int, size: int, r: random.Random) -> list[str]:
    """`count` small square PNGs (avatars, custom emoji) in one ffmpeg run each kind."""
    out = data / folder
    out.mkdir(parents=True, exist_ok=True)
    ffmpeg(["-f", "lavfi", "-i", f"gradients=s={size}x{size}:n=4:speed=0.05:seed={r.randrange(2**31)}:rate=25",
            "-frames:v", str(count), "-vf", "noise=alls=8:allf=t", str(out / "%04d.png")])
    return [f"{folder}/{i + 1:04d}.png" for i in range(count)]


# -- Corpus ----------------------------------------------------------------

def scaled(n: int, scale: float, floor: int = 1) -> int:
    return max(floor, int(round(n * scale)))


def build(out: Path, scale: float, seed: int, image_count: int | None) -> None:
    r = random.Random(seed)
    if out.exists():
        shutil.rmtree(out)
    data = out / "stage" / "data"
    data.mkdir(parents=True)
    t0 = time.time()

    n_users = scaled(500, max(scale, 0.1), 20)
    usernames = [BENCH_USER, ADMIN_USER]
    seen = set(usernames)
    while len(usernames) < n_users:
        name = f"{r.choice(FIRST)}.{r.choice(LAST)}"
        if name in seen:
            name = f"{name}{r.randint(2, 99)}"
        if name not in seen:
            seen.add(name)
            usernames.append(name)
    people = usernames[2:]

    print("generating images…", flush=True)
    images = make_images(data, image_count or scaled(320, scale, 24), r)
    avatars = make_squares(data, "avatars", len(usernames), 256, r)
    n_emoji = scaled(120, scale, 12)
    emoji_imgs = make_squares(data, "emoji", n_emoji, 128, r)
    custom = [f"bench-{i:03d}" for i in range(n_emoji)]

    # Channels: (name, display, type, posts, kind)
    specials = [
        ("firehose", "Firehose", "O", scaled(100_000, scale, 500), "mixed"),
        ("megathread", "Megathread", "O", 40, "megathread"),
        ("emoji-storm", "Emoji Storm", "O", scaled(6_000, scale, 100), "emoji"),
        ("gallery", "Gallery", "O", scaled(2_500, scale, 40), "gallery"),
        ("markdown-zoo", "Markdown Zoo", "O", scaled(4_000, scale, 80), "markdown"),
        ("long-reads", "Long Reads", "O", scaled(1_200, scale, 30), "long"),
    ]
    regular = []
    topics = ["eng", "design", "product", "infra", "mobile", "web", "data", "sales",
              "support", "growth", "security", "qa", "docs", "research", "hiring"]
    for i in range(scaled(70, max(scale, 0.2), 8)):
        name = f"{topics[i % len(topics)]}-{['general', 'standup', 'alerts', 'random', 'review'][i // len(topics) % 5]}-{i:02d}"
        regular.append((name, name.replace("-", " ").title(), "O", scaled(r.randint(300, 3000), scale, 20), "mixed"))
    for i in range(scaled(15, max(scale, 0.2), 3)):
        regular.append((f"private-{i:02d}", f"Private {i:02d}", "P", scaled(r.randint(300, 2000), scale, 20), "mixed"))
    for i in range(scaled(40, max(scale, 0.2), 5)):
        regular.append((f"quiet-{i:02d}", f"Quiet {i:02d}", "O", r.randint(0, 12), "mixed"))
    channels = [("town-square", "Town Square", "O", scaled(2_000, scale, 50), "mixed"),
                ("off-topic", "Off-Topic", "O", scaled(1_500, scale, 30), "mixed"),
                *specials, *regular]
    gen = Gen(seed + 1, people, [c[0] for c in channels])

    lines: list[str] = []
    emit = lambda obj: lines.append(json.dumps(obj, ensure_ascii=False))  # noqa: E731
    emit({"type": "version", "version": 1})
    for name, img in zip(custom, emoji_imgs):
        emit({"type": "emoji", "emoji": {"name": name, "image": img}})
    for team, display in [(TEAM, "Benchmark Co"), ("ops", "Ops"), ("labs", "Labs")]:
        emit({"type": "team", "team": {"name": team, "display_name": display, "type": "O",
                                       "allow_open_invite": True}})
    for name, display, kind, _, _ in channels:
        emit({"type": "channel", "channel": {
            "team": TEAM, "name": name, "display_name": display, "type": kind,
            "header": f"{display}: {gen.sentence()}", "purpose": gen.sentence()}})
    for team in ["ops", "labs"]:
        for name in ["town-square", "off-topic", "alerts", "chatter"]:
            emit({"type": "channel", "channel": {"team": team, "name": name,
                                                 "display_name": name.replace("-", " ").title(), "type": "O"}})

    # Who is where: everyone is in the specials and town square; regular
    # channels get a random slice. The bench user is in everything.
    members: dict[str, list[str]] = {}
    for name, _, _, _, _ in channels:
        if name in ("town-square", "off-topic") or any(name == s[0] for s in specials):
            members[name] = list(usernames)
        else:
            members[name] = [BENCH_USER, ADMIN_USER] + r.sample(people, min(len(people), r.randint(8, 120)))
    by_user: dict[str, list[str]] = {u: [] for u in usernames}
    for ch, us in members.items():
        for u in us:
            by_user[u].append(ch)
    for i, (u, avatar) in enumerate(zip(usernames, avatars)):
        first, _, last = u.partition(".")
        teams = [{"name": TEAM, "roles": "team_user team_admin" if u == ADMIN_USER else "team_user",
                  "channels": [{"name": c, "roles": "channel_user"} for c in by_user[u]]}]
        if u in (BENCH_USER, ADMIN_USER) or i % 5 == 0:
            for team in ["ops", "labs"]:
                teams.append({"name": team, "roles": "team_user", "channels": [
                    {"name": c} for c in ["town-square", "off-topic", "alerts", "chatter"]]})
        emit({"type": "user", "user": {
            "username": u, "email": f"{u}@bench.example.com", "password": PASSWORD,
            "auth_service": "", "nickname": "", "first_name": first.title(),
            "last_name": last.rstrip("0123456789").title() if last else "Bench",
            "position": r.choice(POSITIONS), "locale": "en",
            "roles": "system_user system_admin" if u == ADMIN_USER else "system_user",
            "profile_image": avatar, "teams": teams,
            "military_time": "false", "use_markdown_preview": "false",
        }})

    now = int(time.time() * 1000) // DAY * DAY - DAY  # midnight yesterday, stable within a day
    manifest = {"seed": seed, "scale": scale, "server_user": BENCH_USER, "password": PASSWORD,
                "admin_user": ADMIN_USER, "team": TEAM, "channels": {}, "posts": 0}
    total = 0

    def reactions(base: int, lo: int, hi: int, custom_share: float = 0.3) -> list[dict]:
        out, used = [], set()
        for _ in range(r.randint(lo, hi)):
            emo = r.choice(custom) if r.random() < custom_share else r.choice(REACTIONS)
            for u in r.sample(people, min(len(people), r.choice([1, 1, 1, 2, 3, 5, 12]))):
                if (u, emo) not in used:
                    used.add((u, emo))
                    out.append({"user": u, "emoji_name": emo, "create_at": base + r.randint(1000, DAY)})
        return out

    for name, _, _, count, kind in channels:
        if count == 0:
            manifest["channels"][name] = 0
            continue
        span = DAY * (730 if count > 20_000 else r.randint(30, 400))
        start = now - span
        step = span // (count + 1)
        authors = [u for u in members[name] if u != ADMIN_USER] or people
        posted = 0
        t = start
        for i in range(count):
            t += max(1, int(step * r.uniform(0.2, 1.8)))
            post = {"team": TEAM, "channel": name, "user": r.choice(authors), "create_at": t}
            if kind == "emoji":
                if r.random() < 0.35:
                    post["message"] = gen.emoji_bits(r.randint(1, 3), custom)  # jumbo
                else:
                    post["message"] = f"{gen.emoji_bits(r.randint(4, 40), custom)} {gen.sentence() if r.random() < 0.4 else ''}".strip()
                post["reactions"] = reactions(t, 3, 14, 0.5)
            elif kind == "gallery":
                post["message"] = gen.sentence() if r.random() < 0.7 else ""
                post["attachments"] = [{"path": img} for img in r.sample(images, r.choice([1, 1, 1, 2, 3, 4]))]
                if r.random() < 0.3:
                    post["reactions"] = reactions(t, 1, 4)
            elif kind == "markdown":
                post["message"] = gen.markdown(custom)
            elif kind == "long":
                post["message"] = gen.long(2000, 8000)
            elif kind == "megathread":
                post["message"] = gen.sentence()
            else:
                post["message"] = gen.message(custom)
                if r.random() < 0.012:
                    post["attachments"] = [{"path": r.choice(images)}]
                if r.random() < 0.18:
                    post["reactions"] = reactions(t, 1, 5)
            replies = []
            if kind == "megathread" and i == count - 1:
                post["message"] = ("**The megathread.** Everything about the benchmark goes here. "
                                   + gen.sentence())
                n = scaled(3_000, max(scale, 0.1), 100)
                rt = t
                for _ in range(n):
                    rt += r.randint(5_000, 600_000)
                    reply = {"user": r.choice(authors), "message": gen.message(custom), "create_at": rt}
                    if r.random() < 0.15:
                        reply["reactions"] = reactions(rt, 1, 4)
                    if r.random() < 0.01:
                        reply["attachments"] = [{"path": r.choice(images)}]
                    replies.append(reply)
                manifest["megathread_replies"] = n
                manifest["megathread_root_message"] = post["message"]
            elif kind in ("mixed", "megathread") and r.random() < (0.9 if kind == "megathread" else 0.08):
                rt = t
                for _ in range(r.choice([1, 1, 2, 3, 5, 8, 15, 40]) if kind == "mixed" else r.randint(5, 60)):
                    rt += r.randint(10_000, 3_600_000)
                    replies.append({"user": r.choice(authors), "message": gen.message(custom), "create_at": rt})
            if replies:
                post["replies"] = replies
            emit({"type": "post", "post": post})
            posted += 1 + len(replies)
        manifest["channels"][name] = posted
        total += posted
        print(f"  {name}: {posted:,} posts", flush=True)

    for team in ["ops", "labs"]:
        for ch in ["town-square", "alerts", "chatter"]:
            t = now - 30 * DAY
            for _ in range(scaled(200, scale, 10)):
                t += r.randint(60_000, 6 * 3_600_000)
                emit({"type": "post", "post": {"team": team, "channel": ch, "user": BENCH_USER if r.random() < 0.1 else ADMIN_USER,
                                               "message": gen.message(custom), "create_at": t}})
                total += 1

    dms = r.sample(people, min(len(people), scaled(60, max(scale, 0.2), 5)))
    gms = [r.sample(people, r.randint(2, 6)) for _ in range(scaled(10, max(scale, 0.2), 2))]
    for other in dms:
        emit({"type": "direct_channel", "direct_channel": {"members": [BENCH_USER, other]}})
    for group in gms:
        emit({"type": "direct_channel", "direct_channel": {"members": [BENCH_USER, *group]}})
    for members_ in [[BENCH_USER, o] for o in dms] + [[BENCH_USER, *g] for g in gms]:
        t = now - DAY * r.randint(20, 300)
        n = scaled(r.randint(100, 1500), scale, 10)
        for _ in range(n):
            t += r.randint(30_000, 8 * 3_600_000)
            post = {"channel_members": members_, "user": r.choice(members_),
                    "message": gen.message(custom), "create_at": min(t, now)}
            if r.random() < 0.1:
                post["reactions"] = reactions(t, 1, 3)
            emit({"type": "direct_post", "direct_post": post})
        total += n
    manifest["dms"] = len(dms)
    manifest["gms"] = len(gms)
    manifest["posts"] = total
    manifest["users"] = len(usernames)
    manifest["custom_emoji"] = n_emoji
    manifest["images"] = len(images)

    stage = out / "stage"
    (stage / "import.jsonl").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"zipping {total:,} posts…", flush=True)
    with zipfile.ZipFile(out / "corpus.zip", "w", zipfile.ZIP_STORED) as z:
        z.write(stage / "import.jsonl", "import.jsonl")
        for path in sorted(data.rglob("*")):
            if path.is_file():
                z.write(path, str(path.relative_to(stage)))
    (out / "manifest.json").write_text(json.dumps(manifest, indent=2))
    shutil.rmtree(stage)
    size = (out / "corpus.zip").stat().st_size / 1e6
    print(f"done: {out / 'corpus.zip'} ({size:.0f} MB, {total:,} posts, {time.time() - t0:.0f}s)")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", type=Path, default=HERE / "out" / "corpus")
    ap.add_argument("--scale", type=float, default=1.0, help="multiplies every post count (default 1)")
    ap.add_argument("--seed", type=int, default=20261108)
    ap.add_argument("--images", type=int, help="distinct gallery images (default 320 × scale)")
    args = ap.parse_args()
    if not shutil.which("ffmpeg"):
        sys.exit("ffmpeg is needed for the corpus images (brew install ffmpeg / apt install ffmpeg)")
    build(args.out, args.scale, args.seed, args.images)


if __name__ == "__main__":
    main()
