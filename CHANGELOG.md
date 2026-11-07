# Changelog

Every release of Matty, newest first. A release PR adds the entry and bumps
`version` in `Cargo.toml` in the same commit; merging it publishes the
release ([docs/releasing.md](docs/releasing.md)). The entry becomes the top
of the GitHub release notes. Headings follow
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/): Added, Changed,
Fixed, Removed.

## [Unreleased]

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

[Unreleased]: https://github.com/karyalabs/matty/compare/v0.1.0-beta.7...HEAD
[0.1.0-beta.7]: https://github.com/karyalabs/matty/releases/tag/v0.1.0-beta.7
[0.1.0-beta.6]: https://github.com/karyalabs/matty/releases/tag/v0.1.0-beta.6
[0.1.0-beta.5]: https://github.com/karyalabs/matty/releases/tag/v0.1.0-beta.5
