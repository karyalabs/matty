# Sidebar places, panels and switching: implementation spec

Picked by Arya on 2026-11-12 (thread "Rethink sidebar header and switching"): "Build it as
shown". It replaces parts of `docs/specs/sidebar-and-full-height-window.md` §2.1, §2.3 and
§2.6; where the two differ, this file wins.

- Prototype (the visual contract): https://claude.ai/artifact/CXCE3J8eeuy2PhpEM4jJov. Flip
  View between Before and After, and try 1 or 3 servers and 1, 2 or 6 teams.
- Goal in Arya's words: "good, obvious, albeit boring" and "very usable".

## 1. The rule

The sidebar is a list of **places**. Picking a row shows that place in the main pane, and
exactly one row is selected at any time. Things that open beside a place are **panels**;
they never select a row. A **filter** narrows the tree and selects nothing.

| What | Kind | Opens | Selected state | On arrival |
| --- | --- | --- | --- | --- |
| Channel, DM | Place | Main pane | Its row | As today |
| Threads | Place | Main pane (list and reading area) | Threads row | No thread open, nothing marked read until a thread is picked. A notification or link that names a thread still opens it. |
| Drafts | Place, row only while drafts exist | Main pane | Drafts row | As today |
| Mentions, Saved | Panel | Right pane beside a channel | Pressed header button | Fresh list |
| Unread filter | Filter | Narrows the tree | Pressed filter button and the chip | Open channel stays visible |

- Opening a panel from Threads or Drafts returns to the last channel (the panel needs the
  right pane, which those views do not have) and selects that channel's row.
- Back and Forward step through places only.

## 2. Fixed rows

- **Threads** is the only row that is always shown, at the top of the tree, with its unread
  count as today.
- **Drafts** shows under it only while there are drafts (or while the Drafts view is open).
- The **Unreads**, **Mentions** and **Saved** rows go. The All unreads digest keeps its
  shortcut and View menu item.

## 3. Mentions and Saved buttons

The header that starts the window's top row (the one with Toggle sidebar, Back and Forward)
ends with two 28px ghost icon buttons, before the window controls: `AtSign` "Recent
mentions" and `Bookmark` "Saved messages", tooltips with shortcuts. A button shows pressed
(`accent` background) while the right pane shows its listing; clicking it then closes the
right pane. On channel views a hairline separator sits between the channel's own buttons and
these two. The Threads list header is too narrow for them, so it leaves them out; their
shortcuts still work there.

## 4. Unread filter button

The jump row becomes: Jump to field, a 30px bordered `ListFilter` toggle "Show unread only"
(pressed: `primary` surface), and the New button. It dispatches the existing
`UnreadsFilter` action. The "Unread only" chip above the tree stays.

## 5. Footer

Account button and Settings only. Do not disturb and Help move into the account menu
(Do not disturb as a checkbox item under the status items, Help above Sign out).

## 6. Header: the server switcher

- The sidebar header is one button, filling the row: 24px server tile (initials on the
  server's colour, radius 7), the server name in 14.5px SemiBold (truncating), a 14px
  chevron, and at the right end the total mentions on **other** servers as a `primary`
  count pill, or a dot when they only have unreads.
- Its menu lists servers only, one 48px row each: 30px tile, name (Medium; SemiBold and an
  `accent` row for the current one), host on a META line, and at the end a check for the
  current server, else its mention count or unread dot. No shortcuts in the rows (they stay
  in tooltips and Settings › Keyboard). Then a separator, "Add a server…", "Join a team on
  {server}…" (only when the server has one team), "Manage servers…".

## 7. Team tabs

- When the server has two or more teams, a row of tabs sits under the header: the team name
  (13px Medium; selected: SemiBold, foreground, 2px underline), then its mention count or
  unread dot. The row has a hairline under it.
- Tabs that do not fit go under a last "More" tab (chevron, and the hidden teams' total
  mentions or a dot), which opens a menu of those teams and "Join another team…". When
  nothing overflows, a 24px `Plus` button ends the row ("Join another team").
- The current team is always visible: if it would overflow, it takes the last visible place.
- Right-click on a tab opens the team's menu (Mark all read, Leave team… when available).
- With one team there is no row. Team shortcuts (`Alt+Cmd+1…9`) are unchanged.

## 8. Order of work

1. Places and panels: §1 to §5 (Matty only).
2. gpuicn: what tabs need (trailing badge, end slot, secondary click, colours), release, pin.
3. Server header and team tabs: §6, §7.
4. Docs: DESIGN.md, README screenshots.
