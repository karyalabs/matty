# Changelog

Every release of Matty, newest first. A release PR adds the entry and bumps
`version` in `Cargo.toml` in the same commit; merging it publishes the
release ([docs/releasing.md](docs/releasing.md)). The entry becomes the top
of the GitHub release notes. Headings follow
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/): Added, Changed,
Fixed, Removed.

## [Unreleased]

## [0.1.0-beta.17] - 2026-11-10

Matty on the web starts faster and keeps up while typing, and the app
moves to the latest GPUI.

### Changed

- Matty is built on GPUI Kit 0.9.1, gpui-pre 0.3.12 and gpuicn 0.11.0
  (#830).
- Matty web caches the app in the browser and serves the page from that
  copy, with fewer round trips before the first channel (#817, #822).
- Matty web shows the first channel's counts and purpose at once (#826)
  and runs its startup constructors once (#827).
- Matty web uses the Mac keymap in Apple browsers (#815).
- The local time zone is looked up once a minute, not on every call
  (#820).

### Fixed

- Matty web decodes pictures in a worker and keeps messages laid out
  while typing (#819, #825).
- Matty web keeps the first sign-in when the browser quits soon after
  (#824).

## [0.1.0-beta.16] - 2026-11-09

The performance round from the beta.15 audit (#784): idle CPU, memory and
latency, with no change to what is on screen.

### Added

- The sign-in screen is laid out like the official clients (#779).

### Changed

- Memory stays bounded while reading back through history: posts beyond
  the window around the view are let go and loaded again when scrolled
  to (#786).
- Images drop their encoded bytes once decoded, and the lightbox decodes
  at the window's size; a memory-pressure warning flushes decoded images
  (#789, #794).
- Starting up: the launch server's sync begins before the window opens,
  and the system font list is gathered off the UI thread (#791).

### Fixed

- Presence and unread changes that alter nothing on screen no longer
  rebuild the sidebar, the switcher or the directory (#787).
- Each message body's Markdown source is made once, not every frame
  (#788).
- Transcript rows extend on append instead of being rebuilt, and
  profiles are looked up by name (#790).
- Idle repaints are gone: image passes wait for a frame, the caret stops
  blinking when idle, and the macOS sleep check runs less often (#795).
- Spinners and skeletons redraw 12 times a second instead of every frame
  (#792).
- The quick switcher gathers and lowercases its rows once per sidebar
  change, and does no row work in the switch frame or on an unchanged
  reopen (#793).
- The caret returns to the composer after a quick-switcher pick, so
  typing and Shift+Up work right away (#782).
- The add-reaction button on a message's reactions row shows only on
  hover, so a row of reactions costs nothing while idle (#796).

## [0.1.0-beta.15] - 2026-11-09

Fixes from the first benchmark against the official desktop app (#764).

### Added

- Code blocks are syntax highlighted (#777).

### Changed

- Messages show the way Mattermost shows them: line breaks stay as typed,
  tables are as wide as their content with a bold header, and headings,
  quotes and code blocks are spaced like the webapp (#773).

### Fixed

- Images take far less memory: they are decoded at the size they are
  shown, and ones off screen are let go, so a channel of photos no longer
  holds gigabytes (#774).
- Typing right after Cmd+K always reaches the quick switcher, and it no
  longer opens with the last pick's matches (#771).
- A code fence closed on a line with text after it no longer swallows
  the rest of the message (#773).

### Fixed

- Messages read the way Mattermost shows them (#773): each typed line
  break stays a line break, code blocks are syntax-highlighted, tables are
  as wide as their content with a bold header and cell rules, headings sit
  closer to their text, quotes are no longer faded, and a code fence with
  text after it (```` ``` #783 ````) ends the block.

## [0.1.0-beta.14] - 2026-11-09

### Changed

- Graphite is the default theme again, and the sign-in screen is no
  longer navy when the Mattermost theme is chosen (#768).

### Fixed

- Shortcuts such as Cmd+, keep working after a menu, popover or dialog
  closes, without a click in the composer first (#766).

## [0.1.0-beta.13] - 2026-11-08

### Fixed

- Link previews show their image when it redirects to another site or
  comes without an image type, as Figma's does (#756).

## [0.1.0-beta.12] - 2026-11-08

Fixes for the paper-cuts found in beta.11 (#748).

### Added

- Right-click a link to open it or copy its address; right-clicking no
  longer opens the link (#757).
- The sidebar channel menu has "Copy link" (#754).

### Fixed

- Clicking a control in a message (code block copy, Show more, a reaction,
  an image, a mention) no longer opens the thread (#749).
- Clicking the root of the open thread no longer empties the thread pane
  (#750).
- Pressing a reaction and moving the mouse no longer starts a text
  selection (#758).
- In Settings, the selected row keeps its look on hover (#751).
- Tooltips follow the theme instead of always being light (#752).
- The status tooltip in the sidebar is the same card as a reaction's (#753).
- SVG files preview (#755).
- Link previews show images that the site gives without a size, such as
  Figma's (#756).

## [0.1.0-beta.11] - 2026-11-08

A new Settings window, and a team switcher in place of the team rail.

### Added

- Settings opens in its own window (Cmd/Ctrl+,) with a search field and
  seven sections: General, Appearance, Messages, Notifications, Servers,
  Keyboard and Advanced. Keyboard lists every shortcut; Advanced can cap and
  clear the image and file cache (#740).
- Settings › Servers lists your servers and teams: switch to one, reorder
  servers, sign out or remove one, or join another team (#741).
- Appearance shows your styles as a gallery of cards. Changing a preset or
  your Mattermost theme saves your own copy; rename, delete and share saved
  styles from the gallery (#742).

### Changed

- The team rail is gone. Click the team name at the top of the sidebar to
  switch teams and servers; a badge there counts what waits in the others,
  and Cmd/Ctrl+Alt+1–9 and Cmd/Ctrl+1–9 still jump to a team or server (#741).
- Preferences is now Settings, and the keyboard shortcuts dialog is its
  Keyboard page (#740).

## [0.1.0-beta.10] - 2026-11-08

Matty now looks like your other Mattermost apps.

### Changed

- Matty uses your account's Mattermost theme by default, and Mattermost's
  Denim until your account's theme has loaded. Your saved choice in
  Preferences › Appearance stays as it was.

### Fixed

- Text on macOS no longer looks bold or semi-bold next to the official
  Mattermost client. Matty draws text without macOS font smoothing, as
  Mattermost does (#726).

## [0.1.0-beta.9] - 2026-11-07

Styles you can save and share, and any font you like.

### Added

- Choose any interface and code font installed on your computer, or add
  your own font files.
- Save several named styles and switch between them in Preferences ›
  Appearance › Styles. Share a style by copying it as text or saving it to a
  file, and paste or open one someone shared. Pasting a Mattermost theme's
  colors turns it into your own style, and "Make it my own" does the same
  for your account's Mattermost theme (#707). Saved and shared styles carry
  their interface and code fonts.

### Changed

- Windows uses the Matty icon for the app, its windows and the installer.
- gpuicn 0.9.13: "Show more" works everywhere, dragging and scrolling
  redraw less, and the shortcut sheet and delete dialog are built only
  while open.

## [0.1.0-beta.8] - 2026-11-07

A new look, and styles you can make your own (#698).

### Added

- Appearance's Colors is a gallery: Graphite, Porcelain, Your own and My
  Mattermost theme, each drawn as a small window in its colors. Your own
  opens an editor for the accent (swatches or a color picker), the greys
  and the corners, with a guard that keeps text readable.
- Matty > Check for updates… and a Check now row under Preferences >
  Advanced > Updates.

### Changed

- Matty is set in IBM Plex Sans and Mono, with Graphite and Porcelain
  color presets.
- "Overlap" is Matty's new app icon.
- The channel and the thread pane share one message row, so they look
  and behave the same.
- gpuicn 0.9.12.
- Matty on the web moved to <https://karyalabs.dev/matty/>, which opens the
  sign-in card for your own server, and the demo workspace to
  <https://demo.karyalabs.dev/matty/>. matty.jha.sh redirects to the demo.
  Mattermost servers that allowed `https://matty.jha.sh` for CORS need
  `https://karyalabs.dev` instead.

## [0.1.0-beta.7] - 2026-11-07

The jank and paper-cut review (#670): smoother scrolling, and the rough
edges found when comparing Matty with the official Mattermost clients.

### Added

- Clicking a message opens its thread. Links, buttons, reactions,
  attachments and selecting text work as before.
- Long messages collapse behind "Show more", and code blocks have a copy
  button.
- The thread pane's React button opens the full emoji picker, as in the
  channel.

### Changed

- Quick reactions are a row of your three most-used emoji in the hover
  toolbar, starting from 👍 😀 👋, instead of a named menu of six.
- Hovering a reaction shows the emoji large, who reacted, and whether a
  click adds or removes yours.
- The typing line sits under the composer, so no empty strip is kept above
  it.
- Scrolling is smoother: panes that didn't change are reused instead of
  rebuilt every frame, message menus and the hover toolbar are built only
  when used, and release builds optimize the drawing crates for speed.
- Matty on the web is faster, no longer freezes, and has a much bigger
  demo.
- gpuicn 0.9.11.

### Fixed

- The image viewer closes with Escape or a click outside the image.
- Emoji shortcodes such as `:clown_face:` show as emoji.
- A DM's custom status sits after the name, and hovering it shows the
  status text, so the "more" button no longer covers it.
- Sending while scrolled up jumps to your message.
- In the thread pane, linked images open in the image viewer, preview
  images shrink with the column, and the reply composer has the
  formatting row.

## [0.1.0-beta.6] - 2026-11-07

The first release cut through a release PR, and the first from the
karyalabs organization.

### Added

- Jumping to a permalink for a channel you're not in offers to join it.
- Groundwork for translations: the sidebar, channel view, right pane,
  threads, Preferences, slash commands, embeds and pop-out windows read
  their text from Fluent files (English only for now; #225).

### Changed

- The Threads header's "Mark all as read" is an icon with a tooltip.
- gpuicn 0.9.7, with its built-in strings following the app's locale.
- Repository, release and update links point at `karyalabs/matty`.
- Releases are cut by merging a release PR instead of pushing a tag; the
  version in `Cargo.toml` now carries the pre-release suffix.

## [0.1.0-beta.5] - 2026-11-05

The beta 3 review fixes (#612). Earlier betas: see the
[GitHub releases](https://github.com/karyalabs/matty/releases).

[Unreleased]: https://github.com/karyalabs/matty/compare/v0.1.0-beta.17...HEAD
[0.1.0-beta.17]: https://github.com/karyalabs/matty/releases/tag/v0.1.0-beta.17
[0.1.0-beta.16]: https://github.com/karyalabs/matty/releases/tag/v0.1.0-beta.16
[0.1.0-beta.15]: https://github.com/karyalabs/matty/releases/tag/v0.1.0-beta.15
[0.1.0-beta.14]: https://github.com/karyalabs/matty/releases/tag/v0.1.0-beta.14
[0.1.0-beta.13]: https://github.com/karyalabs/matty/releases/tag/v0.1.0-beta.13
[0.1.0-beta.12]: https://github.com/karyalabs/matty/releases/tag/v0.1.0-beta.12
[0.1.0-beta.11]: https://github.com/karyalabs/matty/releases/tag/v0.1.0-beta.11
[0.1.0-beta.10]: https://github.com/karyalabs/matty/releases/tag/v0.1.0-beta.10
[0.1.0-beta.9]: https://github.com/karyalabs/matty/releases/tag/v0.1.0-beta.9
[0.1.0-beta.8]: https://github.com/karyalabs/matty/releases/tag/v0.1.0-beta.8
[0.1.0-beta.7]: https://github.com/karyalabs/matty/releases/tag/v0.1.0-beta.7
[0.1.0-beta.6]: https://github.com/karyalabs/matty/releases/tag/v0.1.0-beta.6
[0.1.0-beta.5]: https://github.com/karyalabs/matty/releases/tag/v0.1.0-beta.5
