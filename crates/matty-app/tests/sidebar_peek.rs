//! An unread sidebar row's peek card (spec sidebar-and-full-height-window
//! §2.5) opens from the keyboard too: Space on the focused row opens it with
//! focus inside, and Escape closes it and gives focus back to the row.
#![allow(clippy::unwrap_used)]

use gpui_kit::{AnyView, IntoElement, Render, VisualTestContext};

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

/// Whether something labelled `label` is drawn, and whether it has focus.
fn drawn(cx: &mut VisualTestContext, label: &str) -> Option<bool> {
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.update(|window, _| {
        use gpui_kit::test::TestQueryExt as _;
        window
            .elements()
            .into_iter()
            .find(|element| element.label().is_some_and(|l| l == label))
            .map(|element| element.focused() == Some(true))
    })
}

#[gpui_kit::test]
fn space_on_an_unread_row_opens_its_card_and_escape_returns(cx: &mut gpui_kit::TestAppContext) {
    matty_app::preview::set("1".into());
    cx.update(matty_app::test_support::init);
    let window = cx.add_window(|window, cx| Root(matty_app::test_support::main_window(window, cx)));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();

    // Tab into the list, at the open channel, and step down to the unread
    // town-square row under it.
    let mut reached = false;
    for _ in 0..200 {
        if drawn(&mut cx, "general, current destination") == Some(true) {
            reached = true;
            break;
        }
        cx.update(|window, cx| window.focus_next(cx));
    }
    assert!(reached, "Tab never reached the sidebar");
    cx.simulate_keystrokes("down");
    cx.run_until_parked();
    assert_eq!(
        drawn(&mut cx, "town-square"),
        Some(true),
        "Down reaches town-square"
    );
    assert_eq!(drawn(&mut cx, "Mark read"), None, "no card yet");

    cx.simulate_keystrokes("space");
    cx.run_until_parked();
    assert!(
        drawn(&mut cx, "Mark read").is_some(),
        "Space opens the card"
    );
    assert_eq!(
        drawn(&mut cx, "town-square"),
        Some(false),
        "focus is in the card"
    );

    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(drawn(&mut cx, "Mark read"), None, "Escape closes it");
    assert_eq!(
        drawn(&mut cx, "town-square"),
        Some(true),
        "focus is back on the row"
    );
}

/// The center of the first element labelled `label`.
fn center(cx: &mut VisualTestContext, label: &str) -> gpui_kit::Point<gpui_kit::Pixels> {
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.update(|window, _| {
        use gpui_kit::test::TestQueryExt as _;
        window
            .elements()
            .into_iter()
            .find(|element| element.label().is_some_and(|l| l == label))
            .unwrap_or_else(|| panic!("nothing is labelled {label:?}"))
            .bounds()
            .center()
    })
}

/// Rests the pointer on `at` (with the left button held when `held`) for
/// longer than the card's delay.
fn rest(cx: &mut VisualTestContext, at: gpui_kit::Point<gpui_kit::Pixels>, held: bool) {
    let button = held.then_some(gpui_kit::MouseButton::Left);
    cx.simulate_mouse_move(at, button, gpui_kit::Modifiers::none());
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(1));
    cx.run_until_parked();
}

fn preview(cx: &mut gpui_kit::TestAppContext) -> VisualTestContext {
    matty_app::preview::set("1".into());
    cx.update(matty_app::test_support::init);
    let window = cx.add_window(|window, cx| Root(matty_app::test_support::main_window(window, cx)));
    let cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();
    cx
}

#[gpui_kit::test]
fn no_card_opens_while_a_sidebar_menu_is_open(cx: &mut gpui_kit::TestAppContext) {
    let mut cx = preview(cx);
    let random = center(&mut cx, "random");
    cx.simulate_mouse_down(
        random,
        gpui_kit::MouseButton::Right,
        gpui_kit::Modifiers::none(),
    );
    cx.simulate_mouse_up(
        random,
        gpui_kit::MouseButton::Right,
        gpui_kit::Modifiers::none(),
    );
    cx.run_until_parked();
    if drawn(&mut cx, "Leave channel").is_none() {
        let labels: Vec<String> = cx.update(|window, _| {
            use gpui_kit::test::TestQueryExt as _;
            window
                .elements()
                .into_iter()
                .filter_map(|e| e.label().map(|l| l.to_string()))
                .collect()
        });
        panic!("the row menu did not open: {labels:?}");
    }
    let town_square = center(&mut cx, "town-square");
    rest(&mut cx, town_square, false);
    assert_eq!(
        drawn(&mut cx, "Mark read"),
        None,
        "no card over an open menu"
    );

    // With the menu closed, the same rest opens it.
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    rest(&mut cx, town_square, false);
    cx.simulate_mouse_move(town_square, None, gpui_kit::Modifiers::none());
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(1));
    cx.run_until_parked();
    assert!(
        drawn(&mut cx, "Mark read").is_some(),
        "the card opens once the menu is gone"
    );
}

#[gpui_kit::test]
fn no_card_opens_while_a_row_is_dragged(cx: &mut gpui_kit::TestAppContext) {
    let mut cx = preview(cx);
    let (random, town_square) = (center(&mut cx, "random"), center(&mut cx, "town-square"));
    cx.simulate_mouse_down(
        random,
        gpui_kit::MouseButton::Left,
        gpui_kit::Modifiers::none(),
    );
    let step = gpui_kit::point(random.x, random.y - gpui_kit::px(10.));
    cx.simulate_mouse_move(
        step,
        Some(gpui_kit::MouseButton::Left),
        gpui_kit::Modifiers::none(),
    );
    cx.run_until_parked();
    assert!(
        cx.update(|_, cx| cx.has_active_drag()),
        "the row is being dragged"
    );
    rest(&mut cx, town_square, true);
    assert_eq!(drawn(&mut cx, "Mark read"), None, "no card while dragging");
    cx.simulate_mouse_up(
        town_square,
        gpui_kit::MouseButton::Left,
        gpui_kit::Modifiers::none(),
    );
    cx.run_until_parked();
}
