//! User groups (#1057): the groups `@name` mentions on a licensed server,
//! for the composer's suggestions and for opening a group from its mention,
//! and the "User groups" dialog (#1063) that lists them.

use std::sync::Arc;

use gpui_icons::{LucideIcon, lucide};
use gpui_kit::{
    App, AppContext as _, Context, Entity, EventEmitter, Focusable as _, Global,
    InteractiveElement as _, IntoElement, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _,
};
use matty_api::models::Group;

use crate::style::{self, TypeStyleExt as _, metrics, scaled};
use crate::t;
use crate::ui::{
    button::{Button, ButtonVariant},
    composer::ComposerSuggestion,
    dialog::{DialogHandle, dialog_description, dialog_title},
    input::{Input, InputEvent, InputState},
    tabs::{Tab, Tabs},
    theme::UiTheme,
};

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
                div().flex().justify_end().child(
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
}
