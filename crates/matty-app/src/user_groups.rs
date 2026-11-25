//! User groups (#1057): the groups `@name` mentions on a licensed server,
//! for the composer's suggestions and for opening a group from its mention,
//! the "User groups" dialog (#1063) that lists them, and the editor that
//! makes and changes custom groups (#1068).

use std::sync::Arc;

use gpui_icons::{LucideIcon, lucide};
use gpui_kit::{
    App, AppContext as _, Context, Entity, EventEmitter, Focusable as _, Global,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _,
};
use matty_api::models::Group;

use crate::profile::UserProfile;
use crate::style::{self, TypeStyleExt as _, metrics, scaled};
use crate::t;
use crate::ui::{
    avatar::{Avatar, AvatarSize},
    button::{Button, ButtonVariant},
    checkbox::Checkbox,
    composer::ComposerSuggestion,
    dialog::{DialogHandle, dialog_description, dialog_title},
    field::Field,
    input::{Input, InputEvent, InputState},
    tabs::{Tab, Tabs},
    tag::Tag,
    theme::UiTheme,
};
use matty_sync::GroupChange;

/// The signed-in server's mentionable groups and the ids of those the user
/// is in; absent without a license.
#[derive(Clone, Default)]
pub struct UserGroups {
    groups: Arc<[Group]>,
    mine: Arc<[String]>,
}

impl Global for UserGroups {}

impl UserGroups {
    /// Keeps the live groups that have an `@name`.
    pub fn set(groups: &[Group], mine: Arc<[String]>, cx: &mut App) {
        let groups: Arc<[Group]> = groups
            .iter()
            .filter(|group| group.allow_reference && group.delete_at == 0 && !group.name.is_empty())
            .cloned()
            .collect();
        cx.set_global(Self { groups, mine });
    }

    pub fn read(cx: &App) -> &[Group] {
        cx.try_global::<Self>().map_or(&[], |groups| &groups.groups)
    }

    /// Whether the server has user groups at all: it is licensed and sync
    /// loaded them.
    pub fn available(cx: &App) -> bool {
        cx.has_global::<Self>()
    }

    fn is_mine(cx: &App, group_id: &str) -> bool {
        cx.try_global::<Self>()
            .is_some_and(|groups| groups.mine.iter().any(|id| id == group_id))
    }

    /// The group `@name` mentions, ignoring case.
    pub fn named<'a>(cx: &'a App, name: &str) -> Option<&'a Group> {
        Self::read(cx)
            .iter()
            .find(|group| group.name.eq_ignore_ascii_case(name))
    }
}

/// `@` suggestions for the groups whose handle starts with `query`
/// (lowercase), or whose name has it, as the webapp's Group Mentions.
pub(crate) fn suggestions(groups: &[Group], query: &str, limit: usize) -> Vec<ComposerSuggestion> {
    let mut found: Vec<&Group> = groups
        .iter()
        .filter(|group| {
            group.name.to_lowercase().starts_with(query)
                || group.display_name.to_lowercase().contains(query)
        })
        .collect();
    found.sort_by_key(|group| {
        (
            !group.name.to_lowercase().starts_with(query),
            group.name.clone(),
        )
    });
    found
        .into_iter()
        .take(limit)
        .map(|group| {
            ComposerSuggestion::new(format!("@{}", group.name), group.display_name.clone())
                .description(format!("@{}", group.name))
                .icon(LucideIcon::Users)
        })
        .collect()
}

/// What the dialog asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UserGroupsEvent {
    /// Show this group's people.
    Open(Group),
    /// "Create group" (#1068).
    Create,
}

/// "User groups…" (#1063): every group `@name` mentions, or only the
/// user's, with search. A click shows the group's people.
pub struct UserGroupsView {
    search: Entity<InputState>,
    mine_only: bool,
    handle: DialogHandle,
    focus_search: bool,
    _subscription: gpui_kit::Subscription,
}

impl EventEmitter<UserGroupsEvent> for UserGroupsView {}

impl UserGroupsView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(t!("user-groups-search")));
        let subscription = cx.subscribe_in(
            &search,
            window,
            |this: &mut Self, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    if let Some(first) = this.matches(cx).into_iter().next() {
                        this.choose(first, window, cx);
                    }
                } else {
                    cx.notify();
                }
            },
        );
        Self {
            search,
            mine_only: false,
            handle: DialogHandle::new(false),
            focus_search: false,
            _subscription: subscription,
        }
    }

    pub fn handle(&self) -> &DialogHandle {
        &self.handle
    }

    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search
            .update(cx, |search, cx| search.set_value("", window, cx));
        self.handle.open(window, cx);
        self.focus_search = true;
        cx.notify();
    }

    fn matches(&self, cx: &App) -> Vec<Group> {
        let query = self.search.read(cx).value().trim().to_lowercase();
        let mut found: Vec<Group> = UserGroups::read(cx)
            .iter()
            .filter(|group| !self.mine_only || UserGroups::is_mine(cx, &group.id))
            .filter(|group| {
                query.is_empty()
                    || group.name.to_lowercase().contains(&query)
                    || group.display_name.to_lowercase().contains(&query)
            })
            .cloned()
            .collect();
        found.sort_by_key(|group| group.display_name.to_lowercase());
        found
    }

    fn choose(&mut self, group: Group, window: &mut Window, cx: &mut Context<Self>) {
        self.handle.close(window, cx);
        cx.emit(UserGroupsEvent::Open(group));
    }

    fn row(&self, group: &Group, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = UiTheme::read(cx).clone();
        let chosen = group.clone();
        div()
            .id(gpui_kit::ElementId::Name(
                format!("user-groups.{}", group.name).into(),
            ))
            .role(gpui_kit::Role::Button)
            .aria_label(group.display_name.clone())
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
                cx.listener(move |this, _, window, cx| this.choose(chosen.clone(), window, cx)),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .size(scaled(&theme, 24.))
                    .rounded_full()
                    .bg(theme.colors.muted)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        lucide(LucideIcon::Users)
                            .size(scaled(&theme, 14.))
                            .text_color(theme.colors.muted_foreground),
                    ),
            )
            // The handle gives way first, so the name stays readable.
            .child(
                div()
                    .flex_shrink_0()
                    .max_w(scaled(&theme, 200.))
                    .truncate()
                    .type_style(style::UI, &theme)
                    .text_color(theme.colors.foreground)
                    .child(group.display_name.clone()),
            )
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .type_style(style::META, &theme)
                    .text_color(theme.colors.muted_foreground)
                    .child(format!("@{}", group.name)),
            )
            .child(div().flex_1())
            .child(
                div()
                    .flex_shrink_0()
                    .type_style(style::META, &theme)
                    .text_color(theme.colors.muted_foreground)
                    .child(t!("user-groups-members", count = group.member_count)),
            )
    }
}

impl Render for UserGroupsView {
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
            .map(|group| self.row(group, cx).into_any_element())
            .collect();
        let empty = match (rows.is_empty(), query.is_empty(), self.mine_only) {
            (false, ..) => None,
            (true, false, _) => Some(t!("user-groups-none-match", query = query)),
            (true, true, true) => Some(t!("user-groups-none-mine")),
            (true, true, false) => Some(t!("user-groups-none")),
        };
        let view = cx.entity().downgrade();
        div()
            .flex()
            .flex_col()
            .gap(theme.space(4.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.space(1.))
                    .child(dialog_title("user-groups.title", cx).child(t!("user-groups-title")))
                    .child(
                        dialog_description("user-groups.description", cx)
                            .child(t!("user-groups-description")),
                    ),
            )
            .child(
                Tabs::new("user-groups.tabs")
                    .aria_label(t!("user-groups-title"))
                    .selected(if self.mine_only { "mine" } else { "all" })
                    .item(Tab::new(
                        "user-groups.tabs.all",
                        "all",
                        t!("user-groups-all"),
                    ))
                    .item(Tab::new(
                        "user-groups.tabs.mine",
                        "mine",
                        t!("user-groups-mine"),
                    ))
                    .on_change(move |value, _, cx| {
                        view.update(cx, |this, cx| {
                            this.mine_only = value == "mine";
                            cx.notify();
                        })
                        .ok();
                    }),
            )
            .child(Input::new(&self.search).aria_label(t!("user-groups-search")))
            .child(
                div()
                    .id("user-groups.list")
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
                div()
                    .flex()
                    .justify_between()
                    .child(
                        Button::new("user-groups.create")
                            .variant(ButtonVariant::Outline)
                            .label(t!("user-groups-create"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.handle.close(window, cx);
                                cx.emit(UserGroupsEvent::Create);
                            })),
                    )
                    .child(
                        Button::new("user-groups.close")
                            .variant(ButtonVariant::Outline)
                            .label(t!("team-members-close"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.handle.close(window, cx);
                            })),
                    ),
            )
    }
}

/// Words `@name` can't be: they already mention the channel.
const RESERVED: [&str; 3] = ["all", "channel", "here"];
/// The server's limit on a group's name and `@name`.
const MAX_NAME: usize = 64;

/// A group's `@name` from its name: lowercase, with a dash for each run of
/// other characters, as the webapp suggests one.
pub fn handle_for(name: &str) -> String {
    let mut handle = String::new();
    for c in name.trim().chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() || c == '.' || c == '_' {
            handle.push(c);
        } else if !handle.ends_with('-') && !handle.is_empty() {
            handle.push('-');
        }
    }
    handle
        .trim_end_matches('-')
        .chars()
        .take(MAX_NAME)
        .collect()
}

/// Why `handle` can't be the `@name` of the group `group_id` (`None` for a
/// new one), or `None` when it can.
pub fn handle_problem(
    handle: &str,
    group_id: Option<&str>,
    groups: &[Group],
) -> Option<SharedString> {
    if handle.is_empty() {
        return Some(t!("group-editor-handle-empty"));
    }
    if handle.chars().count() > MAX_NAME {
        return Some(t!("group-editor-too-long", max = MAX_NAME));
    }
    let allowed = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || "._-".contains(c);
    if !handle.chars().all(allowed) {
        return Some(t!("group-editor-handle-characters"));
    }
    let taken = groups
        .iter()
        .any(|group| group.name == handle && Some(group.id.as_str()) != group_id);
    if RESERVED.contains(&handle) || taken {
        return Some(t!("group-editor-handle-taken", handle = handle));
    }
    None
}

/// What the editor asks of sync.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GroupEditorEvent {
    Save(Vec<GroupChange>),
}

/// Makes a custom group, or changes one: its name, `@name` and people, or
/// archives it (#1068).
pub struct GroupEditorView {
    /// The group being changed; `None` while making one.
    group: Option<Group>,
    /// Its people when the editor opened.
    original: Vec<String>,
    picked: Vec<UserProfile>,
    people: Vec<UserProfile>,
    name: Entity<InputState>,
    handle_input: Entity<InputState>,
    search: Entity<InputState>,
    /// The user typed an `@name`, so the name no longer suggests one.
    handle_typed: bool,
    /// The editor itself is setting the `@name`.
    setting_handle: bool,
    /// Save was pressed with something wrong, so problems show.
    tried: bool,
    confirm_archive: bool,
    handle: DialogHandle,
    focus_name: bool,
    _subscriptions: Vec<gpui_kit::Subscription>,
}

impl EventEmitter<GroupEditorEvent> for GroupEditorView {}

impl GroupEditorView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx
            .new(|cx| InputState::new(window, cx).placeholder(t!("group-editor-name-placeholder")));
        let handle_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t!("group-editor-handle-placeholder"))
        });
        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder(t!("directory-search-people")));
        let subscriptions = vec![
            cx.subscribe_in(
                &name,
                window,
                |this: &mut Self, _, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        this.save(window, cx);
                        return;
                    }
                    if !this.handle_typed {
                        let suggested = handle_for(this.name.read(cx).value().as_ref());
                        this.setting_handle = true;
                        this.handle_input
                            .update(cx, |input, cx| input.set_value(suggested, window, cx));
                        this.setting_handle = false;
                    }
                    cx.notify();
                },
            ),
            cx.subscribe_in(
                &handle_input,
                window,
                |this: &mut Self, _, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        this.save(window, cx);
                        return;
                    }
                    if !this.setting_handle && matches!(event, InputEvent::Change) {
                        this.handle_typed = true;
                    }
                    cx.notify();
                },
            ),
            cx.subscribe_in(
                &search,
                window,
                |this: &mut Self, _, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::PressEnter { .. })
                        && let Some(first) = this
                            .matches(cx)
                            .into_iter()
                            .find(|person| !this.is_picked(&person.user_id))
                    {
                        this.toggle(first, cx);
                        this.search
                            .update(cx, |search, cx| search.set_value("", window, cx));
                    }
                    cx.notify();
                },
            ),
        ];
        Self {
            group: None,
            original: Vec::new(),
            picked: Vec::new(),
            people: Vec::new(),
            name,
            handle_input,
            search,
            handle_typed: false,
            setting_handle: false,
            tried: false,
            confirm_archive: false,
            handle: DialogHandle::new(false),
            focus_name: false,
            _subscriptions: subscriptions,
        }
    }

    pub fn handle(&self) -> &DialogHandle {
        &self.handle
    }

    /// The people the picker offers, from the team's.
    pub fn set_people(&mut self, people: Vec<UserProfile>, cx: &mut Context<Self>) {
        self.people = people;
        cx.notify();
    }

    /// Opens the editor for a new group, or for `group` with its `members`.
    pub fn open(
        &mut self,
        group: Option<Group>,
        members: Vec<UserProfile>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (name, handle) = group
            .as_ref()
            .map(|group| (group.display_name.clone(), group.name.clone()))
            .unwrap_or_default();
        self.handle_typed = group.is_some();
        self.setting_handle = true;
        self.name
            .update(cx, |input, cx| input.set_value(name, window, cx));
        self.handle_input
            .update(cx, |input, cx| input.set_value(handle, window, cx));
        self.setting_handle = false;
        self.search
            .update(cx, |search, cx| search.set_value("", window, cx));
        self.original = members
            .iter()
            .map(|person| person.user_id.to_string())
            .collect();
        self.picked = members;
        self.group = group;
        self.tried = false;
        self.confirm_archive = false;
        self.handle.open(window, cx);
        self.focus_name = true;
        cx.notify();
    }

    fn matches(&self, cx: &App) -> Vec<UserProfile> {
        let query = self.search.read(cx).value().trim().to_lowercase();
        let mut found: Vec<UserProfile> = self
            .people
            .iter()
            .filter(|person| {
                query.is_empty()
                    || person.display_name.to_lowercase().contains(&query)
                    || person.username.to_lowercase().contains(&query)
            })
            .cloned()
            .collect();
        // People picked from outside the team still show, at the top.
        for person in self.picked.iter().rev() {
            if !found.iter().any(|found| found.user_id == person.user_id)
                && (query.is_empty()
                    || person.display_name.to_lowercase().contains(&query)
                    || person.username.to_lowercase().contains(&query))
            {
                found.insert(0, person.clone());
            }
        }
        found
    }

    fn is_picked(&self, user_id: &str) -> bool {
        self.picked
            .iter()
            .any(|person| person.user_id.as_ref() == user_id)
    }

    fn toggle(&mut self, person: UserProfile, cx: &mut Context<Self>) {
        match self
            .picked
            .iter()
            .position(|picked| picked.user_id == person.user_id)
        {
            Some(index) => {
                self.picked.remove(index);
            }
            None => self.picked.push(person),
        }
        cx.notify();
    }

    fn name_problem(&self, cx: &App) -> Option<SharedString> {
        let name = self.name.read(cx).value().trim().to_string();
        if name.is_empty() {
            Some(t!("group-editor-name-empty"))
        } else if name.chars().count() > MAX_NAME {
            Some(t!("group-editor-too-long", max = MAX_NAME))
        } else {
            None
        }
    }

    fn handle_problem(&self, cx: &App) -> Option<SharedString> {
        let handle = self.handle_input.read(cx).value().trim().to_string();
        let group_id = self.group.as_ref().map(|group| group.id.as_str());
        handle_problem(&handle, group_id, UserGroups::read(cx))
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let nobody = self.picked.is_empty();
        if self.name_problem(cx).is_some() || self.handle_problem(cx).is_some() || nobody {
            self.tried = true;
            cx.notify();
            return;
        }
        let display_name = self.name.read(cx).value().trim().to_string();
        let name = self.handle_input.read(cx).value().trim().to_string();
        let picked: Vec<String> = self
            .picked
            .iter()
            .map(|person| person.user_id.to_string())
            .collect();
        let changes = match &self.group {
            None => vec![GroupChange::Create {
                name,
                display_name,
                user_ids: picked,
            }],
            Some(group) => {
                let mut changes = Vec::new();
                if group.name != name || group.display_name != display_name {
                    changes.push(GroupChange::Rename {
                        group_id: group.id.clone(),
                        name,
                        display_name,
                    });
                }
                let added: Vec<String> = picked
                    .iter()
                    .filter(|id| !self.original.contains(id))
                    .cloned()
                    .collect();
                let removed: Vec<String> = self
                    .original
                    .iter()
                    .filter(|id| !picked.contains(id))
                    .cloned()
                    .collect();
                if !added.is_empty() {
                    changes.push(GroupChange::AddMembers {
                        group_id: group.id.clone(),
                        user_ids: added,
                    });
                }
                if !removed.is_empty() {
                    changes.push(GroupChange::RemoveMembers {
                        group_id: group.id.clone(),
                        user_ids: removed,
                    });
                }
                changes
            }
        };
        self.handle.close(window, cx);
        if !changes.is_empty() {
            cx.emit(GroupEditorEvent::Save(changes));
        }
    }

    fn archive(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(group) = &self.group else {
            return;
        };
        if !self.confirm_archive {
            self.confirm_archive = true;
            cx.notify();
            return;
        }
        let group_id = group.id.clone();
        self.handle.close(window, cx);
        cx.emit(GroupEditorEvent::Save(vec![GroupChange::Archive {
            group_id,
        }]));
    }

    fn person_row(&self, person: &UserProfile, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = UiTheme::read(cx).clone();
        let username = person.username.clone();
        let initial: String = person.display_name.chars().take(1).collect();
        let checked = self.is_picked(&person.user_id);
        let (clicked, ticked) = (person.clone(), person.clone());
        let view = cx.weak_entity();
        div()
            .id(gpui_kit::ElementId::Name(
                format!("group-editor.{username}").into(),
            ))
            .role(gpui_kit::Role::Button)
            .aria_label(person.display_name.clone())
            .h(scaled(&theme, metrics::ROW_HEIGHT + 4.))
            .px(theme.space(2.))
            .mx(theme.space(-2.))
            .rounded(theme.radius.md)
            .flex()
            .items_center()
            .gap(theme.space(3.))
            .cursor_pointer()
            .hover(|style| style.bg(theme.colors.accent))
            .on_click(cx.listener(move |this, _, _, cx| this.toggle(clicked.clone(), cx)))
            .child(
                div()
                    .id(gpui_kit::ElementId::Name(
                        format!("group-editor.{username}.tick").into(),
                    ))
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(
                        Checkbox::new(gpui_kit::ElementId::Name(
                            format!("group-editor.{username}.checkbox").into(),
                        ))
                        .checked(checked)
                        .aria_label(person.display_name.clone())
                        .on_change(move |_, _, cx| {
                            view.update(cx, |this, cx| this.toggle(ticked.clone(), cx))
                                .ok();
                        }),
                    ),
            )
            .child(
                div().flex_shrink_0().child(crate::avatars::with_picture(
                    Avatar::new(gpui_kit::ElementId::Name(
                        format!("group-editor.{username}.avatar").into(),
                    ))
                    .size(AvatarSize::Sm)
                    .fallback(initial)
                    .aria_label(person.display_name.clone()),
                    &person.user_id,
                    cx,
                )),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .max_w(scaled(&theme, 180.))
                    .truncate()
                    .type_style(style::UI, &theme)
                    .text_color(theme.colors.foreground)
                    .child(person.display_name.clone()),
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

impl Render for GroupEditorView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = UiTheme::read(cx).clone();
        if self.focus_name {
            self.focus_name = false;
            let focus = self.name.read(cx).focus_handle(cx);
            window.on_next_frame(move |window, cx| window.focus(&focus, cx));
        }
        let editing = self.group.is_some();
        let description = if self.confirm_archive {
            t!("group-editor-archive-hint")
        } else {
            t!("group-editor-description")
        };
        let title = match &self.group {
            Some(group) => t!("group-editor-edit-title", name = group.display_name.clone()),
            None => t!("group-editor-create-title"),
        };
        let handle_now = self.handle_input.read(cx).value().trim().to_string();
        let name_field = Field::new("group-editor.name", &self.name).label(t!("group-editor-name"));
        let name_field = match self.name_problem(cx).filter(|_| self.tried) {
            Some(problem) => name_field.error(problem),
            None => name_field,
        };
        let handle_field =
            Field::new("group-editor.handle", &self.handle_input).label(t!("group-editor-handle"));
        let handle_field = match self
            .handle_problem(cx)
            .filter(|_| self.tried || self.handle_typed)
        {
            Some(problem) if !handle_now.is_empty() || self.tried => handle_field.error(problem),
            _ => handle_field.description(t!("group-editor-handle-hint", handle = handle_now)),
        };
        let view = cx.weak_entity();
        let tags: Vec<_> = self
            .picked
            .iter()
            .map(|person| {
                let view = view.clone();
                let removed = person.clone();
                Tag::new(
                    gpui_kit::ElementId::Name(
                        format!("group-editor.picked.{}", person.username).into(),
                    ),
                    person.display_name.clone(),
                )
                .on_remove(move |_, _, cx| {
                    view.update(cx, |this, cx| this.toggle(removed.clone(), cx))
                        .ok();
                })
                .into_any_element()
            })
            .collect();
        let rows: Vec<_> = self
            .matches(cx)
            .iter()
            .map(|person| self.person_row(person, cx).into_any_element())
            .collect();
        let nobody = self.tried && self.picked.is_empty();
        div()
            .flex()
            .flex_col()
            .gap(theme.space(4.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.space(1.))
                    .child(dialog_title("group-editor.title", cx).child(title))
                    .child(dialog_description("group-editor.description", cx).child(description)),
            )
            .child(name_field)
            .child(handle_field)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.space(2.))
                    .child(
                        div()
                            .type_style(style::UI, &theme)
                            .text_color(theme.colors.foreground)
                            .child(t!("group-editor-people", count = self.picked.len())),
                    )
                    .when(!tags.is_empty(), |column| {
                        column.child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap(theme.space(1.5))
                                .children(tags),
                        )
                    })
                    .child(Input::new(&self.search).aria_label(t!("directory-search-people")))
                    .when(nobody, |column| {
                        column.child(
                            div()
                                .type_style(style::META, &theme)
                                .text_color(theme.colors.destructive)
                                .child(t!("group-editor-nobody")),
                        )
                    })
                    .child(
                        div()
                            .id("group-editor.people")
                            .overflow_y_scroll()
                            .max_h(scaled(&theme, (metrics::ROW_HEIGHT + 4.) * 5.))
                            .flex()
                            .flex_col()
                            .children(rows),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme.space(2.))
                    .when(editing, |footer| {
                        footer.child(
                            Button::new("group-editor.archive")
                                .variant(ButtonVariant::Destructive)
                                .label(if self.confirm_archive {
                                    t!("group-editor-archive-confirm")
                                } else {
                                    t!("group-editor-archive")
                                })
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.archive(window, cx)),
                                ),
                        )
                    })
                    .child(div().flex_1())
                    .child(
                        Button::new("group-editor.cancel")
                            .variant(ButtonVariant::Outline)
                            .label(t!("group-editor-cancel"))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.handle.close(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("group-editor.save")
                            .label(if editing {
                                t!("group-editor-save")
                            } else {
                                t!("group-editor-create")
                            })
                            .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(name: &str, display: &str) -> Group {
        Group {
            id: format!("{name}-id"),
            name: name.into(),
            display_name: display.into(),
            allow_reference: true,
            member_count: 4,
            ..Group::default()
        }
    }

    #[test]
    fn groups_match_by_handle_first_then_by_name() {
        let groups = [
            group("oncall", "On-call"),
            group("design-team", "Design team"),
            group("developers", "Developers"),
        ];
        let values = |query: &str| {
            suggestions(&groups, query, 8)
                .into_iter()
                .map(|s| s.value.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(values("de"), ["@design-team", "@developers"]);
        assert_eq!(values("call"), ["@oncall"]);
        assert_eq!(values("team"), ["@design-team"]);
        assert!(values("zzz").is_empty());
        assert_eq!(suggestions(&groups, "", 2).len(), 2);
    }

    #[test]
    fn a_name_suggests_its_handle() {
        assert_eq!(handle_for("Design team"), "design-team");
        assert_eq!(handle_for("  QA & Release  "), "qa-release");
        assert_eq!(handle_for("On-call (EU)"), "on-call-eu");
        assert_eq!(handle_for("ops_2.0"), "ops_2.0");
    }

    #[test]
    fn handles_must_be_free_and_plain() {
        let groups = [group("developers", "Developers")];
        assert!(handle_problem("qa", None, &groups).is_none());
        assert!(handle_problem("", None, &groups).is_some());
        assert!(handle_problem("Dev Team", None, &groups).is_some());
        assert!(handle_problem("here", None, &groups).is_some());
        assert!(handle_problem("developers", None, &groups).is_some());
        // A group keeps its own handle when renamed.
        assert!(handle_problem("developers", Some("developers-id"), &groups).is_none());
    }
}
