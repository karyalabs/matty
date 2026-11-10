<p align="center">
  <img src="packaging/icons/matty-icon.svg" width="96" alt="Matty icon">
</p>

<h1 align="center">Matty</h1>

<p align="center">
  A fast, native <a href="https://mattermost.com">Mattermost</a> client for macOS, Linux, Windows and the web.
</p>

<p align="center">
  <a href="https://github.com/karyalabs/matty/releases"><b>Download</b></a> ·
  <a href="https://karyalabs.dev/matty/"><b>Open in your browser</b></a> ·
  <a href="https://demo.karyalabs.dev/matty/"><b>Try the demo</b></a> ·
  <a href="CHANGELOG.md">Changelog</a>
</p>

<a href="https://raw.githubusercontent.com/karyalabs/assets/main/matty/readme/channel-light.png"><picture><source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/karyalabs/assets/main/matty/readme/channel-dark.png"><img src="https://raw.githubusercontent.com/karyalabs/assets/main/matty/readme/channel-light.png" alt="Matty showing the #general channel" width="100%"></picture></a>

Matty is written in Rust on [GPUI](https://www.gpui.rs), the GPU-accelerated
UI framework behind Zed, with the
[gpuicn](https://github.com/karyalabs/gpuicn) component library. It is one
process with no browser engine inside. It opens in a fraction of a second,
switches channels without a spinner, and keeps working offline.

> **Status: beta.** New betas ship about once a week for every platform, and
> the web app updates with each one. The plan of record is
> [docs/PLAN.md](docs/PLAN.md), and work is tracked in
> [GitHub issues](https://github.com/karyalabs/matty/issues). Matty is an
> unofficial client, not made or endorsed by Mattermost, Inc.

## Get Matty

| Platform | How |
| --- | --- |
| **macOS** 13 or later, Apple silicon and Intel | `Matty-<version>.dmg` from [Releases](https://github.com/karyalabs/matty/releases). Signed and notarized, and it updates itself. |
| **Linux** x86_64 | `.deb`, `.rpm`, AppImage or tarball from [Releases](https://github.com/karyalabs/matty/releases). |
| **Windows** x86_64 | The `.msi` from [Releases](https://github.com/karyalabs/matty/releases). It installs for you alone, with no administrator prompt. |
| **Web** | <https://karyalabs.dev/matty/> for your own server, after a one-time [server setting](docs/web.md#what-the-server-needs). |
| **Just looking** | <https://demo.karyalabs.dev/matty/> opens a sample workspace in your browser. No account, no server. |

More on each, including checksums, is under [Install](#install).

## A tour

<table>
<tr>
<td width="50%" valign="top">
<a href="https://raw.githubusercontent.com/karyalabs/assets/main/matty/readme/peek-light.png"><picture><source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/karyalabs/assets/main/matty/readme/peek-dark.png"><img src="https://raw.githubusercontent.com/karyalabs/assets/main/matty/readme/peek-light.png" alt="A peek card over the sidebar showing unread messages in town-square" width="100%"></picture></a>
<br><b>Peek before you switch.</b> Rest on an unread channel to read what is new, then mark it read or open it.
</td>
<td width="50%" valign="top">
<a href="https://raw.githubusercontent.com/karyalabs/assets/main/matty/readme/thread-light.png"><picture><source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/karyalabs/assets/main/matty/readme/thread-dark.png"><img src="https://raw.githubusercontent.com/karyalabs/assets/main/matty/readme/thread-light.png" alt="A thread open beside its channel" width="100%"></picture></a>
<br><b>Threads beside the channel.</b> Reply and follow without losing your place.
</td>
</tr>
<tr>
<td width="50%" valign="top">
<a href="https://raw.githubusercontent.com/karyalabs/assets/main/matty/readme/switcher-light.png"><picture><source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/karyalabs/assets/main/matty/readme/switcher-dark.png"><img src="https://raw.githubusercontent.com/karyalabs/assets/main/matty/readme/switcher-light.png" alt="The quick switcher listing unread channels across teams" width="100%"></picture></a>
<br><b>Jump anywhere.</b> Ctrl K (⌘K on a Mac) finds any channel or person, across teams and servers, unreads first.
</td>
<td width="50%" valign="top">
<a href="https://raw.githubusercontent.com/karyalabs/assets/main/matty/readme/threads-light.png"><picture><source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/karyalabs/assets/main/matty/readme/threads-dark.png"><img src="https://raw.githubusercontent.com/karyalabs/assets/main/matty/readme/threads-light.png" alt="The Threads view with a thread open" width="100%"></picture></a>
<br><b>All your threads in one view,</b> with an Unread filter and the thread open next to the list.
</td>
</tr>
</table>

<p align="center">
<a href="https://raw.githubusercontent.com/karyalabs/assets/main/matty/readme/appearance-light.png"><picture><source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/karyalabs/assets/main/matty/readme/appearance-dark.png"><img src="https://raw.githubusercontent.com/karyalabs/assets/main/matty/readme/appearance-light.png" alt="The Appearance settings with the style gallery and accent colors" width="720"></picture></a>
</p>

**Make it yours.** Pick a built-in style (Graphite is the default), follow
your Mattermost theme, or build your own from an accent color, greys and
corner shape. Bring your own fonts, then share the style as a file or a
snippet a teammate can paste.

The screenshots follow your GitHub theme, light or dark, and open at full
size when clicked. They come from the
preview fixtures (`MATTY_PREVIEW=1`, see [Building](#building)).

## Why it is fast

Measured on 9 November 2026 with the tools in [bench/](bench/README.md):
Matty 0.1.0-beta.16 and Mattermost Desktop 6.3.2 ran the same scripted
journey against the same local server holding 480,000 posts, on a MacBook
Pro (M4 Max).

| | Matty | Mattermost Desktop |
| --- | ---: | ---: |
| First window paint | 217 ms | 483 ms |
| Memory after start | 199 MB | 832 MB |
| Processes | 1 | 10 |
| CPU while idle | 0.0 % | 2.9 % |
| Switch channel (Alt+Up/Down, median) | 50 ms | 117 ms |
| Open a thread with 3,000 replies | 117 ms | 300 ms |
| CPU while scrolling 100,000 posts, both near 60 fps | 59 % | 110 % |
| Memory after that scroll | 699 MB | 1,651 MB |
| CPU at rest after the whole run | 0.8 % | 22.4 % |

Of the measurements that differed by 10% or more, Matty was ahead on 29 and
Mattermost Desktop on 9. Mattermost was ahead on typing latency in that run
(17 ms against 33 ms from keypress to glyph) and uses less GPU memory. The
move to the latest GPUI in beta.17 has since brought Matty's typing latency
to 17 ms. This is one machine and one run, and the benchmark is in the
repository so you can run it on yours.

## What works today

- **Sign in** with a password, MFA or single sign-on (GitLab, Google,
  Office 365, OpenID, SAML). Use several teams and servers, and switch from
  the tiles at the top of the sidebar.
- **Sidebar:** Unreads, Threads, Mentions, Drafts and Saved up top, then
  your own categories with favorites, unread and mention badges, "Show
  unread only", pills that point at unread channels above and below, and a
  peek card on unread rows. The window is full height with no title bar,
  and a hidden sidebar slides in from the left edge.
- **Messages:** Markdown with highlighted code blocks and tables, reactions,
  image and file attachments with a lightbox, uploads and downloads, edit
  and delete, pin and save, copy text or a link, scheduled messages and
  "Mark as unread". A "New messages" line marks where you left off.
- **Threads:** collapsed reply threads, the Threads view, following, and a
  thread pane beside the channel.
- **Finding things:** search, the quick switcher, recent mentions, saved and
  pinned messages, channel files and drafts.
- **Offline first:** channels you have opened read from a local cache, and
  messages you send while offline wait in an outbox and go out in order
  when you reconnect.
- **Status:** online, away, offline, custom status, and Do not disturb for a
  while or until a time.
- **Settings** in its own window, with search: appearance, text size, 12 or
  24-hour clock, Enter or Ctrl+Enter to send, standard or compact messages,
  notifications, servers and keyboard shortcuts.
- **Keyboard and screen readers:** Mattermost's shortcuts for navigation and
  message actions, one tab stop per list, and an accessible name on every
  control.

The full scope by milestone is in [docs/PLAN.md](docs/PLAN.md), and each
release is described in the [changelog](CHANGELOG.md).

## Install

Every build is on
[GitHub Releases](https://github.com/karyalabs/matty/releases). They are
betas for now, so GitHub marks them as pre-releases. Each release lists its
files' SHA-256 checksums in `SHA256SUMS`:

```sh
shasum -a 256 -c SHA256SUMS --ignore-missing
```

**macOS.** Open `Matty-<version>.dmg` and drag Matty to Applications.
Releases are signed and notarized, so macOS opens them without a warning,
and Matty updates itself from signed releases. Your sign-in tokens stay in
the keychain under "Matty" until you sign out. A Homebrew cask
(`karyalabs/matty/matty`) arrives with the first stable release; it is not
published for betas.

**Linux.** Pick the package for your distribution:

```sh
sudo apt install ./matty_<version>_amd64.deb     # Debian, Ubuntu
sudo dnf install ./matty-<version>.x86_64.rpm    # Fedora, RHEL
chmod +x Matty-<version>-x86_64.AppImage && ./Matty-<version>-x86_64.AppImage
```

There is a plain tarball too. To update, install the newer package.

**Windows.** Run `Matty-<version>-x86_64.msi`. It is not code signed yet,
so SmartScreen may warn: choose **More info**, then **Run anyway**. To
update, run the newer installer.

**Web.** <https://karyalabs.dev/matty/> is the same app compiled to
WebAssembly. A server admin has to allow that origin once, as
[docs/web.md](docs/web.md) explains. <https://demo.karyalabs.dev/matty/> is
a sample workspace that talks to no server.

### Supported servers

The current Mattermost release and the extended support releases still
getting patches. CI tests against Mattermost 12.0, 11.7, 10.11 and 10.5.

### Known limitations

- Webapp plugins (Boards, Playbooks and other plugin UIs) can't run in a
  native client. Their posts show a card with "Open in browser".
- Calls isn't supported; call posts open in the browser.
- The System Console isn't in Matty; use the web app.

### Reporting problems

[Open an issue](https://github.com/karyalabs/matty/issues/new) with your
Matty version (Settings → General shows it, and so does `matty --version`),
your operating system and your Mattermost server version. Settings →
Advanced has a button that opens the logs folder. Logs leave out tokens and
message text by default, but look before you attach one.

## Building

Matty builds with the Rust toolchain pinned in `rust-toolchain.toml`
(rustup installs it on first build).

```sh
cargo run -p matty
```

Linux needs the X11, Wayland and fontconfig headers (on Debian or Ubuntu:
`libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libfontconfig-dev`).

To try the interface without a server, run it with the preview fixtures:

```sh
MATTY_PREVIEW=1 cargo run -p matty
```

On macOS, notifications only show from an app bundle. `scripts/dev-bundle`
builds an ad-hoc signed `target/Matty Dev.app` with its own bundle id and
URL schemes, so it can sit next to a release install:

```sh
scripts/dev-bundle && open "target/Matty Dev.app"
```

The web build is `scripts/web serve`; see [docs/web.md](docs/web.md).

## Development

- [docs/PLAN.md](docs/PLAN.md): goals, milestones and architecture.
- [docs/DESIGN.md](docs/DESIGN.md): the visual design and its rules.
- [docs/engineering-process.md](docs/engineering-process.md): how CI,
  releases and the Karya Labs repositories fit together.
- [docs/dev-server.md](docs/dev-server.md): a local Mattermost server in
  Docker for development and integration tests.
- [CONTRIBUTING.md](CONTRIBUTING.md): how pull requests work here, including
  the screenshot rule for visible changes.

The workspace is split into crates: `matty-api` (REST and WebSocket client),
`matty-store` (SQLite cache), `matty-sync` (the sync engine, with no GPUI
dependency), `matty-markdown`, `matty-notify` (notification policy and
platform backends), `matty-update` (the updater), `matty-app` (the GPUI
interface) and `matty-web` (the browser build). `matty` is the binary.

## License

[MIT](LICENSE). Matty is a [Karya Labs](https://github.com/karyalabs)
project and is not affiliated with or endorsed by Mattermost, Inc.
