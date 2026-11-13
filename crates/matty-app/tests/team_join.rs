//! Joining and leaving teams (matty#745) over the preview fixtures: the
//! team tabs' More menu opens Join a team with the server's open teams, joining
//! one adds its tab and shows it, and a tab's right-click menu leaves it
//! after asking.
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

fn labels(cx: &mut VisualTestContext) -> Vec<(String, gpui_kit::Point<gpui_kit::Pixels>)> {
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.update(|window, _| {
        use gpui_kit::test::TestQueryExt as _;
        window
            .elements()
            .iter()
            .filter_map(|element| {
                let label = element.label()?.to_string();
                Some((label, element.bounds().center()))
            })
            .collect()
    })
}

/// Where the last control named `name` drew.
fn find(cx: &mut VisualTestContext, name: &str) -> gpui_kit::Point<gpui_kit::Pixels> {
    let drawn = labels(cx);
    drawn
        .iter()
        .rev()
        .find(|(label, _)| label == name)
        .map(|(_, at)| *at)
        .unwrap_or_else(|| {
            let names: Vec<_> = drawn.iter().map(|(label, _)| label).collect();
            panic!("{name} did not draw: {names:?}")
        })
}

fn drew(cx: &mut VisualTestContext, name: &str) -> bool {
    labels(cx).iter().any(|(label, _)| label == name)
}

#[gpui_kit::test]
fn a_team_joins_from_the_tabs_and_leaves_from_its_menu(cx: &mut gpui_kit::TestAppContext) {
    // Its own test binary, so turning the fixtures on touches nothing else.
    matty_app::preview::set("1".into());
    cx.update(matty_app::test_support::init);
    let window = cx.add_window(|window, cx| Root(matty_app::test_support::main_window(window, cx)));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();

    // The test window is narrow, so the tabs end in More, which holds
    // Join another team.
    let more = find(&mut cx, "More teams");
    cx.simulate_click(more, Modifiers::none());
    cx.run_until_parked();
    let join = find(&mut cx, "Join another team\u{2026}");
    cx.simulate_click(join, Modifiers::none());
    cx.run_until_parked();
    assert!(drew(&mut cx, "Join Platform"));
    assert!(drew(&mut cx, "Paste an invite link"));
    let unnamed = cx.update(|window, _| gpuicn::audit::unnamed_controls(window));
    assert!(unnamed.is_empty(), "unnamed controls: {unnamed:?}");

    let platform = find(&mut cx, "Join Platform");
    cx.simulate_click(platform, Modifiers::none());
    cx.run_until_parked();
    assert!(!drew(&mut cx, "Join Platform"), "the dialog closed");
    assert!(drew(&mut cx, "Platform"), "the joined team has a tab");

    let tab = find(&mut cx, "Platform");
    cx.simulate_mouse_down(tab, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(tab, MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    let leave = find(&mut cx, "Leave team\u{2026}");
    cx.simulate_click(leave, Modifiers::none());
    cx.run_until_parked();
    let confirm = find(&mut cx, "Leave team");
    cx.simulate_click(confirm, Modifiers::none());
    cx.run_until_parked();
    assert!(!drew(&mut cx, "Platform"), "the team's tab is gone");
}
