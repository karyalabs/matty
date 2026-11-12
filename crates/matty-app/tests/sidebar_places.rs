//! The sidebar is a list of places with exactly one selected row; Mentions
//! and Saved are panels that never select a row
//! (docs/specs/sidebar-places-and-switching.md §1, matty#878).
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

fn labels(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.update(|window, _| {
        use gpui_kit::test::TestQueryExt as _;
        window
            .elements()
            .into_iter()
            .filter_map(|element| element.label().map(|label| label.to_string()))
            .collect()
    })
}

/// The rows the sidebar marks as the current destination.
fn current(cx: &mut VisualTestContext) -> Vec<String> {
    labels(cx)
        .into_iter()
        .filter_map(|label| {
            label
                .strip_suffix(", current destination")
                .map(str::to_owned)
        })
        .collect()
}

fn click(cx: &mut VisualTestContext, starts_with: &str) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let center = cx.update(|window, _| {
        use gpui_kit::test::TestQueryExt as _;
        window
            .elements()
            .into_iter()
            .find(|element| element.label().is_some_and(|l| l.starts_with(starts_with)))
            .unwrap_or_else(|| panic!("no element labelled {starts_with}"))
            .bounds()
            .center()
    });
    cx.simulate_mouse_down(center, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(center, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
}

#[gpui_kit::test]
fn one_row_is_selected_and_panels_never_select_one(cx: &mut gpui_kit::TestAppContext) {
    matty_app::preview::set("1".into());
    cx.update(matty_app::test_support::init);
    let window = cx.add_window(|window, cx| Root(matty_app::test_support::main_window(window, cx)));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();

    // The listing rows are gone; Threads stays, Drafts shows with drafts.
    let all = labels(&mut cx);
    for gone in ["Unreads", "Mentions", "Saved"] {
        assert!(!all.iter().any(|l| l == gone), "{gone} is still a row");
    }
    assert_eq!(current(&mut cx), vec!["general".to_owned()]);

    // Threads is a place: it alone is selected.
    click(&mut cx, "Threads");
    assert_eq!(current(&mut cx), vec!["Threads".to_owned()]);

    // Mentions is a panel: from Threads it goes back to the channel, which
    // alone is selected again. (The Threads header has no room for the
    // button; its shortcut does the same.)
    cx.update(|window, cx| {
        window.dispatch_action(Box::new(matty_app::actions::RecentMentions), cx)
    });
    cx.run_until_parked();
    assert_eq!(current(&mut cx), vec!["general".to_owned()]);

    // The header button toggles the panel and still selects no row.
    click(&mut cx, "Recent mentions");
    assert_eq!(current(&mut cx), vec!["general".to_owned()]);
}
