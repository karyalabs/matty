//! Browse channels (PLAN §3 M1 "Finding people and channels"): the team's
//! public channels in a dialog, with search and Join.

use gpui_icons::{LucideIcon, lucide};
use gpui_kit::{
    App, AppContext as _, Context, Entity, EventEmitter, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, div, prelude::FluentBuilder as _,
};

use crate::profile::UserProfile;
use crate::sidebar::Presence;
use crate::style::{self, Palette, TypeStyleExt as _, metrics, scaled};
use crate::t;
use crate::ui::{
    avatar::{Avatar, AvatarSize},
    button::{Button, ButtonSize, ButtonVariant},
    checkbox::Checkbox,
    dialog::{DialogHandle, dialog_description, dialog_title},
    field::Field,
    input::{Input, InputEvent, InputState},
    radio_group::{RadioGroup, RadioItem},
    tag::Tag,
    theme::UiTheme,
};

/// A public channel the user may browse to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectoryChannel {
    pub id: SharedString,
    pub name: SharedString,
    pub purpose: Option<SharedString>,
    pub members: usize,
    /// Already in the user's sidebar: the row opens it instead of joining.
    pub joined: bool,
    /// Archived (#965): it opens to read, and nobody joins it.
    pub archived: bool,
    /// Private: only archived ones the user was in are listed.
    pub private: bool,
}

/// What the browse dialog asks the window to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DirectoryEvent {
    /// Join the channel and open it; the dialog already shows it joined.
    Join(SharedString),
    /// Open a channel the user is already in.
    Open(SharedString),
    /// Start or open a direct message with this person.
    Message(UserProfile),
    /// Start or open the group message with these people (2 to
    /// [`GROUP_MAX_OTHERS`], the user left out).
    MessageGroup(Vec<UserProfile>),
    /// Create a channel; sync owns the server side and the window adds
    /// the row meanwhile.
    Create {
        name: SharedString,
        purpose: Option<SharedString>,
        private: bool,
    },
}

/// The browse-channels dialog body; the window wraps it in `dialog`.
pub struct BrowseView {
    channels: Vec<DirectoryChannel>,
    team_name: SharedString,
    search: Entity<InputState>,
    /// Lists the archived channels instead (#965).
    archived: bool,
    handle: DialogHandle,
    focus_search: bool,
    _subscription: gpui_kit::Subscription,
}

impl EventEmitter<DirectoryEvent> for BrowseView {}

impl BrowseView {
    pub fn new(
        team_name: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder(t!("directory-search-channels")));
        let subscription = cx.subscribe(&search, |_, _, _: &InputEvent, cx| cx.notify());
        Self {
            channels: Vec::new(),
            team_name: team_name.into(),
            search,
            archived: false,
            handle: DialogHandle::new(false),
            focus_search: false,
            _subscription: subscription,
        }
    }

    pub fn handle(&self) -> &DialogHandle {
        &self.handle
    }

    pub fn set_channels(&mut self, channels: Vec<DirectoryChannel>, cx: &mut Context<Self>) {
        self.channels = channels;
        cx.notify();
    }

    pub fn set_team_name(&mut self, name: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.team_name = name.into();
        cx.notify();
    }

    /// Opens the dialog with an empty search and the field focused.
    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search
            .update(cx, |search, cx| search.set_value("", window, cx));
        self.archived = false;
        self.handle.open(window, cx);
        self.focus_search = true;
        cx.notify();
    }

    /// Sync confirmed or reverted a join.
    pub fn set_joined(&mut self, id: &str, joined: bool, cx: &mut Context<Self>) {
        if let Some(channel) = self.channels.iter_mut().find(|c| c.id.as_ref() == id) {
            channel.joined = joined;
            cx.notify();
        }
    }

    fn join(&mut self, id: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        self.set_joined(&id, true, cx);
        self.handle.close(window, cx);
        cx.emit(DirectoryEvent::Join(id));
    }

    fn open_channel(&mut self, id: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        self.handle.close(window, cx);
        cx.emit(DirectoryEvent::Open(id));
    }

    fn row(&self, channel: &DirectoryChannel, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = UiTheme::read(cx).clone();
        let muted = theme.colors.muted_foreground;
        let id = channel.id.clone();
        let (row_id, button_id) = (id.clone(), id.clone());
        // An archived channel opens to read, as a joined one does.
        let joined = channel.joined || channel.archived;
        let members = t!("browse-members", count = channel.members);
        let icon = match (channel.archived, channel.private) {
            (true, _) => LucideIcon::Archive,
            (false, true) => LucideIcon::Lock,
            (false, false) => LucideIcon::Hash,
        };
        let button = if joined {
            Button::new(gpui_kit::ElementId::Name(
                format!("browse.{id}.view").into(),
            ))
            .variant(ButtonVariant::Outline)
            .size(ButtonSize::Sm)
            .label(t!("browse-view"))
            .w(theme.space(16.))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.open_channel(button_id.clone(), window, cx)
            }))
        } else {
            Button::new(gpui_kit::ElementId::Name(
                format!("browse.{id}.join").into(),
            ))
            .variant(ButtonVariant::Default)
            .size(ButtonSize::Sm)
            .label(t!("browse-join"))
            .w(theme.space(16.))
            .on_click(
                cx.listener(move |this, _, window, cx| this.join(button_id.clone(), window, cx)),
            )
        };
        div()
            .id(gpui_kit::ElementId::Name(format!("browse.{id}").into()))
            .role(gpui_kit::Role::Button)
            .aria_label(if joined {
                t!("browse-open-channel", channel = channel.name.to_string())
            } else {
                t!("browse-join-channel", channel = channel.name.to_string())
            })
            .px(theme.space(2.))
            .py(theme.space(1.5))
            .mx(theme.space(-2.))
            .rounded(theme.radius.md)
            .flex()
            .items_center()
            .gap(theme.space(3.))
            .cursor_pointer()
            .hover(|style| style.bg(theme.colors.accent))
            .on_click(cx.listener(move |this, _, window, cx| {
                if joined {
                    this.open_channel(row_id.clone(), window, cx)
                } else {
                    this.join(row_id.clone(), window, cx)
                }
            }))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(theme.space(0.5))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(theme.space(1.5))
                            .min_w_0()
                            .child(
                                lucide(icon)
                                    .size(scaled(&theme, metrics::ICON_SMALL))
                                    .text_color(muted),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .type_style(style::UI_STRONG, &theme)
                                    .text_color(theme.colors.foreground)
                                    .child(channel.name.clone()),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .type_style(style::META, &theme)
                                    .text_color(muted)
                                    .child(members),
                            ),
                    )
                    .children(channel.purpose.clone().map(|purpose| {
                        div()
                            .truncate()
                            .type_style(style::META, &theme)
                            .text_color(muted)
                            .child(purpose)
                    })),
            )
            .child(button)
    }
}

impl Render for BrowseView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = UiTheme::read(cx).clone();
        if self.focus_search {
            self.focus_search = false;
            let focus = self.search.read(cx).focus_handle(cx);
            window.on_next_frame(move |window, cx| window.focus(&focus, cx));
        }
        let query = self.search.read(cx).value().trim().to_lowercase();
        let archived = self.archived;
        let matching: Vec<&DirectoryChannel> = self
            .channels
            .iter()
            .filter(|channel| channel.archived == archived)
            .filter(|channel| {
                query.is_empty()
                    || channel.name.to_lowercase().contains(&query)
                    || channel
                        .purpose
                        .as_ref()
                        .is_some_and(|purpose| purpose.to_lowercase().contains(&query))
            })
            .collect();
        let rows: Vec<_> = matching
            .iter()
            .map(|channel| self.row(channel, cx).into_any_element())
            .collect();
        let listed = self
            .channels
            .iter()
            .filter(|channel| channel.archived == archived)
            .count();
        let any_archived = self.channels.iter().any(|c| c.archived);
        let view = cx.entity().downgrade();
        let filter = crate::ui::toggle_group::ToggleGroup::new("browse.filter")
            .aria_label(t!("browse-filter"))
            .segmented(true)
            .value([SharedString::from(if archived {
                "archived"
            } else {
                "public"
            })])
            .item(
                crate::ui::toggle_group::ToggleGroupItem::new("browse.filter.public", "public")
                    .child(t!("browse-filter-public")),
            )
            .item(
                crate::ui::toggle_group::ToggleGroupItem::new("browse.filter.archived", "archived")
                    .child(t!("browse-filter-archived")),
            )
            .on_change(move |next, _, cx| {
                let archived = next.first().is_some_and(|value| value == "archived");
                view.update(cx, |this, cx| {
                    this.archived = archived;
                    cx.notify();
                })
                .ok();
            });
        div()
            .flex()
            .flex_col()
            .gap(theme.space(4.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.space(1.))
                    .child(dialog_title("browse.title", cx).child(t!("browse-title")))
                    .child(
                        dialog_description("browse.description", cx).child(if archived {
                            t!(
                                "browse-description-archived",
                                count = listed,
                                team = self.team_name.to_string()
                            )
                        } else {
                            t!(
                                "browse-description",
                                count = listed,
                                team = self.team_name.to_string()
                            )
                        }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme.space(2.))
                    .child(div().flex_1().min_w_0().child(
                        Input::new(&self.search).aria_label(t!("directory-search-channels")),
                    ))
                    .when(any_archived, |row| row.child(filter)),
            )
            .child(
                div()
                    .id("browse.list")
                    .overflow_y_scroll()
                    .max_h(scaled(&theme, metrics::ROW_HEIGHT * 8.))
                    .flex()
                    .flex_col()
                    .gap(theme.space(0.5))
                    .when(rows.is_empty(), |list| {
                        list.child(
                            div()
                                .w_full()
                                .py(theme.space(6.))
                                .text_center()
                                .type_style(style::UI, &theme)
                                .text_color(theme.colors.muted_foreground)
                                .child(t!("browse-no-match", query = query.to_string())),
                        )
                    })
                    .children(rows),
            )
    }
}

/// The most people a group message holds besides the user: Mattermost
/// groups have 3 to 8 members.
pub const GROUP_MAX_OTHERS: usize = 7;

/// The "New message" dialog: pick a person to message, or tick several
/// for a group message.
pub struct NewMessageView {
    profiles: Vec<UserProfile>,
    /// The people ticked for a group message, in the order picked.
    selected: Vec<UserProfile>,
    search: Entity<InputState>,
    handle: DialogHandle,
    focus_search: bool,
    _subscription: gpui_kit::Subscription,
}

impl EventEmitter<DirectoryEvent> for NewMessageView {}

impl NewMessageView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder(t!("directory-search-people")));
        let subscription = cx.subscribe_in(
            &search,
            window,
            |this: &mut Self, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.enter(window, cx);
                } else {
                    cx.notify();
                }
            },
        );
        Self {
            profiles: Vec::new(),
            selected: Vec::new(),
            search,
            handle: DialogHandle::new(false),
            focus_search: false,
            _subscription: subscription,
        }
    }

    pub fn handle(&self) -> &DialogHandle {
        &self.handle
    }

    pub fn set_profiles(&mut self, profiles: Vec<UserProfile>, cx: &mut Context<Self>) {
        self.profiles = profiles;
        cx.notify();
    }

    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_with(Vec::new(), window, cx);
    }

    /// Opens the dialog with `people` already ticked: "Add members" on a
    /// group message, whose members are fixed, starts a bigger group.
    pub fn open_with(
        &mut self,
        people: Vec<UserProfile>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected = people;
        self.selected.truncate(GROUP_MAX_OTHERS);
        self.search
            .update(cx, |search, cx| search.set_value("", window, cx));
        self.handle.open(window, cx);
        self.focus_search = true;
        cx.notify();
    }

    /// The people the search matches, in list order.
    fn matches(&self, cx: &gpui_kit::App) -> Vec<UserProfile> {
        let query = self.search.read(cx).value().trim().to_lowercase();
        self.profiles
            .iter()
            .filter(|profile| {
                query.is_empty()
                    || profile.display_name.to_lowercase().contains(&query)
                    || profile.username.to_lowercase().contains(&query)
            })
            .cloned()
            .collect()
    }

    fn is_selected(&self, user_id: &str) -> bool {
        self.selected
            .iter()
            .any(|person| person.user_id.as_ref() == user_id)
    }

    fn is_full(&self) -> bool {
        self.selected.len() >= GROUP_MAX_OTHERS
    }

    /// Enter in the search: with nobody ticked it messages the first match,
    /// as before groups; while picking a group it ticks the first match, and
    /// on an empty search it starts the conversation.
    fn enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query_empty = self.search.read(cx).value().trim().is_empty();
        if self.selected.is_empty() {
            if let Some(first) = self.matches(cx).first().cloned() {
                self.message(first, window, cx);
            }
        } else if query_empty {
            self.go(window, cx);
        } else if let Some(first) = self
            .matches(cx)
            .into_iter()
            .find(|profile| !self.is_selected(&profile.user_id))
        {
            self.toggle(first, cx);
            self.search
                .update(cx, |search, cx| search.set_value("", window, cx));
        }
    }

    fn toggle(&mut self, profile: UserProfile, cx: &mut Context<Self>) {
        match self
            .selected
            .iter()
            .position(|person| person.user_id == profile.user_id)
        {
            Some(index) => {
                self.selected.remove(index);
            }
            None if !self.is_full() => self.selected.push(profile),
            None => {}
        }
        cx.notify();
    }

    /// A click on a person: messages them straight away, unless a group is
    /// being picked, when it ticks them.
    fn pick(&mut self, profile: UserProfile, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            self.message(profile, window, cx);
        } else {
            self.toggle(profile, cx);
        }
    }

    fn message(&mut self, profile: UserProfile, window: &mut Window, cx: &mut Context<Self>) {
        self.handle.close(window, cx);
        cx.emit(DirectoryEvent::Message(profile));
    }

    /// Opens the conversation with everyone ticked: one person is a direct
    /// message, more a group message.
    fn go(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut people = std::mem::take(&mut self.selected);
        match people.len() {
            0 => {}
            1 => {
                if let Some(person) = people.pop() {
                    self.message(person, window, cx);
                }
            }
            _ => {
                self.handle.close(window, cx);
                cx.emit(DirectoryEvent::MessageGroup(people));
            }
        }
    }

    fn row(&self, profile: &UserProfile, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = UiTheme::read(cx).clone();
        let palette = Palette::read(cx);
        let dot = match profile.presence {
            Presence::Online => palette.online,
            Presence::Away => palette.away,
            Presence::Dnd => palette.dnd,
            Presence::Offline | Presence::OutOfOffice => theme.colors.muted_foreground,
        };
        let initial: String = profile.display_name.chars().take(1).collect();
        let username = profile.username.clone();
        let chosen = profile.clone();
        let ticked = profile.clone();
        let checked = self.is_selected(&profile.user_id);
        let box_view = cx.weak_entity();
        div()
            .id(gpui_kit::ElementId::Name(
                format!("new-message.{username}").into(),
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
            .on_click(cx.listener(move |this, _, window, cx| this.pick(chosen.clone(), window, cx)))
            // Ticking someone never also messages them from the row.
            .child(
                div()
                    .id(gpui_kit::ElementId::Name(
                        format!("new-message.{username}.tick").into(),
                    ))
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(
                        Checkbox::new(gpui_kit::ElementId::Name(
                            format!("new-message.{username}.checkbox").into(),
                        ))
                        .checked(checked)
                        .disabled(!checked && self.is_full())
                        .aria_label(t!(
                            "new-message-add-to-group",
                            name = profile.display_name.to_string()
                        ))
                        .on_change(move |_, _, cx| {
                            box_view
                                .update(cx, |this, cx| this.toggle(ticked.clone(), cx))
                                .ok();
                        }),
                    ),
            )
            .child(
                div()
                    .relative()
                    .flex_shrink_0()
                    .child(crate::avatars::with_picture(
                        Avatar::new(gpui_kit::ElementId::Name(
                            format!("new-message.{username}.avatar").into(),
                        ))
                        .size(AvatarSize::Sm)
                        .fallback(initial)
                        .aria_label(profile.display_name.clone()),
                        &profile.user_id,
                        cx,
                    ))
                    .child(
                        div()
                            .absolute()
                            .bottom(gpui_kit::px(-1.))
                            .right(gpui_kit::px(-1.))
                            .size(gpui_kit::px(9.))
                            .rounded_full()
                            .bg(dot)
                            .border_2()
                            .border_color(theme.colors.popover),
                    ),
            )
            .child(
                div()
                    .min_w_0()
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
            .children(profile.position.clone().map(|position| {
                div()
                    .ml_auto()
                    .flex_shrink_0()
                    .type_style(style::META, &theme)
                    .text_color(theme.colors.muted_foreground)
                    .child(position)
            }))
    }

    /// The ticked people as removable tags above the list.
    fn picked(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = UiTheme::read(cx).clone();
        let view = cx.weak_entity();
        div()
            .flex()
            .flex_wrap()
            .gap(theme.space(1.5))
            .children(self.selected.iter().map(|person| {
                let view = view.clone();
                let removed = person.clone();
                Tag::new(
                    gpui_kit::ElementId::Name(
                        format!("new-message.picked.{}", person.username).into(),
                    ),
                    person.display_name.clone(),
                )
                .on_remove(move |_, _, cx| {
                    view.update(cx, |this, cx| this.toggle(removed.clone(), cx))
                        .ok();
                })
            }))
    }
}

impl Render for NewMessageView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = UiTheme::read(cx).clone();
        if self.focus_search {
            self.focus_search = false;
            let focus = self.search.read(cx).focus_handle(cx);
            window.on_next_frame(move |window, cx| window.focus(&focus, cx));
        }
        let query = self.search.read(cx).value().trim().to_lowercase();
        let rows: Vec<_> = self
            .matches(cx)
            .iter()
            .map(|profile| self.row(profile, cx).into_any_element())
            .collect();
        let count = self.selected.len();
        let note = if self.is_full() {
            t!("new-message-group-full")
        } else if count == 0 {
            t!("new-message-group-hint")
        } else {
            t!("new-message-group-room", count = GROUP_MAX_OTHERS - count)
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
                    .child(dialog_title("new-message.title", cx).child(t!("new-message-title")))
                    .child(
                        dialog_description("new-message.description", cx)
                            .child(t!("new-message-description")),
                    ),
            )
            .when(count > 0, |this| this.child(self.picked(cx)))
            .child(Input::new(&self.search).aria_label(t!("directory-search-people")))
            .child(
                div()
                    .id("new-message.list")
                    .overflow_y_scroll()
                    .max_h(scaled(&theme, metrics::ROW_HEIGHT * 8.))
                    .flex()
                    .flex_col()
                    .gap(theme.space(0.5))
                    .when(rows.is_empty(), |list| {
                        list.child(
                            div()
                                .w_full()
                                .py(theme.space(6.))
                                .text_center()
                                .type_style(style::UI, &theme)
                                .text_color(theme.colors.muted_foreground)
                                .child(t!("directory-nobody-matches", query = query.to_string())),
                        )
                    })
                    .children(rows),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme.space(2.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .type_style(style::META, &theme)
                            .text_color(theme.colors.muted_foreground)
                            .child(note),
                    )
                    .child(
                        Button::new("new-message.cancel")
                            .variant(ButtonVariant::Outline)
                            .label(t!("directory-cancel"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.handle.close(window, cx);
                            })),
                    )
                    .child(
                        Button::new("new-message.go")
                            .variant(ButtonVariant::Default)
                            .disabled(count == 0)
                            .label(t!("new-message-go", count = count))
                            .on_click(cx.listener(|this, _, window, cx| this.go(window, cx))),
                    ),
            )
    }
}

/// Mattermost's channel name rules: 2 to 64 lowercase letters, digits,
/// dashes or underscores, derived from the display name.
pub fn channel_handle(name: &str) -> String {
    let mut handle = String::new();
    for c in name.trim().chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() || c == '_' {
            handle.push(c);
        } else if (c == ' ' || c == '-') && !handle.ends_with('-') && !handle.is_empty() {
            handle.push('-');
        }
    }
    while handle.ends_with('-') {
        handle.pop();
    }
    handle.truncate(64);
    handle
}

/// The "Create channel" dialog: name, purpose, public or private.
pub struct CreateChannelView {
    name: Entity<InputState>,
    purpose: Entity<InputState>,
    private: bool,
    handle: DialogHandle,
    focus_name: bool,
    _subscriptions: Vec<gpui_kit::Subscription>,
}

impl EventEmitter<DirectoryEvent> for CreateChannelView {}

impl CreateChannelView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("e.g. design-reviews"));
        let purpose = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t!("create-channel-purpose-placeholder"))
        });
        let subscriptions = vec![
            cx.subscribe(&name, |_, _, _: &InputEvent, cx| cx.notify()),
            cx.subscribe(&purpose, |_, _, _: &InputEvent, cx| cx.notify()),
        ];
        Self {
            name,
            purpose,
            private: false,
            handle: DialogHandle::new(false),
            focus_name: false,
            _subscriptions: subscriptions,
        }
    }

    pub fn handle(&self) -> &DialogHandle {
        &self.handle
    }

    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.name
            .update(cx, |name, cx| name.set_value("", window, cx));
        self.purpose
            .update(cx, |purpose, cx| purpose.set_value("", window, cx));
        self.private = false;
        self.handle.open(window, cx);
        self.focus_name = true;
        cx.notify();
    }

    fn handle_text(&self, cx: &App) -> String {
        channel_handle(&self.name.read(cx).value())
    }

    fn create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = self.handle_text(cx);
        if handle.len() < 2 {
            return;
        }
        let name: SharedString = self.name.read(cx).value().trim().to_string().into();
        let purpose = self.purpose.read(cx).value().trim().to_string();
        let purpose = (!purpose.is_empty()).then(|| purpose.into());
        self.handle.close(window, cx);
        cx.emit(DirectoryEvent::Create {
            name,
            purpose,
            private: self.private,
        });
    }
}

impl Render for CreateChannelView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = UiTheme::read(cx).clone();
        if self.focus_name {
            self.focus_name = false;
            let focus = self.name.read(cx).focus_handle(cx);
            window.on_next_frame(move |window, cx| window.focus(&focus, cx));
        }
        let handle = self.handle_text(cx);
        let description = if handle.is_empty() {
            t!("create-channel-name-hint")
        } else {
            t!("create-channel-address", handle = handle.to_string())
        };
        let can_create = handle.len() >= 2;
        let view = cx.weak_entity();
        div()
            .flex()
            .flex_col()
            .gap(theme.space(4.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.space(1.))
                    .child(
                        dialog_title("create-channel.title", cx).child(t!("create-channel-title")),
                    )
                    .child(
                        dialog_description("create-channel.description", cx)
                            .child(t!("create-channel-description")),
                    ),
            )
            .child(
                Field::new("create-channel.name", &self.name)
                    .label(t!("create-channel-name"))
                    .required(true)
                    .description(description),
            )
            .child(
                Field::new("create-channel.purpose", &self.purpose)
                    .label(t!("create-channel-purpose"))
                    .description(t!("create-channel-purpose-hint")),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.space(2.))
                    .child(
                        div()
                            .type_style(style::UI_STRONG, &theme)
                            .text_color(theme.colors.foreground)
                            .child(t!("create-channel-visibility")),
                    )
                    .child(
                        RadioGroup::new("create-channel.visibility")
                            .aria_label(t!("create-channel-visibility"))
                            .value(if self.private { "private" } else { "public" })
                            .item(
                                RadioItem::new("create-channel.public", "public")
                                    .label(t!("create-channel-public")),
                            )
                            .item(
                                RadioItem::new("create-channel.private", "private")
                                    .label(t!("create-channel-private")),
                            )
                            .on_change(move |value, _, cx| {
                                view.update(cx, |this, cx| {
                                    this.private = value.as_ref() == "private";
                                    cx.notify();
                                })
                                .ok();
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(theme.space(2.))
                    .pt(theme.space(2.))
                    .child(
                        Button::new("create-channel.cancel")
                            .variant(ButtonVariant::Outline)
                            .label(t!("directory-cancel"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.handle.close(window, cx);
                            })),
                    )
                    .child(
                        Button::new("create-channel.create")
                            .variant(ButtonVariant::Default)
                            .disabled(!can_create)
                            .label(t!("create-channel-create"))
                            .on_click(cx.listener(|this, _, window, cx| this.create(window, cx))),
                    ),
            )
    }
}

/// What the Add members dialog decided.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AddMembersEvent {
    /// Add these people (by user id) to the channel the dialog was opened for.
    Add(Vec<UserProfile>),
    /// Invite people to the team the dialog was opened for (#962): those
    /// already on the server, and email invites.
    InviteToTeam {
        people: Vec<UserProfile>,
        emails: Vec<String>,
    },
}

/// What Invite people knows about the team it adds to (#962).
#[derive(Clone, Debug, Default)]
struct TeamInvite {
    /// The team's invite link, when the user may invite.
    link: Option<SharedString>,
    copied: bool,
    emails: Option<Entity<InputState>>,
}

/// The "Add members" dialog: a searchable, multi-select people list.
pub struct AddMembersView {
    channel: SharedString,
    profiles: Vec<UserProfile>,
    /// The people to offer are on their way from the server.
    loading: bool,
    selected: Vec<SharedString>,
    search: Entity<InputState>,
    handle: DialogHandle,
    focus_search: bool,
    /// Set while the dialog invites people to a team instead.
    team: Option<TeamInvite>,
    _subscription: gpui_kit::Subscription,
    _emails_subscription: Option<gpui_kit::Subscription>,
}

impl EventEmitter<AddMembersEvent> for AddMembersView {}

/// The addresses in a comma or space separated list that look like email
/// addresses.
fn email_list(text: &str) -> Vec<String> {
    text.split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .map(str::trim)
        .filter(|part| {
            part.split_once('@')
                .is_some_and(|(user, host)| !user.is_empty() && host.contains('.'))
        })
        .map(str::to_owned)
        .collect()
}

impl AddMembersView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder(t!("directory-search-people")));
        let subscription = cx.subscribe(&search, |_, _, _: &InputEvent, cx| cx.notify());
        Self {
            channel: SharedString::default(),
            profiles: Vec::new(),
            loading: false,
            selected: Vec::new(),
            search,
            handle: DialogHandle::new(false),
            focus_search: false,
            team: None,
            _subscription: subscription,
            _emails_subscription: None,
        }
    }

    /// Opens Invite people for `team` (#962): people on the server who
    /// are not on it, or `None` while the server finds them.
    pub fn open_for_team(
        &mut self,
        team: SharedString,
        profiles: Option<Vec<UserProfile>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let emails =
            cx.new(|cx| InputState::new(window, cx).placeholder(t!("invite-emails-placeholder")));
        self._emails_subscription =
            Some(cx.subscribe(&emails, |_, _, _: &InputEvent, cx| cx.notify()));
        self.open(team, profiles, window, cx);
        self.team = Some(TeamInvite {
            emails: Some(emails),
            ..TeamInvite::default()
        });
    }

    /// The team's invite link, once the server answered.
    pub fn set_invite_link(&mut self, link: Option<SharedString>, cx: &mut Context<Self>) {
        if let Some(team) = self.team.as_mut() {
            team.link = link;
            cx.notify();
        }
    }

    fn copy_invite_link(&mut self, cx: &mut Context<Self>) {
        if let Some(team) = self.team.as_mut()
            && let Some(link) = &team.link
        {
            cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(link.to_string()));
            team.copied = true;
            cx.notify();
        }
    }

    fn typed_emails(&self, cx: &App) -> Vec<String> {
        self.team
            .as_ref()
            .and_then(|team| team.emails.as_ref())
            .map(|emails| email_list(&emails.read(cx).value()))
            .unwrap_or_default()
    }

    /// The dialog's open state, for the window's `dialog` wrapper.
    pub fn handle(&self) -> &DialogHandle {
        &self.handle
    }

    /// Opens the dialog for `channel` with `profiles` to choose from:
    /// people not yet in it, or `None` while the server finds them.
    pub fn open(
        &mut self,
        channel: SharedString,
        profiles: Option<Vec<UserProfile>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.channel = channel;
        self.team = None;
        self.loading = profiles.is_none();
        self.profiles = profiles.unwrap_or_default();
        self.selected.clear();
        self.search
            .update(cx, |search, cx| search.set_value("", window, cx));
        self.handle.open(window, cx);
        self.focus_search = true;
        cx.notify();
    }

    /// The people the server found to add (#208).
    pub fn set_profiles(&mut self, profiles: Vec<UserProfile>, cx: &mut Context<Self>) {
        self.profiles = profiles;
        self.loading = false;
        cx.notify();
    }

    fn toggle(&mut self, user_id: SharedString, cx: &mut Context<Self>) {
        match self.selected.iter().position(|id| *id == user_id) {
            Some(index) => {
                self.selected.remove(index);
            }
            None => self.selected.push(user_id),
        }
        cx.notify();
    }

    fn add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let emails = self.typed_emails(cx);
        if self.selected.is_empty() && emails.is_empty() {
            return;
        }
        let people: Vec<UserProfile> = self
            .profiles
            .iter()
            .filter(|profile| self.selected.contains(&profile.user_id))
            .cloned()
            .collect();
        self.handle.close(window, cx);
        if self.team.is_some() {
            cx.emit(AddMembersEvent::InviteToTeam { people, emails });
        } else {
            cx.emit(AddMembersEvent::Add(people));
        }
    }

    fn row(&self, profile: &UserProfile, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = UiTheme::read(cx).clone();
        let checked = self.selected.contains(&profile.user_id);
        let initial: String = profile.display_name.chars().take(1).collect();
        let username = profile.username.clone();
        let (row_id, box_id) = (profile.user_id.clone(), profile.user_id.clone());
        let box_view = cx.weak_entity();
        div()
            .id(gpui_kit::ElementId::Name(
                format!("add-members.{username}").into(),
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
            .on_click(cx.listener(move |this, _, _, cx| this.toggle(row_id.clone(), cx)))
            .child(
                // The row toggles on a click too; a click on the box must
                // not reach it, or the box toggles twice.
                div()
                    .id(gpui_kit::ElementId::Name(
                        format!("add-members.{username}.checkbox-cell").into(),
                    ))
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(
                        Checkbox::new(gpui_kit::ElementId::Name(
                            format!("add-members.{username}.checkbox").into(),
                        ))
                        .checked(checked)
                        .aria_label(profile.display_name.clone())
                        .on_change(move |_, _, cx| {
                            box_view
                                .update(cx, |this, cx| this.toggle(box_id.clone(), cx))
                                .ok();
                        }),
                    ),
            )
            .child(crate::avatars::with_picture(
                Avatar::new(gpui_kit::ElementId::Name(
                    format!("add-members.{username}.avatar").into(),
                ))
                .size(AvatarSize::Sm)
                .fallback(initial)
                .aria_label(profile.display_name.clone()),
                &profile.user_id,
                cx,
            ))
            .child(
                div()
                    .min_w_0()
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
    }
}

impl Render for AddMembersView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = UiTheme::read(cx).clone();
        if self.focus_search {
            self.focus_search = false;
            let focus = self.search.read(cx).focus_handle(cx);
            window.on_next_frame(move |window, cx| window.focus(&focus, cx));
        }
        let query = self.search.read(cx).value().trim().to_lowercase();
        let rows: Vec<_> = self
            .profiles
            .iter()
            .filter(|profile| {
                query.is_empty()
                    || profile.display_name.to_lowercase().contains(&query)
                    || profile.username.to_lowercase().contains(&query)
            })
            .map(|profile| self.row(profile, cx).into_any_element())
            .collect();
        let emails = self.typed_emails(cx).len();
        let count = self.selected.len();
        let team = self.team.clone();
        let add_label = if team.is_some() {
            t!("invite-send", count = count + emails)
        } else {
            t!("add-members-add", count = count)
        };
        let (title, description) = if team.is_some() {
            (
                t!("invite-title", team = self.channel.to_string()),
                t!("invite-description", team = self.channel.to_string()),
            )
        } else {
            (
                t!("add-members-title"),
                t!(
                    "add-members-description",
                    channel = self.channel.to_string()
                ),
            )
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
                    .child(dialog_title("add-members.title", cx).child(title))
                    .child(dialog_description("add-members.description", cx).child(description)),
            )
            .child(Input::new(&self.search).aria_label(t!("directory-search-people")))
            .child(
                div()
                    .id("add-members.list")
                    .overflow_y_scroll()
                    .max_h(scaled(&theme, metrics::ROW_HEIGHT * 8.))
                    .flex()
                    .flex_col()
                    .gap(theme.space(0.5))
                    .when(rows.is_empty(), |list| {
                        list.child(
                            div()
                                .w_full()
                                .py(theme.space(6.))
                                .text_center()
                                .type_style(style::UI, &theme)
                                .text_color(theme.colors.muted_foreground)
                                .child(if self.loading {
                                    t!("add-members-loading")
                                } else if query.is_empty() && team.is_some() {
                                    t!("invite-everyone-here")
                                } else if query.is_empty() {
                                    t!("add-members-everyone-here")
                                } else {
                                    t!("directory-nobody-matches", query = query.to_string())
                                }),
                        )
                    })
                    .children(rows),
            )
            .when_some(
                team.as_ref().and_then(|team| team.emails.clone()),
                |dialog, emails| {
                    dialog.child(
                        Field::new("invite.emails", &emails)
                            .label(t!("invite-emails-label"))
                            .description(t!("invite-emails-hint")),
                    )
                },
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(theme.space(2.))
                    .when_some(
                        team.and_then(|team| team.link.map(|_| team.copied)),
                        |row, copied| {
                            row.child(
                                div().flex_1().flex().justify_start().child(
                                    Button::new("invite.copy-link")
                                        .variant(ButtonVariant::Ghost)
                                        .label(if copied {
                                            t!("invite-link-copied")
                                        } else {
                                            t!("invite-copy-link")
                                        })
                                        .on_click(
                                            cx.listener(|this, _, _, cx| this.copy_invite_link(cx)),
                                        ),
                                ),
                            )
                        },
                    )
                    .child(
                        Button::new("add-members.cancel")
                            .variant(ButtonVariant::Outline)
                            .label(t!("directory-cancel"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.handle.close(window, cx);
                            })),
                    )
                    .child(
                        Button::new("add-members.add")
                            .variant(ButtonVariant::Default)
                            .disabled(count + emails == 0)
                            .label(add_label)
                            .on_click(cx.listener(|this, _, window, cx| this.add(window, cx))),
                    ),
            )
    }
}

// The tests sit at the end so the check for hard-coded strings reads the
// whole file above them.
/// A team the user may join (#745).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JoinableTeam {
    pub id: SharedString,
    pub name: SharedString,
    pub description: Option<SharedString>,
}

/// What the Join a team dialog shows in its list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum JoinableTeams {
    /// Sync is asking the server.
    #[default]
    Loading,
    /// The open teams the user is not in, by name.
    Loaded(Vec<JoinableTeam>),
    /// The server could not say; an invite link still works.
    Unavailable,
}

/// What the Join a team dialog asks the window to do; the dialog has
/// closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JoinTeamEvent {
    /// Join an open team.
    Join(SharedString),
    /// Join the team an invite link names.
    Invite(SharedString),
    /// Create an open team and join it (#971).
    Create {
        display_name: SharedString,
        name: String,
    },
}

/// A team's URL name for `display_name`, as the web app makes it:
/// lowercase letters, digits and dashes, starting with a letter, at most
/// 64 characters.
pub fn team_handle(display_name: &str) -> String {
    let mut handle = String::new();
    for c in display_name.trim().chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_lowercase() || (c.is_ascii_digit() && !handle.is_empty()) {
            handle.push(c);
        } else if !handle.is_empty() && !handle.ends_with('-') {
            handle.push('-');
        }
    }
    handle.truncate(64);
    while handle.ends_with('-') {
        handle.pop();
    }
    handle
}

/// What an invite link the user pasted names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InviteLink {
    /// An invite for this server: its `id`.
    Invite(String),
    /// A link to another server, by host.
    OtherServer(String),
    /// Not an invite link.
    Invalid,
}

/// Reads a pasted invite link, such as
/// `https://chat.example.com/signup_user_complete/?id=…`, against the
/// shown server's address. A bare invite id is taken as it is.
pub fn invite_link(text: &str, server: &str) -> InviteLink {
    let text = text.trim();
    let bare = |id: &str| !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric());
    if bare(text) {
        return InviteLink::Invite(text.to_owned());
    }
    let Ok(link) = url::Url::parse(text) else {
        return InviteLink::Invalid;
    };
    let Some(id) = link
        .query_pairs()
        .find(|(key, _)| key == "id")
        .map(|(_, id)| id.into_owned())
        .filter(|id| bare(id))
    else {
        return InviteLink::Invalid;
    };
    let host = link.host_str().unwrap_or_default();
    let server_host = url::Url::parse(server)
        .ok()
        .and_then(|server| server.host_str().map(str::to_owned))
        .unwrap_or_default();
    if !server_host.is_empty() && !host.eq_ignore_ascii_case(&server_host) {
        return InviteLink::OtherServer(host.to_owned());
    }
    InviteLink::Invite(id)
}

/// The Join a team dialog body (#745): the server's open teams with
/// search and Join, then a field for an invite link. The window wraps it
/// in `dialog`.
pub struct JoinTeamView {
    teams: JoinableTeams,
    server_name: SharedString,
    server_url: SharedString,
    search: Entity<InputState>,
    invite: Entity<InputState>,
    /// Why the pasted link cannot join, until it changes.
    invite_error: Option<SharedString>,
    /// A new team's name (#971).
    create: Entity<InputState>,
    handle: DialogHandle,
    focus_search: bool,
    _subscriptions: Vec<gpui_kit::Subscription>,
}

impl EventEmitter<JoinTeamEvent> for JoinTeamView {}

impl JoinTeamView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(t!("teams-join-search")));
        let invite =
            cx.new(|cx| InputState::new(window, cx).placeholder(t!("teams-invite-placeholder")));
        let create =
            cx.new(|cx| InputState::new(window, cx).placeholder(t!("teams-create-placeholder")));
        let subscriptions = vec![
            cx.subscribe(&search, |_, _, _: &InputEvent, cx| cx.notify()),
            cx.subscribe_in(
                &create,
                window,
                |this, _, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        this.create_team(window, cx);
                    } else {
                        cx.notify();
                    }
                },
            ),
            cx.subscribe_in(
                &invite,
                window,
                |this, _, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        this.join_invite(window, cx);
                    } else if this.invite_error.take().is_some() {
                        cx.notify();
                    }
                },
            ),
        ];
        Self {
            teams: JoinableTeams::Loading,
            server_name: SharedString::default(),
            server_url: SharedString::default(),
            search,
            invite,
            invite_error: None,
            create,
            handle: DialogHandle::new(false),
            focus_search: false,
            _subscriptions: subscriptions,
        }
    }

    /// The dialog's open state, for the window's `dialog` wrapper.
    pub fn handle(&self) -> &DialogHandle {
        &self.handle
    }

    pub fn teams(&self) -> &JoinableTeams {
        &self.teams
    }

    pub fn set_teams(&mut self, teams: JoinableTeams, cx: &mut Context<Self>) {
        self.teams = teams;
        cx.notify();
    }

    /// Opens the dialog for the shown server, loading its teams, with both
    /// fields empty and the search focused.
    pub fn open(
        &mut self,
        server_name: impl Into<SharedString>,
        server_url: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.server_name = server_name.into();
        self.server_url = server_url.into();
        self.teams = JoinableTeams::Loading;
        self.invite_error = None;
        self.search
            .update(cx, |search, cx| search.set_value("", window, cx));
        self.invite
            .update(cx, |invite, cx| invite.set_value("", window, cx));
        self.create
            .update(cx, |create, cx| create.set_value("", window, cx));
        self.handle.open(window, cx);
        self.focus_search = true;
        cx.notify();
    }

    fn join(&mut self, id: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        self.handle.close(window, cx);
        cx.emit(JoinTeamEvent::Join(id));
    }

    /// The typed team name and its URL name, once the URL name is long
    /// enough for the server.
    fn new_team(&self, cx: &App) -> Option<(SharedString, String)> {
        let display_name = self.create.read(cx).value().trim().to_owned();
        let name = team_handle(&display_name);
        (name.len() >= 2).then(|| (display_name.into(), name))
    }

    fn create_team(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((display_name, name)) = self.new_team(cx) else {
            return;
        };
        self.handle.close(window, cx);
        cx.emit(JoinTeamEvent::Create { display_name, name });
    }

    fn join_invite(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.invite.read(cx).value().to_string();
        if text.trim().is_empty() {
            return;
        }
        match invite_link(&text, &self.server_url) {
            InviteLink::Invite(id) => {
                self.handle.close(window, cx);
                cx.emit(JoinTeamEvent::Invite(id.into()));
            }
            InviteLink::OtherServer(host) => {
                self.invite_error = Some(t!("teams-invite-other-server", host = host));
                cx.notify();
            }
            InviteLink::Invalid => {
                self.invite_error = Some(t!("teams-invite-invalid"));
                cx.notify();
            }
        }
    }

    /// "Create a team": its name, and the address the name gives it.
    fn create_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = UiTheme::read(cx).clone();
        let new_team = self.new_team(cx);
        let typed = !self.create.read(cx).value().trim().is_empty();
        let hint = match &new_team {
            Some((_, name)) => Some((
                t!(
                    "teams-create-url",
                    url = format!("{}/{name}", self.server_url.trim_end_matches('/'))
                ),
                theme.colors.muted_foreground,
            )),
            None if typed => Some((t!("teams-create-short"), theme.colors.destructive)),
            None => None,
        };
        div()
            .pt(theme.space(3.))
            .border_t_1()
            .border_color(theme.colors.border)
            .flex()
            .flex_col()
            .gap(theme.space(1.5))
            .child(
                div()
                    .type_style(style::UI_STRONG, &theme)
                    .text_color(theme.colors.foreground)
                    .child(t!("teams-create-label")),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme.space(2.))
                    .child(
                        div().flex_1().min_w_0().child(
                            Input::new(&self.create).aria_label(t!("teams-create-placeholder")),
                        ),
                    )
                    .child(
                        Button::new("join-team.create")
                            .variant(ButtonVariant::Outline)
                            .label(t!("teams-create-button"))
                            .disabled(new_team.is_none())
                            .on_click(
                                cx.listener(|this, _, window, cx| this.create_team(window, cx)),
                            ),
                    ),
            )
            .children(hint.map(|(text, color)| {
                div()
                    .type_style(style::META, &theme)
                    .text_color(color)
                    .truncate()
                    .child(text)
            }))
    }

    fn row(&self, team: &JoinableTeam, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = UiTheme::read(cx).clone();
        let muted = theme.colors.muted_foreground;
        let id = team.id.clone();
        let (row_id, button_id) = (id.clone(), id.clone());
        let icon =
            crate::teams::team_icon(&crate::teams::Team::new(id.clone(), team.name.clone()), cx);
        div()
            .id(gpui_kit::ElementId::Name(format!("join-team.{id}").into()))
            .role(gpui_kit::Role::Button)
            .aria_label(t!("teams-join-team", team = team.name.to_string()))
            .px(theme.space(2.))
            .py(theme.space(1.5))
            .mx(theme.space(-2.))
            .rounded(theme.radius.md)
            .flex()
            .items_center()
            .gap(theme.space(3.))
            .cursor_pointer()
            .hover(|style| style.bg(theme.colors.accent))
            .on_click(cx.listener(move |this, _, window, cx| this.join(row_id.clone(), window, cx)))
            .child(icon)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(theme.space(0.5))
                    .child(
                        div()
                            .truncate()
                            .type_style(style::UI_STRONG, &theme)
                            .text_color(theme.colors.foreground)
                            .child(team.name.clone()),
                    )
                    .children(team.description.clone().map(|description| {
                        div()
                            .truncate()
                            .type_style(style::META, &theme)
                            .text_color(muted)
                            .child(description)
                    })),
            )
            .child(
                Button::new(gpui_kit::ElementId::Name(
                    format!("join-team.{id}.join").into(),
                ))
                .variant(ButtonVariant::Default)
                .size(ButtonSize::Sm)
                .label(t!("teams-join-button"))
                .aria_label(t!("teams-join-team", team = team.name.to_string()))
                .w(theme.space(16.))
                .on_click(cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.join(button_id.clone(), window, cx)
                })),
            )
    }
}

impl Render for JoinTeamView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = UiTheme::read(cx).clone();
        let query = self.search.read(cx).value().trim().to_lowercase();
        let server = self.server_name.to_string();
        let note = |text: SharedString| {
            // A block, not a centered flex row: a flex item takes its text's
            // one-line width, so a long note overflowed both edges unwrapped.
            div()
                .w_full()
                .py(theme.space(4.))
                .px(theme.space(2.))
                .text_center()
                .type_style(style::UI, &theme)
                .text_color(theme.colors.muted_foreground)
                .child(text)
                .into_any_element()
        };
        let (rows, searchable): (Vec<gpui_kit::AnyElement>, bool) = match &self.teams {
            JoinableTeams::Loading => (vec![note(t!("teams-join-loading"))], false),
            JoinableTeams::Unavailable => (vec![note(t!("teams-join-unavailable"))], false),
            JoinableTeams::Loaded(teams) if teams.is_empty() => (
                vec![note(t!("teams-join-none", server = server.clone()))],
                false,
            ),
            JoinableTeams::Loaded(teams) => {
                let rows: Vec<_> = teams
                    .clone()
                    .iter()
                    .filter(|team| {
                        query.is_empty()
                            || team.name.to_lowercase().contains(&query)
                            || team
                                .description
                                .as_ref()
                                .is_some_and(|text| text.to_lowercase().contains(&query))
                    })
                    .map(|team| self.row(team, cx).into_any_element())
                    .collect();
                if rows.is_empty() {
                    (
                        vec![note(t!("teams-join-no-match", query = query.clone()))],
                        true,
                    )
                } else {
                    (rows, true)
                }
            }
        };
        // Focus goes to a field that draws: the search only shows with
        // teams to search, so with none it is the invite link. Focusing the
        // hidden search left focus outside the dialog, where neither Escape
        // nor a click outside reached it (matty#941). While the teams load,
        // the dialog keeps focus, and a field the user picked keeps it. This
        // render follows the one where the dialog focuses its popup, so the
        // field is focused here rather than on the next frame.
        if self.focus_search && !matches!(self.teams, JoinableTeams::Loading) {
            self.focus_search = false;
            let invite = self.invite.read(cx).focus_handle(cx);
            let focus = if searchable {
                self.search.read(cx).focus_handle(cx)
            } else {
                invite.clone()
            };
            if !invite.is_focused(window) {
                window.focus(&focus, cx);
            }
        }
        div()
            .flex()
            .flex_col()
            .gap(theme.space(4.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.space(1.))
                    .child(dialog_title("join-team.title", cx).child(t!("teams-join-title")))
                    .child(
                        dialog_description("join-team.description", cx)
                            .child(t!("teams-join-description", server = server)),
                    ),
            )
            .when(searchable, |body| {
                body.child(Input::new(&self.search).aria_label(t!("teams-join-search")))
            })
            .child(
                div()
                    .id("join-team.list")
                    .overflow_y_scroll()
                    .max_h(scaled(&theme, metrics::ROW_HEIGHT * 8.))
                    .flex()
                    .flex_col()
                    .gap(theme.space(0.5))
                    .children(rows),
            )
            .child(
                div()
                    .pt(theme.space(3.))
                    .border_t_1()
                    .border_color(theme.colors.border)
                    .flex()
                    .flex_col()
                    .gap(theme.space(1.5))
                    .child(
                        div()
                            .type_style(style::UI_STRONG, &theme)
                            .text_color(theme.colors.foreground)
                            .child(t!("teams-invite-label")),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(theme.space(2.))
                            .child(div().flex_1().min_w_0().child(
                                Input::new(&self.invite).aria_label(t!("teams-invite-placeholder")),
                            ))
                            .child(
                                Button::new("join-team.invite")
                                    .variant(ButtonVariant::Outline)
                                    .label(t!("teams-invite-join"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.join_invite(window, cx)
                                    })),
                            ),
                    )
                    .children(self.invite_error.clone().map(|error| {
                        div()
                            .type_style(style::META, &theme)
                            .text_color(theme.colors.destructive)
                            .child(error)
                    })),
            )
            .child(self.create_section(cx))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn email_invites_take_only_addresses() {
        assert_eq!(
            super::email_list("ada@example.com, sam@example.org;bad, @x.y lena@host"),
            ["ada@example.com", "sam@example.org"]
        );
    }

    use super::channel_handle;
    use crate::t;

    #[test]
    fn team_urls_start_with_a_letter() {
        use super::team_handle;
        assert_eq!(team_handle("Design Guild"), "design-guild");
        assert_eq!(team_handle("  2026 Offsite!  "), "offsite");
        assert_eq!(team_handle("R&D_team"), "r-d-team");
    }

    #[test]
    fn handles_follow_mattermost_rules() {
        assert_eq!(channel_handle("Design Reviews"), "design-reviews");
        assert_eq!(channel_handle("  release -- 2026  "), "release-2026");
        assert_eq!(channel_handle("ok_name!"), "ok_name");
        assert_eq!(channel_handle("-"), "");
    }

    #[test]
    fn the_add_button_counts_who_it_adds() {
        assert_eq!(t!("add-members-add", count = 0), "Add");
        assert_eq!(t!("add-members-add", count = 1), "Add 1 person");
        assert_eq!(t!("add-members-add", count = 3), "Add 3 people");
        assert_eq!(t!("browse-members", count = 1), "1 member");
    }

    #[test]
    fn invite_links_name_their_invite_on_this_server() {
        use super::{InviteLink, invite_link};
        let server = "https://chat.example.com";
        assert_eq!(
            invite_link(
                "https://chat.example.com/signup_user_complete/?id=abc123XYZ",
                server
            ),
            InviteLink::Invite("abc123XYZ".into())
        );
        assert_eq!(
            invite_link("  abc123  ", server),
            InviteLink::Invite("abc123".into()),
            "a bare invite id works too"
        );
        assert_eq!(
            invite_link(
                "https://other.example.org/signup_user_complete/?id=abc",
                server
            ),
            InviteLink::OtherServer("other.example.org".into())
        );
        assert_eq!(
            invite_link("https://chat.example.com/town-square", server),
            InviteLink::Invalid
        );
        assert_eq!(invite_link("not a link", server), InviteLink::Invalid);
    }
}
