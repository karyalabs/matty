//! Starting a direct or group message from New message over the preview
//! fixtures: clicking a person, or Enter on a search, closes the dialog and
//! opens the conversation (matty#955); ticking several people and Go opens
//! their group message.
#![allow(clippy::unwrap_used)]

use gpui_kit::{AnyView, IntoElement, Modifiers, Render, VisualTestContext};

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

fn open_new_message(cx: &mut gpui_kit::TestAppContext) -> VisualTestContext {
    matty_app::preview::set("1".into());
    cx.update(matty_app::test_support::init);
    let window = cx.add_window(|window, cx| Root(matty_app::test_support::main_window(window, cx)));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();
    cx.simulate_resize(gpui_kit::size(gpui_kit::px(1400.), gpui_kit::px(900.)));
    cx.run_until_parked();
    let open = find(&mut cx, "New message in Direct messages");
    cx.simulate_click(open, Modifiers::none());
    cx.run_until_parked();
    assert!(drew(&mut cx, "Search people"), "the dialog opened");
    cx
}

/// The sidebar marks the open conversation.
fn shows_conversation_with(cx: &mut VisualTestContext, name: &str) -> bool {
    labels(cx)
        .iter()
        .any(|(label, _)| label.starts_with(name) && label.ends_with("current destination"))
}

#[gpui_kit::test]
fn clicking_a_person_opens_their_direct_message(cx: &mut gpui_kit::TestAppContext) {
    let mut cx = open_new_message(cx);
    // The person's avatar in the list, which draws after the search.
    let drawn = labels(&mut cx);
    let search = drawn
        .iter()
        .position(|(label, _)| label == "Search people")
        .unwrap();
    let (_, at) = drawn[search + 1..]
        .iter()
        .find(|(label, _)| label == "Lena Fischer")
        .cloned()
        .unwrap();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    assert!(!drew(&mut cx, "Search people"), "the dialog closed");
    assert!(shows_conversation_with(&mut cx, "Lena Fischer"));
}

#[gpui_kit::test]
fn enter_messages_the_first_match(cx: &mut gpui_kit::TestAppContext) {
    let mut cx = open_new_message(cx);
    let search = find(&mut cx, "Search people");
    cx.simulate_click(search, Modifiers::none());
    cx.simulate_input("lena");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(!drew(&mut cx, "Search people"), "the dialog closed");
    assert!(shows_conversation_with(&mut cx, "Lena Fischer"));
}

fn click(cx: &mut VisualTestContext, name: &str) {
    let at = find(cx, name);
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
}

#[gpui_kit::test]
fn ticking_people_and_go_opens_their_group_message(cx: &mut gpui_kit::TestAppContext) {
    let mut cx = open_new_message(cx);
    click(&mut cx, "Add Lena Fischer to the group");
    assert!(
        drew(&mut cx, "Search people"),
        "ticking keeps the dialog open"
    );
    // Once a group is being picked, a click on the row ticks too.
    let drawn = labels(&mut cx);
    let search = drawn
        .iter()
        .position(|(label, _)| label == "Search people")
        .unwrap();
    let (_, sam) = drawn[search + 1..]
        .iter()
        .find(|(label, _)| label == "Sam Okafor")
        .cloned()
        .unwrap();
    cx.simulate_click(sam, Modifiers::none());
    cx.run_until_parked();
    assert!(drew(&mut cx, "Search people"), "the row ticked Sam");
    assert!(drew(&mut cx, "Remove Lena Fischer"), "Lena shows as picked");
    assert!(drew(&mut cx, "Remove Sam Okafor"), "Sam shows as picked");
    click(&mut cx, "Message 2 people");
    assert!(!drew(&mut cx, "Search people"), "the dialog closed");
    assert!(shows_conversation_with(&mut cx, "Lena Fischer, Sam Okafor"));
}

#[gpui_kit::test]
fn enter_ticks_the_first_match_then_opens_the_group(cx: &mut gpui_kit::TestAppContext) {
    let mut cx = open_new_message(cx);
    click(&mut cx, "Add Jonas Weber to the group");
    click(&mut cx, "Search people");
    cx.simulate_input("priya");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(drew(&mut cx, "Remove Priya Nair"), "Enter ticked Priya");
    assert!(drew(&mut cx, "Search people"), "the dialog is still open");
    // The search emptied, so Enter now starts the conversation.
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(!drew(&mut cx, "Search people"), "the dialog closed");
    assert!(shows_conversation_with(&mut cx, "Jonas Weber, Priya Nair"));
}

#[gpui_kit::test]
fn removing_the_last_tick_returns_to_one_click_messages(cx: &mut gpui_kit::TestAppContext) {
    let mut cx = open_new_message(cx);
    click(&mut cx, "Add Sam Okafor to the group");
    click(&mut cx, "Remove Sam Okafor");
    assert!(!drew(&mut cx, "Remove Sam Okafor"));
    click(&mut cx, "Search people");
    cx.simulate_input("lena");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(!drew(&mut cx, "Search people"), "the dialog closed");
    assert!(shows_conversation_with(&mut cx, "Lena Fischer"));
}

#[gpui_kit::test]
fn add_members_on_a_group_starts_a_bigger_one(cx: &mut gpui_kit::TestAppContext) {
    let mut cx = open_new_message(cx);
    click(&mut cx, "Add Lena Fischer to the group");
    click(&mut cx, "Add Sam Okafor to the group");
    click(&mut cx, "Message 2 people");
    assert!(shows_conversation_with(&mut cx, "Lena Fischer, Sam Okafor"));
    click(&mut cx, "Channel menu");
    click(&mut cx, "Add members…");
    assert!(drew(&mut cx, "Search people"), "New message opened");
    assert!(
        drew(&mut cx, "Remove Lena Fischer"),
        "the members come ticked"
    );
    assert!(
        drew(&mut cx, "Remove Sam Okafor"),
        "the members come ticked"
    );
    click(&mut cx, "Add Jonas Weber to the group");
    click(&mut cx, "Message 3 people");
    assert!(shows_conversation_with(
        &mut cx,
        "Lena Fischer, Sam Okafor, Jonas Weber"
    ));
}
