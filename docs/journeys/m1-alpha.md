# M1 alpha journeys

These are the checks a person runs by hand on a signed build (PLAN §11,
§12). They were written for M1 alpha, and they are re-run on each beta
(#201) with the beta additions in §5. There are five journeys: features,
keyboard only, IME, VoiceOver, and the beta additions.

## Running it in sessions

Sessions A to D take about two and a half hours in all. Each one
closes an issue once every step in it has a result:

| Session | Sections | Time | Closes |
|---|---|---|---|
| A | §1 Features, then §5 Beta additions | 75 min | #168, then #201 together with B to D |
| B | §2 Keyboard only (mouse off) | 30 min | #169 |
| C | §3 IME (add the Japanese, Pinyin and Korean input sources first) | 20 min | #170 |
| D | §4 VoiceOver | 30 min | #171 |
| E | `m2-screen-readers.md` on macOS, then NVDA, Narrator and Orca | 30 min each | #217 |

When B to D are done, #166 is done too. Start each session on the newest
beta and write its version in the run heading.

## How to run

- **Build and machine:** use the newest signed, notarized beta from the
  release workflow, not a dev build. Run it on macOS 13 or later, once on
  Apple silicon and once on Intel if one is available.
- **Server:** use the current Mattermost release, seeded with
  `scripts/dev-server` (see `docs/dev-server.md`). Sign in as `tester`
  unless a step says otherwise, and use a second account in the webapp
  for the "other person" steps.
- **Evidence:** follow gpuicn's evidence standard. Every step gets:
  - a screenshot or screen recording, committed under `docs/journeys/evidence/m1-alpha/`
    or linked;
  - the date;
  - the tester;
  - Pass or Fail.
  Use a recording when the step involves motion, focus movement or
  timing.
- **Failures:** file an issue and put its link in the Notes column. Don't
  mark a step Pass with a known problem.
- **Copying:** copy the empty tables into a new section, headed with the
  build version and machine, for each run. Don't overwrite earlier runs.

Columns: **Step**, **Do**, **Expect**, **Evidence**, **Date**,
**Tester**, **Result**, **Notes**.

## 1. Features

### Sign-in

| Step | Do | Expect | Evidence | Date | Tester | Result | Notes |
|---|---|---|---|---|---|---|---|
| 1.1 | Launch Matty for the first time and enter the server URL | The server is checked and its sign-in methods are offered | | | | | |
| 1.2 | Sign in with email and password | The sidebar loads with the user's teams, categories and unread state | | | | | |
| 1.3 | Sign out, then sign in with a personal access token | Same result as 1.2 | | | | | |
| 1.4 | Sign in with GitLab or OpenID SSO (where configured) | The browser opens, returns through `mattermost://`, and Matty is signed in | | | | | |
| 1.5 | Quit and relaunch | Matty opens signed in, showing the cached sidebar before the network answers | | | | | |
| 1.6 | Right after signing in, press Alt-Down | The next channel opens without clicking first (#449) | | | | | |

### Navigation

| Step | Do | Expect | Evidence | Date | Tester | Result | Notes |
|---|---|---|---|---|---|---|---|
| 2.1 | Click channels in each sidebar category | Each opens, and the row shows as selected | | | | | |
| 2.2 | Cmd-K, type part of a channel name, Enter | The switcher finds the channel and opens it | | | | | |
| 2.3 | Alt-Up and Alt-Down, then Alt-Shift-Up and Alt-Shift-Down | Previous and next channel, then previous and next unread | | | | | |
| 2.4 | Cmd-[ and Cmd-] | Back and forward through the channels visited | | | | | |
| 2.5 | Switch teams with the team tabs, then with Cmd-1 to Cmd-9 and Cmd-Alt-Up and Cmd-Alt-Down | The sidebar changes team, and each tab's unread and mention badges are right | | | | | |
| 2.5a | With two servers added, switch servers from the sidebar header | The header names the current server and counts mentions waiting on the others; the sidebar changes server | | | | | |
| 2.6 | Create a category, move a channel into it, rename it, delete it | Each change shows in the webapp after a reload | | | | | |
| 2.7 | Favorite, mute and leave a channel | Each change shows in the webapp | | | | | |

### Reading and the unread rule

| Step | Do | Expect | Evidence | Date | Tester | Result | Notes |
|---|---|---|---|---|---|---|---|
| 3.1 | Open a channel with unread messages | The "New messages" line sits above the first unread post, and the view starts there | | | | | |
| 3.2 | Scroll to the newest post with the window focused | The channel is marked read here and in the webapp | | | | | |
| 3.3 | With the window unfocused, have the other person post | The channel shows as unread, and is not marked read until you come back and see it | | | | | |
| 3.4 | Scroll up through a long channel | Older posts load without jumps, and the scroll position holds | | | | | |
| 3.5 | Open a message with code, a table, an image and a link preview | Each renders. Code is highlighted, and the image opens in the lightbox | | | | | |

### Writing

| Step | Do | Expect | Evidence | Date | Tester | Result | Notes |
|---|---|---|---|---|---|---|---|
| 4.1 | Type a message and press Enter | It shows at once, then is confirmed. The webapp shows it | | | | | |
| 4.2 | Shift-Enter, then format with Cmd-B, Cmd-I and Cmd-Alt-K | A new line is inserted and the formatting wraps the selection | | | | | |
| 4.3 | Type `@` and `~` and `:` | Autocomplete offers people, channels and emoji | | | | | |
| 4.4 | Up arrow in an empty composer, edit, Enter | The last message is edited here and in the webapp | | | | | |
| 4.5 | Delete a message from its menu | It disappears here and in the webapp | | | | | |
| 4.6 | Type a draft, switch channel, come back | The draft is still there, and it shows under Drafts | | | | | |

### Threads and reactions

| Step | Do | Expect | Evidence | Date | Tester | Result | Notes |
|---|---|---|---|---|---|---|---|
| 5.1 | Open a thread and reply | The reply shows in the right pane and in the webapp thread | | | | | |
| 5.2 | Have the other person type in the thread | "… is typing" shows above the thread composer (#422) | | | | | |
| 5.3 | Open Threads | Followed threads list with unread replies, and opening one marks it read | | | | | |
| 5.4 | Add and remove a reaction | The count updates here and in the webapp | | | | | |

### Search, saved, pinned and mentions

| Step | Do | Expect | Evidence | Date | Tester | Result | Notes |
|---|---|---|---|---|---|---|---|
| 6.1 | Cmd-Shift-F, search a word, open a result | Results show in the right pane, and the result opens at its post | | | | | |
| 6.2 | Save a message, then open Saved messages | It is listed | | | | | |
| 6.3 | Pin a message, then open the pinned list | It is listed, and the header's pin count updates | | | | | |
| 6.4 | Cmd-Shift-M | Recent mentions list the user's mentions | | | | | |

### Presence and profiles

| Step | Do | Expect | Evidence | Date | Tester | Result | Notes |
|---|---|---|---|---|---|---|---|
| 7.1 | Set Away, Do not disturb and Offline from the account menu | The dot changes here and in the webapp | | | | | |
| 7.2 | Have the other person go away | Their dot on messages, direct messages and the member list changes within a few seconds (#420) | | | | | |
| 7.3 | Click an avatar | The profile card shows name, position, local time and a Message action | | | | | |
| 7.4 | Edit profile: change the position | The webapp shows the new position (#450) | | | | | |

### Notifications

| Step | Do | Expect | Evidence | Date | Tester | Result | Notes |
|---|---|---|---|---|---|---|---|
| 8.1 | With Matty in the background, get a direct message | A system notification shows, and clicking it opens the conversation at the post | | | | | |
| 8.2 | Mute the channel and get a message there | No notification | | | | | |
| 8.3 | Turn on Do not disturb and get a mention | No notification. The badge still counts it | | | | | |
| 8.4 | Check the dock badge | It counts mentions and direct messages | | | | | |

### Files

| Step | Do | Expect | Evidence | Date | Tester | Result | Notes |
|---|---|---|---|---|---|---|---|
| 9.1 | Cmd-U and pick an image and a PDF | Both upload with progress and post | | | | | |
| 9.2 | Drag a file onto the channel | It is added to the upload tray | | | | | |
| 9.3 | Open the image | The lightbox opens. Escape closes it | | | | | |

### Settings

| Step | Do | Expect | Evidence | Date | Tester | Result | Notes |
|---|---|---|---|---|---|---|---|
| 10.1 | Cmd-, opens Settings. In Appearance, pick each theme (Graphite, Porcelain, Harbor, Linen, Tide, Dusk, Ember), then switch between Light, Dark and System | The window follows at once, and System follows the Mac's setting | | | | | |
| 10.2 | Change the text size and zoom (Cmd-= and Cmd--) | Text and spacing scale, and nothing overlaps | | | | | |
| 10.3 | Turn on Reduce motion | Transitions snap | | | | | |

### Offline

| Step | Do | Expect | Evidence | Date | Tester | Result | Notes |
|---|---|---|---|---|---|---|---|
| 11.1 | Turn off Wi-Fi | The offline banner shows, and cached channels still open | | | | | |
| 11.2 | Send a message while offline | It waits, and the banner counts it | | | | | |
| 11.3 | Turn Wi-Fi back on | The waiting message sends once, missed posts arrive, and unread state is right | | | | | |

## 2. Keyboard only (#169)

Unplug the mouse or turn off the trackpad first. Every step must work
without them. Any step that needs the mouse is a blocker: file it and link
it here. Record the whole run.

| Step | Do | Expect | Evidence | Date | Tester | Result | Notes |
|---|---|---|---|---|---|---|---|
| K.1 | Sign in with Tab, typing and Enter only | Focus order follows reading order, and the focus ring is visible on every stop | | | | | |
| K.2 | Cmd-K to switch channels, then Alt-Up and Alt-Down | Works straight after sign-in | | | | | |
| K.3 | Shift-Tab from the composer into selection mode, then move with Up and Down | The selected message is outlined, and its actions are reachable | | | | | |
| K.4 | React to the last message (Cmd-Shift-\\) | The emoji picker opens, and the chosen reaction is added | | | | | |
| K.5 | Reply in a thread, then close the right pane | Focus returns to where it was | | | | | |
| K.6 | Edit the last message (Up), then delete a message | Both are possible without the mouse | | | | | |
| K.7 | Search, then jump to a result | Focus lands on the post | | | | | |
| K.8 | Upload a file (Cmd-U), then open it in the lightbox | The file dialog and the lightbox both work by keyboard, and Escape returns focus | | | | | |
| K.9 | Cmd-, to open Settings, move between its sections, change a setting, then close the window | Every section and control is reachable with Tab and the arrows, and closing returns focus to the main window | | | | | |
| K.10 | Sign out from the account menu | The menu and the confirmation are reachable and confirmable by keyboard | | | | | |
| K.11 | Tab onto a sidebar section header (Channels, Direct messages) | Its + and ⋯ buttons show while it has focus, and Tab reaches both (#901) | | | | | |
| K.12 | Switch teams with Cmd-1 and Cmd-Alt-Down, then open the server switcher in the sidebar header | Each works with the caret in the composer, and the switcher's menu is usable by keyboard | | | | | |
| K.13 | Cmd-/ | The shortcuts sheet opens, and Escape closes it with focus back where it was | | | | | |

## 3. IME (#170)

Run each input source in the composer, the thread composer, the search
box and the quick switcher. Record each one.

| Step | Do | Expect | Evidence | Date | Tester | Result | Notes |
|---|---|---|---|---|---|---|---|
| I.1 | Japanese: type `nihongo`, convert to kanji, Enter, Enter | The first Enter commits the conversion. Only the second sends | | | | | |
| I.2 | Chinese Pinyin: type `zhongwen`, pick a candidate, Enter | The candidate commits, and nothing is sent mid-composition | | | | | |
| I.3 | Korean 2-set: type a syllable block, then Enter | The block commits before sending | | | | | |
| I.4 | ABC-Extended dead keys: Option-E then e, Option-U then u | `é` and `ü` are inserted | | | | | |
| I.5 | Type `@` or `:` during composition | Autocomplete does not open until the text is committed | | | | | |

## 4. VoiceOver (#171)

Turn on VoiceOver (Cmd-F5) and use VoiceOver keys only.

| Step | Do | Expect | Evidence | Date | Tester | Result | Notes |
|---|---|---|---|---|---|---|---|
| V.1 | Move through the sidebar | Each row reads its name, kind, unread and mention count. Each avatar reads its presence ("Ana Lopez, away") | | | | | |
| V.2 | Move through the message list | Each message reads its author, time and text. New messages are announced | | | | | |
| V.3 | Open and close a dialog | Focus moves into the dialog, its title is read, and focus returns when it closes | | | | | |
| V.4 | Use the composer | It is announced as a text field with its placeholder, and typing echoes | | | | | |
| V.5 | Open each icon-only button | Each has a name, and none reads as just "button" | | | | | |
| V.6 | Move through the team tabs and the server switcher | Each reads its name and its mention count | | | | | |
| V.7 | Open Settings (Cmd-,) and move through its sections | Each section's name is read, and each control reads its label and value | | | | | |

## 5. Beta additions (#201)

Run after §1 on the same build.

| Step | Do | Expect | Evidence | Date | Tester | Result | Notes |
|---|---|---|---|---|---|---|---|
| B.1 | Cmd-Shift-U (Unreads filter), then again | The sidebar shows only unread channels, then everything again | | | | | |
| B.2 | Post a link that has a preview, collapse the preview, then hide its image | The arrow sits on the image's corner; a hidden image leaves "Show image preview" | | | | | |
| B.3 | Set Do not disturb for 30 minutes from the account menu | The status shows when it ends, and it clears by itself then | | | | | |
| B.4 | Add a keyword in Settings › Notifications, then have the other person post it | The post is highlighted, shows in Recent mentions, and notifies | | | | | |
| B.5 | Offline, try to react, edit and save a message | Each is refused with a "Try again when you're back online" notice, and nothing is lost | | | | | |
| B.6 | Join another team with the + after the team tabs, then leave it from Settings › Servers | The team appears once joined. Leaving asks first, then the team goes away here and in the webapp | | | | | |
| B.7 | In Settings › Servers, drag a server to a new place | The order holds after a relaunch, and the sidebar header's switcher follows it | | | | | |

## Runs

Add one section per run below this line.
