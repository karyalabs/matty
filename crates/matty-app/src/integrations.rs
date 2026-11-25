//! Integrations (#1071): a team's incoming webhooks and custom slash
//! commands, listed, made and deleted, as the webapp's Integrations pages.

use std::sync::Arc;

use gpui_icons::{LucideIcon, lucide};
use gpui_kit::{
    App, AppContext as _, ClipboardItem, Context, Entity, EventEmitter, Focusable as _,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _,
};
use matty_api::models::{CustomCommand, IncomingWebhook};
use matty_sync::IntegrationChange;

use crate::style::{self, TypeStyleExt as _, metrics, scaled};
use crate::t;
use crate::ui::{
    button::{Button, ButtonSize, ButtonVariant},
    checkbox::Checkbox,
    dialog::{DialogHandle, dialog_description, dialog_title},
    field::Field,
    input::{InputEvent, InputState},
    select::{Select, SelectEvent, SelectItem, SelectState},
    tabs::{Tab, Tabs},
    theme::UiTheme,
};

/// A team's integrations as the server sent them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Integrations {
    pub team_id: String,
    pub hooks: Arc<[IncomingWebhook]>,
    pub commands: Arc<[CustomCommand]>,
    /// The webhook or command just made.
    pub created: Option<String>,
}

/// What the dialog asks of sync.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IntegrationsEvent {
    Change(IntegrationChange),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Webhooks,
    Commands,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Mode {
    List,
    Add,
    /// Waiting for the server to make it.
    Saving,
    /// Made: its URL or token shows once here.
    Created(String),
}

/// The server's limits on a command's trigger.
const TRIGGER_MAX: usize = 128;

/// A slash command's trigger as typed, without its slash.
fn trigger_problem(trigger: &str) -> Option<SharedString> {
    if trigger.is_empty() {
        Some(t!("integrations-trigger-empty"))
    } else if trigger.starts_with('/') {
        Some(t!("integrations-trigger-slash"))
    } else if trigger.contains(char::is_whitespace) {
        Some(t!("integrations-trigger-space"))
    } else if trigger.chars().count() > TRIGGER_MAX {
        Some(t!("channel-edit-too-long", max = TRIGGER_MAX))
    } else {
        None
    }
}

fn url_problem(url: &str) -> Option<SharedString> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"));
    match rest {
        Some(host) if !host.is_empty() => None,
        _ => Some(t!("integrations-url-invalid")),
    }
}

/// "Integrations…": the team's incoming webhooks and slash commands.
pub struct IntegrationsView {
    team_id: String,
    server: String,
    /// The team's channels the user is in, by id, for a webhook's channel.
    channels: Vec<(SharedString, SharedString)>,
    kind: Kind,
    mode: Mode,
    loaded: Option<Integrations>,
    error: Option<SharedString>,
    confirm_delete: Option<String>,
    copied: bool,
    tried: bool,
    name: Entity<InputState>,
    description: Entity<InputState>,
    channel: Entity<SelectState>,
    trigger: Entity<InputState>,
    url: Entity<InputState>,
    method: Entity<SelectState>,
    hint: Entity<InputState>,
    autocomplete: bool,
    handle: DialogHandle,
    focus_first: bool,
    /// The dialog itself, focused when the focused control goes away (a
    /// deleted row, a finished form), so Escape still closes it.
    focus: gpui_kit::FocusHandle,
    refocus: bool,
    _subscriptions: Vec<gpui_kit::Subscription>,
}

impl EventEmitter<IntegrationsEvent> for IntegrationsView {}

impl IntegrationsView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = |placeholder: SharedString, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
        };
        let name = input(t!("integrations-name-placeholder"), window, cx);
        let description = input(t!("integrations-description-placeholder"), window, cx);
        let trigger = input(t!("integrations-trigger-placeholder"), window, cx);
        let url = input(t!("integrations-url-placeholder"), window, cx);
        let hint = input(t!("integrations-hint-placeholder"), window, cx);
        let channel = cx.new(|cx| SelectState::new(Vec::new(), window, cx));
        let method = cx.new(|cx| {
            let mut state = SelectState::new(
                [
                    SelectItem::new("P", "POST"), // i18n-ignore
                    SelectItem::new("G", "GET"),  // i18n-ignore
                ],
                window,
                cx,
            );
            state.set_value(Some("P".into()), window, cx);
            state
        });
        let mut subscriptions = Vec::new();
        for field in [&name, &description, &trigger, &url, &hint] {
            subscriptions.push(cx.subscribe_in(
                field,
                window,
                |this: &mut Self, _, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        this.save(window, cx);
                    } else {
                        cx.notify();
                    }
                },
            ));
        }
        for select in [&channel, &method] {
            subscriptions.push(cx.subscribe(select, |_, _, _: &SelectEvent, cx| cx.notify()));
        }
        Self {
            team_id: String::new(),
            server: String::new(),
            channels: Vec::new(),
            kind: Kind::Webhooks,
            mode: Mode::List,
            loaded: None,
            error: None,
            confirm_delete: None,
            copied: false,
            tried: false,
            name,
            description,
            channel,
            trigger,
            url,
            method,
            hint,
            autocomplete: false,
            handle: DialogHandle::new(false),
            focus_first: false,
            focus: cx.focus_handle(),
            refocus: false,
            _subscriptions: subscriptions,
        }
    }

    pub fn handle(&self) -> &DialogHandle {
        &self.handle
    }

    /// Opens on the team's lists, which sync is asked for; `channels` are
    /// the team's channels the user is in.
    pub fn open(
        &mut self,
        team_id: String,
        server: String,
        channels: Vec<(SharedString, SharedString)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.team_id != team_id {
            self.loaded = None;
        }
        self.team_id = team_id;
        self.server = server.trim_end_matches('/').to_owned();
        let items: Vec<SelectItem> = channels
            .iter()
            .map(|(id, name)| SelectItem::new(id.clone(), name.clone()))
            .collect();
        self.channels = channels;
        self.channel
            .update(cx, |select, cx| select.set_items(items, cx));
        self.mode = Mode::List;
        self.error = None;
        self.confirm_delete = None;
        self.handle.open(window, cx);
        cx.notify();
    }

    /// The server's answer; a list for another team is dropped.
    pub fn set(&mut self, result: Result<Integrations, SharedString>, cx: &mut Context<Self>) {
        match result {
            Ok(integrations) => {
                if integrations.team_id != self.team_id {
                    return;
                }
                if let (Mode::Saving, Some(created)) = (&self.mode, &integrations.created) {
                    self.mode = Mode::Created(created.clone());
                    self.copied = false;
                    self.refocus = true;
                }
                self.error = None;
                self.loaded = Some(integrations);
            }
            Err(error) => {
                if self.mode == Mode::Saving {
                    self.mode = Mode::Add;
                }
                self.error = Some(error);
            }
        }
        cx.notify();
    }

    fn start_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for field in [
            &self.name,
            &self.description,
            &self.trigger,
            &self.url,
            &self.hint,
        ] {
            field.update(cx, |input, cx| input.set_value("", window, cx));
        }
        let first = self.channels.first().map(|(id, _)| id.clone());
        self.channel
            .update(cx, |select, cx| select.set_value(first, window, cx));
        self.method.update(cx, |select, cx| {
            select.set_value(Some("P".into()), window, cx)
        });
        self.autocomplete = false;
        self.tried = false;
        self.error = None;
        self.mode = Mode::Add;
        self.focus_first = true;
        cx.notify();
    }

    fn value(input: &Entity<InputState>, cx: &App) -> String {
        input.read(cx).value().trim().to_owned()
    }

    fn problems(&self, cx: &App) -> bool {
        match self.kind {
            Kind::Webhooks => self.channel.read(cx).value().is_none(),
            Kind::Commands => {
                trigger_problem(&Self::value(&self.trigger, cx)).is_some()
                    || url_problem(&Self::value(&self.url, cx)).is_some()
            }
        }
    }

    fn save(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.mode != Mode::Add {
            return;
        }
        if self.problems(cx) {
            self.tried = true;
            cx.notify();
            return;
        }
        let display_name = Self::value(&self.name, cx);
        let description = Self::value(&self.description, cx);
        let change = match self.kind {
            Kind::Webhooks => IntegrationChange::CreateWebhook {
                team_id: self.team_id.clone(),
                channel_id: self
                    .channel
                    .read(cx)
                    .value()
                    .map(|id| id.to_string())
                    .unwrap_or_default(),
                display_name,
                description,
            },
            Kind::Commands => {
                let hint = Self::value(&self.hint, cx);
                IntegrationChange::CreateCommand(Box::new(CustomCommand {
                    team_id: self.team_id.clone(),
                    trigger: Self::value(&self.trigger, cx).to_lowercase(),
                    url: Self::value(&self.url, cx),
                    method: self
                        .method
                        .read(cx)
                        .value()
                        .map_or_else(|| "P".to_owned(), |method| method.to_string()),
                    display_name,
                    auto_complete: self.autocomplete,
                    auto_complete_desc: if self.autocomplete {
                        description.clone()
                    } else {
                        String::new()
                    },
                    auto_complete_hint: if self.autocomplete {
                        hint
                    } else {
                        String::new()
                    },
                    description,
                    ..CustomCommand::default()
                }))
            }
        };
        self.mode = Mode::Saving;
        self.error = None;
        cx.emit(IntegrationsEvent::Change(change));
        cx.notify();
    }

    fn delete(&mut self, id: String, cx: &mut Context<Self>) {
        if self.confirm_delete.as_deref() != Some(id.as_str()) {
            self.confirm_delete = Some(id);
            cx.notify();
            return;
        }
        self.confirm_delete = None;
        self.refocus = true;
        let team_id = self.team_id.clone();
        cx.emit(IntegrationsEvent::Change(match self.kind {
            Kind::Webhooks => IntegrationChange::DeleteWebhook {
                team_id,
                hook_id: id,
            },
            Kind::Commands => IntegrationChange::DeleteCommand {
                team_id,
                command_id: id,
            },
        }));
        cx.notify();
    }

    fn hook_url(&self, hook_id: &str) -> String {
        format!("{}/hooks/{hook_id}", self.server)
    }

    fn channel_name(&self, channel_id: &str) -> SharedString {
        self.channels
            .iter()
            .find(|(id, _)| id.as_ref() == channel_id)
            .map_or_else(
                || t!("integrations-channel-unknown"),
                |(_, name)| name.clone(),
            )
    }

    fn row(
        &self,
        id: String,
        title: SharedString,
        detail: SharedString,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let theme = UiTheme::read(cx).clone();
        let confirming = self.confirm_delete.as_deref() == Some(id.as_str());
        let deleted = id.clone();
        div()
            .id(gpui_kit::ElementId::Name(
                format!("integrations.row.{id}").into(),
            ))
            .min_h(scaled(&theme, metrics::ROW_HEIGHT + 12.))
            .py(theme.space(1.5))
            .flex()
            .items_center()
            .gap(theme.space(3.))
            .child(
                div()
                    .flex_shrink_0()
                    .size(scaled(&theme, 28.))
                    .rounded(theme.radius.md)
                    .bg(theme.colors.muted)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        lucide(match self.kind {
                            Kind::Webhooks => LucideIcon::Webhook,
                            Kind::Commands => LucideIcon::SquareSlash,
                        })
                        .size(scaled(&theme, 15.))
                        .text_color(theme.colors.muted_foreground),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .truncate()
                            .type_style(style::UI, &theme)
                            .text_color(theme.colors.foreground)
                            .child(title),
                    )
                    .child(
                        div()
                            .truncate()
                            .type_style(style::META, &theme)
                            .text_color(theme.colors.muted_foreground)
                            .child(detail),
                    ),
            )
            .when(confirming, |row| {
                row.child(
                    Button::new(gpui_kit::ElementId::Name(
                        format!("integrations.keep.{id}").into(),
                    ))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .label(t!("integrations-keep"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.confirm_delete = None;
                        cx.notify();
                    })),
                )
            })
            .child(
                Button::new(gpui_kit::ElementId::Name(
                    format!("integrations.delete.{id}").into(),
                ))
                .variant(if confirming {
                    ButtonVariant::Destructive
                } else {
                    ButtonVariant::Ghost
                })
                .size(ButtonSize::Sm)
                .label(if confirming {
                    t!("integrations-delete-confirm")
                } else {
                    t!("integrations-delete")
                })
                .on_click(cx.listener(move |this, _, _, cx| this.delete(deleted.clone(), cx))),
            )
    }

    fn list(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let theme = UiTheme::read(cx).clone();
        let rows: Vec<gpui_kit::AnyElement> = match (&self.loaded, self.kind) {
            (None, _) => Vec::new(),
            (Some(loaded), Kind::Webhooks) => loaded
                .hooks
                .iter()
                .filter(|hook| hook.delete_at == 0)
                .map(|hook| {
                    let title: SharedString = if hook.display_name.is_empty() {
                        t!("integrations-untitled")
                    } else {
                        hook.display_name.clone().into()
                    };
                    let channel = self.channel_name(&hook.channel_id);
                    let detail = if hook.description.is_empty() {
                        t!("integrations-hook-detail", channel = channel.to_string())
                    } else {
                        t!(
                            "integrations-hook-detail-described",
                            channel = channel.to_string(),
                            description = hook.description.clone()
                        )
                    };
                    self.row(hook.id.clone(), title, detail, cx)
                        .into_any_element()
                })
                .collect(),
            (Some(loaded), Kind::Commands) => loaded
                .commands
                .iter()
                .filter(|command| command.delete_at == 0)
                .map(|command| {
                    let title: SharedString = if command.display_name.is_empty() {
                        format!("/{}", command.trigger).into()
                    } else {
                        format!("/{} · {}", command.trigger, command.display_name).into()
                    };
                    let detail: SharedString = if command.description.is_empty() {
                        command.url.clone().into()
                    } else {
                        command.description.clone().into()
                    };
                    self.row(command.id.clone(), title, detail, cx)
                        .into_any_element()
                })
                .collect(),
        };
        let empty = match (&self.loaded, rows.is_empty(), self.kind) {
            (None, ..) if self.error.is_none() => Some(t!("integrations-loading")),
            (_, false, _) | (None, ..) => None,
            (Some(_), true, Kind::Webhooks) => Some(t!("integrations-no-hooks")),
            (Some(_), true, Kind::Commands) => Some(t!("integrations-no-commands")),
        };
        div()
            .id("integrations.list")
            .overflow_y_scroll()
            .max_h(scaled(&theme, (metrics::ROW_HEIGHT + 12.) * 7.))
            .flex()
            .flex_col()
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
            .children(rows)
            .into_any_element()
    }

    fn form(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let theme = UiTheme::read(cx).clone();
        let name = Field::new("integrations.name", &self.name).label(t!("integrations-name"));
        let description = Field::new("integrations.description", &self.description)
            .label(t!("integrations-description"));
        let body = div().flex().flex_col().gap(theme.space(3.));
        match self.kind {
            Kind::Webhooks => {
                let mut channel = Field::from_control(
                    "integrations.channel",
                    Select::new(&self.channel)
                        .aria_label(t!("integrations-channel"))
                        .placeholder(t!("integrations-channel-placeholder")),
                )
                .label(t!("integrations-channel"));
                if self.tried && self.channel.read(cx).value().is_none() {
                    channel = channel.error(t!("integrations-channel-missing"));
                }
                body.child(name)
                    .child(description)
                    .child(channel)
                    .into_any_element()
            }
            Kind::Commands => {
                let trigger_now = Self::value(&self.trigger, cx);
                let url_now = Self::value(&self.url, cx);
                let mut trigger = Field::new("integrations.trigger", &self.trigger)
                    .label(t!("integrations-trigger"));
                trigger = match trigger_problem(&trigger_now)
                    .filter(|_| self.tried || !trigger_now.is_empty())
                {
                    Some(problem) => trigger.error(problem),
                    None => trigger.description(t!("integrations-trigger-hint")),
                };
                let mut url =
                    Field::new("integrations.url", &self.url).label(t!("integrations-url"));
                url = match url_problem(&url_now).filter(|_| self.tried) {
                    Some(problem) => url.error(problem),
                    None => url.description(t!("integrations-url-hint")),
                };
                let method = Field::from_control(
                    "integrations.method",
                    Select::new(&self.method).aria_label(t!("integrations-method")),
                )
                .label(t!("integrations-method"));
                let view = cx.weak_entity();
                let autocomplete = div()
                    .flex()
                    .items_center()
                    .gap(theme.space(2.))
                    .child(
                        Checkbox::new("integrations.autocomplete")
                            .checked(self.autocomplete)
                            .aria_label(t!("integrations-autocomplete"))
                            .on_change(move |&next, _, cx| {
                                view.update(cx, |this, cx| {
                                    this.autocomplete = next;
                                    cx.notify();
                                })
                                .ok();
                            }),
                    )
                    .child(
                        div()
                            .type_style(style::UI, &theme)
                            .text_color(theme.colors.foreground)
                            .child(t!("integrations-autocomplete")),
                    );
                body.child(trigger)
                    .child(url)
                    .child(method)
                    .child(name)
                    .child(description)
                    .child(autocomplete)
                    .when(self.autocomplete, |body| {
                        body.child(
                            Field::new("integrations.hint", &self.hint)
                                .label(t!("integrations-hint")),
                        )
                    })
                    .into_any_element()
            }
        }
    }

    fn created(&self, id: &str, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let theme = UiTheme::read(cx).clone();
        let (note, secret) = match self.kind {
            Kind::Webhooks => (t!("integrations-hook-created"), self.hook_url(id)),
            Kind::Commands => (
                t!("integrations-command-created"),
                self.loaded
                    .as_ref()
                    .and_then(|loaded| loaded.commands.iter().find(|command| command.id == id))
                    .map(|command| command.token.clone())
                    .unwrap_or_default(),
            ),
        };
        let copy = secret.clone();
        div()
            .flex()
            .flex_col()
            .gap(theme.space(2.))
            .child(
                div()
                    .type_style(style::UI, &theme)
                    .text_color(theme.colors.foreground)
                    .child(note),
            )
            .child(
                div()
                    .id("integrations.secret")
                    .px(theme.space(3.))
                    .py(theme.space(2.))
                    .rounded(theme.radius.md)
                    .bg(theme.colors.muted)
                    .font_family(theme.fonts.mono.clone())
                    .type_style(style::META, &theme)
                    .text_color(theme.colors.foreground)
                    .child(secret),
            )
            .child(
                div().flex().child(
                    Button::new("integrations.copy")
                        .variant(ButtonVariant::Outline)
                        .label(if self.copied {
                            t!("integrations-copied")
                        } else {
                            t!("integrations-copy")
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()));
                            this.copied = true;
                            cx.notify();
                        })),
                ),
            )
            .into_any_element()
    }
}

impl Render for IntegrationsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = UiTheme::read(cx).clone();
        if self.focus_first {
            self.focus_first = false;
            let first = match self.kind {
                Kind::Webhooks => &self.name,
                Kind::Commands => &self.trigger,
            };
            let focus = first.read(cx).focus_handle(cx);
            window.on_next_frame(move |window, cx| window.focus(&focus, cx));
        } else if self.refocus {
            self.refocus = false;
            let focus = self.focus.clone();
            window.on_next_frame(move |window, cx| window.focus(&focus, cx));
        }
        let (title, description) = match (&self.mode, self.kind) {
            (Mode::List, _) => (
                t!("integrations-title"),
                t!("integrations-description-text"),
            ),
            (Mode::Add | Mode::Saving, Kind::Webhooks) => (
                t!("integrations-add-hook"),
                t!("integrations-add-hook-text"),
            ),
            (Mode::Add | Mode::Saving, Kind::Commands) => (
                t!("integrations-add-command"),
                t!("integrations-add-command-text"),
            ),
            (Mode::Created(_), Kind::Webhooks) => (
                t!("integrations-add-hook"),
                t!("integrations-hook-created-text"),
            ),
            (Mode::Created(_), Kind::Commands) => (
                t!("integrations-add-command"),
                t!("integrations-command-created-text"),
            ),
        };
        let view = cx.entity().downgrade();
        let body = match &self.mode {
            Mode::List => self.list(cx),
            Mode::Add | Mode::Saving => self.form(cx),
            Mode::Created(id) => {
                let id = id.clone();
                self.created(&id, cx)
            }
        };
        let footer = div().flex().items_center().gap(theme.space(2.));
        let footer = match &self.mode {
            Mode::List => footer
                .child(
                    Button::new("integrations.add")
                        .variant(ButtonVariant::Outline)
                        .label(match self.kind {
                            Kind::Webhooks => t!("integrations-add-hook"),
                            Kind::Commands => t!("integrations-add-command"),
                        })
                        .on_click(cx.listener(|this, _, window, cx| this.start_add(window, cx))),
                )
                .child(div().flex_1())
                .child(
                    Button::new("integrations.close")
                        .variant(ButtonVariant::Outline)
                        .label(t!("team-members-close"))
                        .on_click(cx.listener(|this, _, window, cx| this.handle.close(window, cx))),
                ),
            Mode::Add | Mode::Saving => footer
                .child(div().flex_1())
                .child(
                    Button::new("integrations.cancel")
                        .variant(ButtonVariant::Outline)
                        .label(t!("group-editor-cancel"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.mode = Mode::List;
                            this.error = None;
                            this.refocus = true;
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("integrations.save")
                        .disabled(self.mode == Mode::Saving)
                        .label(t!("integrations-save"))
                        .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                ),
            Mode::Created(_) => footer.child(div().flex_1()).child(
                Button::new("integrations.done")
                    .label(t!("integrations-done"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.mode = Mode::List;
                        this.refocus = true;
                        cx.notify();
                    })),
            ),
        };
        div()
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .gap(theme.space(4.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.space(1.))
                    .child(dialog_title("integrations.title", cx).child(title))
                    .child(dialog_description("integrations.description", cx).child(description)),
            )
            .when(self.mode == Mode::List, |column| {
                column.child(
                    Tabs::new("integrations.tabs")
                        .aria_label(t!("integrations-title"))
                        .selected(match self.kind {
                            Kind::Webhooks => "hooks",
                            Kind::Commands => "commands",
                        })
                        .item(Tab::new(
                            "integrations.tabs.hooks",
                            "hooks",
                            t!("integrations-hooks"),
                        ))
                        .item(Tab::new(
                            "integrations.tabs.commands",
                            "commands",
                            t!("integrations-commands"),
                        ))
                        .on_change(move |value, _, cx| {
                            view.update(cx, |this, cx| {
                                this.kind = if value == "commands" {
                                    Kind::Commands
                                } else {
                                    Kind::Webhooks
                                };
                                this.confirm_delete = None;
                                cx.notify();
                            })
                            .ok();
                        }),
                )
            })
            .when_some(self.error.clone(), |column, error| {
                column.child(
                    div()
                        .type_style(style::META, &theme)
                        .text_color(theme.colors.destructive)
                        .child(error),
                )
            })
            .child(body)
            .child(footer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triggers_are_one_plain_word() {
        assert!(trigger_problem("deploy").is_none());
        assert!(trigger_problem("").is_some());
        assert!(trigger_problem("/deploy").is_some());
        assert!(trigger_problem("deploy now").is_some());
        assert!(trigger_problem(&"x".repeat(TRIGGER_MAX + 1)).is_some());
    }

    #[test]
    fn command_urls_are_web_addresses() {
        assert!(url_problem("https://ci.example.com/hook").is_none());
        assert!(url_problem("http://10.0.0.2:8080").is_none());
        assert!(url_problem("ci.example.com").is_some());
        assert!(url_problem("https://").is_some());
    }
}
