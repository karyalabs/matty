//! Clicking a sidebar channel puts focus in the composer, so Alt+Shift+Up
//! and Down keep walking unread channels after a click, and only move a row
//! when the list was reached from the keyboard (#505).
#![allow(clippy::unwrap_used)]

use gpui_kit::{AnyView, IntoElement, Modifiers, MouseButton, Render, VisualTestContext};

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

/// The sidebar row named `name`; the open one is "name, current destination".
fn row(cx: &mut VisualTestContext, name: &str) -> gpui_kit::test::ElementSnapshot {
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.update(|window, _| {
        use gpui_kit::test::TestQueryExt as _;
        window
            .elements()
            .into_iter()
            .find(|element| {
                element.label().is_some_and(|label| {
                    label == name || label == format!("{name}, current destination")
                })
            })
            .unwrap()
    })
}

fn row_top(cx: &mut VisualTestContext, name: &str) -> gpui_kit::Pixels {
    row(cx, name).bounds().origin.y
}

fn is_open(cx: &mut VisualTestContext, name: &str) -> bool {
    row(cx, name)
        .label()
        .is_some_and(|label| label.ends_with("current destination"))
}

#[gpui_kit::test]
fn a_clicked_channel_focuses_the_composer_and_keeps_its_place(cx: &mut gpui_kit::TestAppContext) {
    matty_app::preview::set("1".into());
    cx.update(matty_app::test_support::init);
    let window = cx.add_window(|window, cx| Root(matty_app::test_support::main_window(window, cx)));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();

    let (random, town_square) = (row_top(&mut cx, "random"), row_top(&mut cx, "town-square"));
    assert!(town_square < random, "the fixtures list town-square first");
    let center = row(&mut cx, "random").bounds().center();
    cx.simulate_mouse_down(center, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(center, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    // Focus sits in a text input (the composer) and not in the sidebar.
    let contexts = cx.update(|window, _| format!("{:?}", window.context_stack()));
    let composer_focused = contexts.contains("Input") && !contexts.contains("SidebarSortable");
    assert!(composer_focused, "focus is not in the composer: {contexts}");

    // From the composer the keys walk unreads; the row stays where it was.
    cx.simulate_keystrokes("alt-shift-up");
    cx.run_until_parked();
    assert!(
        is_open(&mut cx, "town-square"),
        "the previous unread opened"
    );
    assert_eq!(row_top(&mut cx, "random"), random);
    assert_eq!(row_top(&mut cx, "town-square"), town_square);
}

/// Losing focus, as when the menu or channel that held it closes, hands it
/// back to the window, so its shortcuts (Settings, the quick switcher) keep
/// working without a click in the composer first.
#[gpui_kit::test]
fn shortcuts_keep_working_after_focus_is_lost(cx: &mut gpui_kit::TestAppContext) {
    matty_app::preview::set("1".into());
    cx.update(matty_app::test_support::init);
    let window = cx.add_window(|window, cx| Root(matty_app::test_support::main_window(window, cx)));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));

    cx.update(|window, cx| window.blur(cx));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let contexts = cx.update(|window, _| format!("{:?}", window.context_stack()));
    assert!(
        contexts.contains("MainWindow"),
        "the window did not take focus back: {contexts}"
    );

    // Cmd/Ctrl+K opens the quick switcher and its list of channels.
    let quick_switcher = if cfg!(target_os = "macos") {
        "cmd-k"
    } else {
        "ctrl-k"
    };
    cx.simulate_keystrokes(quick_switcher);
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let options = cx.update(|window, _| {
        use gpui_kit::test::TestQueryExt as _;
        window
            .elements()
            .into_iter()
            .filter(|element| element.role() == Some(gpui_kit::Role::ListBoxOption))
            .count()
    });
    assert!(options > 0, "Cmd/Ctrl+K did not open the quick switcher");
}

/// The labels of everything drawn now.
fn labels(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.update(|window, _| {
        use gpui_kit::test::TestQueryExt as _;
        window
            .elements()
            .into_iter()
            .filter_map(|element| element.label().map(str::to_owned))
            .collect()
    })
}

/// A channel picked in the quick switcher opens with the caret in its
/// composer, as in the official clients, so Shift+Up right after it
/// replies to the last message: the benchmark's large-thread step pressed
/// it and Matty, with focus handed back to the window, did nothing.
#[gpui_kit::test]
fn a_switcher_pick_focuses_the_composer_for_shift_up(cx: &mut gpui_kit::TestAppContext) {
    matty_app::preview::set("1".into());
    cx.update(matty_app::test_support::init);
    let window = cx.add_window(|window, cx| Root(matty_app::test_support::main_window(window, cx)));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    // Nothing in the window holds focus, as after launch or a closed menu.
    cx.update(|window, cx| window.blur(cx));
    cx.update(|window, cx| window.draw(cx).clear(cx));

    let quick_switcher = if cfg!(target_os = "macos") {
        "cmd-k"
    } else {
        "ctrl-k"
    };
    cx.simulate_keystrokes(quick_switcher);
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.simulate_input("random");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(
        is_open(&mut cx, "random"),
        "the switcher did not open random"
    );
    let contexts = cx.update(|window, _| format!("{:?}", window.context_stack()));
    assert!(
        contexts.contains("Composer"),
        "focus is not in the composer: {contexts}"
    );

    assert!(!labels(&mut cx).iter().any(|label| label == "Reply"));
    cx.simulate_keystrokes("shift-up");
    cx.run_until_parked();
    assert!(
        labels(&mut cx).iter().any(|label| label == "Reply"),
        "Shift+Up did not open the last message's thread"
    );
}
