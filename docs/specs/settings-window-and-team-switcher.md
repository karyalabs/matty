# Settings window and team switcher: implementation spec

Approved by Arya on 2026-11-08 (thread "Redesign Matty's settings screen"). Mockups:
https://claude.ai/artifact/EHhFFEKvpn5X5NpgqoguTy (sources in project files
`matty/settings-redesign/round-1/project/`). Decisions taken on the cards: Settings is
**its own window**; the team rail is **removed**, not kept as an option.

This document is the contract. The implementation thread (Opus 5.5) builds it as the
GitHub issues listed at the end; the design thread (Fable) reviews the result against
this text and the mockups. Anything not covered here follows `docs/DESIGN.md` and the
existing code's conventions.

## 0. What is wrong today (so the fix is not accidental)

- `main_window/preferences.rs` renders a gpuicn `Drawer` from the right edge over the
  chat, 760px wide. It is neither a page nor a window; it hides half the chat and has no
  room.
- Every choice is a full-width joined `ToggleGroup`; group labels are uppercase `LABEL`;
  "Synced with your Mattermost account" repeats under every synced control.
- Appearance is one ~2,000px scroll that mixes theme, colours, saved styles, text size,
  fonts, message display, clock, names and motion.
- Sign out lives under Advanced; Check for updates lives under Advanced.
- `Settings.format_bar`, `Settings.skin_tone` and `Settings.cache_size_mb` exist with no
  UI in Preferences.
- The sidebar header's team button (`sidebar.rs`, id `sidebar.team`, label "Team
  menu") has no `on_click`: it does nothing.
- The rail (`rail.rs`, 56px) holds servers (round, from two) and teams (squares, from
  two) and the Join/Add buttons; Arya's colleagues find it a waste of space.

## 1. Settings window

### 1.1 Where it lives

- **Desktop (macOS, Linux, Windows):** a second GPUI window, opened the way
  `popout.rs::open_window` opens pop-outs. Title "Settings". Default 880×640 logical px,
  minimum 720×520, resizable; last bounds saved in `Settings` (`settings_window:
  Option<WindowGeometry>`) and restored. One instance: opening again focuses it.
  Closes with Cmd/Ctrl+W, Esc (when no popover or dialog is open inside it) and the
  window's close button. Closing the main window closes it too. Signing out of the last
  server closes it.
- **Web (wasm):** GPUI on the web has one window, so the same `SettingsView` renders as a
  full-viewport overlay inside the main window: it covers the workspace (title bar
  stays), with a "Done" button at the top right of the section list and Esc to close.
  Same layout, same sizes, same `SettingsView`; only the host differs. Put the host
  choice behind one function (`settings::open(window, cx)`), `cfg(target_family =
  "wasm")` for the overlay.
- **Opens from:** Cmd/Ctrl+, (`OpenPreferences`, rename to `OpenSettings`), the app
  menu "Settings…" (macOS puts it under the app menu; Linux and Windows under Edit, keep
  where `menu.rs` has it), the account menu item (rename "Preferences…" to
  "Settings…"), the `/settings` slash command, and `OpenSettings` may carry a `Section`
  so callers can deep-link (the team menu's "Manage servers…" opens Servers).
- Every change applies immediately to the main window, as today. There is no Save or
  Cancel anywhere.
- The word is **Settings** everywhere (window title, menu, account menu, strings
  file). "Preferences" goes.

### 1.2 Layout

```
┌───────────────────────────────────────────────────────────────┐
│ ● ● ●                      Settings                            │ 38 title bar
├────────────────┬──────────────────────────────────────────────┤
│ [🔍 Search ]   │  Appearance                        (TITLE 20)│
│ ▣ General      │  One muted sentence under the title (META)   │
│ ▣ Appearance ◀ │                                               │
│ ▣ Messages     │  Theme                                        │ group label
│ ▣ Notifications│  ┌──────────────────────────────────────────┐│ card
│ ▣ Servers      │  │ Light or dark        [System|Light|Dark] ││ row 48
│ ▣ Keyboard     │  │ description in META                      ││
│ ▣ Advanced     │  └──────────────────────────────────────────┘│
│                │                                               │
│ Signed in as…  │  Style                 ☁ Denim comes from…    │ label + synced tag
│ Matty 0.1.0    │  [card][card][card][card]                     │
└────────────────┴──────────────────────────────────────────────┘
   212                        flex, scrolls
```

- **Section list:** 212px, surface `secondary`/`sidebar`-muted (light `#f5f6f8` under
  Denim, i.e. `theme.colors.muted`), hairline on its right. Search field at the top
  (gpuicn `Input` with a search icon, 30px, placeholder "Search settings"). Rows are
  gpuicn `SidebarItem` 30px with a 16px Lucide icon in `muted-foreground`; the selected
  row is `primary` at 10% with `primary` text and icon (not `sidebar-accent`). Icons:
  General `SlidersHorizontal`, Appearance `Palette`, Messages `MessageSquare`,
  Notifications `Bell`, Servers `Server`, Keyboard `Keyboard`, Advanced `Wrench`.
  Footer in META: "Signed in as **Arya** on karya.team" and "Matty 0.1.0-beta.9" (the
  version string, update channel in parentheses when Beta).
- **Page:** padding 28px 32px, title `TITLE` at 20px/600 (add a `style::PAGE_TITLE`
  constant; do not reuse the 17px `TITLE`), one META sentence, then groups 24px apart.
  Content scrolls in a `ScrollArea`; the section list does not.
- **Group:** label row (13px/600 `foreground`, no uppercase) with an optional tag at the
  right: `☁ Synced with your Mattermost account` or a plain META note such as "This
  computer only"; then one card.
- **Card:** gpuicn `ItemGroup` (new, §3.2): 1px `border`, radius `lg` (8px), rows
  separated by hairlines.
- **Row:** gpuicn `Item`: title `UI` 13.5px `foreground`, description META 12px
  `muted-foreground` (one line, may be omitted), control at the right, vertically
  centred, min-height 48px, padding 10px 16px. Rows never wrap the control under the
  title. A row that needs a wide control (keywords, the accent swatches) uses the
  `Item` "stacked" variant: title and description on top, the control under them, full
  width.
- **Controls, by rule:**
  - on/off: `Switch`.
  - three or more choices, or labels longer than one word: `Select` (28px, bordered,
    chevron, 160–220px wide).
  - two to four choices of one short word each: joined `ToggleGroup` sized to its
    content (28px), never full width.
  - an action: `Button` outline `Sm` with a verb label ("Change…", "Check now",
    "Open folder").
  - a colour or tone: 22px round swatches, the chosen one ringed in `primary`.
- **Hints that depend on the choice** (e.g. the send key explanation) become the row's
  description and change with the value; no extra line under the control.
- **Keyboard:** Tab moves search → section list → page controls; Up/Down in the
  section list switch pages immediately (as today's test
  `the_keyboard_walks_the_section_list` expects); Cmd/Ctrl+F focuses search.
- Light and dark follow the app theme (the window uses `UiTheme` like every other).
  Works under every style source (Mattermost theme, Graphite, Porcelain, custom):
  the section list surface is `muted`, cards are `card`/`background` with `border`.

### 1.3 Sections and rows (the IA)

Tags: **synced** = a server preference (`set_synced` / notify props), **local** =
`Settings` file only. Rows marked NEW have no UI today.

**General**
- Startup and window (local): "Open at login" switch; "Keep running when closed"
  switch (Linux and Windows only, as today).
- Files (local): "Download folder" with the path as description (truncated, full path in
  tooltip), "Change…" button.
- Software update (local): "Matty 0.1.0-beta.9" with "Last checked 10 minutes ago" /
  "Up to date" / "Update ready: 0.1.0-beta.10" as the description and a "Check now"
  button (`CheckForUpdates`; show the result inline in the description, not a dialog
  unless the updater already uses one); "Update channel" Select Stable/Beta with the
  existing hint as description. Moved here from Advanced.
- Language (local, NEW, only once a second locale ships; otherwise omit the group):
  "App language" Select System/English/….

**Appearance** (look only)
- Theme (local): "Light or dark" joined ToggleGroup System/Light/Dark with icons; the
  description says "Your Mattermost theme decides this itself" when the Mattermost
  source is in use and the group is disabled, as today.
- Style (local, tag: "☁ <name> comes from your Mattermost account" when the server
  theme is in use): a **gallery grid** of cards, 4 per row at 880px (`repeat(auto-fill,
  minmax(140px, 1fr))` equivalent: 3 at the minimum width). Order: the Mattermost theme
  of each team (name of the theme, "Your Mattermost theme" / "Mattermost theme for
  <team>"), Graphite ("Matty preset"), Porcelain ("Matty preset"), then saved styles
  ("Saved by you"). A card is the existing mini-window preview (`style_editor::card`)
  at 64px tall, name 13px/500, subtitle META; selected has a 2px `primary` ring and a
  `primary` check badge at the top right. Below the grid, one row of outline buttons:
  "New style from <current>" (forks the current style into a new saved style named
  "<current> copy", selects it, scrolls to Customise), "Paste style", "Open file…", and
  a "Share ▾" menu at the right with "Copy style" and "Save to file…". A saved style's
  card has a ⋯ menu (hover or focus): "Rename…", "Delete…". Presets and Mattermost
  themes cannot be renamed or deleted. The status line ("Saved as …", "Copied…") is a
  toast (gpuicn `toast`), not a line in the page.
- Customise <name> (local): "Accent" swatches + a custom swatch that opens the existing
  colour picker in a popover; "Greys" ToggleGroup Neutral/Cool/Warm/Tinted; "Corners"
  ToggleGroup Square/Soft/Round; "Interface font" Select (families + "Add font file…"
  at the end, as today's menus); "Code font" Select (monospace families); "Text size"
  ToggleGroup 90/100/115/130 %. **Fork semantics:** editing while a preset or a
  Mattermost theme is selected creates a saved style "<name> copy" from it on the first
  change and selects it (toast "Saved as "<name> copy""); editing a saved style edits it
  in place. `custom_style` stays the storage for the style in use; `saved_styles` the
  library. Fonts stay part of a style (#724).
- Motion (local): "Reduce motion" switch.
- The contrast warning for a Mattermost theme (`contrast_warning`) becomes a META line
  under the gallery with the destructive tag, as today, only when that theme is selected.

**Messages**
- Reading (synced): "Message display" Select Standard/Compact (moved from Appearance),
  "Clock" ToggleGroup 12-hour/24-hour, "Show names as" Select Nickname/Full name/
  Username with today's hints as descriptions, "Collapse link previews" switch,
  "Collapsed reply threads" switch (only when the server lets the user choose, as
  today).
- Writing (synced except where noted): "Send messages with" Select Enter / Cmd+Enter /
  Enter, except in code (description changes with the value); "Formatting bar" switch
  (local, NEW, `Settings.format_bar`); "Emoji skin tone" swatches (local, NEW,
  `Settings.skin_tone`, the six tones from `emoji.rs`, default first).
- Privacy (local): "Load remote images" Select Always/Ask first with today's hints.

**Notifications** (as today, relaid)
- Status (synced): "Do not disturb" switch, description from `dnd::preferences_detail`.
- Desktop (synced): "Notify me about" Select All activity/Mentions/Nothing; "Replies in
  my threads" switch; "Sound" row with the sound Select (disabled when off) and the
  switch.
- Mentions (synced): "My first name, "Arya"" switch; "Channel-wide mentions" switch
  (description "@channel, @all and @here"); "Keywords" stacked row: tags inline with
  the add field in one wrapping line (gpuicn `Tag` + `Input`, Enter adds, Backspace on
  an empty field removes the last).
- Highlights (synced): only when `highlight_keys`, same stacked row.
- Offline: the page shows the existing "Notification settings show once Matty is
  connected" sentence in the Desktop group's card, the Status group still works.

**Servers** (NEW section; takes over the rail's jobs)
- Saved servers (local): one `Item` per saved server: 24px round icon with initials (or
  the server's icon once we fetch one), name, host and "Signed in as @arya" as the
  description, unread dot / mention count at the right (from the rail's data), then a
  "Switch to" button on non-current servers and a ⋯ menu: "Sign out…", "Remove…"
  (the existing confirm dialogs from `rail.rs` and `account.rs`). Rows are a gpuicn
  `Sortable` list: drag to reorder; the order sets Cmd/Ctrl+1–9 and the team menu's
  "Other servers" order. "Add server…" outline button under the card (`AddServer`).
- Teams on <server> (server-backed): one `Item` per team the user belongs to: 24px
  square icon/initials, name, "Leave team…" in a ⋯ menu where the server allows it;
  "Join another team…" button under the card (`RailEvent::Join`'s target).
- With one server and one team the section still exists (it is where Add server and
  Join team live).

**Keyboard**
- Shortcuts: the table from `main_window::shortcut_sheet` grouped as `Kbd` rows inside
  `ItemGroup`s: Navigation, Messages, Teams and servers, Window. Read-only. The
  `ShowShortcuts` action and the title bar "?" open Settings on this page; the
  shortcuts dialog is deleted.
- Teams and servers: Cmd+Alt+1–9 teams, Cmd+1–9 servers (as today), plus the new
  Cmd+Alt+Up/Down for previous/next team (§2.4).

**Advanced**
- Logs (local): "Log detail" Select; "Logs" with "Open folder" button.
- Storage (local): "Cache size limit" Select 250 MB / 500 MB / 1 GB / 2 GB (NEW,
  `Settings.cache_size_mb`, with the current usage in the description if cheap to
  compute, else omit); "Clear cached images and files" button with a confirm dialog
  (NEW; clears the image cache and attachment downloads, not the message store).
- Reset (local): "Reset appearance to defaults" button (clears `custom_style`,
  `ui_font`, `code_font`, `text_scale`, `theme_source` → Mattermost, keeps saved
  styles); "Reset all local settings…" button with a confirm dialog (writes
  `Settings::default()` keeping `account`, `servers`, `recent_servers`, `saved_styles`,
  window geometry).
- No Sign out here: it lives in the account menu and in Servers.

### 1.4 Search

- Typing in the search field filters across all sections: the page shows a flat list
  of matching rows, each inside a one-row card with the section name as the group
  label, in section order. Matching is case-insensitive substring on the row title,
  description and the section name, over the English strings (`t!` values) so it works
  under the pseudo-locale too. The section list dims (keeps rendering) while a query
  is set; Esc clears the query and returns to the last section. No results: the
  `style::empty_state` with "No settings match "x"".
- Every row therefore needs a stable id and a title: give `Item` an `id` and keep the
  row definitions as data (a `Row { section, id, title, description, render }` table) so
  search and the pages render from one source.

## 2. Team switcher

### 2.1 The header button

- `sidebar.rs::header`'s team button becomes a gpuicn `Menu` trigger: team name in
  `TITLE`, chevron, and, when another team or another server has mentions, a count
  pill (`mention` colours: `sidebar-primary`/`sidebar-primary-foreground`, i.e. white on
  Denim); when only unread (no mentions) elsewhere, an 8px dot in `sidebar-foreground`
  at 72%. The count sums mentions in other teams of this server plus other servers.
  Tooltip "Switch team or server".
- With one server and one team the button still opens the menu (it holds Join team and
  Add server), but shows no badge.

### 2.2 The menu

Width 300px, anchored under the header button, gpuicn `Menu` with the new rich items
(§3.1):

1. Label "TEAMS ON <HOST>" (the server's display name, upper-case label style).
2. One radio item per team of the current server in rail order: 24px square icon
   (team icon or initials on a per-team colour, as the rail draws it), name (`UI_STRONG`
   when unread), trailing mention pill or unread dot, shortcut `⌘⌥1`… for the first
   nine. The current team is checked (`primary` check) and highlighted. Selecting
   switches team as `RailEvent::Select` does today.
3. "Join another team…" with a `Plus` icon (today's `RailEvent::Join`).
4. Separator, label "OTHER SERVERS" (only with two or more saved servers): one item per
   other server: 24px round icon/initials, name, host as description, trailing mention
   pill / unread dot, shortcut `⌘2`… (its Cmd+number). Selecting switches server as the
   rail's click does today, landing on that server's last team.
5. "Add server…" with a `Plus` icon (`AddServer`).
6. Separator, "Manage servers…" with a `Settings` icon, trailing META "Settings"; opens
   Settings on the Servers section.

Keyboard inside the menu as gpuicn's Menu already does (Up/Down, Enter, Esc, type to
jump). Cmd+Alt+1–9 and Cmd+1–9 keep working from anywhere.

### 2.3 Removing the rail

- Delete `rail.rs`'s view; keep its model (`Team`, `Server`, unread bookkeeping,
  `set_unread`, ordering, the server remove dialog) as `teams.rs` (`TeamsModel`), which
  feeds the header button, the menu and the Servers settings page. The sortable server
  order moves to the Servers page.
- `DESIGN.md` §4: drop the 56px column from the diagram and the "Team rail" bullet;
  describe the header switcher and its badge in the Sidebar bullet. `PLAN.md` §4.1
  likewise. Update `docs/screenshots/readme/*` and the README screenshots.
- The sidebar's default width stays 240; the window simply gains 56px of chat.
- `update_badge` (dock badge) reads the model, not the rail view.
- Window shortcuts that referred to "the rail" in comments and strings (`rail-*` in the
  locales) move to `teams-*` / `switcher-*` keys; remove unused strings.

### 2.4 New shortcut

- Cmd+Alt+Up / Cmd+Alt+Down (Ctrl+Alt elsewhere): previous / next team on this server,
  wrapping; listed in Keyboard. (Mattermost's own binding.)

## 3. gpuicn additions (ship first, release 0.10.x, then bump Matty's pin)

Matty pins gpuicn `=0.9.13`; gpuicn is at 0.10.0 (one-value handlers take `&value`).
The first Matty PR of this work bumps to the release that carries §3.1 and §3.2 and
fixes the handler signatures.

### 3.1 Rich `MenuItem`

Add to `MenuItem` (registry `menu`): `.icon(impl IntoElement)` (16px slot at the left),
`.description(impl Into<SharedString>)` (META line under the label), `.trailing(impl
IntoElement)` (right-aligned slot before the shortcut, for pills and dots),
`.shortcut(impl Into<SharedString>)` (rendered with `Kbd`-style META text at the far
right, as shadcn's `DropdownMenuShortcut`). Radio and checked items keep their check at
the left when there is no icon and move it to the right of the trailing slot when
there is one (shadcn puts the indicator at the left; with an icon, the right keeps the
icon column aligned: follow the mockup). Height 36px with an icon or description, 32px
otherwise. Add a preview to the gallery and a test that the shortcut text renders.

### 3.2 `Item` and `ItemGroup`

Port shadcn/ui's `Item` (`item.tsx`: `Item`, `ItemGroup`, `ItemMedia`, `ItemContent`,
`ItemTitle`, `ItemDescription`, `ItemActions`, `ItemSeparator`) as registry `item`:
`Item::new(id).media(el).title(text).description(text).actions(el)`, variants
`default` (no border), `outline` (bordered card), `muted`; sizes `default` (min 48px,
padding 10px 16px) and `sm`; `.stacked(true)` puts the actions under the content, full
width. `ItemGroup::new(id)` lays Items in a column with hairline separators and, in the
`outline` variant, the card border and radius `lg`. Items can be `Button`-like
(`.on_click`) for list rows that open something. Accessible names: the title is the
row's label; an `Item` with a control forwards `aria-labelledby` so a `Switch` in it
is announced with the title. Gallery preview and a layout test (control stays on the
title's row at 320px width, stacked puts it under).

## 4. Order of work and issues

Stack order; each item is one PR with before/after screenshots (light and dark, same
view and window size; `cargo build -p matty`, Xvfb at 1200×800 and the Settings
window at 880×640) pushed to `karyalabs/assets` under `matty/<PR>/`.

1. gpuicn: rich `MenuItem` (§3.1).
2. gpuicn: `Item` + `ItemGroup` (§3.2). Release gpuicn.
3. matty: bump gpuicn, Settings window shell: host (window / web overlay), section
   list, search field (no filtering yet), page/group/row grammar with `Item`, General,
   Keyboard and Advanced pages, Notifications page relaid; delete the drawer and the
   shortcuts dialog; rename Preferences → Settings everywhere; `OpenSettings(Section)`.
   (§1.1, §1.2, §1.3 General/Keyboard/Advanced/Notifications)
4. matty: Appearance page (§1.3 Appearance: gallery, fork semantics, share menu, card
   ⋯ menu, toasts).
5. matty: Messages page (§1.3 Messages, incl. format bar and skin tone rows).
6. matty: team switcher menu and rail removal (§2); Servers page (§1.3 Servers) in the
   same PR or the next, since both read `TeamsModel`.
7. matty: search across settings (§1.4).
8. matty: docs: DESIGN.md §4, PLAN.md §4.1/§7, README screenshots.

3 and 6 are independent and can run in parallel after 2.

## 5. Acceptance (what the review checks)

- Cmd+, opens one Settings window at the saved size; it looks like the mockups in
  light (Denim) and dark (Onyx), and still looks right under Graphite and Porcelain.
- No full-width toggle groups; no uppercase group labels; "Synced…" appears once per
  group as a tag.
- Every row in §1.3 exists with its control type; the three NEW rows change
  `Settings` and take effect (format bar shows/hides, skin tone changes the picker,
  cache limit is honoured by the image cache).
- Editing a preset forks a saved style; editing a saved style edits it in place;
  share/paste/open still round-trip (existing tests in `style_library.rs` pass).
- The rail is gone; the header menu lists teams and servers with badges and shortcuts;
  Cmd+Alt+1–9, Cmd+1–9 and Cmd+Alt+Up/Down switch; other-team mentions show on the
  header button; the dock badge still counts.
- Search finds "skin" under Messages and "update" under General.
- Web build: Settings opens as the overlay, Esc closes it.
- Keyboard-only: the section list, every control and the team menu are reachable;
  `scripts/check-accessible-names.py` passes.
- CI green; integration label not needed.
