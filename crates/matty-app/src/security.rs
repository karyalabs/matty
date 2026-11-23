//! "Security" (#964): change the password, and see and sign out the
//! devices signed in to the account, as the web app's Profile > Security.

use std::sync::Arc;

use base64::Engine as _;
use gpui_kit::{
    AppContext as _, Context, Entity, EventEmitter, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window, div, prelude::FluentBuilder as _,
};
use matty_api::{MfaSecret, models::Session};

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
    ChangePassword {
        current: String,
        new: String,
    },
    SignOut {
        session_id: String,
    },
    /// A new secret to set up multi-factor authentication with.
    MfaSecret,
    /// Turn multi-factor authentication on with `code`, or off.
    SetMfa {
        activate: bool,
        code: String,
    },
}

/// Where multi-factor authentication stands, as the Security dialog shows
/// it (#1049).
#[derive(Clone)]
enum Mfa {
    Off,
    /// Waiting for the server's secret.
    Starting,
    /// The secret and its QR code, until a code from the app confirms it.
    SettingUp {
        secret: Arc<MfaSecret>,
        qr: Option<Arc<gpui_kit::Image>>,
    },
    On,
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
    /// `None` when the server or the account has no multi-factor
    /// authentication to offer.
    mfa: Option<Mfa>,
    /// Whether the server requires it to stay on.
    mfa_required: bool,
    mfa_code: Entity<InputState>,
    /// Why the server refused the last multi-factor step.
    mfa_error: Option<SharedString>,
    /// A multi-factor step is with the server.
    mfa_busy: bool,
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
        let mfa_code = cx.new(|cx| InputState::new(window, cx));
        let mut subscriptions: Vec<Subscription> =
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
        subscriptions.push(cx.subscribe_in(
            &mfa_code,
            window,
            |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { .. } => this.turn_on_mfa(window, cx),
                _ => {
                    this.mfa_error = None;
                    cx.notify();
                }
            },
        ));
        Self {
            dialog: DialogHandle::new(false),
            current,
            new,
            confirm,
            password: true,
            result: None,
            sessions: None,
            mfa: None,
            mfa_required: false,
            mfa_code,
            mfa_error: None,
            mfa_busy: false,
            focus_first: false,
            _subscriptions: subscriptions,
        }
    }

    /// Opens the dialog; `password` says whether the account has a
    /// Mattermost password to change rather than single sign-on, and `mfa`
    /// whether multi-factor authentication is on and whether the server
    /// requires it, when the server offers it.
    pub fn open(
        &mut self,
        password: bool,
        mfa: Option<(bool, bool)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.password = password;
        self.result = None;
        self.sessions = None;
        self.mfa = mfa
            .filter(|_| password)
            .map(|(on, _)| if on { Mfa::On } else { Mfa::Off });
        self.mfa_required = mfa.is_some_and(|(_, required)| required);
        self.mfa_error = None;
        self.mfa_busy = false;
        self.mfa_code
            .update(cx, |input, cx| input.set_value("", window, cx));
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

    /// The server's new secret: shows its QR code and asks for a code.
    pub fn set_mfa_secret(
        &mut self,
        secret: Arc<MfaSecret>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.mfa, Some(Mfa::Starting)) {
            return;
        }
        let qr = base64::engine::general_purpose::STANDARD
            .decode(secret.qr_code.trim())
            .ok()
            .map(|png| Arc::new(gpui_kit::Image::from_bytes(gpui_kit::ImageFormat::Png, png)));
        self.mfa = Some(Mfa::SettingUp { secret, qr });
        self.mfa_busy = false;
        let focus = self.mfa_code.read(cx).focus_handle(cx);
        window.on_next_frame(move |window, cx| window.focus(&focus, cx));
        cx.notify();
    }

    /// How turning multi-factor authentication on or off went: whether it
    /// is on now, or the server's reason.
    pub fn mfa_changed(
        &mut self,
        result: Result<bool, SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.mfa.is_none() {
            return;
        }
        self.mfa_busy = false;
        match result {
            Ok(on) => {
                self.mfa = Some(if on { Mfa::On } else { Mfa::Off });
                self.mfa_error = None;
                self.mfa_code
                    .update(cx, |input, cx| input.set_value("", window, cx));
            }
            Err(error) => {
                if matches!(self.mfa, Some(Mfa::Starting)) {
                    self.mfa = Some(Mfa::Off);
                }
                self.mfa_error = Some(error);
            }
        }
        cx.notify();
    }

    fn start_mfa(&mut self, cx: &mut Context<Self>) {
        self.mfa = Some(Mfa::Starting);
        self.mfa_error = None;
        self.mfa_busy = true;
        cx.emit(SecurityEvent::MfaSecret);
        cx.notify();
    }

    fn turn_on_mfa(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let code = self.mfa_code.read(cx).value().trim().to_string();
        if self.mfa_busy || code.is_empty() || !matches!(self.mfa, Some(Mfa::SettingUp { .. })) {
            return;
        }
        self.mfa_busy = true;
        cx.emit(SecurityEvent::SetMfa {
            activate: true,
            code,
        });
        cx.notify();
    }

    fn turn_off_mfa(&mut self, cx: &mut Context<Self>) {
        if self.mfa_busy || self.mfa_required {
            return;
        }
        self.mfa_busy = true;
        self.mfa_error = None;
        cx.emit(SecurityEvent::SetMfa {
            activate: false,
            code: String::new(),
        });
        cx.notify();
    }

    fn cancel_mfa(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.mfa = Some(Mfa::Off);
        self.mfa_error = None;
        self.mfa_busy = false;
        self.mfa_code
            .update(cx, |input, cx| input.set_value("", window, cx));
        cx.notify();
    }

    fn mfa_section(&self, mfa: &Mfa, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = UiTheme::read(cx).clone();
        let note = |text: SharedString, color| {
            div()
                .text_size(theme.text(13.))
                .text_color(color)
                .child(text)
        };
        let error = self
            .mfa_error
            .clone()
            .map(|error| note(error, theme.colors.destructive));
        let body = match mfa {
            Mfa::Off | Mfa::Starting => div()
                .flex()
                .items_center()
                .gap(theme.space(3.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(note(t!("security-mfa-off"), theme.colors.muted_foreground)),
                )
                .child(
                    div().flex_none().child(
                        Button::new("security.mfa.set-up")
                            .variant(ButtonVariant::Outline)
                            .label(t!("security-mfa-set-up"))
                            .disabled(self.mfa_busy)
                            .on_click(cx.listener(|this, _, _, cx| this.start_mfa(cx))),
                    ),
                )
                .into_any_element(),
            Mfa::SettingUp { secret, qr } => {
                let size = gpui_kit::px(148.);
                let code_typed = !self.mfa_code.read(cx).value().trim().is_empty();
                div()
                    .flex()
                    .gap(theme.space(4.))
                    .child(
                        div()
                            .flex_none()
                            .size(size)
                            .p(theme.space(1.))
                            .rounded(theme.radius.md)
                            .bg(gpui_kit::white())
                            .children(qr.clone().map(|qr| {
                                gpui_kit::img(gpui_kit::ImageSource::Image(qr)).size_full()
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(theme.space(2.))
                            .child(note(t!("security-mfa-scan"), theme.colors.muted_foreground))
                            .child(
                                div()
                                    .id("security.mfa.key")
                                    .text_size(theme.text(13.))
                                    .font_family(theme.fonts.mono.clone())
                                    .child(t!("security-mfa-key", key = secret.secret.clone())),
                            )
                            .child(
                                Field::new("security.mfa.code", &self.mfa_code)
                                    .label(t!("security-mfa-code")),
                            )
                            .child(
                                div()
                                    .flex()
                                    .justify_end()
                                    .gap(theme.space(2.))
                                    .child(
                                        Button::new("security.mfa.cancel")
                                            .variant(ButtonVariant::Ghost)
                                            .label(t!("security-mfa-cancel"))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.cancel_mfa(window, cx)
                                            })),
                                    )
                                    .child(
                                        Button::new("security.mfa.turn-on")
                                            .variant(ButtonVariant::Default)
                                            .label(t!("security-mfa-turn-on"))
                                            .disabled(self.mfa_busy || !code_typed)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.turn_on_mfa(window, cx)
                                            })),
                                    ),
                            ),
                    )
                    .into_any_element()
            }
            Mfa::On => div()
                .flex()
                .items_center()
                .gap(theme.space(3.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(note(t!("security-mfa-on"), theme.colors.muted_foreground))
                        .when(self.mfa_required, |column| {
                            column.child(note(
                                t!("security-mfa-required"),
                                theme.colors.muted_foreground,
                            ))
                        }),
                )
                .when(!self.mfa_required, |row| {
                    row.child(
                        div().flex_none().child(
                            Button::new("security.mfa.turn-off")
                                .variant(ButtonVariant::Outline)
                                .label(t!("security-mfa-turn-off"))
                                .disabled(self.mfa_busy)
                                .on_click(cx.listener(|this, _, _, cx| this.turn_off_mfa(cx))),
                        ),
                    )
                })
                .into_any_element(),
        };
        div()
            .flex()
            .flex_col()
            .gap(theme.space(2.))
            .child(section_heading(t!("security-mfa"), &theme))
            .child(body)
            .children(error)
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
        let mfa = self.mfa.clone().map(|mfa| self.mfa_section(&mfa, cx));
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
                    .children(mfa)
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

    /// #1049: Set up asks for a secret, a code turns it on, and Turn off
    /// goes only when the server does not require it.
    #[gpui_kit::test]
    fn multi_factor_set_up_turns_on_with_a_code(cx: &mut gpui_kit::TestAppContext) {
        cx.update(|cx| {
            crate::ui::theme::init(cx);
            crate::settings::SettingsStore::init(None, cx);
        });
        let window = cx.add_window(SecurityView::new);
        let view = window.root(cx).unwrap();
        let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        cx.update(|cx| {
            let events = events.clone();
            cx.subscribe(&view, move |_, event: &SecurityEvent, _| {
                events.borrow_mut().push(match event {
                    SecurityEvent::MfaSecret => "secret".to_owned(),
                    SecurityEvent::SetMfa { activate, code } => format!("{activate} {code}"),
                    _ => "other".to_owned(),
                });
            })
            .detach();
        });
        let mut cx = gpui_kit::VisualTestContext::from_window(window.into(), cx);
        cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                view.open(true, Some((false, false)), window, cx);
                view.start_mfa(cx);
                view.set_mfa_secret(
                    Arc::new(MfaSecret {
                        secret: "JBSWY3DPEHPK3PXP".into(),
                        qr_code: "not base64!".into(),
                    }),
                    window,
                    cx,
                );
                assert!(matches!(view.mfa, Some(Mfa::SettingUp { qr: None, .. })));
                view.turn_on_mfa(window, cx);
                view.mfa_code
                    .update(cx, |input, cx| input.set_value("123456", window, cx));
                view.turn_on_mfa(window, cx);
                view.mfa_changed(Err("Invalid code.".into()), window, cx);
                assert!(matches!(view.mfa, Some(Mfa::SettingUp { .. })));
                assert_eq!(view.mfa_error.as_deref(), Some("Invalid code."));
                view.mfa_changed(Ok(true), window, cx);
                assert!(matches!(view.mfa, Some(Mfa::On)));
                view.turn_off_mfa(cx);
                view.mfa_changed(Ok(false), window, cx);
                assert!(matches!(view.mfa, Some(Mfa::Off)));

                view.open(true, Some((true, true)), window, cx);
                view.turn_off_mfa(cx);
                view.open(false, Some((false, false)), window, cx);
                assert!(view.mfa.is_none(), "single sign-on has no MFA here");
            })
        });
        assert_eq!(
            *events.borrow(),
            ["secret", "true 123456", "false "],
            "no code sends nothing, and a required MFA stays on"
        );
    }

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
