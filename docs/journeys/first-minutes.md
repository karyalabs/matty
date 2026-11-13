# First minutes

A five minute walk through the main window with the pointer and the
keyboard. Run it on every pull request that touches the window's chrome
(sidebar, pane headers, right pane), focus, or the keymap, and before every
release. It exists because of a run of bugs that anyone would hit in their
first minutes and that no test caught: a hidden sidebar that could not be
brought back with its own button, shortcuts that did nothing depending on
where the caret was, a button with no click handler.

## Why they slipped through

Each of these had tests, and the tests passed:

- **Tests dispatched actions.** `cx.dispatch_action(ToggleSidebar)` proves
  the handler works. It says nothing about whether the button can be
  clicked or the key reaches the handler.
- **Tests pressed keys from the window's root.** A shortcut that works with
  focus on the window can be dead with the caret in the composer, where it
  is nearly all the time, because GPUI gives a key to the binding whose
  context matches deepest and a text field binds editing keys of its own.
  The same goes for a key the toolkit binds with no context at all.
- **Tests replayed keys as the keymap writes them.** `cmd-shift-.` in a
  test is not what a keyboard sends: macOS and Linux report `cmd->`, a
  browser `cmd-shift->`.
- **Screenshots are still.** Before and after pictures show a hidden
  sidebar and a shown one. They do not show the pointer travelling to the
  button.

So the rules for a change in this area:

1. Test it the way it is used: move the pointer and click
   (`simulate_mouse_move`, `simulate_mouse_down`), press keys with focus
   where a user has it. `crates/matty-app/tests/first_minutes.rs` is the
   pattern.
2. A new shortcut is a row in `actions::bindings()`. Then
   `tests/keymap_reach.rs` presses it from the window, the composer, a
   draft, a reply and the sidebar, in every spelling a platform sends, with
   no further work. Never bind a key anywhere else.
3. Run the app and do the walk below before opening the pull request.
   An agent runs it too: in the Linux build under Xvfb with `xdotool`, or
   in the web build with Playwright. Say in the pull request's "How it was
   tested" that it was run, and where.

## The walk

Use preview data (`MATTY_PREVIEW=1`, or `?preview=1` on the web), a
1200×800 window, and no test hooks. Cmd on macOS, Ctrl elsewhere.

| Step | Do | Expect |
|---|---|---|
| 1 | Click Toggle sidebar in the header | The sidebar hides; the button stays where it was |
| 2 | Rest the pointer on Toggle sidebar | The sidebar peeks under the header; the button is not covered |
| 3 | Click Toggle sidebar | The sidebar is pinned and stays when the pointer leaves |
| 4 | Hide it again, move the pointer to the window's left edge, click a channel in the peek, move away | The channel opens; the peek leaves with the pointer |
| 5 | Click in the message list, press Cmd+B twice | The sidebar hides, then shows |
| 6 | Click in the composer, press Cmd+Shift+D twice | Same, with the caret still in the composer |
| 7 | With the caret in the composer, press each of: Cmd+K, Cmd+Shift+K, Cmd+., Cmd+Shift+., Cmd+Shift+I, Cmd+Shift+M, Cmd+/, Back and Forward (Cmd+[ and Cmd+], or Alt+Left and Alt+Right) | Each does what the Cmd+/ sheet says; none types into the message |
| 8 | Click a channel in the sidebar, then repeat three shortcuts from step 7 | Same result as from the composer |
| 9 | Click Channel info in the header, then type a word | The word lands in the composer |
| 10 | Click Expand in the right pane, then Collapse | The pane takes the channel's place, then gives it back |
| 11 | Press Shift+Up in the empty composer and type | The thread opens and the text lands in its reply box |
| 12 | Click a thread's reply count and type | Same |
| 13 | Open a menu (channel name, a row's Options), press Escape, press Cmd+K | The menu closes and the switcher opens without a click first |
| 14 | Hover every button in the header and the composer's toolbar and click each once | Every button does something, and its tooltip names its shortcut where it has one |

On the web also check, in Chrome and Firefox and on a Mac browser if one is
at hand, that steps 5 to 8 work as soon as the page loads, before any
click. A key the browser keeps for itself (Cmd+W, Cmd+Shift+N, Cmd+1…9 in
some) cannot work there; that is known and tracked, not a failure of the
walk.

A step that fails is a bug to fix or file before the pull request merges,
whether or not the pull request caused it.
