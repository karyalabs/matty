//! Every row of the keymap reaches its action from each place focus rests
//! in the main window, pressed the way a keyboard sends it.
//!
//! A shortcut can be bound, listed in Settings and shown in a tooltip and
//! still do nothing: no view handles its action, a text field binds the
//! same key deeper and wins while the caret is in the composer, or the row
//! names Shift with a symbol key, which no platform reports as written.
//! Tests that dispatch the action, or replay the row's keys from the
//! window's root, pass through all three. This one presses the keys with
//! focus where a user has it and asks GPUI which action ran.
#![allow(clippy::unwrap_used)]

use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::{
    AnyView, IntoElement, KeyBinding, KeyBindingContextPredicate, KeyContext, Modifiers,
    MouseButton, Render, TestAppContext, VisualTestContext,
};
use matty_app::actions::{self, Platform};

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

/// Rows that are not expected to run an action here, and why.
const ELSEWHERE: [(&str, &str); 8] = [
    // The composer reads these keys itself, so that the same keys format
    // an edit and a reply (`channel::format_shortcut`, tested there).
    ("Bold", "the composer handles the key"),
    ("Italic", "the composer handles the key"),
    ("Link", "the composer handles the key"),
    ("Strikethrough", "the composer handles the key"),
    ("Inline code", "the composer handles the key"),
    // Up and Shift+Up in an empty composer, read the same way
    // (`Place::Reply` gets to the reply box with Shift+Up).
    ("Edit last message", "the composer handles the key"),
    ("Reply to last message", "the composer handles the key"),
    // Handled for every window by `menu::init`, which tests leave out.
    ("Close window", "handled app-wide"),
];

/// Where focus rests when the keys are pressed.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Place {
    /// On the window itself, as after a click on a header button or a
    /// closed menu.
    Window,
    /// In the channel's composer, where it is nearly all the time.
    Composer,
    /// In the composer with a message typed, so the rows of an empty
    /// composer are out and its editing keys are live.
    Draft,
    /// In a thread's reply composer, in the right pane.
    Reply,
    /// On a row of the sidebar's list.
    Sidebar,
}

fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn open(cx: &mut TestAppContext) -> VisualTestContext {
    let window = cx.add_window(|window, cx| Root(matty_app::test_support::main_window(window, cx)));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.update(|window, _| window.activate_window());
    draw(&mut cx);
    cx
}

fn click(cx: &mut VisualTestContext, label: &str) {
    use gpui_kit::test::TestQueryExt as _;
    draw(cx);
    let at = cx.update(|window, _| {
        window
            .elements()
            .into_iter()
            .find(|element| element.label() == Some(label))
            .unwrap_or_else(|| panic!("nothing is labelled {label:?}"))
            .bounds()
            .center()
    });
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    draw(cx);
}

/// Puts focus at `place` and returns the key contexts from the window down
/// to it.
fn settle(place: Place, cx: &mut VisualTestContext) -> Vec<KeyContext> {
    match place {
        Place::Window => cx.update(|window, cx| window.blur(cx)),
        Place::Composer => cx.dispatch_action(actions::FocusComposer),
        Place::Draft => {
            cx.dispatch_action(actions::FocusComposer);
            draw(cx);
            cx.simulate_input("hello");
        }
        Place::Reply => {
            cx.dispatch_action(actions::FocusComposer);
            draw(cx);
            // Shift+Up in an empty composer replies to the last message.
            cx.simulate_keystrokes("shift-up");
        }
        Place::Sidebar => click(cx, "Channels"),
    }
    draw(cx);
    draw(cx);
    let stack = cx.update(|window, _| window.context_stack());
    let has = |name: &str| stack.iter().any(|context| context.contains(name));
    let there = match place {
        Place::Window => stack.len() == 1 && has(actions::MAIN_WINDOW),
        Place::Composer => has("Input") && has("empty") && has(actions::CHANNEL_VIEW),
        Place::Draft => has("Input") && !has("empty") && has(actions::CHANNEL_VIEW),
        Place::Reply => has("Input") && has(actions::RIGHT_PANE),
        Place::Sidebar => has(actions::SIDEBAR_SORTABLE),
    };
    assert!(there, "focus is not at {place:?}: {stack:?}");
    stack
}

/// Presses every row's keys at `place`, each in a window of its own since
/// most rows open something, and checks the row's action ran, or did not
/// where the row's context leaves the place out.
fn reach(place: Place, cx: &mut TestAppContext) {
    matty_app::preview::set("1".into());
    cx.update(matty_app::test_support::init);
    let platform = Platform::current();
    let ran: Rc<RefCell<Option<String>>> = Rc::default();
    let _watch = cx.update(|cx| {
        let ran = ran.clone();
        cx.observe_keystrokes(move |event, _, _| {
            *ran.borrow_mut() = event.action.as_ref().map(|action| action.name().to_owned());
        })
    });
    let mut dead = Vec::new();
    for row in actions::bindings() {
        if ELSEWHERE.iter().any(|(label, _)| *label == row.label) {
            continue;
        }
        for keys in actions::spellings(row.keys(platform)) {
            let mut cx = open(cx);
            let stack = settle(place, &mut cx);
            let applies = row.context.is_none_or(|context| {
                KeyBindingContextPredicate::parse(context)
                    .unwrap()
                    .depth_of(&stack)
                    .is_some()
            });
            *ran.borrow_mut() = None;
            cx.simulate_keystrokes(keys);
            let wanted = (row.action)().name().to_owned();
            let ran = ran.borrow().clone();
            if applies && ran.as_deref() != Some(wanted.as_str()) {
                dead.push(format!(
                    "{} ({keys}) ran {} instead of {wanted}",
                    row.label,
                    ran.as_deref().unwrap_or("nothing")
                ));
            }
            // A row that leaves this place out stays out of it: Focus
            // search is not for a text field, nor the unread jump for the
            // sidebar's list.
            if !applies && ran.as_deref() == Some(wanted.as_str()) {
                dead.push(format!(
                    "{} ({keys}) ran {wanted}, which its row keeps out of here",
                    row.label
                ));
            }
            cx.update(|window, _| window.remove_window());
            cx.run_until_parked();
        }
    }
    assert!(
        dead.is_empty(),
        "with focus at {place:?}, these shortcuts do not reach their action:\n  {}",
        dead.join("\n  ")
    );
}

#[gpui_kit::test]
fn every_shortcut_works_with_focus_on_the_window(cx: &mut TestAppContext) {
    reach(Place::Window, cx);
}

#[gpui_kit::test]
fn every_shortcut_works_from_the_composer(cx: &mut TestAppContext) {
    reach(Place::Composer, cx);
}

#[gpui_kit::test]
fn every_shortcut_works_while_typing_a_message(cx: &mut TestAppContext) {
    reach(Place::Draft, cx);
}

#[gpui_kit::test]
fn every_shortcut_works_from_a_reply(cx: &mut TestAppContext) {
    reach(Place::Reply, cx);
}

#[gpui_kit::test]
fn every_shortcut_works_from_the_sidebar(cx: &mut TestAppContext) {
    reach(Place::Sidebar, cx);
}

/// A text field that binds a window shortcut's key for its own editing
/// does not take the shortcut away: on the web Alt+Left and Alt+Right move
/// by word in a field, and they stayed Back and Forward only outside it.
#[gpui_kit::test]
fn a_window_shortcut_beats_a_text_field_that_binds_its_key(cx: &mut TestAppContext) {
    use gpui_kit::base::input::MoveToPreviousWord;
    matty_app::preview::set("1".into());
    cx.update(|cx| {
        matty_app::test_support::init(cx);
        // As GPUI Kit and gpuicn bind their fields' keys: before Matty's.
        cx.bind_keys([KeyBinding::new(
            "alt-left",
            MoveToPreviousWord,
            Some("Input"),
        )]);
        actions::bind(cx);
    });
    let ran: Rc<RefCell<Option<String>>> = Rc::default();
    let _watch = cx.update(|cx| {
        let ran = ran.clone();
        cx.observe_keystrokes(move |event, _, _| {
            *ran.borrow_mut() = event.action.as_ref().map(|action| action.name().to_owned());
        })
    });
    let mut cx = open(cx);
    settle(Place::Draft, &mut cx);
    let back = actions::bindings()
        .into_iter()
        .find(|row| row.label == "Back")
        .unwrap();
    // Only where Back is Alt+Left; macOS has Cmd+[.
    if back.keys(Platform::current()) == "alt-left" {
        cx.simulate_keystrokes("alt-left");
        assert_eq!(ran.borrow().as_deref(), Some("matty::HistoryBack"));
    }
}
