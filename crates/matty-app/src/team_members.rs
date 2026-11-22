//! Team members (#988): everyone on a team, with search. A click messages
//! someone; a team admin also makes or unmakes admins and removes people.

use gpui_icons::{LucideIcon, lucide};
use gpui_kit::{
    AppContext as _, Context, Entity, EventEmitter, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, div, prelude::FluentBuilder as _,
};

use crate::profile::UserProfile;
use crate::style::{self, Palette, TypeStyleExt as _, metrics, scaled};
use crate::t;
use crate::ui::{
    avatar::{Avatar, AvatarSize},
    badge::{Badge, BadgeVariant},
    button::{Button, ButtonSize, ButtonVariant},
    dialog::{DialogHandle, dialog_description, dialog_title},
    input::{Input, InputEvent, InputState},
    theme::UiTheme,
};

/// One person on the team.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamPerson {
    pub profile: UserProfile,
    pub admin: bool,
    /// The user themself, whom the dialog does not offer to change.
    pub me: bool,
}

/// What the dialog asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TeamMembersEvent {
    Message(UserProfile),
    SetAdmin {
        team_id: SharedString,
        user_id: SharedString,
        admin: bool,
    },
    Remove {
        team_id: SharedString,
        user_id: SharedString,
    },
}

pub struct TeamMembersView {
    team_id: SharedString,
    team_name: SharedString,
    manage: bool,
    people: Option<Vec<TeamPerson>>,
    search: Entity<InputState>,
    handle: DialogHandle,
    focus_search: bool,
    _subscription: gpui_kit::Subscription,
}

impl EventEmitter<TeamMembersEvent> for TeamMembersView {}

impl TeamMembersView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder(t!("directory-search-people")));
        let subscription = cx.subscribe_in(
            &search,
            window,
            |this: &mut Self, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    if let Some(first) = this.matches(cx).first() {
                        let profile = first.profile.clone();
                        this.message(profile, window, cx);
                    }
                } else {
                    cx.notify();
                }
            },
        );
        Self {
            team_id: SharedString::default(),
            team_name: SharedString::default(),
            manage: false,
            people: None,
            search,
            handle: DialogHandle::new(false),
            focus_search: false,
            _subscription: subscription,
        }
    }

    pub fn handle(&self) -> &DialogHandle {
        &self.handle
    }

    /// Opens the dialog for a team; [`Self::set_people`] fills it.
    /// `manage` offers the admin's actions.
    pub fn open(
        &mut self,
        team_id: SharedString,
        team_name: SharedString,
        manage: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.team_id != team_id {
            self.people = None;
        }
        self.team_id = team_id;
        self.team_name = team_name;
        self.manage = manage;
        self.search
            .update(cx, |search, cx| search.set_value("", window, cx));
        self.handle.open(window, cx);
        self.focus_search = true;
        cx.notify();
    }

    /// The team's people, sorted by name, for the open team.
    pub fn set_people(
        &mut self,
        team_id: &str,
        mut people: Vec<TeamPerson>,
        cx: &mut Context<Self>,
    ) {
        if self.team_id.as_ref() != team_id {
            return;
        }
        people.sort_by_key(|person| person.profile.display_name.to_lowercase());
        self.people = Some(people);
        cx.notify();
    }

    fn matches(&self, cx: &gpui_kit::App) -> Vec<TeamPerson> {
        let query = self.search.read(cx).value().trim().to_lowercase();
        self.people
            .iter()
            .flatten()
            .filter(|person| {
                query.is_empty()
                    || person.profile.display_name.to_lowercase().contains(&query)
                    || person.profile.username.to_lowercase().contains(&query)
            })
            .cloned()
            .collect()
    }

    fn message(&mut self, profile: UserProfile, window: &mut Window, cx: &mut Context<Self>) {
        self.handle.close(window, cx);
        cx.emit(TeamMembersEvent::Message(profile));
    }

    fn row(&self, person: &TeamPerson, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = UiTheme::read(cx).clone();
        let palette = Palette::read(cx);
        let profile = &person.profile;
        let username = profile.username.clone();
        let initial: String = profile.display_name.chars().take(1).collect();
        let chosen = profile.clone();
        let (team_id, user_id, admin) =
            (self.team_id.clone(), profile.user_id.clone(), person.admin);
        let (remove_team, remove_user) = (team_id.clone(), user_id.clone());
        let actions = self.manage && !person.me;
        div()
            .id(gpui_kit::ElementId::Name(
                format!("team-members.{username}").into(),
            ))
            .role(gpui_kit::Role::Button)
            .aria_label(t!(
                "new-message-person",
                name = profile.display_name.to_string()
            ))
            .h(scaled(&theme, metrics::ROW_HEIGHT + 8.))
            .px(theme.space(2.))
            .mx(theme.space(-2.))
            .rounded(theme.radius.md)
            .flex()
            .items_center()
            .gap(theme.space(3.))
            .cursor_pointer()
            .hover(|style| style.bg(theme.colors.accent))
            .on_click(
                cx.listener(move |this, _, window, cx| this.message(chosen.clone(), window, cx)),
            )
            .child(
                div().flex_shrink_0().child(crate::avatars::with_picture(
                    Avatar::new(gpui_kit::ElementId::Name(
                        format!("team-members.{username}.avatar").into(),
                    ))
                    .size(AvatarSize::Sm)
                    .fallback(initial)
                    .aria_label(profile.display_name.clone())
                    .status(style::avatar_status(profile.presence, &palette))
                    .ring_color(theme.colors.popover),
                    &profile.user_id,
                    cx,
                )),
            )
            // The handle gives way first, so the name stays readable.
            .child(
                div()
                    .flex_shrink_0()
                    .max_w(scaled(&theme, 160.))
                    .truncate()
                    .type_style(style::UI, &theme)
                    .text_color(theme.colors.foreground)
                    .child(profile.display_name.clone()),
            )
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .type_style(style::META, &theme)
                    .text_color(theme.colors.muted_foreground)
                    .child(format!("@{username}")),
            )
            .when(person.admin, |row| {
                row.child(
                    Badge::new(t!("pane-admin"))
                        .variant(BadgeVariant::Secondary)
                        .flex_shrink_0()
                        .h(theme.space(4.5))
                        .px(theme.space(1.5))
                        .text_size(theme.text(10.)),
                )
            })
            .child(div().flex_1())
            .when(actions, |row| {
                row.child(
                    div()
                        .id(gpui_kit::ElementId::Name(
                            format!("team-members.{username}.actions").into(),
                        ))
                        .flex()
                        .flex_shrink_0()
                        .gap(theme.space(1.))
                        // The buttons act on the person, never also message them.
                        .on_click(|_, _, cx| cx.stop_propagation())
                        .child(
                            Button::new(gpui_kit::ElementId::Name(
                                format!("team-members.{username}.admin").into(),
                            ))
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .label(if admin {
                                t!("team-members-remove-admin")
                            } else {
                                t!("team-members-make-admin")
                            })
                            .on_click(cx.listener(
                                move |_, _, _, cx| {
                                    cx.emit(TeamMembersEvent::SetAdmin {
                                        team_id: team_id.clone(),
                                        user_id: user_id.clone(),
                                        admin: !admin,
                                    });
                                },
                            )),
                        )
                        .child(
                            Button::new(gpui_kit::ElementId::Name(
                                format!("team-members.{username}.remove").into(),
                            ))
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::IconSm)
                            .aria_label(t!(
                                "team-members-remove",
                                name = profile.display_name.to_string()
                            ))
                            .child(
                                lucide(LucideIcon::UserMinus)
                                    .size(scaled(&theme, metrics::ICON))
                                    .text_color(theme.colors.muted_foreground),
                            )
                            .on_click(cx.listener(
                                move |_, _, _, cx| {
                                    cx.emit(TeamMembersEvent::Remove {
                                        team_id: remove_team.clone(),
                                        user_id: remove_user.clone(),
                                    });
                                },
                            )),
                        ),
                )
            })
    }
}

impl Render for TeamMembersView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = UiTheme::read(cx).clone();
        if self.focus_search {
            self.focus_search = false;
            let focus = self.search.read(cx).focus_handle(cx);
            window.on_next_frame(move |window, cx| window.focus(&focus, cx));
        }
        let query = self.search.read(cx).value().trim().to_string();
        let matches = self.matches(cx);
        let rows: Vec<_> = matches
            .iter()
            .map(|person| self.row(person, cx).into_any_element())
            .collect();
        let empty = match &self.people {
            None => Some(t!("team-settings-loading")),
            Some(_) if rows.is_empty() => Some(t!("directory-nobody-matches", query = query)),
            Some(_) => None,
        };
        let count = self.people.as_ref().map_or(0, Vec::len);
        let title = if self.manage {
            t!(
                "team-members-manage-title",
                team = self.team_name.to_string()
            )
        } else {
            t!("team-members-title", team = self.team_name.to_string())
        };
        div()
            .flex()
            .flex_col()
            .gap(theme.space(4.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.space(1.))
                    .child(dialog_title("team-members.title", cx).child(title))
                    .child(
                        dialog_description("team-members.description", cx)
                            .child(t!("team-members-count", count = count)),
                    ),
            )
            .child(Input::new(&self.search).aria_label(t!("directory-search-people")))
            .child(
                div()
                    .id("team-members.list")
                    .overflow_y_scroll()
                    .max_h(scaled(&theme, metrics::ROW_HEIGHT * 10.))
                    .flex()
                    .flex_col()
                    .gap(theme.space(0.5))
                    .when_some(empty, |list, empty| {
                        list.child(
                            div()
                                .w_full()
                                .py(theme.space(6.))
                                .text_center()
                                .type_style(style::UI, &theme)
                                .text_color(theme.colors.muted_foreground)
                                .child(empty),
                        )
                    })
                    .children(rows),
            )
            .child(
                div().flex().justify_end().child(
                    Button::new("team-members.close")
                        .variant(ButtonVariant::Outline)
                        .label(t!("team-members-close"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.handle.close(window, cx);
                        })),
                ),
            )
    }
}
