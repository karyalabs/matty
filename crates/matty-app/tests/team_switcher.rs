//! The team switcher in the sidebar header (#735) lists the teams and the
//! other saved servers with every control named, over the
//! `MATTY_PREVIEW=servers` fixtures, and the header says what waits
//! elsewhere.
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

#[gpui_kit::test]
fn the_switcher_lists_teams_and_servers(cx: &mut gpui_kit::TestAppContext) {
    // Its own test binary, so turning the fixtures on touches nothing else.
    matty_app::preview::set("servers".into());
    cx.update(matty_app::test_support::init);
    let window = cx.add_window(|window, cx| Root(matty_app::test_support::main_window(window, cx)));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();

    // No rail: a tile in the header counts the other server's mentions, and
    // the team name beside it is the switcher.
    let drawn = labels(&mut cx);
    assert!(
        drawn
            .iter()
            .any(|(label, _)| label == "Acme Corp, 4 mentions"),
        "no server tile in the header: {drawn:?}"
    );
    let (_, header) = drawn
        .iter()
        .find(|(label, _)| label == "Matty")
        .unwrap_or_else(|| panic!("no switcher in the header: {drawn:?}"))
        .clone();
    cx.simulate_click(header, gpui_kit::Modifiers::none());
    cx.run_until_parked();

    let drawn: Vec<String> = labels(&mut cx).into_iter().map(|(l, _)| l).collect();
    for expected in [
        "Acme Corp",
        "chat.example.org",
        "Join another team\u{2026}",
        "Add server\u{2026}",
        "Manage servers\u{2026}",
    ] {
        assert!(
            drawn.iter().any(|label| label.starts_with(expected)),
            "{expected} did not draw: {drawn:?}"
        );
    }
    let unnamed = cx.update(|window, _| gpuicn::audit::unnamed_controls(window));
    assert!(unnamed.is_empty(), "unnamed controls: {unnamed:?}");
}
