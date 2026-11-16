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

/// Join another team, from any team's tabs, closes on Escape and on a click
/// outside it, with one backdrop under it (matty#941: on a second team
/// neither worked and the backdrop was dimmer).
#[gpui_kit::test]
fn join_a_team_dismisses_from_every_team(cx: &mut gpui_kit::TestAppContext) {
    matty_app::preview::set("1".into());
    cx.update(matty_app::test_support::init);
    let window = cx.add_window(|window, cx| Root(matty_app::test_support::main_window(window, cx)));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();

    // Wide enough for every tab, so the tabs end in the Join button.
    cx.simulate_resize(gpui_kit::size(gpui_kit::px(1400.), gpui_kit::px(900.)));
    cx.run_until_parked();
    let grip = find(&mut cx, "Resize sidebar");
    cx.simulate_mouse_down(grip, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(
        grip + gpui_kit::point(gpui_kit::px(300.), gpui_kit::px(0.)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    cx.simulate_mouse_up(
        grip + gpui_kit::point(gpui_kit::px(300.), gpui_kit::px(0.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    let open = |cx: &mut VisualTestContext| {
        let join = find(cx, "Join another team");
        cx.simulate_click(join, Modifiers::none());
        cx.run_until_parked();
        assert!(drew(cx, "Paste an invite link"), "the dialog opened");
    };

    for team in [
        "Matty",
        "Design Guild, 3 mentions",
        "Open Source, unread messages",
    ] {
        let tab = find(&mut cx, team);
        cx.simulate_click(tab, Modifiers::none());
        cx.run_until_parked();

        open(&mut cx);
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(
            !drew(&mut cx, "Paste an invite link"),
            "Escape closes it on {team}"
        );

        open(&mut cx);
        let corner = gpui_kit::point(gpui_kit::px(4.), gpui_kit::px(4.));
        cx.simulate_click(corner, Modifiers::none());
        cx.run_until_parked();
        assert!(
            !drew(&mut cx, "Paste an invite link"),
            "a click outside closes it on {team}"
        );
    }
}

/// With no team left to join there is no search field, so the dialog must
/// not send focus to it: Escape and a click outside still close it, and
/// typing goes into the invite link (matty#941).
#[gpui_kit::test]
fn join_a_team_with_nothing_to_join_still_dismisses(cx: &mut gpui_kit::TestAppContext) {
    matty_app::preview::set("1".into());
    cx.update(matty_app::test_support::init);
    let window = cx.add_window(|window, cx| Root(matty_app::test_support::main_window(window, cx)));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();

    let open = |cx: &mut VisualTestContext| {
        let more = find(cx, "More teams");
        cx.simulate_click(more, Modifiers::none());
        cx.run_until_parked();
        let join = find(cx, "Join another team\u{2026}");
        cx.simulate_click(join, Modifiers::none());
        cx.run_until_parked();
    };
    for team in ["Customer Success", "Platform", "Social"] {
        open(&mut cx);
        let button = find(&mut cx, &format!("Join {team}"));
        cx.simulate_click(button, Modifiers::none());
        cx.run_until_parked();
    }

    open(&mut cx);
    assert!(drew(&mut cx, "Paste an invite link"), "the dialog opened");
    assert!(!drew(&mut cx, "Search teams"), "nothing to search");
    cx.simulate_input("x");
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let typed = cx.update(|window, _| {
        use gpui_kit::test::TestQueryExt as _;
        window.elements().iter().any(|element| {
            element
                .label()
                .is_some_and(|label| label == "Paste an invite link")
                && element.value().is_some_and(|value| value == "x")
        })
    });
    assert!(typed, "typing goes into the invite link");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(!drew(&mut cx, "Paste an invite link"), "Escape closes it");

    open(&mut cx);
    let corner = gpui_kit::point(gpui_kit::px(4.), gpui_kit::px(4.));
    cx.simulate_click(corner, Modifiers::none());
    cx.run_until_parked();
    assert!(
        !drew(&mut cx, "Paste an invite link"),
        "a click outside closes it"
    );
}

/// A team tab's menu takes the keyboard like any menu: Escape closes it,
/// and Down and Enter pick Leave team (matty#943).
#[gpui_kit::test]
fn a_team_tabs_menu_answers_the_keyboard(cx: &mut gpui_kit::TestAppContext) {
    matty_app::preview::set("1".into());
    cx.update(matty_app::test_support::init);
    let window = cx.add_window(|window, cx| Root(matty_app::test_support::main_window(window, cx)));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();

    for team in ["Matty", "Design Guild, 3 mentions"] {
        let tab = find(&mut cx, team);
        cx.simulate_mouse_down(tab, MouseButton::Right, Modifiers::none());
        cx.simulate_mouse_up(tab, MouseButton::Right, Modifiers::none());
        cx.run_until_parked();
        assert!(
            drew(&mut cx, "Leave team\u{2026}"),
            "the menu opened on {team}"
        );
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(
            !drew(&mut cx, "Leave team\u{2026}"),
            "Escape closes the menu on {team}"
        );
    }

    let tab = find(&mut cx, "Matty");
    cx.simulate_mouse_down(tab, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(tab, MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(drew(&mut cx, "Leave team"), "Enter picks Leave team");
}

/// Leaving the team on screen moves the window to the team sync shows
/// next, and the channel left behind does not stay open over that team's
/// sidebar (matty#948).
#[gpui_kit::test]
fn leaving_the_team_on_screen_leaves_its_channel_too(cx: &mut gpui_kit::TestAppContext) {
    matty_app::preview::set("1".into());
    cx.update(matty_app::test_support::init);
    let window = cx.add_window(|window, cx| Root(matty_app::test_support::main_window(window, cx)));
    let main = window.read_with(cx, |root, _| root.0.clone()).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();
    let channel = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| matty_app::test_support::main_channel(&main, cx))
    };
    let before = channel(&mut cx).expect("the preview opens a channel");

    cx.update(|window, cx| {
        matty_app::test_support::sync_teams(&main, &["design-guild", "open-source"], window, cx)
    });
    cx.run_until_parked();
    assert!(drew(&mut cx, "critique"), "Design Guild's sidebar shows");
    let matty: Vec<String> = matty_app::preview::team_sidebar("matty")
        .categories
        .iter()
        .flat_map(|category| category.rows.iter().map(|row| row.id.to_string()))
        .collect();
    assert!(matty.contains(&before));
    let after = channel(&mut cx);
    assert!(
        after.as_ref().is_none_or(|id| !matty.contains(id)),
        "no channel of the left team stays open: {after:?}"
    );
}
