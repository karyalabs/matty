//! Integrations (#1071, #1073): a team's incoming webhooks, outgoing
//! webhooks and custom slash commands, listed, made, changed and deleted,
//! as the webapp's Integrations pages.

use std::sync::Arc;

use gpui_icons::{LucideIcon, lucide};
use gpui_kit::{
    App, AppContext as _, ClipboardItem, Context, Entity, EventEmitter, Focusable as _,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _,
};
use matty_api::models::{Bot, CustomCommand, IncomingWebhook, OutgoingWebhook};
use matty_sync::IntegrationChange;

use crate::priority::Features;
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
    pub outgoing: Arc<[OutgoingWebhook]>,
    pub bots: Arc<[Bot]>,
    /// The webhook, command or bot just made.
    pub created: Option<String>,
    /// A bot token just made.
    pub secret: Option<String>,
}

/// What the dialog asks of sync.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IntegrationsEvent {
    Change(IntegrationChange),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Webhooks,
    Outgoing,
    Commands,
    Bots,
}

impl Kind {
    fn id(self) -> &'static str {
        match self {
            Self::Webhooks => "hooks",
            Self::Outgoing => "outgoing",
            Self::Commands => "commands",
            Self::Bots => "bots",
        }
    }

    /// The kinds the server has on, in the webapp's order.
    fn available(cx: &App) -> Vec<Self> {
        let features = Features::read(cx);
        [
            (Self::Webhooks, features.incoming_webhooks),
            (Self::Outgoing, features.outgoing_webhooks),
            (Self::Commands, features.custom_commands),
            (Self::Bots, features.bot_accounts),
        ]
        .into_iter()
        .filter_map(|(kind, on)| on.then_some(kind))
        .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Mode {
    List,
    /// Making one, or changing the one `editing` names.
    Form,
    /// Waiting for the server.
    Saving,
    /// Made: its URL or token shows once here.
    Created(String),
    /// A bot's new access token, shown once (#1075).
    Secret(String),
}

/// The server's limits on a command's trigger.
const TRIGGER_MAX: usize = 128;
/// The outgoing webhook channel choice for "any channel".
const ANY_CHANNEL: &str = "*";

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

/// What is wrong with a bot's `@name`, if anything: the server's rule of
/// 3 to 22 lowercase letters, digits, `.`, `-` or `_`, starting with a
/// letter.
fn username_problem(username: &str) -> Option<SharedString> {
    let plain = username
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | '_'));
    let starts = username
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_lowercase());
    (!(3..=22).contains(&username.chars().count()) || !plain || !starts)
        .then(|| t!("integrations-username-invalid"))
}

/// Words or URLs typed into one field, split on commas and spaces.
fn split_list(text: &str) -> Vec<String> {
    text.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

/// What is wrong with an outgoing webhook's URLs, if anything.
fn callbacks_problem(urls: &[String]) -> Option<SharedString> {
    if urls.is_empty() {
        return Some(t!("integrations-callbacks-empty"));
    }
    urls.iter().find_map(|url| url_problem(url))
}

/// "Integrations…": the team's webhooks and slash commands.
pub struct IntegrationsView {
    team_id: String,
    server: String,
    /// The team's channels the user is in, by id, for a webhook's channel.
    channels: Vec<(SharedString, SharedString)>,
    kind: Kind,
    mode: Mode,
    /// The webhook or command the form changes; `None` while making one.
    editing: Option<String>,
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
    out_channel: Entity<SelectState>,
    words: Entity<InputState>,
    when: Entity<SelectState>,
    callbacks: Entity<InputState>,
    content_type: Entity<SelectState>,
    username: Entity<InputState>,
    /// The pending save is a bot token, so a refusal goes back to the list.
    making_token: bool,
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
        let words = input(t!("integrations-words-placeholder"), window, cx);
        let callbacks = input(t!("integrations-callbacks-placeholder"), window, cx);
        let username = input(t!("integrations-username-placeholder"), window, cx);
        let select =
            |items: Vec<SelectItem>, first: &str, window: &mut Window, cx: &mut Context<Self>| {
                let first: SharedString = first.to_owned().into();
                cx.new(|cx| {
                    let mut state = SelectState::new(items, window, cx);
                    if !first.is_empty() {
                        state.set_value(Some(first), window, cx);
                    }
                    state
                })
            };
        let channel = select(Vec::new(), "", window, cx);
        let out_channel = select(Vec::new(), ANY_CHANNEL, window, cx);
        let method = select(
            vec![
                SelectItem::new("P", "POST"), // i18n-ignore
                SelectItem::new("G", "GET"),  // i18n-ignore
            ],
            "P",
            window,
            cx,
        );
        let when = select(
            vec![
                SelectItem::new("0", t!("integrations-when-first")),
                SelectItem::new("1", t!("integrations-when-starts")),
            ],
            "0",
            window,
            cx,
        );
        let content_type = select(
            vec![
                SelectItem::new(
                    "application/x-www-form-urlencoded",
                    "application/x-www-form-urlencoded",
                ), // i18n-ignore
                SelectItem::new("application/json", "application/json"), // i18n-ignore
            ],
            "application/x-www-form-urlencoded",
            window,
            cx,
        );
        let mut subscriptions = Vec::new();
        for field in [
            &name,
            &description,
            &trigger,
            &url,
            &hint,
            &words,
            &callbacks,
            &username,
        ] {
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
        for select in [&channel, &method, &out_channel, &when, &content_type] {
            subscriptions.push(cx.subscribe(select, |_, _, _: &SelectEvent, cx| cx.notify()));
        }
        Self {
            team_id: String::new(),
            server: String::new(),
            channels: Vec::new(),
            kind: Kind::Webhooks,
            mode: Mode::List,
            editing: None,
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
            out_channel,
            words,
            when,
            callbacks,
            content_type,
            username,
            making_token: false,
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
        channels: Vec<(SharedString, SharedString, bool)>,
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
            .map(|(id, name, _)| SelectItem::new(id.clone(), name.clone()))
            .collect();
        // Outgoing webhooks only hear public channels.
        let mut any = vec![SelectItem::new(ANY_CHANNEL, t!("integrations-any-channel"))];
        any.extend(
            channels
                .iter()
                .filter(|(_, _, public)| *public)
                .map(|(id, name, _)| SelectItem::new(id.clone(), name.clone())),
        );
        self.channels = channels
            .into_iter()
            .map(|(id, name, _)| (id, name))
            .collect();
        self.channel
            .update(cx, |select, cx| select.set_items(items, cx));
        self.out_channel
            .update(cx, |select, cx| select.set_items(any, cx));
        let kinds = Kind::available(cx);
        if !kinds.contains(&self.kind)
            && let Some(first) = kinds.first()
        {
            self.kind = *first;
        }
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
                if self.mode == Mode::Saving {
                    self.refocus = true;
                    self.copied = false;
                    self.mode = match (&integrations.secret, &integrations.created) {
                        (Some(secret), _) => Mode::Secret(secret.clone()),
                        (None, Some(created)) => Mode::Created(created.clone()),
                        (None, None) => Mode::List,
                    };
                }
                self.error = None;
                self.loaded = Some(integrations);
            }
            Err(error) => {
                if self.mode == Mode::Saving {
                    self.mode = if self.making_token {
                        Mode::List
                    } else {
                        Mode::Form
                    };
                }
                self.error = Some(error);
            }
        }
        cx.notify();
    }

    fn set_text(&self, input: &Entity<InputState>, text: &str, window: &mut Window, cx: &mut App) {
        let text = text.to_owned();
        input.update(cx, |input, cx| input.set_value(text, window, cx));
    }

    fn set_choice(select: &Entity<SelectState>, value: &str, window: &mut Window, cx: &mut App) {
        let value: Option<SharedString> = (!value.is_empty()).then(|| value.to_owned().into());
        select.update(cx, |select, cx| select.set_value(value, window, cx));
    }

    /// The form, empty for a new one or filled from the one with `id`.
    fn start_form(&mut self, id: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let loaded = self.loaded.clone().unwrap_or_default();
        let hook = id
            .as_ref()
            .and_then(|id| loaded.hooks.iter().find(|hook| &hook.id == id))
            .cloned()
            .unwrap_or_default();
        let command = id
            .as_ref()
            .and_then(|id| loaded.commands.iter().find(|command| &command.id == id))
            .cloned()
            .unwrap_or_default();
        let outgoing = id
            .as_ref()
            .and_then(|id| loaded.outgoing.iter().find(|hook| &hook.id == id))
            .cloned()
            .unwrap_or_default();
        let bot = id
            .as_ref()
            .and_then(|id| loaded.bots.iter().find(|bot| &bot.user_id == id))
            .cloned()
            .unwrap_or_default();
        let (name, description) = match self.kind {
            Kind::Webhooks => (hook.display_name.clone(), hook.description.clone()),
            Kind::Commands => (command.display_name.clone(), command.description.clone()),
            Kind::Outgoing => (outgoing.display_name.clone(), outgoing.description.clone()),
            Kind::Bots => (bot.display_name.clone(), bot.description.clone()),
        };
        self.set_text(&self.username, &bot.username, window, cx);
        self.set_text(&self.name, &name, window, cx);
        self.set_text(&self.description, &description, window, cx);
        self.set_text(&self.trigger, &command.trigger, window, cx);
        self.set_text(&self.url, &command.url, window, cx);
        self.set_text(&self.hint, &command.auto_complete_hint, window, cx);
        self.set_text(&self.words, &outgoing.trigger_words.join(", "), window, cx);
        self.set_text(
            &self.callbacks,
            &outgoing.callback_urls.join(" "),
            window,
            cx,
        );
        let first = self
            .channels
            .first()
            .map(|(id, _)| id.to_string())
            .unwrap_or_default();
        let channel = if hook.channel_id.is_empty() {
            first
        } else {
            hook.channel_id.clone()
        };
        Self::set_choice(&self.channel, &channel, window, cx);
        let out_channel = if outgoing.channel_id.is_empty() {
            ANY_CHANNEL.to_owned()
        } else {
            outgoing.channel_id.clone()
        };
        Self::set_choice(&self.out_channel, &out_channel, window, cx);
        let method = if command.method.is_empty() {
            "P"
        } else {
            &command.method
        };
        Self::set_choice(&self.method, method, window, cx);
        Self::set_choice(&self.when, &outgoing.trigger_when.to_string(), window, cx);
        let content_type = if outgoing.content_type.is_empty() {
            "application/x-www-form-urlencoded"
        } else {
            &outgoing.content_type
        };
        Self::set_choice(&self.content_type, content_type, window, cx);
        self.autocomplete = command.auto_complete;
        self.editing = id;
        self.tried = false;
        self.error = None;
        self.confirm_delete = None;
        self.mode = Mode::Form;
        self.focus_first = true;
        cx.notify();
    }

    fn value(input: &Entity<InputState>, cx: &App) -> String {
        input.read(cx).value().trim().to_owned()
    }

    fn choice(select: &Entity<SelectState>, cx: &App) -> String {
        select
            .read(cx)
            .value()
            .map(|value| value.to_string())
            .unwrap_or_default()
    }

    /// An outgoing webhook needs trigger words unless it has a channel.
    fn words_problem(&self, cx: &App) -> Option<SharedString> {
        let any = Self::choice(&self.out_channel, cx) == ANY_CHANNEL;
        (any && split_list(&Self::value(&self.words, cx)).is_empty())
            .then(|| t!("integrations-words-needed"))
    }

    fn problems(&self, cx: &App) -> bool {
        match self.kind {
            Kind::Webhooks => self.channel.read(cx).value().is_none(),
            Kind::Commands => {
                trigger_problem(&Self::value(&self.trigger, cx)).is_some()
                    || url_problem(&Self::value(&self.url, cx)).is_some()
            }
            Kind::Outgoing => {
                self.words_problem(cx).is_some()
                    || callbacks_problem(&split_list(&Self::value(&self.callbacks, cx))).is_some()
            }
            Kind::Bots => username_problem(&Self::value(&self.username, cx)).is_some(),
        }
    }

    /// Asks for a new token for the bot with `user_id`.
    fn make_token(&mut self, user_id: String, cx: &mut Context<Self>) {
        let username = self
            .loaded
            .as_ref()
            .and_then(|loaded| loaded.bots.iter().find(|bot| bot.user_id == user_id))
            .map(|bot| bot.username.clone())
            .unwrap_or_default();
        self.mode = Mode::Saving;
        self.making_token = true;
        self.error = None;
        self.refocus = true;
        cx.emit(IntegrationsEvent::Change(IntegrationChange::BotToken {
            team_id: self.team_id.clone(),
            user_id,
            description: t!("integrations-token-description", username = username).to_string(),
        }));
        cx.notify();
    }

    fn set_bot_active(&mut self, user_id: String, active: bool, cx: &mut Context<Self>) {
        self.refocus = true;
        cx.emit(IntegrationsEvent::Change(IntegrationChange::SetBotActive {
            team_id: self.team_id.clone(),
            user_id,
            active,
        }));
        cx.notify();
    }

    fn save(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.mode != Mode::Form {
            return;
        }
        if self.problems(cx) {
            self.tried = true;
            cx.notify();
            return;
        }
        let display_name = Self::value(&self.name, cx);
        let description = Self::value(&self.description, cx);
        let loaded = self.loaded.clone().unwrap_or_default();
        let editing = self.editing.clone();
        let change = match self.kind {
            Kind::Webhooks => {
                let channel_id = Self::choice(&self.channel, cx);
                match editing.and_then(|id| loaded.hooks.iter().find(|hook| hook.id == id).cloned())
                {
                    Some(hook) => IntegrationChange::UpdateWebhook(Box::new(IncomingWebhook {
                        channel_id,
                        display_name,
                        description,
                        ..hook
                    })),
                    None => IntegrationChange::CreateWebhook {
                        team_id: self.team_id.clone(),
                        channel_id,
                        display_name,
                        description,
                    },
                }
            }
            Kind::Commands => {
                let hint = Self::value(&self.hint, cx);
                let base = editing
                    .as_ref()
                    .and_then(|id| loaded.commands.iter().find(|command| &command.id == id))
                    .cloned()
                    .unwrap_or_else(|| CustomCommand {
                        team_id: self.team_id.clone(),
                        ..CustomCommand::default()
                    });
                let command = Box::new(CustomCommand {
                    trigger: Self::value(&self.trigger, cx).to_lowercase(),
                    url: Self::value(&self.url, cx),
                    method: Self::choice(&self.method, cx),
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
                    ..base
                });
                if editing.is_some() {
                    IntegrationChange::UpdateCommand(command)
                } else {
                    IntegrationChange::CreateCommand(command)
                }
            }
            Kind::Outgoing => {
                let base = editing
                    .as_ref()
                    .and_then(|id| loaded.outgoing.iter().find(|hook| &hook.id == id))
                    .cloned()
                    .unwrap_or_else(|| OutgoingWebhook {
                        team_id: self.team_id.clone(),
                        ..OutgoingWebhook::default()
                    });
                let channel = Self::choice(&self.out_channel, cx);
                let hook = Box::new(OutgoingWebhook {
                    channel_id: if channel == ANY_CHANNEL {
                        String::new()
                    } else {
                        channel
                    },
                    trigger_words: split_list(&Self::value(&self.words, cx)),
                    trigger_when: Self::choice(&self.when, cx).parse().unwrap_or_default(),
                    callback_urls: split_list(&Self::value(&self.callbacks, cx)),
                    content_type: Self::choice(&self.content_type, cx),
                    display_name,
                    description,
                    ..base
                });
                if editing.is_some() {
                    IntegrationChange::UpdateOutgoing(hook)
                } else {
                    IntegrationChange::CreateOutgoing(hook)
                }
            }
            Kind::Bots => {
                let username = Self::value(&self.username, cx);
                match editing
                    .and_then(|id| loaded.bots.iter().find(|bot| bot.user_id == id).cloned())
                {
                    Some(bot) => IntegrationChange::UpdateBot {
                        team_id: self.team_id.clone(),
                        bot: Box::new(Bot {
                            username,
                            display_name,
                            description,
                            ..bot
                        }),
                    },
                    None => IntegrationChange::CreateBot {
                        team_id: self.team_id.clone(),
                        username,
                        display_name,
                        description,
                    },
                }
            }
        };
        self.making_token = false;
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
            Kind::Outgoing => IntegrationChange::DeleteOutgoing {
                team_id,
                hook_id: id,
            },
            Kind::Commands => IntegrationChange::DeleteCommand {
                team_id,
                command_id: id,
            },
            // A bot is disabled, not deleted, as in the webapp.
            Kind::Bots => IntegrationChange::SetBotActive {
                team_id,
                user_id: id,
                active: false,
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
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let theme = UiTheme::read(cx).clone();
        let confirming = self.confirm_delete.as_deref() == Some(id.as_str());
        let bot = self.kind == Kind::Bots;
        let deleted = id.clone();
        let edited = id.clone();
        let tokened = id.clone();
        let enabled = id.clone();
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
                            Kind::Outgoing => LucideIcon::Send,
                            Kind::Commands => LucideIcon::SquareSlash,
                            Kind::Bots => LucideIcon::Bot,
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
            .when(disabled, |row| {
                row.child(
                    Button::new(gpui_kit::ElementId::Name(
                        format!("integrations.enable.{id}").into(),
                    ))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .label(t!("integrations-enable"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_bot_active(enabled.clone(), true, cx)
                    })),
                )
            })
            .when(bot && !disabled && !confirming, |row| {
                row.child(
                    Button::new(gpui_kit::ElementId::Name(
                        format!("integrations.token.{id}").into(),
                    ))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .label(t!("integrations-add-token"))
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.make_token(tokened.clone(), cx)),
                    ),
                )
            })
            .when(!disabled && !confirming, |row| {
                row.child(
                    Button::new(gpui_kit::ElementId::Name(
                        format!("integrations.edit.{id}").into(),
                    ))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .label(t!("integrations-edit"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.start_form(Some(edited.clone()), window, cx)
                    })),
                )
            })
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
            .when(!disabled, |row| {
                row.child(
                    Button::new(gpui_kit::ElementId::Name(
                        format!("integrations.delete.{id}").into(),
                    ))
                    .variant(if confirming {
                        ButtonVariant::Destructive
                    } else {
                        ButtonVariant::Ghost
                    })
                    .size(ButtonSize::Sm)
                    .label(match (confirming, bot) {
                        (true, true) => t!("integrations-disable-confirm"),
                        (true, false) => t!("integrations-delete-confirm"),
                        (false, true) => t!("integrations-disable"),
                        (false, false) => t!("integrations-delete"),
                    })
                    .on_click(cx.listener(move |this, _, _, cx| this.delete(deleted.clone(), cx))),
                )
            })
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
                    self.row(hook.id.clone(), title, detail, false, cx)
                        .into_any_element()
                })
                .collect(),
            (Some(loaded), Kind::Outgoing) => loaded
                .outgoing
                .iter()
                .filter(|hook| hook.delete_at == 0)
                .map(|hook| {
                    let title: SharedString = if hook.display_name.is_empty() {
                        t!("integrations-untitled")
                    } else {
                        hook.display_name.clone().into()
                    };
                    let words = hook.trigger_words.join(", ");
                    let detail = match (hook.channel_id.is_empty(), words.is_empty()) {
                        (true, _) => t!("integrations-outgoing-detail-any", words = words),
                        (false, true) => t!(
                            "integrations-outgoing-detail-all",
                            channel = self.channel_name(&hook.channel_id).to_string()
                        ),
                        (false, false) => t!(
                            "integrations-outgoing-detail",
                            channel = self.channel_name(&hook.channel_id).to_string(),
                            words = words
                        ),
                    };
                    self.row(hook.id.clone(), title, detail, false, cx)
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
                    self.row(command.id.clone(), title, detail, false, cx)
                        .into_any_element()
                })
                .collect(),
            (Some(loaded), Kind::Bots) => {
                // Enabled bots first, as the webapp lists them.
                let mut bots: Vec<&Bot> = loaded.bots.iter().collect();
                bots.sort_by_key(|bot| (bot.delete_at != 0, bot.username.clone()));
                bots.into_iter()
                    .map(|bot| {
                        let title: SharedString = if bot.display_name.is_empty() {
                            format!("@{}", bot.username).into()
                        } else {
                            format!("{} · @{}", bot.display_name, bot.username).into()
                        };
                        let disabled = bot.delete_at != 0;
                        let detail: SharedString = match (disabled, bot.description.is_empty()) {
                            (true, _) => t!("integrations-bot-disabled"),
                            (false, true) => t!("integrations-bot-detail"),
                            (false, false) => bot.description.clone().into(),
                        };
                        self.row(bot.user_id.clone(), title, detail, disabled, cx)
                            .into_any_element()
                    })
                    .collect()
            }
        };
        let empty = match (&self.loaded, rows.is_empty(), self.kind) {
            (None, ..) if self.error.is_none() => Some(t!("integrations-loading")),
            (_, false, _) | (None, ..) => None,
            (Some(_), true, Kind::Webhooks) => Some(t!("integrations-no-hooks")),
            (Some(_), true, Kind::Outgoing) => Some(t!("integrations-no-outgoing")),
            (Some(_), true, Kind::Commands) => Some(t!("integrations-no-commands")),
            (Some(_), true, Kind::Bots) => Some(t!("integrations-no-bots")),
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
            Kind::Outgoing => {
                let channel = Field::from_control(
                    "integrations.out-channel",
                    Select::new(&self.out_channel).aria_label(t!("integrations-channel")),
                )
                .label(t!("integrations-channel"));
                let mut words =
                    Field::new("integrations.words", &self.words).label(t!("integrations-words"));
                words = match self.words_problem(cx).filter(|_| self.tried) {
                    Some(problem) => words.error(problem),
                    None => words.description(t!("integrations-words-hint")),
                };
                let when = Field::from_control(
                    "integrations.when",
                    Select::new(&self.when).aria_label(t!("integrations-when")),
                )
                .label(t!("integrations-when"));
                let mut callbacks = Field::new("integrations.callbacks", &self.callbacks)
                    .label(t!("integrations-callbacks"));
                callbacks = match callbacks_problem(&split_list(&Self::value(&self.callbacks, cx)))
                    .filter(|_| self.tried)
                {
                    Some(problem) => callbacks.error(problem),
                    None => callbacks.description(t!("integrations-callbacks-hint")),
                };
                let content_type = Field::from_control(
                    "integrations.content-type",
                    Select::new(&self.content_type).aria_label(t!("integrations-content-type")),
                )
                .label(t!("integrations-content-type"));
                body.child(name)
                    .child(description)
                    .child(channel)
                    .child(words)
                    .child(when)
                    .child(callbacks)
                    .child(content_type)
                    .into_any_element()
            }
            Kind::Bots => {
                let username_now = Self::value(&self.username, cx);
                let mut username = Field::new("integrations.username", &self.username)
                    .label(t!("integrations-username"));
                username = match username_problem(&username_now)
                    .filter(|_| self.tried || !username_now.is_empty())
                {
                    Some(problem) => username.error(problem),
                    None => username.description(t!("integrations-username-hint")),
                };
                body.child(username)
                    .child(name)
                    .child(description)
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
        let loaded = self.loaded.clone().unwrap_or_default();
        if self.kind == Kind::Bots {
            let username = loaded
                .bots
                .iter()
                .find(|bot| bot.user_id == id)
                .map(|bot| bot.username.clone())
                .unwrap_or_default();
            let user_id = id.to_owned();
            return div()
                .flex()
                .flex_col()
                .gap(theme.space(3.))
                .child(
                    div()
                        .type_style(style::UI, &theme)
                        .text_color(theme.colors.foreground)
                        .child(t!("integrations-bot-created", username = username)),
                )
                .child(
                    div().flex().child(
                        Button::new("integrations.bot-token")
                            .variant(ButtonVariant::Outline)
                            .label(t!("integrations-add-token"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.make_token(user_id.clone(), cx)
                            })),
                    ),
                )
                .into_any_element();
        }
        let (note, secret) = match self.kind {
            Kind::Bots => (SharedString::default(), String::new()),
            Kind::Webhooks => (t!("integrations-hook-created"), self.hook_url(id)),
            Kind::Commands => (
                t!("integrations-command-created"),
                loaded
                    .commands
                    .iter()
                    .find(|command| command.id == id)
                    .map(|command| command.token.clone())
                    .unwrap_or_default(),
            ),
            Kind::Outgoing => (
                t!("integrations-outgoing-created"),
                loaded
                    .outgoing
                    .iter()
                    .find(|hook| hook.id == id)
                    .map(|hook| hook.token.clone())
                    .unwrap_or_default(),
            ),
        };
        self.secret_view(note, secret, cx)
    }

    fn secret_view(
        &self,
        note: SharedString,
        secret: String,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let theme = UiTheme::read(cx).clone();
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

    fn title(&self) -> (SharedString, SharedString) {
        let editing = self.editing.is_some();
        match (&self.mode, self.kind) {
            (Mode::List, _) => (
                t!("integrations-title"),
                t!("integrations-description-text"),
            ),
            (Mode::Form | Mode::Saving, Kind::Webhooks) => (
                if editing {
                    t!("integrations-edit-hook")
                } else {
                    t!("integrations-add-hook")
                },
                t!("integrations-add-hook-text"),
            ),
            (Mode::Form | Mode::Saving, Kind::Outgoing) => (
                if editing {
                    t!("integrations-edit-outgoing")
                } else {
                    t!("integrations-add-outgoing")
                },
                t!("integrations-add-outgoing-text"),
            ),
            (Mode::Form | Mode::Saving, Kind::Commands) => (
                if editing {
                    t!("integrations-edit-command")
                } else {
                    t!("integrations-add-command")
                },
                t!("integrations-add-command-text"),
            ),
            (Mode::Created(_), Kind::Webhooks) => (
                t!("integrations-add-hook"),
                t!("integrations-hook-created-text"),
            ),
            (Mode::Created(_), Kind::Outgoing) => (
                t!("integrations-add-outgoing"),
                t!("integrations-outgoing-created-text"),
            ),
            (Mode::Created(_), Kind::Commands) => (
                t!("integrations-add-command"),
                t!("integrations-command-created-text"),
            ),
            (Mode::Form | Mode::Saving, Kind::Bots) if self.making_token => {
                (t!("integrations-add-token"), t!("integrations-token-text"))
            }
            (Mode::Form | Mode::Saving, Kind::Bots) => (
                if editing {
                    t!("integrations-edit-bot")
                } else {
                    t!("integrations-add-bot")
                },
                t!("integrations-add-bot-text"),
            ),
            (Mode::Created(_), Kind::Bots) => {
                (t!("integrations-add-bot"), t!("integrations-add-bot-text"))
            }
            (Mode::Secret(_), _) => (t!("integrations-add-token"), t!("integrations-token-text")),
        }
    }
}

impl Render for IntegrationsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = UiTheme::read(cx).clone();
        if self.focus_first {
            self.focus_first = false;
            let first = match self.kind {
                Kind::Webhooks | Kind::Outgoing => &self.name,
                Kind::Commands => &self.trigger,
                Kind::Bots => &self.username,
            };
            let focus = first.read(cx).focus_handle(cx);
            window.on_next_frame(move |window, cx| window.focus(&focus, cx));
        } else if self.refocus {
            self.refocus = false;
            let focus = self.focus.clone();
            window.on_next_frame(move |window, cx| window.focus(&focus, cx));
        }
        let (title, description) = self.title();
        let view = cx.entity().downgrade();
        let body = match &self.mode {
            Mode::List => self.list(cx),
            Mode::Saving if self.making_token => div()
                .type_style(style::UI, &theme)
                .text_color(theme.colors.muted_foreground)
                .child(t!("integrations-loading"))
                .into_any_element(),
            Mode::Form | Mode::Saving => self.form(cx),
            Mode::Created(id) => {
                let id = id.clone();
                self.created(&id, cx)
            }
            Mode::Secret(token) => {
                let token = token.clone();
                self.secret_view(t!("integrations-token-created"), token, cx)
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
                            Kind::Outgoing => t!("integrations-add-outgoing"),
                            Kind::Commands => t!("integrations-add-command"),
                            Kind::Bots => t!("integrations-add-bot"),
                        })
                        .on_click(
                            cx.listener(|this, _, window, cx| this.start_form(None, window, cx)),
                        ),
                )
                .child(div().flex_1())
                .child(
                    Button::new("integrations.close")
                        .variant(ButtonVariant::Outline)
                        .label(t!("team-members-close"))
                        .on_click(cx.listener(|this, _, window, cx| this.handle.close(window, cx))),
                ),
            Mode::Form | Mode::Saving => footer
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
            Mode::Created(_) | Mode::Secret(_) => footer.child(div().flex_1()).child(
                Button::new("integrations.done")
                    .label(t!("integrations-done"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.mode = Mode::List;
                        this.refocus = true;
                        cx.notify();
                    })),
            ),
        };
        let tabs = Kind::available(cx).into_iter().fold(
            Tabs::new("integrations.tabs")
                .aria_label(t!("integrations-title"))
                .selected(self.kind.id()),
            |tabs, kind| {
                tabs.item(Tab::new(
                    gpui_kit::ElementId::Name(format!("integrations.tabs.{}", kind.id()).into()),
                    kind.id(),
                    match kind {
                        Kind::Webhooks => t!("integrations-hooks"),
                        Kind::Outgoing => t!("integrations-outgoing"),
                        Kind::Commands => t!("integrations-commands"),
                        Kind::Bots => t!("integrations-bots"),
                    },
                ))
            },
        );
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
                column.child(tabs.on_change(move |value, _, cx| {
                    view.update(cx, |this, cx| {
                        this.kind = match value.as_ref() {
                            "commands" => Kind::Commands,
                            "bots" => Kind::Bots,
                            "outgoing" => Kind::Outgoing,
                            _ => Kind::Webhooks,
                        };
                        this.confirm_delete = None;
                        cx.notify();
                    })
                    .ok();
                }))
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

    #[test]
    fn lists_split_on_commas_and_spaces() {
        assert_eq!(
            split_list("build, deploy  ship,,"),
            vec!["build", "deploy", "ship"]
        );
        assert!(callbacks_problem(&split_list("")).is_some());
        assert!(callbacks_problem(&split_list("https://a.example https://b.example")).is_none());
        assert!(callbacks_problem(&split_list("https://a.example b.example")).is_some());
    }
}
