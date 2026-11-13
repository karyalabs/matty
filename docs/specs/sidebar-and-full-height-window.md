# Classic sidebar in a full-height window: implementation spec

Picked by Arya on 2026-11-10 (thread "Redesign Matty's sidebar"): the **Classic** layout with
**Full height** chrome from the prototype.

- Prototype (the visual contract): https://claude.ai/artifact/7oj4cJM4ZRJWoJgkJvjyPc, source
  in project files `matty/sidebar-redesign/round-2/matty-sidebar-lab.html`. Open the file in
  a browser, set Layout = Classic, Chrome = Full height, and flip Teams, Activity, Sidebar
  and Theme. Sizes below are logical px at text scale 1; where this text and the prototype
  differ, this text wins.
- Not in scope (other presets in the prototype): the "For you" feed, the destination tiles,
  the quiet-channels fold, the Recent layout, a layout setting. Do not build them.
- Everything else follows `docs/DESIGN.md` and the code's conventions. UI comes from gpuicn;
  extend gpuicn (PR, release, pin bump) when a piece is missing, never a local substitute.
  Commit this file as `docs/specs/sidebar-and-full-height-window.md` with the first PR and
  update `docs/DESIGN.md` §1 "Native chrome" and §4 in the last one.

## 1. Window: no title bar

The 34px title bar (`main_window.rs::title_bar`) goes on every platform. The sidebar runs
the full height of the window; the channel header and right pane header become the top edge
of their panes.

| What the title bar held | Where it goes |
| --- | --- |
| Toggle sidebar, Back, Forward | Left end of the channel header, in that order (§3) |
| Search pill (quick switcher) | Sidebar jump field (§2.2) |
| All unreads (Inbox), Recent mentions, Saved | Sidebar rows (§2.3) |
| Help | Sidebar footer (§2.6) |
| Avatar and account menu | Sidebar footer (§2.6) |

- **Window controls.** macOS: traffic lights stay native at the top left; the sidebar gets a
  28px empty drag strip above its header so the lights never overlap the team button (when
  the sidebar is hidden, the channel header gets a 78px left inset instead). Windows and
  client-decorated Linux: minimise, maximise and close sit at the right end of the top-right
  header (the right pane's header when it is open, else the channel header), drawn with the
  same gpuicn title-bar control buttons as today. Web: nothing.
- **Dragging.** Empty space in the sidebar header (and the macOS strip), the channel header
  and the right pane header drags the window; double-click zooms or maximises as the title
  bar does today. Buttons and the channel title's menu stay clickable.
- **Connection banner** stays a 32px strip across the whole window, now at the very top.
- **Views with their own header** (Threads, Drafts, Browse, etc.) get the same left group
  (toggle, back, forward) and the same window controls rule. Put the group and the controls
  behind one helper so every header gets them.
- **Sign in, Settings window, pop-outs** are unchanged (they keep whatever chrome they have).
- All headers are 46px tall now (was 48; `metrics::HEADER_HEIGHT`), so the three panes line
  up.

## 2. Sidebar

Width as today (default 260, 200 to 400), surface `sidebar`, hairline on its right. Top to
bottom: header, jump row, fixed rows, scrolling tree, footer. Only the tree scrolls.

### 2.1 Header (46px, padding 0 8px)

- Left: the team button, a gpuicn `Menu` trigger as today (`teams.rs::header_button`):
  22px team tile (team icon, or initials on the team's colour; the current team's initials
  tile uses `primary`/`primary-foreground`), the team name in 15.5px SemiBold, truncating,
  and a 13px chevron. The menu is today's menu, unchanged. **The count pill and dot on
  this button go away**; the tiles carry them.
- Right: one 22px tile per *other* team of this server, in menu order, 7px apart. Each is a
  button that switches to that team (same as picking it in the menu). A tile shows its own
  mention count as a 15px `sidebar-primary` pill at its top-right corner, or a 10px
  `sidebar-foreground` dot when it only has unreads; both have a 2px `sidebar` ring so they
  read against the tile. Tooltip: "Design Guild · 2 mentions · Ctrl+Alt+2" (name, state when
  any, shortcut when it has one).
- With more than three other teams: show the first two, then a "+N" tile (`secondary`
  surface) that opens the team menu and carries a dot when any hidden team has unreads or
  mentions. With one team there are no tiles.
- Other servers stay in the menu only. When another server has mentions or unreads and
  there is no "+N" tile, show one extra round tile (the server's initials, `secondary`)
  after the team tiles with that server's count or dot; clicking opens the menu. This keeps
  the "something is waiting elsewhere" signal the old header pill gave.
- The unreads filter toggle and the compose button leave the header (§2.2, §2.4).

### 2.2 Jump row (padding 0 10px 8px)

- A 30px bordered field look-alike (`background` surface, `border`, radius 7, search icon
  14px, placeholder "Jump to or search", trailing `Kbd` hint "Ctrl K" / "⌘K") that fills the
  row, then a 30px bordered icon button with `SquarePen` that opens today's New menu
  (`sidebar-new`).
- **Step 1 (this spec):** the field is a button. Clicking it or pressing Cmd/Ctrl+K opens
  the existing quick switcher exactly as the title bar pill does today; Cmd/Ctrl+F and the
  Search action keep their current behaviour. The switcher popup anchors to the window as
  today.
- Filtering the sidebar in place as you type is a follow-up issue, not part of this work.

### 2.3 Fixed rows

Five rows drawn like channel rows (today's `fixed_entries`), 1px apart, 12px gap below:

| Row | Icon | Opens | Count |
| --- | --- | --- | --- |
| Unreads | `Inbox` | The unread digest in the right pane (today's All unreads) | unread unmuted channels in this team, `secondary` badge; none at zero |
| Threads | `MessageSquareText` | Threads view (today) | today's `threads_count` |
| Mentions | `AtSign` | Recent mentions in the right pane (`RecentMentions`) | none |
| Drafts | `FilePenLine` | Drafts view (today) | today's count, `secondary` badge |
| Saved | `Bookmark` | Saved messages in the right pane (`SavedMessages`) | none |

Unreads, Mentions and Saved are not selectable places: the row shows as selected only while its
listing is the open right pane content. Their shortcuts (Cmd/Ctrl+Shift+M, +B) are
unchanged. Rows with a count use `ui_strong`; the rest use `muted-foreground`.

### 2.4 Tree

Same model, order, drag-to-reorder, context menus, collapse rule (a collapsed section still
shows its unread rows and the open channel) and row anatomy as today. Changes:

- **Section headers** are sentence case, 12px Medium, `sidebar-muted-foreground`, 24px tall
  ("Favorites", "Channels", "Direct messages", custom names as the user typed them). Drop
  the uppercase `LABEL` style here and in the loading skeleton.
- **Collapsed header** shows "· N unread" after the name (11.5px Regular) whenever the
  collapsed section holds unread unmuted rows.
- **Header hover or focus** reveals two 22px ghost buttons at the right: `Plus` and
  `Ellipsis`. Plus: Channels and custom sections open Browse channels (the New menu's
  browse item); Direct messages opens New message; Favorites has no Plus. Ellipsis opens
  the section menu that right-click opens today, with one new checkbox item at the top,
  "Show unread only", bound to the existing `UnreadsFilter` action (it filters the whole
  sidebar, as today).
- **Filter state is visible:** while the unreads filter is on, a 24px row sits above the
  first section: a `secondary` chip "Unread only" with an `X` that turns it off. The
  filter's shortcut and View menu item are unchanged.
- **Unread rows below the fold:** when at least one unread row (bold, unmuted) is entirely
  below the visible part of the tree, a pill floats 2px above the tree's bottom edge,
  centred: `ArrowDown` 12px and "2 unread below", 24px tall, `primary` on
  `primary-foreground`, 11.5px Medium. Clicking scrolls the first such row into view (no
  smooth scroll under reduced motion). Mirror it at the top ("2 unread above") with
  `ArrowUp`. Compute it from row state and the scroll offset; do not measure text. It is a
  button with an accessible name.
- Rows keep 28px, 8px padding, 6px radius, 16px icons, bold plus dot for unread, `primary`
  pills for mentions, 60% for muted, the draft pencil, status emoji and typing as today.
- Bottom padding of the scroll area is 30px so the pill never hides the last row.

### 2.5 Peek (separate, last PR; may slip to a follow-up if it grows)

Resting the pointer for 380ms on an unread row that is not the open channel, or pressing
Space on it when focused, opens a gpuicn `PreviewCard` to the right of the row (10px gap,
300px wide): header with the channel icon and name and "2 mentions" or "unread" in META;
the newest three posts as compact rows (20px avatar, author Medium, time META, plain text
only, no Markdown, images or reactions, each body clamped to three lines); a footer with
"Mark read" (outline `Sm`) and "Open" (primary `Sm`). Posts come from the store when
present, else one fetch of the latest page; show the card only once the posts are there
(no spinner). Leaving the row and the card closes it after 220ms; Esc closes it; it never
opens while a menu or drag is active.

### 2.6 Footer (48px, hairline on top, padding 0 8px 0 12px)

- Left, filling: the account button, which opens today's account menu upward: 26px avatar
  with the presence dot, the display name in `ui_strong`, and under it one META line: the
  custom status text when set ("In review until 15:00"), else the presence word ("Online",
  "Away", "Do not disturb", "Offline"). Truncates; never wraps.
- Right: three 28px ghost icon buttons with tooltips: `Bell` "Do not disturb" (a toggle;
  pressed state uses `sidebar-accent`; it flips the same synced setting as Settings ›
  Notifications › Do not disturb), `CircleHelp` "Help" (what the title bar's ? does today),
  `SlidersHorizontal` "Settings" (`OpenSettings`).
- Under 232px of sidebar width, drop Help first, then the status line.

## 3. Channel header and hiding the sidebar

- Left group in every pane header: `PanelLeft` (ToggleSidebar, tooltip with the shortcut),
  `ChevronLeft` (HistoryBack), `ChevronRight` (HistoryForward), 28px ghost buttons, 2px
  apart, then the title 6px after. Disabled states as today.
- **Hidden sidebar** (`sidebar_visible == false`): the toggle carries the total mention
  count for everything the sidebar would show (all teams, servers, DMs) as a pill, or a dot
  for unreads only, so nothing is missed while hidden.
- **Edge peek:** while hidden, moving the pointer within 14px of the window's left edge, or
  hovering the toggle, slides the sidebar in over the content as an overlay (140ms ease-out,
  instant under reduced motion; shadow `lg`; it does not resize the panes). It stays while
  the pointer is over it, the toggle, or one of its menus or the peek card, and slides out
  200ms after the pointer leaves. Clicking a row navigates and leaves it open until the
  pointer leaves. Clicking the toggle while peeking pins it (same as showing the sidebar).
- The peeked sidebar hangs **under the pane headers**, from the header's bottom edge to
  the window's, and draws no traffic-light strip. A full-height overlay covered the toggle
  that had just been hovered, so the click that pins it landed on the sidebar and nothing
  in the window could bring the sidebar back. The header, and with it the toggle, Back and
  Forward, stays clickable throughout a peek.
- Cmd/Ctrl+Shift+D toggles the sidebar from anywhere. Cmd/Ctrl+B does too, except in a
  text field, where it is Bold.
- The narrow-window drawer (`sidebar_drawer_open`) keeps its current behaviour; edge peek
  applies only when the window is wide enough for the docked sidebar.
- Touch input has no hover: tapping the toggle shows the sidebar as today.

## 4. Order of work

Each item is one PR with before and after screenshots, light and dark, same view, same
window size (1200×800), pushed to `karyalabs/assets` under `matty/<PR>/`; a short video for
the edge peek and the pill.

1. **Sidebar, title bar still present:** header tiles (§2.1), jump row (§2.2), fixed rows
   (§2.3), tree changes (§2.4), footer (§2.6). The title bar loses the search pill, inbox,
   @, bookmark, ? and avatar in this PR since the sidebar now has them.
2. **Full-height window:** remove the title bar, header left group and
   window controls (§1, §3 first bullet). Check macOS, Linux CSD, Windows and web.
3. **Hidden sidebar:** badge on the toggle and edge peek (§3).
4. **Peek** (§2.5).
5. **Docs:** DESIGN.md §1 and §4, PLAN.md where it describes the title bar, README
   screenshots, this spec in `docs/specs/`.

1 is independent of 2; 3 and 4 need 2 and 1.

## 5. Acceptance (what the design review checks)

- With Layout = Classic and Chrome = Full height, the app matches the prototype in light and
  dark at 1200×800, under Graphite and Porcelain and a Mattermost theme.
- No title bar on any platform; window controls and dragging work on macOS, Windows, Linux
  CSD and are absent on web; the three pane headers line up at 46px.
- Other teams are one click away with their own badges; with six teams there are two tiles
  and a "+3"; with one team there are none; another server's mentions still show.
- Threads, Mentions, Drafts, Saved rows work; the footer shows presence and status, toggles
  Do not disturb, opens Help, Settings and the account menu.
- Section headers are sentence case, show "+" and "⋯" on hover and focus, and a collapsed
  section says how many are unread. The unread filter shows its chip.
- The "N unread below/above" pill appears only when it is true and scrolls to the row.
- Hiding the sidebar keeps a badge on the toggle; the left edge peeks it; reduced motion
  removes the slide.
- Keyboard only: every new control is reachable in order (team button, tiles, jump, new,
  fixed rows, tree, footer); `scripts/check-accessible-names.py` and the contrast tests
  pass. Roving focus in the tree is unchanged.
- No new per-frame work: scrolling the sidebar and typing in the composer profile no worse
  than before on web (the pill and tile badges read cached counts; the peek loads on demand).
- CI green.
