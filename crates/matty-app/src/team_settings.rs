//! Team settings for a team admin (#987): the team's icon, name, description,
//! whether anyone on the server may join, which email domains may, and
//! its invite link.

use gpui_kit::{
    App, AppContext as _, Context, Entity, EventEmitter, Focusable as _, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, StyledImage as _, Window, div,
};

use crate::style::{self, TypeStyleExt as _};
use crate::t;
use crate::ui::{
    button::{Button, ButtonSize, ButtonVariant},
    dialog::{DialogHandle, dialog_description, dialog_title},
    field::Field,
    input::{InputEvent, InputState},
    switch::Switch,
    theme::UiTheme,
};

/// Mattermost's limits on a team's display name and description.
const NAME_MAX: usize = 64;
const DESCRIPTION_MAX: usize = 255;

/// A team as the dialog shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamSettings {
    pub id: SharedString,
    pub name: SharedString,
    pub description: SharedString,
    /// Anyone on the server may join without an invite.
    pub open: bool,
    /// Email domains a person must have to join, as the server keeps
    /// them; empty for any.
    pub domains: SharedString,
    /// The team's `last_team_icon_update`; 0 when it has no icon.
    pub icon: i64,
    /// The invite link, when the server gave its id.
    pub link: Option<SharedString>,
}

/// What the dialog asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TeamSettingsEvent {
    Save {
        team_id: SharedString,
        name: Option<String>,
        description: Option<String>,
        open: Option<bool>,
        domains: Option<String>,
    },
    RegenerateInvite {
        team_id: SharedString,
    },
    /// A new icon (its bytes and file name), or `None` to remove it.
    Icon {
        team_id: SharedString,
        icon: Option<(Vec<u8>, String)>,
    },
}

pub struct TeamSettingsView {
    team: Option<TeamSettings>,
    name: Entity<InputState>,
    description: Entity<InputState>,
    domains: Entity<InputState>,
    open: bool,
    copied: bool,
    /// Why the last picked icon was refused.
    icon_error: Option<SharedString>,
    handle: DialogHandle,
    focus_name: bool,
    _subscriptions: Vec<gpui_kit::Subscription>,
}

impl EventEmitter<TeamSettingsEvent> for TeamSettingsView {}

impl TeamSettingsView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx));
        let description = cx.new(|cx| InputState::new(window, cx));
        let domains = cx.new(|cx| InputState::new(window, cx));
        let on_input = |this: &mut Self,
                        _: &Entity<InputState>,
                        event: &InputEvent,
                        window: &mut Window,
                        cx: &mut Context<Self>| {
            if let InputEvent::PressEnter { .. } = event {
                this.save(window, cx);
            } else {
                cx.notify();
            }
        };
        let subscriptions = vec![
            cx.subscribe_in(&name, window, on_input),
            cx.subscribe_in(&description, window, on_input),
            cx.subscribe_in(&domains, window, on_input),
        ];
        Self {
            team: None,
            name,
            description,
            domains,
            open: false,
            copied: false,
            icon_error: None,
            handle: DialogHandle::new(false),
            focus_name: false,
            _subscriptions: subscriptions,
        }
    }

    pub fn handle(&self) -> &DialogHandle {
        &self.handle
    }

    /// The team the dialog is open for.
    pub fn team_id(&self) -> Option<&SharedString> {
        self.team.as_ref().map(|team| &team.id)
    }

    /// Opens the dialog for `team` as far as it is known; [`Self::set_team`]
    /// fills in the server's answer.
    pub fn open(&mut self, team: TeamSettings, window: &mut Window, cx: &mut Context<Self>) {
        self.fill(&team, window, cx);
        self.team = Some(team);
        self.copied = false;
        self.icon_error = None;
        self.handle.open(window, cx);
        self.focus_name = true;
        cx.notify();
    }

    /// The server's answer for the open team: its fields, unless the user
    /// started typing, and its invite link.
    pub fn set_team(&mut self, team: TeamSettings, window: &mut Window, cx: &mut Context<Self>) {
        let Some(shown_link) = self
            .team
            .as_ref()
            .filter(|shown| shown.id == team.id)
            .map(|shown| shown.link.clone())
        else {
            return;
        };
        if !self.edited(cx) {
            self.fill(&team, window, cx);
        }
        if shown_link != team.link {
            self.copied = false;
        }
        self.team = Some(team);
        cx.notify();
    }

    fn fill(&mut self, team: &TeamSettings, window: &mut Window, cx: &mut Context<Self>) {
        self.name.update(cx, |input, cx| {
            input.set_value(team.name.as_ref(), window, cx);
            input.set_placeholder(t!("team-settings-name-placeholder"), window, cx);
        });
        self.description.update(cx, |input, cx| {
            input.set_value(team.description.as_ref(), window, cx);
            input.set_placeholder(t!("team-settings-description-placeholder"), window, cx);
        });
        self.domains.update(cx, |input, cx| {
            input.set_value(team.domains.as_ref(), window, cx);
            input.set_placeholder(t!("team-settings-domains-placeholder"), window, cx);
        });
        self.open = team.open;
    }

    fn name(&self, cx: &App) -> String {
        self.name.read(cx).value().trim().to_string()
    }

    fn description(&self, cx: &App) -> String {
        self.description.read(cx).value().trim().to_string()
    }

    /// The typed domains as the server keeps them: lowercase, without a
    /// leading `@`, joined by ", ".
    fn domains(&self, cx: &App) -> String {
        normalize_domains(self.domains.read(cx).value().as_ref())
    }

    fn edited(&self, cx: &App) -> bool {
        self.team.as_ref().is_some_and(|team| {
            self.name(cx) != team.name.as_ref()
                || self.description(cx) != team.description.as_ref()
                || self.domains(cx) != normalize_domains(&team.domains)
                || self.open != team.open
        })
    }

    fn can_save(&self, cx: &App) -> bool {
        let name = self.name(cx).chars().count();
        self.edited(cx)
            && (2..=NAME_MAX).contains(&name)
            && self.description(cx).chars().count() <= DESCRIPTION_MAX
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_save(cx) {
            return;
        }
        let Some(team) = &self.team else {
            return;
        };
        let (name, description) = (self.name(cx), self.description(cx));
        let domains = self.domains(cx);
        let event = TeamSettingsEvent::Save {
            team_id: team.id.clone(),
            name: (name != team.name.as_ref()).then_some(name),
            description: (description != team.description.as_ref()).then_some(description),
            open: (self.open != team.open).then_some(self.open),
            domains: (domains != normalize_domains(&team.domains)).then_some(domains),
        };
        self.handle.close(window, cx);
        cx.emit(event);
    }

    /// Asks for an image and sends it as the team's icon straight away,
    /// as the web app does; the dialog's Save is for the text fields.
    fn pick_icon(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(team_id) = self.team_id().cloned() else {
            return;
        };
        let chosen =
            crate::attachments::choose_files(false, Some(t!("team-settings-icon-prompt")), cx);
        cx.spawn_in(window, async move |this, cx| {
            let Some(file) = chosen.await.into_iter().next() else {
                return;
            };
            let name = file.file_name().unwrap_or_else(|| "icon".into());
            if crate::profile_editor::picture_problem(std::path::Path::new(&name)).is_some() {
                this.update(cx, |this, cx| {
                    this.icon_error = Some(t!("team-settings-icon-type"));
                    cx.notify();
                })
                .ok();
                return;
            }
            let bytes = cx
                .background_executor()
                .spawn(async move { file.read() })
                .await;
            this.update(cx, |this, cx| {
                match bytes {
                    Ok(bytes) => {
                        this.icon_error = None;
                        cx.emit(TeamSettingsEvent::Icon {
                            team_id,
                            icon: Some((bytes, name)),
                        });
                    }
                    Err(_) => this.icon_error = Some(t!("profile-picture-unreadable")),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn remove_icon(&mut self, cx: &mut Context<Self>) {
        let Some(team_id) = self.team_id().cloned() else {
            return;
        };
        self.icon_error = None;
        cx.emit(TeamSettingsEvent::Icon {
            team_id,
            icon: None,
        });
        cx.notify();
    }

    /// The team's icon, or its initials, beside Upload and Remove.
    fn icon_row(&self, theme: &UiTheme, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (id, name, version) = self
            .team
            .as_ref()
            .map(|team| (team.id.clone(), team.name.clone(), team.icon))
            .unwrap_or_default();
        let team = crate::teams::Team::new(id, name).icon(version);
        let square = div()
            .size(gpui_kit::px(48.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(theme.radius.lg)
            .bg(theme.colors.secondary)
            .text_color(theme.colors.secondary_foreground)
            .text_size(theme.text(18.))
            .font_weight(gpui_kit::FontWeight::SEMIBOLD);
        let square = match team.icon_image(cx) {
            Some(image) => square.child(
                gpui_kit::img(image)
                    .size_full()
                    .rounded(theme.radius.lg)
                    .object_fit(gpui_kit::ObjectFit::Cover),
            ),
            None => square.child(team.initials().to_uppercase()),
        };
        div()
            .flex()
            .items_center()
            .gap(theme.space(3.))
            .child(square)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.space(1.))
                    .child(
                        div()
                            .flex()
                            .gap(theme.space(2.))
                            .child(
                                Button::new("team-settings.icon")
                                    .variant(ButtonVariant::Outline)
                                    .size(ButtonSize::Sm)
                                    .label(t!("team-settings-icon-upload"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.pick_icon(window, cx)
                                    })),
                            )
                            .children((version > 0).then(|| {
                                Button::new("team-settings.icon.remove")
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::Sm)
                                    .label(t!("team-settings-icon-remove"))
                                    .on_click(cx.listener(|this, _, _, cx| this.remove_icon(cx)))
                            })),
                    )
                    .child(
                        div()
                            .type_style(style::META, theme)
                            .text_color(match self.icon_error {
                                Some(_) => theme.colors.destructive,
                                None => theme.colors.muted_foreground,
                            })
                            .child(
                                self.icon_error
                                    .clone()
                                    .unwrap_or_else(|| t!("team-settings-icon-hint")),
                            ),
                    ),
            )
    }

    fn copy_link(&mut self, cx: &mut Context<Self>) {
        if let Some(link) = self.team.as_ref().and_then(|team| team.link.as_ref()) {
            cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(link.to_string()));
            self.copied = true;
            cx.notify();
        }
    }
}

impl Render for TeamSettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = UiTheme::read(cx).clone();
        if self.focus_name {
            self.focus_name = false;
            let focus = self.name.read(cx).focus_handle(cx);
            window.on_next_frame(move |window, cx| window.focus(&focus, cx));
        }
        let name_len = self.name(cx).chars().count();
        let description_len = self.description(cx).chars().count();
        let mut name = Field::new("team-settings.name", &self.name).label(t!("team-settings-name"));
        if name_len > NAME_MAX {
            name = name.error(t!("channel-edit-too-long", max = NAME_MAX));
        } else if name_len < 2 {
            name = name.error(t!("teams-create-short"));
        }
        let mut description = Field::new("team-settings.description", &self.description)
            .label(t!("team-settings-description"));
        description = if description_len > DESCRIPTION_MAX {
            description.error(t!("channel-edit-too-long", max = DESCRIPTION_MAX))
        } else {
            description.description(t!(
                "channel-edit-count",
                len = description_len,
                max = DESCRIPTION_MAX
            ))
        };
        let domains = Field::new("team-settings.domains", &self.domains)
            .label(t!("team-settings-domains"))
            .description(t!("team-settings-domains-detail"));
        let open = Switch::new("team-settings.open.switch")
            .checked(self.open)
            .aria_label(t!("team-settings-open"))
            .on_change(cx.listener(|this, &next: &bool, _, cx| {
                this.open = next;
                cx.notify();
            }));
        let team_id = self.team.as_ref().map(|team| team.id.clone());
        let link = self.team.as_ref().and_then(|team| team.link.clone());
        let label = |text: SharedString| {
            div()
                .type_style(style::BODY, &theme)
                .text_color(theme.colors.foreground)
                .child(text)
        };
        let detail = |text: SharedString| {
            div()
                .type_style(style::META, &theme)
                .text_color(theme.colors.muted_foreground)
                .child(text)
        };
        let mut invite = div()
            .flex()
            .flex_col()
            .gap(theme.space(1.))
            .child(label(t!("team-settings-invite")));
        invite = match link {
            Some(link) => invite
                .child(
                    div()
                        .type_style(style::META, &theme)
                        .text_color(theme.colors.muted_foreground)
                        .truncate()
                        .child(link),
                )
                .child(
                    div()
                        .flex()
                        .gap(theme.space(2.))
                        .pt(theme.space(1.))
                        .child(
                            Button::new("team-settings.copy")
                                .variant(ButtonVariant::Outline)
                                .label(if self.copied {
                                    t!("invite-link-copied")
                                } else {
                                    t!("invite-copy-link")
                                })
                                .on_click(cx.listener(|this, _, _, cx| this.copy_link(cx))),
                        )
                        .child(
                            Button::new("team-settings.regenerate")
                                .variant(ButtonVariant::Ghost)
                                .label(t!("team-settings-regenerate"))
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    if let Some(team_id) = team_id.clone() {
                                        cx.emit(TeamSettingsEvent::RegenerateInvite { team_id });
                                    }
                                })),
                        ),
                )
                .child(detail(t!("team-settings-regenerate-detail"))),
            None => invite.child(detail(t!("team-settings-loading"))),
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
                    .child(dialog_title("team-settings.title", cx).child(t!("team-settings-title")))
                    .child(
                        dialog_description("team-settings.description-text", cx)
                            .child(t!("team-settings-detail")),
                    ),
            )
            .child(self.icon_row(&theme, cx))
            .child(name)
            .child(description)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(theme.space(4.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(theme.space(0.5))
                            .child(label(t!("team-settings-open")))
                            .child(detail(t!("team-settings-open-detail"))),
                    )
                    .child(div().flex_none().child(open)),
            )
            .child(domains)
            .child(invite)
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(theme.space(2.))
                    .pt(theme.space(2.))
                    .child(
                        Button::new("team-settings.cancel")
                            .variant(ButtonVariant::Outline)
                            .label(t!("channel-settings-cancel"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.handle.close(window, cx);
                            })),
                    )
                    .child(
                        Button::new("team-settings.save")
                            .variant(ButtonVariant::Default)
                            .disabled(!self.can_save(cx))
                            .label(t!("channel-settings-save"))
                            .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                    ),
            )
    }
}

/// `domains` as Mattermost keeps a team's allowed domains: each one
/// lowercase and without a leading `@`, joined by ", ".
fn normalize_domains(domains: &str) -> String {
    domains
        .split(|c: char| c == ',' || c.is_whitespace())
        .map(|domain| domain.trim().trim_start_matches('@').to_lowercase())
        .filter(|domain| !domain.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::normalize_domains;

    #[test]
    fn domains_are_kept_as_the_server_keeps_them() {
        assert_eq!(normalize_domains(""), "");
        assert_eq!(
            normalize_domains(" @Example.com,corp.example.org  lab.example.net,,"),
            "example.com, corp.example.org, lab.example.net"
        );
        assert_eq!(
            normalize_domains("example.com, corp.example.org"),
            "example.com, corp.example.org"
        );
    }
}
