//! "Security" (#964): change the password, and see and sign out the
//! devices signed in to the account, as the web app's Profile > Security.

use std::sync::Arc;

use gpui_kit::{
    AppContext as _, Context, Entity, EventEmitter, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window, div, prelude::FluentBuilder as _,
};
use matty_api::models::Session;

use crate::t;
use crate::ui::{
    button::{Button, ButtonSize, ButtonVariant},
    dialog::{DialogHandle, dialog, dialog_description, dialog_popup, dialog_title},
    field::Field,
    input::{InputEvent, InputState},
    theme::UiTheme,
};

/// What the Security dialog asks the server for.
pub enum SecurityEvent {
    ChangePassword { current: String, new: String },
    SignOut { session_id: String },
}

impl EventEmitter<SecurityEvent> for SecurityView {}

pub struct SecurityView {
    dialog: DialogHandle,
    current: Entity<InputState>,
    new: Entity<InputState>,
    confirm: Entity<InputState>,
    /// Whether the account signs in with a password Mattermost keeps.
    password: bool,
    /// How the last change went: `Ok`, or the server's reason.
    result: Option<Result<(), SharedString>>,
    /// `None` until the server answers.
    sessions: Option<Arc<[Session]>>,
    focus_first: bool,
    _subscriptions: Vec<Subscription>,
}

/// Why the typed passwords can't be sent yet, if they can't.
fn password_problem(current: &str, new: &str, confirm: &str) -> Option<SharedString> {
    if current.is_empty() || new.is_empty() {
        None
    } else if new != confirm {
        Some(t!("security-mismatch"))
    } else if new == current {
        Some(t!("security-same"))
    } else {
        None
    }
}

impl SecurityView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut masked = || cx.new(|cx| InputState::new(window, cx).masked(true));
        let (current, new, confirm) = (masked(), masked(), masked());
        let subscriptions =
            [&current, &new, &confirm]
                .into_iter()
                .map(|input| {
                    cx.subscribe_in(input, window, |this, _, event: &InputEvent, window, cx| {
                        match event {
                            InputEvent::PressEnter { .. } => this.change_password(window, cx),
                            _ => {
                                this.result = None;
                                cx.notify();
                            }
                        }
                    })
                })
                .collect();
        Self {
            dialog: DialogHandle::new(false),
            current,
            new,
            confirm,
            password: true,
            result: None,
            sessions: None,
            focus_first: false,
            _subscriptions: subscriptions,
        }
    }

    /// Opens the dialog; `password` says whether the account has a
    /// Mattermost password to change rather than single sign-on.
    pub fn open(&mut self, password: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.password = password;
        self.result = None;
        self.sessions = None;
        self.clear(window, cx);
        self.dialog.open(window, cx);
        self.focus_first = password;
        cx.notify();
    }

    pub fn set_sessions(&mut self, sessions: Arc<[Session]>, cx: &mut Context<Self>) {
        self.sessions = Some(sessions);
        cx.notify();
    }

    pub fn password_changed(
        &mut self,
        result: Result<(), SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if result.is_ok() {
            self.clear(window, cx);
        }
        self.result = Some(result);
        cx.notify();
    }

    fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for input in [&self.current, &self.new, &self.confirm] {
            input.update(cx, |input, cx| input.set_value("", window, cx));
        }
    }

    fn typed(&self, cx: &gpui_kit::App) -> (String, String, String) {
        let value = |input: &Entity<InputState>| input.read(cx).value().to_string();
        (value(&self.current), value(&self.new), value(&self.confirm))
    }

    fn can_change(&self, cx: &gpui_kit::App) -> bool {
        let (current, new, confirm) = self.typed(cx);
        !current.is_empty()
            && !new.is_empty()
            && password_problem(&current, &new, &confirm).is_none()
    }

    fn change_password(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if !self.dialog.is_open() || !self.can_change(cx) {
            return;
        }
        let (current, new, _) = self.typed(cx);
        cx.emit(SecurityEvent::ChangePassword { current, new });
    }

    fn password_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = UiTheme::read(cx).clone();
        let (current, new, confirm) = self.typed(cx);
        let problem = password_problem(&current, &new, &confirm);
        let status = match (&self.result, problem) {
            (_, Some(problem)) => Some((problem, theme.colors.destructive)),
            (Some(Ok(())), None) => Some((t!("security-changed"), theme.colors.muted_foreground)),
            (Some(Err(error)), None) => Some((error.clone(), theme.colors.destructive)),
            (None, None) => None,
        };
        div()
            .flex()
            .flex_col()
            .gap(theme.space(3.))
            .child(section_heading(t!("security-password"), &theme))
            .child(Field::new("security.current", &self.current).label(t!("security-current")))
            .child(
                div()
                    .flex()
                    .gap(theme.space(3.))
                    .child(
                        div()
                            .flex_1()
                            .child(Field::new("security.new", &self.new).label(t!("security-new"))),
                    )
                    .child(div().flex_1().child(
                        Field::new("security.confirm", &self.confirm).label(t!("security-confirm")),
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme.space(3.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .children(status.map(|(text, color)| {
                                div()
                                    .text_size(theme.text(13.))
                                    .text_color(color)
                                    .child(text)
                            })),
                    )
                    .child(
                        div().flex_none().child(
                            Button::new("security.change")
                                .variant(ButtonVariant::Default)
                                .label(t!("security-change"))
                                .disabled(!self.can_change(cx))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.change_password(window, cx)
                                })),
                        ),
                    ),
            )
    }

    fn sessions_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = UiTheme::read(cx).clone();
        let now = crate::clock::now();
        let clock = crate::settings::ClockFormat::current(cx);
        let rows: Vec<_> = match &self.sessions {
            None => vec![
                div()
                    .text_size(theme.text(13.))
                    .text_color(theme.colors.muted_foreground)
                    .child(t!("security-loading"))
                    .into_any_element(),
            ],
            Some(sessions) => sessions
                .iter()
                .enumerate()
                .map(|(index, session)| {
                    let session_id = session.id.clone();
                    let active =
                        crate::scheduled::when_label(session.last_activity_at, &now, clock);
                    div()
                        .flex()
                        .items_center()
                        .gap(theme.space(3.))
                        .py(theme.space(2.))
                        .when(index > 0, |row| {
                            row.border_t_1().border_color(theme.colors.border)
                        })
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(div().text_size(theme.text(14.)).child(device(session)))
                                .child(
                                    div()
                                        .text_size(theme.text(12.))
                                        .text_color(theme.colors.muted_foreground)
                                        .child(t!(
                                            "security-last-active",
                                            when = active.to_string()
                                        )),
                                ),
                        )
                        .child(
                            Button::new(gpui_kit::ElementId::Name(
                                format!("security.revoke.{}", session.id).into(),
                            ))
                            .variant(ButtonVariant::Outline)
                            .size(ButtonSize::Sm)
                            .label(t!("security-sign-out"))
                            .on_click(cx.listener(
                                move |_, _, _, cx| {
                                    cx.emit(SecurityEvent::SignOut {
                                        session_id: session_id.clone(),
                                    });
                                },
                            )),
                        )
                        .into_any_element()
                })
                .collect(),
        };
        div()
            .flex()
            .flex_col()
            .gap(theme.space(1.))
            .child(section_heading(t!("security-devices"), &theme))
            .child(
                div()
                    .text_size(theme.text(13.))
                    .text_color(theme.colors.muted_foreground)
                    .child(t!("security-devices-hint")),
            )
            .child(
                div()
                    .id("security.sessions")
                    .max_h(gpui_kit::px(260.))
                    .overflow_y_scroll()
                    .children(rows),
            )
    }
}

fn section_heading(text: SharedString, theme: &UiTheme) -> impl IntoElement + use<> {
    div()
        .text_size(theme.text(14.))
        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
        .child(text)
}

/// How a session's device reads: "Mac OS · Chrome", or the platform alone.
fn device(session: &Session) -> SharedString {
    let prop = |key: &str| {
        session
            .props
            .get(key)
            .map(|value| value.trim())
            // The server's word for a user agent it could not read.
            .filter(|value| !value.is_empty() && !value.starts_with("Unknown")) // i18n-ignore
    };
    let browser = prop("browser").map(|browser| browser.split('/').next().unwrap_or(browser));
    let parts: Vec<&str> = [prop("os").or_else(|| prop("platform")), browser]
        .into_iter()
        .flatten()
        .collect();
    if parts.is_empty() {
        if session
            .props
            .get("isMobile")
            .is_some_and(|value| value == "true")
        {
            t!("security-mobile")
        } else {
            t!("security-unknown-device")
        }
    } else {
        parts.join(" · ").into()
    }
}

impl Render for SecurityView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = UiTheme::read(cx).clone();
        let password = self.password.then(|| self.password_section(cx));
        let sessions = self.sessions_section(cx);
        let popup = dialog_popup("security.popup", t!("security-title"), cx)
            .child(dialog_title("security.title", cx).child(t!("security-title")))
            .child(dialog_description("security.description", cx).child(t!("security-hint")))
            .child(
                div()
                    .pt(theme.space(2.))
                    .flex()
                    .flex_col()
                    .gap(theme.space(5.))
                    .children(password)
                    .child(sessions),
            )
            .child(
                div().flex().justify_end().pt(theme.space(2.)).child(
                    Button::new("security.done")
                        .variant(ButtonVariant::Outline)
                        .label(t!("security-done"))
                        .on_click(cx.listener(|this, _, window, cx| this.dialog.close(window, cx))),
                ),
            );
        let element = dialog("security", &self.dialog, popup, window, cx);
        if self.focus_first && self.dialog.is_open() {
            self.focus_first = false;
            let focus = self.current.read(cx).focus_handle(cx);
            window.on_next_frame(move |window, cx| window.focus(&focus, cx));
        }
        div().child(element)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devices_read_from_the_session_props() {
        let session = |props: &[(&str, &str)]| Session {
            props: props
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
            ..Session::default()
        };
        assert_eq!(
            device(&session(&[("os", "Mac OS"), ("browser", "Chrome/120.0")])),
            "Mac OS · Chrome"
        );
        assert_eq!(
            device(&session(&[
                ("os", ""),
                ("platform", "Linux"),
                ("browser", "Unknown/0.0")
            ])),
            "Linux"
        );
    }
}
