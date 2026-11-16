//! Closing the main window where Matty keeps running (macOS) hides the app
//! and leaves the window, and with it sync and notifications, in place
//! (#929). Closing it outright dropped every server's connection until a
//! dock click opened a new window, so no new message and no notification
//! arrived while it was closed.
#![allow(clippy::unwrap_used)]

use gpui_kit::{AnyView, IntoElement, Render, TestAppContext, VisualTestContext};

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

fn open(cx: &mut TestAppContext) -> (VisualTestContext, std::rc::Rc<std::cell::Cell<usize>>) {
    matty_app::preview::set("1".into());
    cx.update(matty_app::test_support::init);
    let hidden = cx.update(matty_app::test_support::close_hides);
    let window = cx.add_window(|window, cx| Root(matty_app::test_support::main_window(window, cx)));
    let cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();
    (cx, hidden)
}

#[gpui_kit::test]
fn the_close_shortcut_hides_matty_and_keeps_the_window(cx: &mut TestAppContext) {
    let (mut cx, hidden) = open(cx);
    // Focus where it rests at launch: the window, or the composer.
    cx.simulate_keystrokes("secondary-w");
    cx.run_until_parked();
    assert_eq!(hidden.get(), 1, "the close shortcut did not hide Matty");
    assert_eq!(cx.windows().len(), 1, "the main window closed");
}

#[gpui_kit::test]
fn the_red_button_hides_matty_and_keeps_the_window(cx: &mut TestAppContext) {
    let (mut cx, hidden) = open(cx);
    assert!(!cx.simulate_close(), "the window would close");
    cx.run_until_parked();
    assert_eq!(hidden.get(), 1, "closing did not hide Matty");
}
