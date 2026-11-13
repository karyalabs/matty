//! What someone does in the first minutes with the window, done the way
//! they do it: with the pointer and the keyboard, not by dispatching an
//! action. Each of these was broken while every test that dispatched its
//! action passed. `docs/dogfood-checklist.md` lists the same walk for a
//! person.
#![allow(clippy::unwrap_used)]

use std::time::Duration;

use gpui_kit::test::ElementSnapshot;
use gpui_kit::{
    AnyView, IntoElement, Modifiers, MouseButton, Pixels, Point, Render, TestAppContext,
    VisualTestContext, point, px,
};
use matty_app::actions::Platform;

struct Root(AnyView);

impl Render for Root {
    fn render(
        &mut self,
        _: &mut gpui_kit::Window,
        _: &mut gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        self.0.clone()
    }
}

fn open(cx: &mut TestAppContext) -> VisualTestContext {
    matty_app::preview::set("1".into());
    cx.update(matty_app::test_support::init);
    let window = cx.add_window(|window, cx| Root(matty_app::test_support::main_window(window, cx)));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.update(|window, _| window.activate_window());
    settle(&mut cx);
    cx
}

/// Lets timers and animations run out, then draws.
fn settle(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.executor().advance_clock(Duration::from_millis(400));
    }
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn find(cx: &mut VisualTestContext, label: &str) -> Option<ElementSnapshot> {
    use gpui_kit::test::TestQueryExt as _;
    cx.update(|window, _| {
        window
            .elements()
            .into_iter()
            .find(|element| element.label() == Some(label))
    })
}

fn center(cx: &mut VisualTestContext, label: &str) -> Point<Pixels> {
    find(cx, label)
        .unwrap_or_else(|| panic!("nothing is labelled {label:?}"))
        .bounds()
        .center()
}

fn hover(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.simulate_mouse_move(at, None, Modifiers::none());
    settle(cx);
}

fn click_at(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    settle(cx);
}

fn click(cx: &mut VisualTestContext, label: &str) {
    let at = center(cx, label);
    click_at(cx, at);
}

/// The sidebar is in the window's layout (its resize handle is there), not
/// hidden and not floating over the channel.
fn pinned(cx: &mut VisualTestContext) -> bool {
    find(cx, "Resize sidebar").is_some()
}

/// The sidebar's rows are on screen, pinned or peeking.
fn listed(cx: &mut VisualTestContext) -> bool {
    find(cx, "general, current destination").is_some()
}

fn message(cx: &mut VisualTestContext) -> String {
    find(cx, "Message")
        .and_then(|composer| composer.value().map(|value| value.to_string()))
        .unwrap_or_default()
}

fn in_text_field(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, _| {
        window
            .context_stack()
            .iter()
            .any(|context| context.contains("Input"))
    })
}

/// Cmd on macOS, Ctrl elsewhere.
fn primary(key: &str) -> String {
    match Platform::current() {
        Platform::MacOs => format!("cmd-{key}"),
        Platform::Other => format!("ctrl-{key}"),
    }
}

/// The pointer far from the sidebar, its edge and the header.
const AWAY: Point<Pixels> = point(px(1200.), px(600.));

/// Hiding the sidebar with its button left no way back with the pointer:
/// resting on the button peeked the sidebar over the button itself, so the
/// click that should pin it landed on the sidebar.
#[gpui_kit::test]
fn the_button_that_hides_the_sidebar_brings_it_back(cx: &mut TestAppContext) {
    let mut cx = open(cx);
    assert!(pinned(&mut cx) && listed(&mut cx));

    click(&mut cx, "Toggle sidebar");
    hover(&mut cx, AWAY);
    assert!(!pinned(&mut cx), "the button hides the sidebar");
    assert!(!listed(&mut cx), "nothing peeks with the pointer away");

    // Resting on the button peeks the sidebar, and the button stays
    // clear of it.
    let toggle = center(&mut cx, "Toggle sidebar");
    hover(&mut cx, toggle);
    assert!(listed(&mut cx), "resting on the button peeks the sidebar");
    assert!(!pinned(&mut cx), "a peek floats; it does not pin");
    let row = find(&mut cx, "general, current destination").unwrap();
    let button = find(&mut cx, "Toggle sidebar").unwrap();
    assert_eq!(
        button.bounds().center(),
        toggle,
        "the button did not move under the pointer"
    );
    assert!(
        row.bounds().origin.y >= button.bounds().bottom(),
        "the peeked sidebar hangs under the header"
    );

    // The click it was always meant to take.
    click_at(&mut cx, toggle);
    hover(&mut cx, AWAY);
    assert!(pinned(&mut cx), "a click on the button pins the sidebar");
    assert!(listed(&mut cx));
}

/// A peek ends when the pointer leaves, and a row clicked in it opens.
#[gpui_kit::test]
fn a_peeked_sidebar_leaves_with_the_pointer_and_opens_what_is_clicked(cx: &mut TestAppContext) {
    let mut cx = open(cx);
    click(&mut cx, "Toggle sidebar");
    hover(&mut cx, AWAY);

    let toggle = center(&mut cx, "Toggle sidebar");
    hover(&mut cx, toggle);
    // From the button down onto the sidebar: it stays.
    let row = center(&mut cx, "random");
    hover(&mut cx, row);
    assert!(listed(&mut cx), "the peek holds while the pointer is on it");

    click_at(&mut cx, row);
    assert!(
        find(&mut cx, "random, current destination").is_some() || !listed(&mut cx),
        "the clicked channel opened"
    );
    hover(&mut cx, AWAY);
    assert!(!listed(&mut cx), "the peek leaves with the pointer");
    assert!(!pinned(&mut cx));
}

/// Cmd/Ctrl+B is Bold in a message and shows or hides the sidebar anywhere
/// else, and Cmd/Ctrl+Shift+D does so from anywhere.
#[gpui_kit::test]
fn the_sidebar_keys_hide_and_show_it(cx: &mut TestAppContext) {
    let mut cx = open(cx);
    let (b, shift_d) = (primary("b"), primary("shift-d"));

    cx.update(|window, cx| window.blur(cx));
    settle(&mut cx);
    cx.simulate_keystrokes(&b);
    settle(&mut cx);
    assert!(!pinned(&mut cx), "{b} hides the sidebar");
    cx.simulate_keystrokes(&b);
    settle(&mut cx);
    assert!(pinned(&mut cx), "{b} shows it again");

    // With the caret in the composer the key is Bold.
    cx.dispatch_action(matty_app::actions::FocusComposer);
    settle(&mut cx);
    assert!(in_text_field(&mut cx));
    cx.simulate_keystrokes(&b);
    settle(&mut cx);
    assert!(pinned(&mut cx), "{b} is Bold while typing");
    assert_eq!(message(&mut cx), "****");

    cx.simulate_keystrokes(&shift_d);
    settle(&mut cx);
    assert!(!pinned(&mut cx), "{shift_d} hides the sidebar while typing");
    cx.simulate_keystrokes(&shift_d);
    settle(&mut cx);
    assert!(pinned(&mut cx), "{shift_d} shows it again");
}

/// After a click on a header button focus rests on the window, and what is
/// typed next went nowhere. It goes to the composer.
#[gpui_kit::test]
fn typing_after_a_click_on_a_button_goes_to_the_composer(cx: &mut TestAppContext) {
    let mut cx = open(cx);
    click(&mut cx, "Channel info");
    assert!(!in_text_field(&mut cx), "the click left the composer");

    cx.simulate_input("hi");
    settle(&mut cx);
    assert_eq!(message(&mut cx), "hi");
    assert!(in_text_field(&mut cx), "and the caret is in the composer");
}

/// A click on the right pane leaves focus resting on the pane. Typing then
/// goes to the composer in reach: the channel's beside a listing, the
/// reply box in a thread.
#[gpui_kit::test]
fn typing_after_a_click_on_the_right_pane_goes_to_the_composer_in_reach(cx: &mut TestAppContext) {
    let in_pane = |cx: &mut VisualTestContext| {
        cx.update(|window, _| {
            window
                .context_stack()
                .iter()
                .any(|context| context.contains(matty_app::actions::RIGHT_PANE))
        })
    };
    // An empty stretch of the pane's header, left of its Expand button.
    let header = |cx: &mut VisualTestContext| {
        let expand = center(cx, "Expand");
        point(expand.x - px(120.), expand.y)
    };

    let mut cx = open(cx);
    click(&mut cx, "Channel info");
    let at = header(&mut cx);
    click_at(&mut cx, at);
    assert!(in_pane(&mut cx) && !in_text_field(&mut cx));
    cx.simulate_input("hi");
    settle(&mut cx);
    assert_eq!(message(&mut cx), "hi", "beside a listing: the channel");
    assert!(!in_pane(&mut cx) && in_text_field(&mut cx));

    cx.simulate_keystrokes(&primary("a"));
    cx.simulate_keystrokes("backspace shift-up");
    settle(&mut cx);
    let at = header(&mut cx);
    click_at(&mut cx, at);
    assert!(in_pane(&mut cx) && !in_text_field(&mut cx));
    cx.simulate_input("ok");
    settle(&mut cx);
    assert!(
        in_pane(&mut cx) && in_text_field(&mut cx),
        "in a thread: its reply box"
    );
    assert_eq!(message(&mut cx), "", "and not the channel's composer");
}

/// The right pane's Expand button and its shortcut did nothing.
#[gpui_kit::test]
fn the_right_pane_expands_over_the_channel_and_back(cx: &mut TestAppContext) {
    let mut cx = open(cx);
    click(&mut cx, "Channel info");
    assert!(find(&mut cx, "Message").is_some());

    let expand = center(&mut cx, "Expand");
    click_at(&mut cx, expand);
    assert!(
        find(&mut cx, "Message").is_none(),
        "the pane takes the channel's place"
    );
    assert_eq!(
        center(&mut cx, "Collapse"),
        expand,
        "Collapse is where Expand was, so a second click undoes the first"
    );
    click(&mut cx, "Collapse");
    assert!(find(&mut cx, "Message").is_some(), "and gives it back");

    // Shift with the period key arrives as ">" without Shift on macOS and
    // Linux, and as ">" with Shift in a browser.
    for keys in [primary(">"), primary("shift->")] {
        cx.simulate_keystrokes(&keys);
        settle(&mut cx);
        assert!(
            find(&mut cx, "Message").is_none(),
            "{keys} expands the pane"
        );
        cx.simulate_keystrokes(&keys);
        settle(&mut cx);
        assert!(find(&mut cx, "Message").is_some(), "{keys} collapses it");
    }
}

/// A thread opened to reply puts the caret in its reply box.
#[gpui_kit::test]
fn opening_a_thread_to_reply_focuses_the_reply_box(cx: &mut TestAppContext) {
    let mut cx = open(cx);
    cx.dispatch_action(matty_app::actions::FocusComposer);
    settle(&mut cx);
    cx.simulate_keystrokes("shift-up");
    settle(&mut cx);
    let stack = cx.update(|window, _| window.context_stack());
    let has = |name: &str| stack.iter().any(|context| context.contains(name));
    assert!(
        has(matty_app::actions::RIGHT_PANE) && has("Input"),
        "focus is not in the reply box: {stack:?}"
    );
    cx.simulate_input("on it");
    settle(&mut cx);
    assert_eq!(message(&mut cx), "", "the channel's composer is untouched");
}
