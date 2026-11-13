//! Settings' pages (#732, docs/specs/settings-window-and-team-switcher.md
//! §1.2–1.4). Each section is data: groups of rows, each row a title, an
//! optional description and one control. The page and the search results
//! both render from that one table, in the grammar the spec sets: a page
//! title, a META sentence, then groups (a label, an optional "Synced" tag
//! and one gpuicn `ItemGroup` card of `Item` rows).
//!
//! The account's display and advanced settings (#156, #157) are mirrored
//! into the local settings file: [`mirror`] takes what the server sent, and
//! the setters here write both the file and the server's preference.

use gpui_icons::{LucideIcon, lucide};
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Div, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _,
};
use matty_api::models::Preference;

use super::settings_view::PageSelects;
use super::{MainWindow, TEXT_SIZES};
use crate::actions::{self, Platform};
use crate::dnd;
use crate::mattermost_theme::ServerThemes;
use crate::settings::{
    ClockFormat, MessageDisplay, NameFormat, RemoteImages, SendKey, Settings, ThemeChoice,
    ThemeSource, UpdateChannel,
};
use crate::sidebar::Presence;
use crate::style::{self, TypeStyleExt as _, metrics, scaled};
use crate::t;
use crate::ui::{
    alert_dialog::{alert_dialog, alert_dialog_action, alert_dialog_cancel},
    button::{Button, ButtonSize, ButtonVariant, style_button},
    dialog::{DialogHandle, dialog_description, dialog_popup, dialog_title},
    input::{Input, InputEvent, InputState},
    item::{Item, ItemGroup, ItemSize},
    kbd::Kbd,
    menu::{Menu, MenuItem, MenuState},
    select::Select,
    sidebar::{Sidebar, SidebarItem},
    sortable::{Sortable, SortableGroup, SortableRow},
    switch::Switch,
    tag::{Tag, TagVariant},
    theme::UiTheme,
    toggle_group::{ToggleGroup, ToggleGroupItem},
};

/// The Mattermost preference categories Settings mirrors.
const DISPLAY: &str = "display_settings";
const ADVANCED: &str = "advanced_settings";

/// The cache size limits Advanced offers, in MB.
const CACHE_SIZES: [u32; 4] = [250, 500, 1000, 2000];

/// One page of Settings, picked from the section list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Section {
    /// Start-up, downloads and updates.
    #[default]
    General,
    /// Light or dark, the style, fonts, text size and motion.
    Appearance,
    /// Reading, writing and remote images.
    Messages,
    /// Desktop notifications and what counts as a mention.
    Notifications,
    /// Saved servers and the teams on this one.
    Servers,
    /// Every keyboard shortcut.
    Keyboard,
    /// Logs, the cache and resetting.
    Advanced,
}

impl Section {
    /// The sections in list order.
    pub(crate) const ALL: [Self; 7] = [
        Self::General,
        Self::Appearance,
        Self::Messages,
        Self::Notifications,
        Self::Servers,
        Self::Keyboard,
        Self::Advanced,
    ];

    pub(crate) fn label(self) -> SharedString {
        match self {
            Self::General => t!("prefs-section-general"),
            Self::Appearance => t!("prefs-section-appearance"),
            Self::Messages => t!("prefs-section-messages"),
            Self::Notifications => t!("prefs-section-notifications"),
            Self::Servers => t!("prefs-section-servers"),
            Self::Keyboard => t!("prefs-section-keyboard"),
            Self::Advanced => t!("prefs-section-advanced"),
        }
    }

    /// The META sentence under the page title.
    fn summary(self) -> SharedString {
        match self {
            Self::General if cfg!(target_family = "wasm") => t!("prefs-summary-general-web"),
            Self::General => t!("prefs-summary-general"),
            Self::Appearance => t!("prefs-summary-appearance"),
            Self::Messages => t!("prefs-summary-messages"),
            Self::Notifications => t!("prefs-summary-notifications"),
            Self::Servers => t!("prefs-summary-servers"),
            Self::Keyboard => t!("prefs-summary-keyboard"),
            Self::Advanced => t!("prefs-summary-advanced"),
        }
    }

    /// Where the section is in [`Self::ALL`].
    pub(crate) fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    fn id(self) -> &'static str {
        match self {
            Self::General => "preferences.section.general",
            Self::Appearance => "preferences.section.appearance",
            Self::Messages => "preferences.section.messages",
            Self::Notifications => "preferences.section.notifications",
            Self::Servers => "preferences.section.servers",
            Self::Keyboard => "preferences.section.keyboard",
            Self::Advanced => "preferences.section.advanced",
        }
    }

    fn icon(self) -> LucideIcon {
        match self {
            Self::General => LucideIcon::SlidersHorizontal,
            Self::Appearance => LucideIcon::Palette,
            Self::Messages => LucideIcon::MessageSquare,
            Self::Notifications => LucideIcon::Bell,
            Self::Servers => LucideIcon::Server,
            Self::Keyboard => LucideIcon::Keyboard,
            Self::Advanced => LucideIcon::Wrench,
        }
    }
}

/// The section list: a gpuicn `Sidebar` whose rows are one tab stop, Up
/// and Down move between them and Enter or a click opens one. `dimmed`
/// while a search is shown.
pub(crate) fn section_list(
    selected: Section,
    dimmed: bool,
    on_select: impl Fn(Section, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Sidebar {
    let theme = UiTheme::read(cx);
    let on_select = std::rc::Rc::new(on_select);
    let mut list = Sidebar::new("preferences.sections", t!("prefs-sections"))
        .roving_focus(true)
        .surface(true);
    for section in Section::ALL {
        let on_select = on_select.clone();
        let current = section == selected && !dimmed;
        let color = if current {
            theme.colors.primary
        } else {
            theme.colors.muted_foreground
        };
        list = list.child(
            SidebarItem::new(section.id(), section.label())
                .icon(
                    lucide(section.icon())
                        .size(scaled(theme, metrics::ICON))
                        .text_color(color),
                )
                .surface(true)
                .selected(current)
                // Selection is a neutral fill two steps from the list, as on
                // the sidebar, never the accent (DESIGN.md §2).
                .when(current, |item| {
                    item.bg(theme.colors.foreground.opacity(0.08))
                        .text_color(theme.colors.foreground)
                })
                .when(dimmed, |item| item.opacity(0.6))
                .on_activate(move |_, window, cx| on_select(section, window, cx)),
        );
    }
    list
}

/// What a group says beside its label.
#[derive(Clone, Default)]
pub(super) enum GroupTag {
    /// Nothing.
    #[default]
    None,
    /// "Synced with your Mattermost account": the server keeps these.
    Synced,
    /// A plain note, such as where a style comes from.
    Note(SharedString),
}

/// One row: a title, an optional description and its control.
pub(super) struct Row {
    pub(super) id: SharedString,
    pub(super) title: SharedString,
    pub(super) description: Option<SharedString>,
    /// An icon or avatar before the copy.
    pub(super) media: Option<AnyElement>,
    pub(super) control: Option<AnyElement>,
    /// The control goes under the copy, full width.
    pub(super) stacked: bool,
    /// A compact row, as in the shortcut table.
    pub(super) small: bool,
}

impl Row {
    pub(super) fn new(id: impl Into<SharedString>, title: SharedString) -> Self {
        Self {
            id: id.into(),
            title,
            media: None,
            description: None,
            control: None,
            stacked: false,
            small: false,
        }
    }

    pub(super) fn small(mut self) -> Self {
        self.small = true;
        self
    }

    pub(super) fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    pub(super) fn media(mut self, media: impl IntoElement) -> Self {
        self.media = Some(media.into_any_element());
        self
    }

    pub(super) fn control(mut self, control: impl IntoElement) -> Self {
        self.control = Some(control.into_any_element());
        self
    }

    pub(super) fn stacked(mut self) -> Self {
        self.stacked = true;
        self
    }

    fn matches(&self, query: &str) -> bool {
        self.title.to_lowercase().contains(query)
            || self
                .description
                .as_ref()
                .is_some_and(|d| d.to_lowercase().contains(query))
    }

    fn render(self) -> Item {
        Item::new(self.id)
            .title(self.title)
            .when_some(self.media, |item, media| item.media(media))
            .when_some(self.description, |item, d| item.description(d))
            .when_some(self.control, |item, control| item.actions(control))
            .stacked(self.stacked)
            .when(self.small, |item| item.size(ItemSize::Sm))
    }
}

/// A labelled card of rows, with anything that goes under the card.
pub(super) struct Group {
    pub(super) id: SharedString,
    pub(super) label: SharedString,
    pub(super) tag: GroupTag,
    pub(super) rows: Vec<Row>,
    /// Under the card, such as "Add server…".
    pub(super) after: Option<AnyElement>,
    /// Rows drag to reorder, reported as `(from, to)` with `to` counted
    /// after the row has left `from`.
    pub(super) reorder: Option<Reorder>,
}

type Reorder = std::rc::Rc<dyn Fn(usize, usize, &mut Window, &mut App)>;

impl Group {
    pub(super) fn new(id: impl Into<SharedString>, label: SharedString) -> Self {
        Self {
            id: id.into(),
            label,
            tag: GroupTag::None,
            rows: Vec::new(),
            after: None,
            reorder: None,
        }
    }

    pub(super) fn synced(mut self) -> Self {
        self.tag = GroupTag::Synced;
        self
    }

    pub(super) fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.tag = GroupTag::Note(note.into());
        self
    }

    pub(super) fn row(mut self, row: Row) -> Self {
        self.rows.push(row);
        self
    }

    pub(super) fn rows(mut self, rows: impl IntoIterator<Item = Row>) -> Self {
        self.rows.extend(rows);
        self
    }

    pub(super) fn after(mut self, element: impl IntoElement) -> Self {
        self.after = Some(element.into_any_element());
        self
    }

    /// Lets the rows be dragged into a new order (and moved with
    /// Alt+Shift+Up and Down), as gpuicn's `Sortable` does.
    pub(super) fn reorder(
        mut self,
        handler: impl Fn(usize, usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.reorder = Some(std::rc::Rc::new(handler));
        self
    }
}

/// A group's label row: the label, then its tag at the right.
fn group_label(label: SharedString, tag: &GroupTag, theme: &UiTheme) -> Div {
    let tag = match tag {
        GroupTag::None => None,
        GroupTag::Synced => Some(
            div()
                .flex()
                .items_center()
                .gap(theme.space(1.))
                .child(
                    lucide(LucideIcon::Cloud)
                        .size(scaled(theme, metrics::ICON_TINY))
                        .text_color(theme.colors.muted_foreground),
                )
                .child(t!("prefs-synced")),
        ),
        GroupTag::Note(note) => Some(div().child(note.clone())),
    };
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(theme.space(3.))
        .child(
            div()
                .type_style(style::GROUP_LABEL, theme)
                .text_color(theme.colors.foreground)
                .child(label),
        )
        .children(tag.map(|tag| {
            tag.type_style(style::META, theme)
                .text_color(theme.colors.muted_foreground)
                .truncate()
        }))
}

/// One group: its label row, then its card.
fn render_group(group: Group, theme: &UiTheme) -> Div {
    let card = match group.reorder {
        Some(reorder) if group.rows.len() > 1 => Some(sortable_card(
            group.id,
            group.label.clone(),
            group.rows,
            reorder,
            theme,
        )),
        _ => (!group.rows.is_empty()).then(|| {
            let mut card = ItemGroup::new(group.id);
            for row in group.rows {
                card = card.child(row.render());
            }
            card.into_any_element()
        }),
    };
    div()
        .flex()
        .flex_col()
        .gap(theme.space(2.))
        .child(group_label(group.label, &group.tag, theme))
        .children(card)
        .children(group.after.map(|after| div().flex().child(after)))
}

/// A card whose rows drag into a new order, drawn like an `ItemGroup`.
fn sortable_card(
    id: SharedString,
    label: SharedString,
    rows: Vec<Row>,
    reorder: Reorder,
    theme: &UiTheme,
) -> AnyElement {
    let border = theme.colors.border;
    let (card, radius) = (theme.colors.card, theme.radius.lg);
    let rows = rows.into_iter().enumerate().map(|(index, row)| {
        let drag_id = SharedString::from(format!("{}.drag", row.id));
        let (title, description) = (row.title.clone(), row.description.clone());
        let preview_id = SharedString::from(format!("{}.preview", row.id));
        // The row is built once per frame with its live controls; the drag
        // preview, drawn later from the same closure, gets a plain copy.
        let item = std::cell::RefCell::new(Some(row.render()));
        SortableRow::new(
            drag_id,
            title.clone(),
            move |_: &mut Window, _: &mut App| {
                let item = item.borrow_mut().take().unwrap_or_else(|| {
                    Item::new(preview_id.clone())
                        .title(title.clone())
                        .when_some(description.clone(), |item, d| item.description(d))
                        .bg(card)
                        .border_1()
                        .border_color(border)
                        .rounded(radius)
                        .shadow_md()
                });
                div()
                    .when(index > 0, |row| row.border_t_1().border_color(border))
                    .child(item)
            },
        )
    });
    div()
        .w_full()
        .border_1()
        .border_color(border)
        .rounded(theme.radius.lg)
        .bg(theme.colors.card)
        .overflow_hidden()
        .child(
            Sortable::new(id.clone(), label.clone())
                // One group, so no group header of its own: the page's
                // label row names it.
                .group(
                    SortableGroup::new(id, label)
                        .header(|_, _| div())
                        .rows(rows),
                )
                .on_reorder(move |movement, window, cx| {
                    reorder(movement.from.index, movement.to.index, window, cx)
                }),
        )
        .into_any_element()
}

/// A page: its title, one sentence, then its groups 24px apart.
/// The widest a Settings page gets: a bit more than it has in the window at
/// its default size, so that size looks as before.
const PAGE_MAX_WIDTH: f32 = 760.;

fn render_page(
    title: SharedString,
    summary: Option<SharedString>,
    groups: impl IntoIterator<Item = Div>,
    theme: &UiTheme,
) -> Div {
    let page = div()
        .w_full()
        .max_w(scaled(theme, PAGE_MAX_WIDTH))
        .flex()
        .flex_col()
        .px(theme.space(8.))
        .py(theme.space(7.))
        .child(
            div()
                .id("preferences.section-title")
                .role(gpui_kit::Role::Heading)
                .aria_level(2)
                .type_style(style::PAGE_TITLE, theme)
                .text_color(theme.colors.foreground)
                .child(title),
        )
        .children(summary.map(|summary| {
            div()
                .mt(theme.space(1.))
                .type_style(style::META, theme)
                .text_color(theme.colors.muted_foreground)
                .child(summary)
        }))
        .child(
            div()
                .mt(theme.space(6.))
                .flex()
                .flex_col()
                .gap(theme.space(6.))
                .children(groups),
        );
    // Centred in a wide window (the web page, a maximised window) rather
    // than stretched across it.
    div().w_full().flex().justify_center().child(page)
}

// Mirroring the account's settings (#156, #157).

fn find<'a>(preferences: &'a [Preference], category: &str, name: &str) -> Option<&'a str> {
    preferences
        .iter()
        .find(|p| p.category == category && p.name == name)
        .map(|p| p.value.as_str())
}

/// Takes the account's message display, clock, name format and send key
/// from `preferences` into `settings`. A preference the user never set on
/// the server leaves the local choice as it is.
pub(crate) fn mirror(preferences: &[Preference], settings: &mut Settings) {
    if let Some(value) = find(preferences, DISPLAY, "message_display") {
        settings.display = match value {
            "compact" => MessageDisplay::Compact,
            _ => MessageDisplay::Standard,
        };
    }
    if let Some(value) = find(preferences, DISPLAY, "use_military_time") {
        settings.clock = match value {
            "true" => ClockFormat::TwentyFourHour,
            _ => ClockFormat::TwelveHour,
        };
    }
    if let Some(value) = find(preferences, DISPLAY, "collapse_previews") {
        settings.collapse_previews = value == "true";
    }
    if let Some(value) = find(preferences, DISPLAY, "name_format") {
        settings.name_format = match value {
            "username" => NameFormat::Username,
            "full_name" => NameFormat::FullName,
            _ => NameFormat::NicknameFullName,
        };
    }
    let ctrl = find(preferences, ADVANCED, "send_on_ctrl_enter");
    let code = find(preferences, ADVANCED, "code_block_ctrl_enter");
    if ctrl.is_some() || code.is_some() {
        // The web app's defaults: off for every message, on for code blocks.
        settings.send_on = match (ctrl.unwrap_or("false"), code.unwrap_or("true")) {
            ("true", _) => SendKey::ModEnter,
            (_, "true") => SendKey::CodeBlocks,
            _ => SendKey::Enter,
        };
    }
}

fn preference(category: &str, name: &str, value: &str) -> Preference {
    Preference {
        category: category.into(),
        name: name.into(),
        value: value.into(),
        ..Preference::default()
    }
}

/// The server preference for a message display.
pub(crate) fn display_preference(display: MessageDisplay) -> Vec<Preference> {
    let value = match display {
        MessageDisplay::Standard => "clean",
        MessageDisplay::Compact => "compact",
    };
    vec![preference(DISPLAY, "message_display", value)]
}

/// The server preference for whether link previews start collapsed.
pub(crate) fn collapse_previews_preference(collapsed: bool) -> Vec<Preference> {
    vec![preference(
        DISPLAY,
        "collapse_previews",
        if collapsed { "true" } else { "false" },
    )]
}

/// The server preference for a clock.
pub(crate) fn clock_preference(clock: ClockFormat) -> Vec<Preference> {
    let value = match clock {
        ClockFormat::TwelveHour => "false",
        ClockFormat::TwentyFourHour => "true",
    };
    vec![preference(DISPLAY, "use_military_time", value)]
}

/// The server preference for a name format.
pub(crate) fn name_format_preference(format: NameFormat) -> Vec<Preference> {
    let value = match format {
        NameFormat::Username => "username",
        NameFormat::NicknameFullName => "nickname_full_name",
        NameFormat::FullName => "full_name",
    };
    vec![preference(DISPLAY, "name_format", value)]
}

/// The server preferences for a send key, both written as the web app
/// writes them.
pub(crate) fn send_key_preferences(send_on: SendKey) -> Vec<Preference> {
    let (ctrl, code) = match send_on {
        SendKey::Enter => ("false", "false"),
        SendKey::ModEnter => ("true", "true"),
        SendKey::CodeBlocks => ("false", "true"),
    };
    vec![
        preference(ADVANCED, "send_on_ctrl_enter", ctrl),
        preference(ADVANCED, "code_block_ctrl_enter", code),
    ]
}

impl MainWindow {
    /// Changes a setting the account keeps too: the local file now, the
    /// server's preference behind it.
    pub(super) fn set_synced(
        &mut self,
        change: impl FnOnce(&mut Settings),
        preferences: impl FnOnce(&Settings) -> Vec<Preference>,
        cx: &mut Context<Self>,
    ) {
        let before = self.settings.read(cx).get().clone();
        self.settings
            .update(cx, |store, cx| store.update(cx, change));
        let after = self.settings.read(cx).get().clone();
        if after == before {
            return;
        }
        if let Some(live) = &self.live {
            live.send(matty_sync::Command::SetPreferences {
                preferences: preferences(&after),
            });
        }
    }

    /// Takes the account's settings the server sent into the local file.
    pub(super) fn mirror_account_preferences(
        &mut self,
        preferences: &[Preference],
        cx: &mut Context<Self>,
    ) {
        self.settings
            .update(cx, |store, cx| store.update(cx, |s| mirror(preferences, s)));
    }

    /// Keeps the account's Mattermost theme for the team on screen current
    /// (#214): `preferences`, when sync sent them, hold every account
    /// preference, themes among them. Reinstalls the theme when the one to
    /// show changed and Preferences says to use it.
    pub(super) fn sync_server_theme(
        &mut self,
        preferences: Option<&[Preference]>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let signed_in = self.live.is_some();
        let changed = match &self.live {
            Some(live) => {
                let team = live.team().map(str::to_owned);
                ServerThemes::update(cx, preferences, team.as_deref())
            }
            None => ServerThemes::clear(cx),
        };
        // The theme is cached for the next start, and dropped on signing
        // out; writing settings reapplies the theme.
        if changed || !signed_in {
            let value = ServerThemes::current_value(cx).map(str::to_owned);
            if value != self.settings.read(cx).get().mattermost_theme {
                self.settings.update(cx, |store, cx| {
                    store.update(cx, |s| s.mattermost_theme = value)
                });
            }
        }
        if changed {
            crate::apply_theme(window, cx);
            cx.notify();
        }
    }

    /// Adds the keywords typed in `list`'s field, then empties it (#192).
    pub(super) fn add_keywords(
        &mut self,
        list: matty_notify::KeywordList,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = match list {
            matty_notify::KeywordList::Mention => self.keywords.clone(),
            matty_notify::KeywordList::Highlight => self.highlight_keywords.clone(),
        };
        let typed = input.read(cx).value().to_string();
        if typed.trim().is_empty() {
            return;
        }
        let Some(props) = &self.notify_props else {
            return;
        };
        let next = matty_notify::mention::add_keywords(&self.username(cx), props, list, &typed);
        input.update(cx, |input, cx| input.set_value("", window, cx));
        self.write_notify_props(next, cx);
    }

    /// Takes `keyword` out of `list` (#192).
    fn remove_keyword(
        &mut self,
        list: matty_notify::KeywordList,
        keyword: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(props) = &self.notify_props else {
            return;
        };
        let next = matty_notify::mention::remove_keyword(props, list, keyword);
        self.write_notify_props(next, cx);
    }

    /// Writes one of the user's `notify_props` to the server (#158). The
    /// server sends the user back, which updates the policy's preferences.
    pub(super) fn set_notify_prop(&mut self, key: &str, value: &str, cx: &mut Context<Self>) {
        let Some(props) = &self.notify_props else {
            return;
        };
        if props.get(key).map(String::as_str) == Some(value) {
            return;
        }
        let mut next = props.clone();
        next.insert(key.to_owned(), value.to_owned());
        self.write_notify_props(next, cx);
    }

    /// Sends `next` as the user's `notify_props` when it changes them.
    fn write_notify_props(
        &mut self,
        next: std::collections::HashMap<String, String>,
        cx: &mut Context<Self>,
    ) {
        if self.notify_props.as_ref() == Some(&next) {
            return;
        }
        if let Some(live) = &self.live {
            live.send(matty_sync::Command::SetNotifyProps {
                notify_props: next.clone(),
            });
        }
        self.notify_props = Some(next);
        cx.notify();
    }

    /// Turns collapsed reply threads on or off for the user (#157). The
    /// server's preferences come back and every count follows.
    fn set_collapsed_threads(&mut self, on: bool, cx: &mut Context<Self>) {
        let Some((choice, current)) = self.collapsed_threads else {
            return;
        };
        if current == on {
            return;
        }
        if let Some(live) = &self.live {
            live.send(matty_sync::Command::SetCollapsedThreads { on });
        }
        self.collapsed_threads = Some((choice, on));
        cx.notify();
    }

    /// Asks for a folder and saves downloads there from now on (#160).
    fn choose_download_folder(&mut self, cx: &mut Context<Self>) {
        let answer = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(t!("prefs-download-folder-choose")),
        });
        let settings = self.settings.clone();
        cx.spawn(async move |_, cx| {
            let Ok(Ok(Some(paths))) = answer.await else {
                return;
            };
            let Some(folder) = paths.into_iter().next() else {
                return;
            };
            settings.update(cx, |store, cx| {
                store.update(cx, |s| s.download_folder = Some(folder))
            });
        })
        .detach();
    }
}

impl MainWindow {
    /// The page for `section`, in Settings.
    pub(super) fn settings_page(
        &mut self,
        section: Section,
        selects: &mut PageSelects,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = UiTheme::read(cx).clone();
        let groups = self.settings_groups(section, selects, window, cx);
        let extra = self.settings_extra(section, window, cx);
        let dialogs = self.settings_dialogs(selects, window, cx);
        render_page(
            section.label(),
            Some(section.summary()),
            groups.into_iter().map(|group| render_group(group, &theme)),
            &theme,
        )
        .children(extra)
        .child(dialogs)
        .into_any_element()
    }

    /// Every row matching `query`, across the sections in order, each in a
    /// card of its own under its section's name (§1.4).
    pub(super) fn settings_search(
        &mut self,
        query: &str,
        selects: &mut PageSelects,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = UiTheme::read(cx).clone();
        let query = query.to_lowercase();
        let mut cards = Vec::new();
        for section in Section::ALL {
            let label = section.label();
            let whole = label.to_lowercase().contains(&query);
            for group in self.settings_groups(section, selects, window, cx) {
                let group_hit = group.label.to_lowercase().contains(&query);
                for row in group.rows {
                    if whole || group_hit || row.matches(&query) {
                        cards.push(render_group(
                            Group::new(row.id.clone(), label.clone()).row(row),
                            &theme,
                        ));
                    }
                }
            }
        }
        let dialogs = self.settings_dialogs(selects, window, cx);
        if cards.is_empty() {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .p(theme.space(8.))
                .child(style::empty_state(
                    LucideIcon::Search,
                    t!("settings-search-none", query = query.clone()),
                    t!("settings-search-none-detail"),
                    cx,
                ))
                .child(dialogs)
                .into_any_element();
        }
        render_page(t!("settings-search-results"), None, cards, &theme)
            .child(dialogs)
            .into_any_element()
    }

    /// The groups of `section`, as data.
    pub(super) fn settings_groups(
        &mut self,
        section: Section,
        selects: &mut PageSelects,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Group> {
        match section {
            Section::General => self.general(selects, window, cx),
            Section::Appearance => self.appearance(selects, window, cx),
            Section::Messages => self.messages(selects, window, cx),
            Section::Notifications => self.notifications(selects, window, cx),
            Section::Servers => self.servers_page(selects, window, cx),
            Section::Keyboard => keyboard(),
            Section::Advanced => self.advanced(selects, window, cx),
        }
    }

    /// What a page shows besides its groups.
    fn settings_extra(
        &mut self,
        section: Section,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        match section {
            Section::Appearance => Some(self.appearance_extra(window, cx)),
            _ => None,
        }
    }

    /// A switch for a local setting.
    fn local_switch(
        &self,
        id: &'static str,
        label: SharedString,
        checked: bool,
        write: fn(&mut Settings, bool),
    ) -> Switch {
        let store = self.settings.clone();
        Switch::new(id)
            .checked(checked)
            .aria_label(label)
            .on_change(move |&next, _, cx| {
                store.update(cx, |store, cx| store.update(cx, |s| write(s, next)))
            })
    }

    fn general(
        &mut self,
        selects: &mut PageSelects,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Group> {
        // A browser tab has no login items, tray, download folder or
        // updater: the page loads the latest Matty each time.
        if cfg!(target_family = "wasm") {
            return vec![
                Group::new("preferences.updates", t!("prefs-updates")).row(
                    Row::new(
                        "preferences.version",
                        t!("prefs-version", version = crate::VERSION),
                    )
                    .description(t!("prefs-update-web")),
                ),
            ];
        }
        let settings = self.settings.read(cx).get().clone();
        let download_folder = settings
            .download_folder
            .clone()
            .or_else(dirs::download_dir)
            .map(|path| SharedString::from(path.display().to_string()))
            .unwrap_or_else(|| t!("prefs-download-folder-default"));
        let mut startup = Group::new("preferences.startup", t!("prefs-window")).row(
            Row::new("preferences.launch-at-login", t!("prefs-launch-at-login"))
                .description(t!("prefs-launch-at-login-detail"))
                .control(self.local_switch(
                    "preferences.launch-at-login.switch",
                    t!("prefs-launch-at-login"),
                    settings.launch_at_login,
                    |s, v| s.launch_at_login = v,
                )),
        );
        // macOS keeps Matty in the dock when its window closes, so only
        // Linux and Windows have a tray to keep it in.
        if !cfg!(target_os = "macos") {
            startup = startup.row(
                Row::new("preferences.close-to-tray", t!("prefs-close-to-tray"))
                    .description(t!("prefs-close-to-tray-detail"))
                    .control(self.local_switch(
                        "preferences.close-to-tray.switch",
                        t!("prefs-close-to-tray"),
                        settings.close_to_tray,
                        |s, v| s.close_to_tray = v,
                    )),
            );
        }
        let files = Group::new("preferences.files", t!("prefs-files")).row(
            Row::new("preferences.download-folder", t!("prefs-download-folder"))
                .description(download_folder)
                .control(
                    Button::new("preferences.download-folder.change")
                        .variant(ButtonVariant::Outline)
                        .size(ButtonSize::Sm)
                        .aria_label(t!("prefs-download-folder-change-label"))
                        .label(t!("prefs-download-folder-change"))
                        .on_click(cx.listener(|this, _, _, cx| this.choose_download_folder(cx))),
                ),
        );
        let status = self.updater.read(cx).status();
        let status_text = match status {
            crate::updater::UpdateStatus::Unsupported => t!("prefs-update-status-unsupported"),
            crate::updater::UpdateStatus::NotChecked => t!("prefs-update-status-not-checked"),
            crate::updater::UpdateStatus::Checking => t!("prefs-update-status-checking"),
            crate::updater::UpdateStatus::UpToDate { minutes } => {
                t!("prefs-update-status-up-to-date", minutes = minutes)
            }
            crate::updater::UpdateStatus::Failed => t!("prefs-update-status-failed"),
            crate::updater::UpdateStatus::Ready(version) => {
                t!("prefs-update-status-ready", version = version)
            }
        };
        let channel = selects.get(
            "preferences.update-channel",
            &[
                ("stable", t!("prefs-update-stable")),
                ("beta", t!("prefs-update-beta")),
            ],
            match settings.update_channel {
                UpdateChannel::Stable => "stable",
                UpdateChannel::Beta => "beta",
            },
            |value, this, cx| {
                let channel = match value {
                    "beta" => UpdateChannel::Beta,
                    _ => UpdateChannel::Stable,
                };
                this.settings.update(cx, |store, cx| {
                    store.update(cx, |s| s.update_channel = channel)
                });
            },
            window,
            cx,
        );
        let updates = Group::new("preferences.updates", t!("prefs-updates"))
            .row(
                Row::new(
                    "preferences.check-updates",
                    t!("prefs-version", version = crate::VERSION),
                )
                .description(status_text)
                .control(
                    Button::new("preferences.check-updates.button")
                        .variant(ButtonVariant::Outline)
                        .size(ButtonSize::Sm)
                        .aria_label(t!("prefs-check-updates"))
                        .label(t!("prefs-check-updates-button"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.updater
                                .update(cx, |updater, cx| updater.check_now(window, cx))
                        })),
                ),
            )
            .row(
                Row::new("preferences.update-channel", t!("prefs-update-channel"))
                    .description(match settings.update_channel {
                        UpdateChannel::Stable => t!("prefs-update-stable-hint"),
                        UpdateChannel::Beta => t!("prefs-update-beta-hint"),
                    })
                    .control(select(
                        &channel,
                        t!("prefs-update-channel"),
                        &self.theme_of(cx),
                    )),
            );
        vec![startup, files, updates]
    }

    fn theme_of(&self, cx: &App) -> UiTheme {
        UiTheme::read(cx).clone()
    }

    fn messages(
        &mut self,
        selects: &mut PageSelects,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Group> {
        let theme = UiTheme::read(cx).clone();
        let settings = self.settings.read(cx).get().clone();
        let display = selects.get(
            "preferences.display",
            &[
                ("standard", t!("prefs-display-standard")),
                ("compact", t!("prefs-display-compact")),
            ],
            match settings.display {
                MessageDisplay::Standard => "standard",
                MessageDisplay::Compact => "compact",
            },
            |value, this, cx| {
                let display = match value {
                    "compact" => MessageDisplay::Compact,
                    _ => MessageDisplay::Standard,
                };
                this.set_synced(
                    |s| s.display = display,
                    |s| display_preference(s.display),
                    cx,
                )
            },
            window,
            cx,
        );
        let view = cx.entity().downgrade();
        let clock = choices(
            "preferences.clock",
            t!("prefs-clock"),
            match settings.clock {
                ClockFormat::TwelveHour => "12",
                ClockFormat::TwentyFourHour => "24",
            },
            &[("12", t!("prefs-clock-12")), ("24", t!("prefs-clock-24"))],
            {
                let view = view.clone();
                move |value, cx| {
                    let clock = match value {
                        "12" => ClockFormat::TwelveHour,
                        _ => ClockFormat::TwentyFourHour,
                    };
                    view.update(cx, |this, cx| {
                        this.set_synced(|s| s.clock = clock, |s| clock_preference(s.clock), cx)
                    })
                    .ok();
                }
            },
        );
        let names = selects.get(
            "preferences.names",
            &[
                ("nickname", t!("prefs-names-nickname")),
                ("full", t!("prefs-names-full")),
                ("username", t!("prefs-names-username")),
            ],
            match settings.name_format {
                NameFormat::NicknameFullName => "nickname",
                NameFormat::FullName => "full",
                NameFormat::Username => "username",
            },
            |value, this, cx| {
                let format = match value {
                    "full" => NameFormat::FullName,
                    "username" => NameFormat::Username,
                    _ => NameFormat::NicknameFullName,
                };
                this.set_synced(
                    |s| s.name_format = format,
                    |s| name_format_preference(s.name_format),
                    cx,
                )
            },
            window,
            cx,
        );
        let previews = Switch::new("preferences.collapse-previews.switch")
            .checked(settings.collapse_previews)
            .aria_label(t!("prefs-collapse-previews"))
            .on_change({
                let view = view.clone();
                move |&next, _, cx| {
                    view.update(cx, |this, cx| {
                        this.set_synced(
                            |s| s.collapse_previews = next,
                            |s| collapse_previews_preference(s.collapse_previews),
                            cx,
                        )
                    })
                    .ok();
                }
            });
        let mut reading = Group::new("preferences.reading", t!("prefs-reading"))
            .synced()
            .row(
                Row::new("preferences.display", t!("prefs-display"))
                    .description(match settings.display {
                        MessageDisplay::Standard => t!("prefs-display-standard-hint"),
                        MessageDisplay::Compact => t!("prefs-display-compact-hint"),
                    })
                    .control(select(&display, t!("prefs-display"), &theme)),
            )
            .row(Row::new("preferences.clock", t!("prefs-clock")).control(clock))
            .row(
                Row::new("preferences.names", t!("prefs-names-label"))
                    .description(match settings.name_format {
                        NameFormat::NicknameFullName => t!("prefs-names-nickname-hint"),
                        NameFormat::FullName => t!("prefs-names-full-hint"),
                        NameFormat::Username => t!("prefs-names-username-hint"),
                    })
                    .control(select(&names, t!("prefs-names-label"), &theme)),
            )
            .row(
                Row::new(
                    "preferences.collapse-previews",
                    t!("prefs-collapse-previews"),
                )
                .description(t!("prefs-collapse-previews-detail"))
                .control(previews),
            );
        if let Some((true, on)) = self.collapsed_threads {
            let view = view.clone();
            reading = reading.row(
                Row::new(
                    "preferences.collapsed-threads",
                    t!("prefs-collapsed-threads"),
                )
                .description(t!("prefs-collapsed-threads-detail"))
                .control(
                    Switch::new("preferences.collapsed-threads.switch")
                        .checked(on)
                        .aria_label(t!("prefs-collapsed-threads"))
                        .on_change(move |&next, _, cx| {
                            view.update(cx, |this, cx| this.set_collapsed_threads(next, cx))
                                .ok();
                        }),
                ),
            );
        }
        let mod_enter = match Platform::current() {
            Platform::MacOs => t!("prefs-send-cmd-enter"),
            Platform::Other => t!("prefs-send-ctrl-enter"),
        };
        let send = selects.get(
            "preferences.send",
            &[
                ("enter", t!("prefs-send-enter")),
                ("mod-enter", mod_enter.clone()),
                ("code-blocks", t!("prefs-send-code-blocks")),
            ],
            match settings.send_on {
                SendKey::Enter => "enter",
                SendKey::ModEnter => "mod-enter",
                SendKey::CodeBlocks => "code-blocks",
            },
            |value, this, cx| {
                let send_on = match value {
                    "mod-enter" => SendKey::ModEnter,
                    "code-blocks" => SendKey::CodeBlocks,
                    _ => SendKey::Enter,
                };
                this.set_synced(
                    |s| s.send_on = send_on,
                    |s| send_key_preferences(s.send_on),
                    cx,
                )
            },
            window,
            cx,
        );
        let writing = Group::new("preferences.writing", t!("prefs-writing"))
            .synced()
            .row(
                Row::new("preferences.send", t!("prefs-send"))
                    .description(match settings.send_on {
                        SendKey::Enter => t!("prefs-send-enter-hint"),
                        SendKey::ModEnter => t!("prefs-send-mod-enter-hint"),
                        SendKey::CodeBlocks => {
                            t!("prefs-send-code-blocks-hint", key = mod_enter.to_string())
                        }
                    })
                    .control(select(&send, t!("prefs-send"), &theme)),
            )
            .row(
                Row::new("preferences.format-bar", t!("prefs-format-bar"))
                    .description(t!("prefs-format-bar-detail"))
                    .control(self.local_switch(
                        "preferences.format-bar.switch",
                        t!("prefs-format-bar"),
                        settings.format_bar,
                        |s, v| s.format_bar = v,
                    )),
            )
            .row(
                Row::new("preferences.skin-tone", t!("prefs-skin-tone"))
                    .description(t!("prefs-skin-tone-detail"))
                    .control(skin_tones(settings.skin_tone, &theme)),
            );
        let images = selects.get(
            "preferences.remote-images",
            &[
                ("always", t!("prefs-remote-images-always")),
                ("ask", t!("prefs-remote-images-ask")),
            ],
            match settings.load_remote_images {
                RemoteImages::Always => "always",
                RemoteImages::Ask => "ask",
            },
            |value, this, cx| {
                let choice = match value {
                    "ask" => RemoteImages::Ask,
                    _ => RemoteImages::Always,
                };
                this.settings.update(cx, |store, cx| {
                    store.update(cx, |s| s.load_remote_images = choice)
                });
            },
            window,
            cx,
        );
        let privacy = Group::new("preferences.privacy", t!("prefs-privacy"))
            .note(t!("prefs-this-computer"))
            .row(
                Row::new("preferences.remote-images", t!("prefs-remote-images"))
                    .description(match settings.load_remote_images {
                        RemoteImages::Always => t!("prefs-remote-images-always-hint"),
                        RemoteImages::Ask => t!("prefs-remote-images-ask-hint"),
                    })
                    .control(select(&images, t!("prefs-remote-images"), &theme)),
            );
        vec![reading, writing, privacy]
    }

    /// The Notifications section, from the user's `notify_props` with
    /// Mattermost's defaults (#158).
    fn notifications(
        &mut self,
        selects: &mut PageSelects,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Group> {
        let theme = UiTheme::read(cx).clone();
        let status = Group::new("preferences.status", t!("prefs-status"))
            .synced()
            .row(self.dnd_row(cx));
        let Some(props) = self.notify_props.clone() else {
            let offline = Group::new("preferences.desktop", t!("prefs-desktop")).row(Row::new(
                "preferences.desktop.offline",
                t!("prefs-notifications-offline"),
            ));
            return vec![status, offline];
        };
        let user = matty_notify::UserPrefs::from_props(&self.username(cx), "", &props);
        let level = match user.desktop {
            matty_notify::Level::All => "all",
            matty_notify::Level::Mention => "mention",
            matty_notify::Level::None => "none",
        };
        let desktop = selects.get(
            "preferences.desktop",
            &[
                ("all", t!("prefs-desktop-all")),
                ("mention", t!("prefs-desktop-mention")),
                ("none", t!("prefs-desktop-none")),
            ],
            level,
            |value, this, cx| this.set_notify_prop("desktop", value, cx),
            window,
            cx,
        );
        let sound = selects.get(
            "preferences.notification-sound",
            &super::sounds::options(),
            &super::sounds::chosen(Some(&props)),
            |value, this, cx| this.choose_sound(value, cx),
            window,
            cx,
        );
        let view = cx.entity().downgrade();
        let on = |key: &str, default: bool| props.get(key).map_or(default, |v| v == "true");
        let switch = |id: &'static str,
                      label: SharedString,
                      checked: bool,
                      key: &'static str,
                      values: (&'static str, &'static str)| {
            let view = view.clone();
            Switch::new(id)
                .checked(checked)
                .aria_label(label)
                .on_change(move |&next, _, cx| {
                    let value = if next { values.0 } else { values.1 };
                    view.update(cx, |this, cx| this.set_notify_prop(key, value, cx))
                        .ok();
                })
        };
        let desktop_group = Group::new("preferences.desktop", t!("prefs-desktop"))
            .synced()
            .row(
                Row::new("preferences.desktop", t!("prefs-desktop-label")).control(select(
                    &desktop,
                    t!("prefs-desktop-label"),
                    &theme,
                )),
            )
            .row(
                Row::new("preferences.desktop-threads", t!("prefs-desktop-threads"))
                    .description(t!("prefs-desktop-threads-detail"))
                    .control(switch(
                        "preferences.desktop-threads.switch",
                        t!("prefs-desktop-threads"),
                        user.desktop_threads == matty_notify::Level::All,
                        "desktop_threads",
                        ("all", "mention"),
                    )),
            )
            .row(
                Row::new("preferences.desktop-sound", t!("prefs-sound"))
                    .description(t!("prefs-sound-detail"))
                    .control(
                        div()
                            .flex()
                            .items_center()
                            .gap(theme.space(3.))
                            .child(
                                div().w(scaled(&theme, metrics::FIELD_NARROW_WIDTH)).child(
                                    Select::new(&sound)
                                        .aria_label(t!("prefs-notification-sound"))
                                        .disabled(!user.desktop_sound),
                                ),
                            )
                            .child(switch(
                                "preferences.desktop-sound.switch",
                                t!("prefs-sound"),
                                user.desktop_sound,
                                "desktop_sound",
                                ("true", "false"),
                            )),
                    ),
            );
        let first_name = self.account.read(cx).account().display_name.clone();
        let first_name = first_name
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_owned();
        let first_name_title: SharedString = if first_name.is_empty() {
            t!("prefs-first-name")
        } else {
            t!("prefs-first-name-named", name = first_name)
        };
        let mentions = Group::new("preferences.mentions", t!("prefs-mentions"))
            .synced()
            .row(
                Row::new("preferences.first-name", first_name_title.clone())
                    .description(t!("prefs-first-name-detail"))
                    .control(switch(
                        "preferences.first-name.switch",
                        first_name_title,
                        on("first_name", false),
                        "first_name",
                        ("true", "false"),
                    )),
            )
            .row(
                Row::new("preferences.channel-wide", t!("prefs-channel-wide"))
                    .description(t!("prefs-channel-wide-detail"))
                    .control(switch(
                        "preferences.channel-wide.switch",
                        t!("prefs-channel-wide"),
                        on("channel", true),
                        "channel",
                        ("true", "false"),
                    )),
            )
            .row(self.keyword_row(matty_notify::KeywordList::Mention, &props, cx));
        let mut groups = vec![status, desktop_group, mentions];
        if self.highlight_keys {
            groups.push(
                Group::new("preferences.highlights", t!("prefs-highlights"))
                    .synced()
                    .row(self.keyword_row(matty_notify::KeywordList::Highlight, &props, cx)),
            );
        }
        groups
    }

    /// Do not disturb as the status menu set it, with a switch (#158). On
    /// turns it on until the user turns it off; a set time is the status
    /// menu's job, with its durations and custom picker (#190).
    fn dnd_row(&self, cx: &mut Context<Self>) -> Row {
        let account = self.account.read(cx).account();
        let dnd = account.presence == Presence::Dnd;
        let now = crate::clock::now();
        let on = dnd::is_on(dnd, account.dnd_end, &now);
        let detail = dnd::preferences_detail(dnd, account.dnd_end, &now, ClockFormat::current(cx));
        let (view, account) = (cx.entity().downgrade(), self.account.clone());
        Row::new("preferences.dnd", t!("prefs-dnd"))
            .description(detail)
            .control(
                Switch::new("preferences.dnd.switch")
                    .checked(on)
                    .aria_label(t!("prefs-dnd"))
                    .on_change(move |&next, _, cx| {
                        let presence = if next {
                            Presence::Dnd
                        } else {
                            Presence::Online
                        };
                        account.update(cx, |account, cx| account.set_presence(presence, None, cx));
                        view.update(cx, |_, cx| cx.notify()).ok();
                    }),
            )
    }

    /// A keyword list as a stacked row: its tags inline with the field
    /// that adds more (#192).
    fn keyword_row(
        &self,
        list: matty_notify::KeywordList,
        props: &std::collections::HashMap<String, String>,
        cx: &mut Context<Self>,
    ) -> Row {
        let theme = UiTheme::read(cx).clone();
        let keywords = matty_notify::mention::keywords(&self.username(cx), props, list);
        let (id, label, description, input) = match list {
            matty_notify::KeywordList::Mention => (
                "preferences.keywords",
                t!("prefs-keywords"),
                t!("prefs-keywords-detail"),
                &self.keywords,
            ),
            matty_notify::KeywordList::Highlight => (
                "preferences.highlight-keywords",
                t!("prefs-highlight-keywords"),
                t!("prefs-highlight-keywords-detail"),
                &self.highlight_keywords,
            ),
        };
        let view = cx.entity().downgrade();
        Row::new(id, label.clone())
            .description(description)
            .stacked()
            .control(keyword_editor(
                id,
                label,
                input,
                keywords,
                move |keyword, cx| {
                    view.update(cx, |this, cx| this.remove_keyword(list, keyword, cx))
                        .ok();
                },
                &theme,
            ))
    }

    fn advanced(
        &mut self,
        selects: &mut PageSelects,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Group> {
        let theme = UiTheme::read(cx).clone();
        let settings = self.settings.read(cx).get().clone();
        let level = selects.get(
            "preferences.log-detail",
            &super::LOG_LEVELS.map(|(_, value, label)| (value, SharedString::from(label))),
            super::LOG_LEVELS
                .iter()
                .find(|(level, _, _)| *level == settings.log_level)
                .map_or("info", |(_, value, _)| *value),
            |value, this, cx| {
                if let Some((level, _, _)) = super::LOG_LEVELS.iter().find(|(_, v, _)| *v == value)
                {
                    let level = *level;
                    this.settings
                        .update(cx, |store, cx| store.update(cx, |s| s.log_level = level));
                }
            },
            window,
            cx,
        );
        let logs = Group::new("preferences.logs", t!("prefs-logs"))
            .row(
                Row::new("preferences.log-detail", t!("prefs-log-detail"))
                    .description(t!("prefs-log-detail-hint"))
                    .control(select(&level, t!("prefs-log-detail"), &theme)),
            )
            .row(
                Row::new("preferences.open-logs", t!("prefs-logs"))
                    .description(t!("prefs-logs-detail"))
                    .control(
                        Button::new("preferences.open-logs.button")
                            .variant(ButtonVariant::Outline)
                            .size(ButtonSize::Sm)
                            .aria_label(t!("prefs-open-logs-label"))
                            .label(t!("prefs-open-logs"))
                            .on_click(|_, _, cx| {
                                if let Some(dir) = crate::logging::default_dir() {
                                    let _ = std::fs::create_dir_all(&dir);
                                    cx.open_with_system(&dir);
                                }
                            }),
                    ),
            );
        let sizes: Vec<(&'static str, SharedString)> = CACHE_SIZES
            .iter()
            .map(|mb| (cache_value(*mb), cache_label(*mb)))
            .collect();
        let current = CACHE_SIZES
            .iter()
            .min_by_key(|mb| mb.abs_diff(settings.cache_size_mb))
            .copied()
            .unwrap_or(500);
        let cache = selects.get(
            "preferences.cache-size",
            &sizes,
            cache_value(current),
            |value, this, cx| {
                if let Ok(mb) = value.parse::<u32>() {
                    this.settings
                        .update(cx, |store, cx| store.update(cx, |s| s.cache_size_mb = mb));
                }
            },
            window,
            cx,
        );
        let clear = selects.clear_cache.clone();
        let reset_all = selects.reset_all.clone();
        let storage = Group::new("preferences.storage", t!("prefs-storage"))
            .row(
                Row::new("preferences.cache-size", t!("prefs-cache-size"))
                    .description(t!("prefs-cache-size-detail"))
                    .control(select(&cache, t!("prefs-cache-size"), &theme)),
            )
            .row(
                Row::new("preferences.clear-cache", t!("prefs-clear-cache"))
                    .description(t!("prefs-clear-cache-detail"))
                    .control(
                        Button::new("preferences.clear-cache.button")
                            .variant(ButtonVariant::Outline)
                            .size(ButtonSize::Sm)
                            .label(t!("prefs-clear-cache-button"))
                            .on_click(move |_, window, cx| clear.open(window, cx)),
                    ),
            );
        let reset = Group::new("preferences.reset", t!("prefs-reset"))
            .row(
                Row::new("preferences.reset-appearance", t!("prefs-reset-appearance"))
                    .description(t!("prefs-reset-appearance-detail"))
                    .control(
                        Button::new("preferences.reset-appearance.button")
                            .variant(ButtonVariant::Outline)
                            .size(ButtonSize::Sm)
                            .label(t!("prefs-reset-button"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.settings
                                    .update(cx, |store, cx| store.update(cx, reset_appearance))
                            })),
                    ),
            )
            .row(
                Row::new("preferences.reset-all", t!("prefs-reset-all"))
                    .description(t!("prefs-reset-all-detail"))
                    .control(
                        Button::new("preferences.reset-all.button")
                            .variant(ButtonVariant::Outline)
                            .size(ButtonSize::Sm)
                            .label(t!("prefs-reset-all-button"))
                            .on_click(move |_, window, cx| reset_all.open(window, cx)),
                    ),
            );
        vec![logs, storage, reset]
    }

    /// The confirmations Advanced's buttons open, drawn in the Settings
    /// window.
    fn settings_dialogs(
        &mut self,
        selects: &mut PageSelects,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let clear_popup = confirm_popup(
            "preferences.clear-cache.dialog",
            t!("prefs-clear-cache-confirm-title"),
            t!("prefs-clear-cache-confirm-detail"),
            t!("prefs-clear-cache-confirm"),
            cx,
        );
        let reset_popup = confirm_popup(
            "preferences.reset-all.dialog",
            t!("prefs-reset-all-confirm-title"),
            t!("prefs-reset-all-confirm-detail"),
            t!("prefs-reset-all-confirm"),
            cx,
        );
        let view = cx.entity().downgrade();
        let clear = alert_dialog(
            "preferences.clear-cache.dialog",
            &selects.clear_cache,
            clear_popup,
            window,
            cx,
        )
        .on_ok({
            let view = view.clone();
            move |_, _, cx| {
                view.update(cx, |this, _| {
                    if let Some(live) = &this.live {
                        live.send(matty_sync::Command::ClearMedia);
                    }
                })
                .ok();
                true
            }
        });
        let reset = alert_dialog(
            "preferences.reset-all.dialog",
            &selects.reset_all,
            reset_popup,
            window,
            cx,
        )
        .on_ok(move |_, _, cx| {
            view.update(cx, |this, cx| {
                this.settings
                    .update(cx, |store, cx| store.update(cx, reset_all_local))
            })
            .ok();
            true
        });
        let server = selects
            .server_action
            .get()
            .and_then(|index| self.teams_model.read(cx).servers().get(index).cloned())
            .map(|server| server.name.to_string())
            .unwrap_or_default();
        let remove_popup = confirm_popup(
            "preferences.remove-server.dialog",
            t!("prefs-remove-title", server = server.clone()),
            t!("prefs-remove-detail"),
            t!("prefs-remove"),
            cx,
        );
        let sign_out_popup = confirm_popup(
            "preferences.sign-out.dialog",
            t!("prefs-sign-out-title", server = server),
            t!("prefs-sign-out-detail"),
            t!("prefs-sign-out"),
            cx,
        );
        let chosen = selects.server_action.clone();
        let remove = alert_dialog(
            "preferences.remove-server.dialog",
            &selects.remove_server,
            remove_popup,
            window,
            cx,
        )
        .on_ok({
            let main = cx.entity().downgrade();
            let chosen = chosen.clone();
            move |_, _, cx| {
                if let Some(index) = chosen.take() {
                    main.update(cx, |this, cx| {
                        this.in_main_window(cx, move |this, window, cx| {
                            this.remove_server(index, window, cx)
                        })
                    })
                    .ok();
                }
                true
            }
        });
        let sign_out = alert_dialog(
            "preferences.sign-out.dialog",
            &selects.sign_out,
            sign_out_popup,
            window,
            cx,
        )
        .on_ok({
            let main = cx.entity().downgrade();
            move |_, _, cx| {
                chosen.take();
                main.update(cx, |this, cx| {
                    this.in_main_window(cx, |this, window, cx| this.sign_out(window, cx))
                })
                .ok();
                true
            }
        });
        let team_name = selects
            .team_action
            .borrow()
            .as_ref()
            .map(|(_, name)| name.to_string())
            .unwrap_or_default();
        let leave_popup = confirm_popup(
            "preferences.leave-team.dialog",
            t!("teams-leave-title", team = team_name),
            t!("teams-leave-detail"),
            t!("teams-leave-confirm"),
            cx,
        );
        let leave_team = alert_dialog(
            "preferences.leave-team.dialog",
            &selects.leave_team,
            leave_popup,
            window,
            cx,
        )
        .on_ok({
            let main = cx.entity().downgrade();
            let chosen = selects.team_action.clone();
            move |_, _, cx| {
                if let Some((team, _)) = chosen.borrow_mut().take() {
                    main.update(cx, |this, cx| {
                        this.in_main_window(cx, move |this, window, cx| {
                            this.leave_team(team, window, cx)
                        })
                    })
                    .ok();
                }
                true
            }
        });
        let style_name = selects.style_target.borrow().clone().unwrap_or_default();
        let delete_popup = confirm_popup(
            "preferences.delete-style.dialog",
            t!("prefs-style-delete-title", name = style_name.clone()),
            t!("prefs-style-delete-detail"),
            t!("prefs-style-delete-confirm"),
            cx,
        );
        let store = self.settings.clone();
        let target = selects.style_target.clone();
        let delete_style = alert_dialog(
            "preferences.delete-style.dialog",
            &selects.delete_style,
            delete_popup,
            window,
            cx,
        )
        .on_ok({
            let store = store.clone();
            let target = target.clone();
            move |_, _, cx| {
                if let Some(name) = target.borrow_mut().take() {
                    super::style_library::delete(&store, &name, cx);
                }
                true
            }
        });
        if selects.rename_input.is_none() {
            let input = cx.new(|cx| InputState::new(window, cx));
            let (store, target, dialog) = (
                self.settings.clone(),
                selects.style_target.clone(),
                selects.rename_style.clone(),
            );
            // Enter renames, as the Rename button does.
            let subscription = cx.subscribe_in(
                &input,
                window,
                move |_, input: &Entity<InputState>, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        let to = input.read(cx).value().to_string();
                        if let Some(from) = target.borrow_mut().take() {
                            super::style_library::rename(&store, &from, &to, cx);
                        }
                        dialog.close(window, cx);
                    }
                },
            );
            selects.rename_input = Some((input, subscription));
        }
        let input = selects
            .rename_input
            .as_ref()
            .map(|(input, _)| input.clone())
            .expect("made above"); // i18n-ignore
        let theme = UiTheme::read(cx).clone();
        let rename_popup = dialog_popup(
            "preferences.rename-style.popup",
            t!("prefs-style-rename-title"),
            cx,
        )
        .child(
            dialog_title("preferences.rename-style.title", cx)
                .child(t!("prefs-style-rename-title")),
        )
        .child(Input::new(&input).aria_label(t!("prefs-styles-name")))
        .child(
            div()
                .flex()
                .justify_end()
                .gap(theme.space(2.))
                .pt(theme.space(2.))
                .child(
                    alert_dialog_cancel("preferences.rename-style.cancel")
                        .variant(ButtonVariant::Outline)
                        .label(t!("prefs-cancel")),
                )
                .child(
                    alert_dialog_action("preferences.rename-style.confirm")
                        .label(t!("prefs-style-rename-confirm")),
                ),
        );
        // The field starts with the name, once per opening.
        if !selects.rename_style.is_open() {
            selects.rename_for = None;
        } else if selects.rename_for.as_deref() != Some(style_name.as_str()) {
            selects.rename_for = Some(style_name.clone());
            input.update(cx, |input, cx| {
                input.set_value(style_name.clone(), window, cx)
            });
            // Once the dialog has drawn and taken focus for itself.
            let focus = gpui_kit::Focusable::focus_handle(input.read(cx), cx);
            window.on_next_frame(move |window, cx| window.focus(&focus, cx));
        }
        let rename_style = alert_dialog(
            "preferences.rename-style.dialog",
            &selects.rename_style,
            rename_popup,
            window,
            cx,
        )
        .on_ok(move |_, _, cx| {
            let to = input.read(cx).value().to_string();
            if let Some(from) = target.borrow_mut().take() {
                super::style_library::rename(&store, &from, &to, cx);
            }
            true
        });
        div()
            .child(clear)
            .child(reset)
            .child(remove)
            .child(sign_out)
            .child(leave_team)
            .child(delete_style)
            .child(rename_style)
            .into_any_element()
    }
}

/// The select value for a cache limit.
fn cache_value(mb: u32) -> &'static str {
    match mb {
        250 => "250",
        1000 => "1000",
        2000 => "2000",
        _ => "500",
    }
}

/// How a cache limit reads: "500 MB", "1 GB".
fn cache_label(mb: u32) -> SharedString {
    if mb >= 1000 {
        t!("prefs-cache-gb", size = mb / 1000)
    } else {
        t!("prefs-cache-mb", size = mb)
    }
}

/// Clears the look back to Matty's defaults, keeping saved styles.
pub(super) fn reset_appearance(settings: &mut Settings) {
    let defaults = Settings::default();
    settings.custom_style = None;
    settings.selected_style = None;
    settings.ui_font = None;
    settings.code_font = None;
    settings.text_scale = defaults.text_scale;
    settings.theme_source = defaults.theme_source;
}

/// Every local setting back to its default, keeping the accounts, saved
/// styles and window places.
pub(super) fn reset_all_local(settings: &mut Settings) {
    let kept = settings.clone();
    *settings = Settings {
        account: kept.account,
        servers: kept.servers,
        recent_servers: kept.recent_servers,
        saved_styles: kept.saved_styles,
        window: kept.window,
        settings_window: kept.settings_window,
        popouts: kept.popouts,
        mattermost_theme: kept.mattermost_theme,
        ..Settings::default()
    };
}

/// A select sized to its options, 160–220px wide.
fn select(
    state: &gpui_kit::Entity<crate::ui::select::SelectState>,
    label: SharedString,
    theme: &UiTheme,
) -> Div {
    div()
        .flex_shrink_0()
        .min_w(scaled(theme, 160.))
        .max_w(scaled(theme, 220.))
        .child(Select::new(state).aria_label(label))
}

/// The emoji pickers' skin tones as round swatches: the default yellow,
/// then Mattermost's five, lightest first (#209).
fn skin_tones(chosen: Option<usize>, theme: &UiTheme) -> gpui_kit::Stateful<Div> {
    const HANDS: [&str; 6] = ["👋", "👋🏻", "👋🏼", "👋🏽", "👋🏾", "👋🏿"];
    let mut row = div()
        .id("preferences.skin-tone.swatches")
        .role(gpui_kit::Role::RadioGroup)
        .aria_label(t!("prefs-skin-tone"))
        .flex()
        .items_center()
        .gap(theme.space(1.5));
    for (index, hand) in HANDS.iter().enumerate() {
        let tone = index.checked_sub(1);
        let selected = tone == chosen;
        row = row.child(
            Button::new(gpui_kit::ElementId::Name(
                format!("preferences.skin-tone.{index}").into(),
            ))
            .variant(ButtonVariant::Ghost)
            .size(ButtonSize::IconSm)
            .aria_label(t!("prefs-skin-tone-option", tone = index))
            .rounded_full()
            .border_2()
            .border_color(if selected {
                theme.colors.primary
            } else {
                theme.colors.border.opacity(0.)
            })
            .child(*hand)
            .on_click(move |_, _, cx| crate::emoji::save_skin_tone(tone, cx)),
        );
    }
    row
}

/// The Keyboard page: every binding, in four groups, read only.
fn keyboard() -> Vec<Group> {
    let platform = Platform::current();
    let mut navigation = Group::new("preferences.keys.navigation", t!("prefs-keys-navigation"));
    let mut messages = Group::new("preferences.keys.messages", t!("prefs-keys-messages"));
    let mut teams = Group::new("preferences.keys.teams", t!("prefs-keys-teams"));
    let mut windows = Group::new("preferences.keys.window", t!("prefs-keys-window"));
    for binding in actions::bindings() {
        let row = Row::new(binding.label, binding.name())
            .small()
            .control(Kbd::shortcut(binding.keys(platform)));
        match binding.group() {
            actions::KeyGroup::Navigation => navigation = navigation.row(row),
            actions::KeyGroup::Messages => messages = messages.row(row),
            actions::KeyGroup::TeamsAndServers => teams = teams.row(row),
            actions::KeyGroup::Window => windows = windows.row(row),
        }
    }
    let first_team = actions::format_keys("cmd-alt-1", "ctrl-alt-1");
    let first_server = actions::format_keys("cmd-1", "ctrl-1");
    teams = teams
        .row(
            Row::new("keys.teams-1-9", t!("prefs-keys-teams-by-number"))
                .small()
                .control(Kbd::new(format!("{first_team}–9"))),
        )
        .row(
            Row::new("keys.servers-1-9", t!("shortcuts-servers"))
                .small()
                .control(Kbd::new(format!("{first_server}–9"))),
        );
    for (key, label) in actions::MESSAGE_KEYS {
        messages = messages.row(
            Row::new(label, actions::message_key_name(label))
                .small()
                .control(Kbd::new(key)),
        );
    }
    vec![navigation, messages, teams, windows]
}

/// A joined toggle group of `options` (value, text) with `value` chosen,
/// sized to its content; `change` gets the value picked.
pub(super) fn choices(
    id: &'static str,
    label: SharedString,
    value: &'static str,
    options: &[(&'static str, SharedString)],
    change: impl Fn(&str, &mut App) + 'static,
) -> ToggleGroup {
    let mut group = ToggleGroup::new(id)
        .aria_label(label)
        .segmented(true)
        .value([SharedString::from(value)]);
    for (option, text) in options {
        group = group.item(
            ToggleGroupItem::new(
                gpui_kit::ElementId::Name(format!("{id}.{option}").into()),
                *option,
            )
            .aria_label(text.clone())
            .child(text.clone()),
        );
    }
    group.on_change(move |next, _, cx| {
        if let Some(value) = next.first() {
            change(value, cx);
        }
    })
}

/// A keyword list as removable tags, then the field that adds more, in
/// one wrapping line (#192). Each tag's remove button calls `on_remove`.
fn keyword_editor(
    id: &'static str,
    label: SharedString,
    input: &gpui_kit::Entity<crate::ui::input::InputState>,
    keywords: Vec<String>,
    on_remove: impl Fn(&str, &mut App) + 'static,
    theme: &UiTheme,
) -> Div {
    let on_remove = std::rc::Rc::new(on_remove);
    let tags = keywords.into_iter().map(|keyword| {
        let on_remove = on_remove.clone();
        // Keywords are unique ignoring case, so the keyword names its tag.
        let tag_id: SharedString = format!("{id}.{}", keyword.to_lowercase()).into();
        let removed = keyword.clone();
        div()
            .debug_selector({
                let tag_id = tag_id.clone();
                move || tag_id.to_string()
            })
            .child(Tag::new(tag_id, keyword).on_remove(move |_, _, cx| on_remove(&removed, cx)))
    });
    div()
        .w_full()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(theme.space(1.5))
        .children(tags)
        .child(
            div()
                .flex_1()
                .min_w(scaled(theme, 160.))
                .child(Input::new(input).aria_label(label)),
        )
}

impl MainWindow {
    fn appearance(
        &mut self,
        selects: &mut PageSelects,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Group> {
        let theme = UiTheme::read(cx).clone();
        let settings = self.settings.read(cx).get().clone();
        let store = self.settings.clone();
        let theme_value: SharedString = match settings.theme {
            ThemeChoice::System => "system",
            ThemeChoice::Light => "light",
            ThemeChoice::Dark => "dark",
        }
        .into();
        let use_imported = settings.theme_source == ThemeSource::Mattermost;
        let theme_item =
            |id: &'static str, value: &'static str, glyph: LucideIcon, text: SharedString| {
                ToggleGroupItem::new(id, value)
                    .aria_label(text.clone())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(theme.space(1.5))
                            .child(lucide(glyph).size(scaled(&theme, metrics::ICON_SMALL)))
                            .child(text),
                    )
            };
        let theme_store = store.clone();
        let mode = ToggleGroup::new("preferences.theme")
            .aria_label(t!("prefs-theme"))
            .segmented(true)
            .disabled(use_imported)
            .value([theme_value])
            .item(theme_item(
                "preferences.theme.system",
                "system",
                LucideIcon::Monitor,
                t!("prefs-theme-system"),
            ))
            .item(theme_item(
                "preferences.theme.light",
                "light",
                LucideIcon::Sun,
                t!("prefs-theme-light"),
            ))
            .item(theme_item(
                "preferences.theme.dark",
                "dark",
                LucideIcon::Moon,
                t!("prefs-theme-dark"),
            ))
            .on_change(move |next, _, cx| {
                let choice = match next.first().map(|v| v.as_ref()) {
                    Some("light") => ThemeChoice::Light,
                    Some("dark") => ThemeChoice::Dark,
                    _ => ThemeChoice::System,
                };
                theme_store.update(cx, |store, cx| store.update(cx, |s| s.theme = choice));
            });
        let light_dark = Group::new("preferences.theme", t!("prefs-mode")).row(
            Row::new("preferences.theme", t!("prefs-light-dark"))
                .description(if use_imported {
                    t!("prefs-theme-from-mattermost")
                } else {
                    t!("prefs-theme-detail")
                })
                .control(mode),
        );
        let imported = crate::mattermost_theme::shown(&settings, cx);
        let failing = imported
            .as_ref()
            .filter(|_| use_imported)
            .map(|imported| imported.warnings())
            .unwrap_or_default();
        // The gallery: 4 cards a row at the default width, 3 at the least.
        let columns = if f32::from(window.viewport_size().width) >= 840. {
            4
        } else {
            3
        };
        let mut cards = super::style_library::cards(&settings, cx);
        for card in &mut cards {
            if let super::style_library::Choice::Saved(name) = &card.choice {
                card.menu = Some(self.style_menu(selects, name, cx));
            }
        }
        let share = selects
            .share_menu
            .get_or_insert_with(|| {
                cx.new(|cx| MenuState::new(super::style_library::share_items(&store), cx))
            })
            .clone();
        let (_, shown_name) = super::style_library::shown(&settings, cx);
        let button = |id: &'static str, label: SharedString| {
            Button::new(id)
                .variant(ButtonVariant::Outline)
                .size(ButtonSize::Sm)
                .label(label)
        };
        let (new_store, paste_store, open_store) = (store.clone(), store.clone(), store.clone());
        let share_theme = theme.clone();
        let actions = div()
            .w_full()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(theme.space(2.))
            .child(
                button(
                    "preferences.style.new",
                    t!("prefs-style-new", name = shown_name.clone()),
                )
                .on_click(move |_, _, cx| super::style_library::new_style(&new_store, cx)),
            )
            .child(
                button("preferences.style.paste", t!("prefs-styles-paste"))
                    .on_click(move |_, _, cx| super::style_library::paste_style(&paste_store, cx)),
            )
            .when(!cfg!(target_family = "wasm"), |row| {
                row.child(
                    button("preferences.style.open", t!("prefs-styles-open"))
                        .on_click(move |_, _, cx| super::style_library::open_file(&open_store, cx)),
                )
            })
            .child(div().flex_1())
            .child(
                Menu::new(&share, t!("prefs-style-share"))
                    .trigger(
                        div()
                            .flex()
                            .items_center()
                            .gap(theme.space(1.))
                            .child(t!("prefs-style-share"))
                            .child(
                                lucide(LucideIcon::ChevronDown)
                                    .size(scaled(&theme, metrics::ICON_SMALL))
                                    .text_color(theme.colors.muted_foreground),
                            ),
                    )
                    .trigger_style_with(move |button| {
                        style_button(
                            button,
                            false,
                            ButtonVariant::Outline,
                            ButtonSize::Sm,
                            &share_theme,
                        )
                    }),
            );
        let gallery = div()
            .w_full()
            .flex()
            .flex_col()
            .gap(theme.space(4.))
            .child(super::style_library::gallery(&store, cards, columns, cx))
            .child(actions)
            .when(!failing.is_empty(), |gallery| {
                gallery.child(contrast_warning(&failing, &theme))
            });
        let mut style = Group::new("preferences.style", t!("prefs-style")).after(gallery);
        if use_imported {
            let name = imported
                .as_ref()
                .map(|theme| theme.name())
                .unwrap_or_else(|| shown_name.clone());
            style = style.note(t!("prefs-style-from-mattermost", name = name));
        } else {
            style = style.note(t!("prefs-this-computer"));
        }
        let editor = super::style_editor::editor(&store, cx);
        // The accent as a hex color (#855): it shows the accent, and takes
        // a typed or pasted one, such as a brand color.
        let accent_hex = super::style_editor::accent_hex(&settings, cx);
        if selects.accent_hex.is_none() {
            let input = cx.new(|cx| InputState::new(window, cx));
            let hex_store = store.clone();
            let subscription = cx.subscribe_in(
                &input,
                window,
                move |_, input: &Entity<InputState>, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        let typed = input.read(cx).value().to_string();
                        super::style_editor::set_accent_hex(&hex_store, &typed, cx);
                    }
                },
            );
            selects.accent_hex = Some((input, subscription));
        }
        let hex_input = selects.accent_hex.as_ref().map(|(input, _)| input.clone());
        if let Some(input) = &hex_input
            && selects.accent_hex_shown.as_deref() != Some(accent_hex.as_str())
        {
            // A color typed here stays as typed; one picked elsewhere shows.
            if !gpui_kit::Focusable::focus_handle(input.read(cx), cx).is_focused(window) {
                input.update(cx, |input, cx| {
                    input.set_value(accent_hex.clone(), window, cx)
                });
            }
            selects.accent_hex_shown = Some(accent_hex);
        }
        let accent = div()
            .w_full()
            .flex()
            .flex_col()
            .gap(theme.space(4.))
            .child(editor.accent)
            .when_some(hex_input, |accent, input| {
                accent.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(theme.space(3.))
                        .child(
                            div()
                                .w(scaled(&theme, 120.))
                                .child(Input::new(&input).aria_label(t!("prefs-accent-hex"))),
                        )
                        .child(
                            div()
                                .type_style(style::META, &theme)
                                .text_color(theme.colors.muted_foreground)
                                .child(t!("prefs-accent-hex-detail")),
                        ),
                )
            });
        let (ui_font, code_font, fonts_under) = self.font_controls(window, cx);
        let size_value: SharedString = TEXT_SIZES
            .iter()
            .min_by(|a, b| {
                (a.1 - settings.text_scale)
                    .abs()
                    .total_cmp(&(b.1 - settings.text_scale).abs())
            })
            .map(|(name, _)| *name)
            .unwrap_or("default")
            .into();
        let size_store = store.clone();
        let mut size_group = ToggleGroup::new("preferences.text-size")
            .aria_label(t!("prefs-text-size"))
            .segmented(true)
            .value([size_value]);
        for (name, scale) in TEXT_SIZES {
            let text = format!("{}%", (scale * 100.).round());
            size_group = size_group.item(
                ToggleGroupItem::new(
                    gpui_kit::ElementId::Name(format!("preferences.text-size.{name}").into()),
                    *name,
                )
                .aria_label(t!("prefs-text-size-option", size = text.clone()))
                .child(text),
            );
        }
        let size_group = size_group.on_change(move |next, _, cx| {
            let scale = TEXT_SIZES
                .iter()
                .find(|(name, _)| next.first().map(|v| v.as_ref()) == Some(*name))
                .map(|(_, scale)| *scale)
                .unwrap_or(1.);
            size_store.update(cx, |store, cx| store.update(cx, |s| s.text_scale = scale));
        });
        let forks = !matches!(
            super::style_library::chosen(&settings),
            super::style_library::Choice::Saved(_)
        );
        let readable = editor.readability.failing.is_empty();
        let verdict = if readable {
            Tag::new("preferences.readability.tag", t!("prefs-readability-good"))
                .variant(TagVariant::Outline)
                .icon(LucideIcon::CircleCheck)
                .text_color(theme.colors.success)
        } else {
            Tag::new("preferences.readability.tag", t!("prefs-contrast-tag"))
                .variant(TagVariant::Destructive)
                .icon(LucideIcon::TriangleAlert)
        };
        let customise = Group::new(
            "preferences.customise",
            t!("prefs-customise", name = shown_name.clone()),
        )
        .note(if forks {
            t!("prefs-customise-forks", name = shown_name.clone())
        } else {
            t!("prefs-this-computer")
        })
        .row(
            Row::new("preferences.preview", t!("prefs-preview"))
                .description(t!("prefs-preview-detail"))
                .stacked()
                .control(editor.preview),
        )
        .row(
            Row::new("preferences.accent", t!("prefs-accent"))
                .description(t!("prefs-accent-detail"))
                .stacked()
                .control(accent),
        )
        .row(
            Row::new("preferences.greys", t!("prefs-greys"))
                .description(t!("prefs-greys-detail"))
                .control(editor.background),
        )
        .row(
            Row::new("preferences.sidebar-fill", t!("prefs-sidebar-fill"))
                .description(t!("prefs-sidebar-fill-detail"))
                .control(editor.sidebar),
        )
        .row(
            Row::new("preferences.contrast", t!("prefs-contrast"))
                .description(t!("prefs-contrast-detail"))
                .control(editor.contrast),
        )
        .row(Row::new("preferences.corners", t!("prefs-corners")).control(editor.corners))
        .row(
            Row::new("preferences.readability", t!("prefs-readability"))
                .description(editor.readability.summary())
                .control(verdict),
        );
        let text = Group::new("preferences.text", t!("prefs-text"))
            .note(t!("prefs-this-computer"))
            .row(
                Row::new("preferences.font-interface", t!("prefs-font-interface")).control(ui_font),
            )
            .row(Row::new("preferences.font-code", t!("prefs-font-code")).control(code_font))
            .row(Row::new("preferences.text-size", t!("prefs-text-size")).control(size_group))
            .after(fonts_under);
        let motion = Group::new("preferences.motion", t!("prefs-motion"))
            .note(t!("prefs-this-computer"))
            .row(
                Row::new("preferences.reduce-motion", t!("prefs-reduce-motion"))
                    .description(t!("prefs-reduce-motion-detail"))
                    .control(self.local_switch(
                        "preferences.reduce-motion.switch",
                        t!("prefs-reduce-motion"),
                        settings.reduce_motion,
                        |s, v| s.reduce_motion = v,
                    )),
            );
        vec![light_dark, style, customise, text, motion]
    }

    /// A saved style's ⋯ menu: Rename… and Delete….
    fn style_menu(
        &mut self,
        selects: &mut PageSelects,
        name: &str,
        cx: &mut Context<Self>,
    ) -> Entity<MenuState> {
        if let Some(menu) = selects.style_menus.get(name) {
            return menu.clone();
        }
        let item = |id: &'static str, label: SharedString, dialog: DialogHandle| {
            let target = selects.style_target.clone();
            let name = name.to_owned();
            MenuItem::new(id, label).on_click(move |_, window, cx| {
                *target.borrow_mut() = Some(name.clone());
                dialog.open(window, cx);
            })
        };
        let items = [
            item(
                "preferences.style.rename",
                t!("prefs-style-rename"),
                selects.rename_style.clone(),
            ),
            item(
                "preferences.style.delete",
                t!("prefs-style-delete"),
                selects.delete_style.clone(),
            ),
        ];
        let menu = cx.new(|cx| MenuState::new(items, cx));
        selects.style_menus.insert(name.to_owned(), menu.clone());
        menu
    }

    /// Appearance's extras beside its groups: none yet.
    fn appearance_extra(&mut self, _: &mut Window, _: &mut Context<Self>) -> AnyElement {
        div().into_any_element()
    }

    /// The Servers page: the saved servers, then Add server.
    /// Servers (spec §1.3): the saved servers, in the order the team menu
    /// and Cmd/Ctrl+1–9 use, with Switch to and a ⋯ menu to move, sign out
    /// or remove one; then the current server's teams.
    fn servers_page(
        &mut self,
        selects: &mut PageSelects,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Group> {
        let settings = self.settings.read(cx).get().clone();
        let main = cx.entity().downgrade();
        let model = self.teams_model.read(cx);
        let unread: Vec<_> = model.servers().to_vec();
        let teams: Vec<_> = model.teams().to_vec();
        let current_team = model.current().cloned();
        // The preview's servers are the model's; it saves none.
        let (servers, current): (Vec<(SharedString, SharedString, SharedString)>, _) =
            if crate::preview::enabled() {
                let entries = unread
                    .iter()
                    .map(|server| {
                        let detail = t!(
                            "prefs-server-detail",
                            host = server.host.to_string(),
                            username = "arya",
                        );
                        (server.key.clone(), server.name.clone(), detail)
                    })
                    .collect();
                (entries, model.current_server())
            } else {
                let entries = settings
                    .servers
                    .iter()
                    .map(|server| {
                        let detail = t!(
                            "prefs-server-detail",
                            host = crate::live::url_host(&server.server).to_owned(),
                            username = server.username.clone(),
                        );
                        (
                            SharedString::from(server.data_key()),
                            SharedString::from(server.label().to_owned()),
                            detail,
                        )
                    })
                    .collect();
                (entries, settings.current_server())
            };
        let count = servers.len();
        let rows: Vec<Row> = servers
            .iter()
            .enumerate()
            .map(|(index, (key, name, detail))| {
                let id = SharedString::from(format!("preferences.server.{key}"));
                let shown = Some(index) == current;
                let menu = self.server_menu(selects, index, count, shown, key, cx);
                let badge = unread
                    .get(index)
                    .filter(|_| !shown)
                    .and_then(|s| crate::teams::unread_badge(s.mentions, s.unread, cx));
                let controls = div()
                    .flex()
                    .items_center()
                    .gap(UiTheme::read(cx).space(2.))
                    .children(badge)
                    .when(!shown, |row| {
                        row.child(
                            Button::new(gpui_kit::ElementId::Name(format!("{id}.switch").into()))
                                .variant(ButtonVariant::Outline)
                                .size(ButtonSize::Sm)
                                .label(t!("prefs-server-switch"))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.in_main_window(cx, move |this, window, cx| {
                                        this.switch_server(index, window, cx)
                                    })
                                })),
                        )
                    })
                    .child(
                        Menu::new(&menu, t!("prefs-server-options", server = name.to_string()))
                            .trigger(
                                lucide(LucideIcon::Ellipsis)
                                    .size(scaled(UiTheme::read(cx), metrics::ICON))
                                    .text_color(UiTheme::read(cx).colors.muted_foreground),
                            )
                            .trigger_style_with({
                                let theme = UiTheme::read(cx).clone();
                                move |button| {
                                    style_button(
                                        button,
                                        false,
                                        ButtonVariant::Ghost,
                                        ButtonSize::IconSm,
                                        &theme,
                                    )
                                }
                            }),
                    );
                Row::new(id, name.clone())
                    .media(crate::teams::server_icon(name, cx))
                    .description(detail.clone())
                    .control(controls)
            })
            .collect();
        let mut groups = vec![
            Group::new("preferences.servers", t!("prefs-servers"))
                .note(t!("prefs-this-computer"))
                .rows(rows)
                .reorder(move |from, to, _, cx| {
                    main.update(cx, |this, cx| this.move_server(from, to, cx))
                        .ok();
                })
                .after(
                    div().child(
                        Button::new("preferences.add-server")
                            .variant(ButtonVariant::Outline)
                            .size(ButtonSize::Sm)
                            .label(t!("prefs-add-server"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.in_main_window(cx, |this, window, cx| {
                                    this.add_server(None, window, cx)
                                })
                            })),
                    ),
                ),
        ];
        let server_name = current
            .and_then(|index| servers.get(index))
            .map(|(_, name, _)| name.to_string())
            .unwrap_or_else(|| "Matty".to_owned()); // i18n-ignore
        let team_rows: Vec<Row> = teams
            .iter()
            .map(|team| {
                let id = SharedString::from(format!("preferences.team.{}", team.id));
                let shown = current_team.as_ref() == Some(&team.id);
                let team_id = team.id.clone();
                let badge = (!shown)
                    .then(|| crate::teams::unread_badge(team.mentions, team.unread, cx))
                    .flatten();
                let menu = self.team_menu(selects, team, cx);
                let mut row = Row::new(id.clone(), team.name.clone())
                    .media(crate::teams::team_icon(team, cx));
                if shown {
                    row = row.description(t!("prefs-team-current"));
                }
                row.control(
                    div()
                        .flex()
                        .items_center()
                        .gap(UiTheme::read(cx).space(2.))
                        .children(badge)
                        .when(!shown, |controls| {
                            controls.child(
                                Button::new(gpui_kit::ElementId::Name(
                                    format!("{id}.switch").into(),
                                ))
                                .variant(ButtonVariant::Outline)
                                .size(ButtonSize::Sm)
                                .label(t!("prefs-team-switch"))
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        let team = team_id.clone();
                                        this.in_main_window(cx, move |this, window, cx| {
                                            this.switch_team(team, window, cx)
                                        })
                                    },
                                )),
                            )
                        })
                        .child(
                            Menu::new(&menu, t!("teams-options", team = team.name.to_string()))
                                .trigger(
                                    lucide(LucideIcon::Ellipsis)
                                        .size(scaled(UiTheme::read(cx), metrics::ICON))
                                        .text_color(UiTheme::read(cx).colors.muted_foreground),
                                )
                                .trigger_style_with({
                                    let theme = UiTheme::read(cx).clone();
                                    move |button| {
                                        style_button(
                                            button,
                                            false,
                                            ButtonVariant::Ghost,
                                            ButtonSize::IconSm,
                                            &theme,
                                        )
                                    }
                                }),
                        ),
                )
            })
            .collect();
        groups.push(
            Group::new("preferences.teams", t!("prefs-teams", server = server_name))
                .rows(team_rows)
                .after(
                    div().child(
                        Button::new("preferences.join-team")
                            .variant(ButtonVariant::Outline)
                            .size(ButtonSize::Sm)
                            .label(t!("prefs-join-team"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.in_main_window(cx, |this, window, cx| {
                                    this.join_team(window, cx)
                                })
                            })),
                    ),
                ),
        );
        groups
    }

    /// A team's ⋯ menu: Leave team…, which asks first (#745).
    fn team_menu(
        &mut self,
        selects: &mut PageSelects,
        team: &crate::teams::Team,
        cx: &mut Context<Self>,
    ) -> Entity<MenuState> {
        let action = selects.team_action.clone();
        let dialog = selects.leave_team.clone();
        let chosen = (team.id.clone(), team.name.clone());
        let items = vec![
            MenuItem::new("preferences.team.leave", t!("teams-leave"))
                .icon(LucideIcon::LogOut)
                .on_click(move |_, window, cx| {
                    action.replace(Some(chosen.clone()));
                    dialog.open(window, cx);
                }),
        ];
        match selects.team_menus.get(&team.id) {
            Some(menu) => {
                let menu = menu.clone();
                menu.update(cx, |menu, cx| menu.set_items(items, cx));
                menu
            }
            None => {
                let menu = cx.new(|cx| MenuState::new(items, cx));
                selects.team_menus.insert(team.id.clone(), menu.clone());
                menu
            }
        }
    }

    /// A saved server's ⋯ menu: Move up and down, then Sign out… for the
    /// shown server or Remove… for another. Built once per place.
    fn server_menu(
        &mut self,
        selects: &mut PageSelects,
        index: usize,
        count: usize,
        shown: bool,
        key: &SharedString,
        cx: &mut Context<Self>,
    ) -> Entity<MenuState> {
        let built = (index, count, shown);
        if let Some((menu, at)) = selects.server_menus.get(key)
            && *at == built
        {
            return menu.clone();
        }
        let main = cx.entity().downgrade();
        let moved = move |to: usize| {
            let main = main.clone();
            move |_: &gpui_kit::ClickEvent, _: &mut Window, cx: &mut App| {
                main.update(cx, |this, cx| this.move_server(index, to, cx))
                    .ok();
            }
        };
        let action = selects.server_action.clone();
        let dialog = if shown {
            selects.sign_out.clone()
        } else {
            selects.remove_server.clone()
        };
        let mut items = Vec::new();
        if index > 0 {
            items.push(
                MenuItem::new("preferences.server.up", t!("prefs-server-move-up"))
                    .icon(LucideIcon::ArrowUp)
                    .on_click(moved(index - 1)),
            );
        }
        if index + 1 < count {
            items.push(
                MenuItem::new("preferences.server.down", t!("prefs-server-move-down"))
                    .icon(LucideIcon::ArrowDown)
                    .on_click(moved(index + 1)),
            );
        }
        if !items.is_empty() {
            items.push(MenuItem::separator());
        }
        let (id, label) = if shown {
            ("preferences.server.sign-out", t!("prefs-server-sign-out"))
        } else {
            ("preferences.server.remove", t!("prefs-server-remove"))
        };
        items.push(MenuItem::new(id, label).on_click(move |_, window, cx| {
            action.set(Some(index));
            dialog.open(window, cx);
        }));
        let menu = match selects.server_menus.get(key) {
            Some((menu, _)) => {
                let menu = menu.clone();
                menu.update(cx, |menu, cx| menu.set_items(items, cx));
                menu
            }
            None => cx.new(|cx| MenuState::new(items, cx)),
        };
        selects
            .server_menus
            .insert(key.clone(), (menu.clone(), built));
        menu
    }
}

/// A confirmation's popup: its title, what it does, Cancel and a
/// destructive `action` button.
pub(super) fn confirm_popup(
    id: &'static str,
    title: SharedString,
    description: SharedString,
    action: SharedString,
    cx: &mut App,
) -> gpui_kit::Stateful<Div> {
    let theme = UiTheme::read(cx).clone();
    dialog_popup(
        gpui_kit::ElementId::Name(format!("{id}.popup").into()),
        title.clone(),
        cx,
    )
    .child(dialog_title(gpui_kit::ElementId::Name(format!("{id}.title").into()), cx).child(title))
    .child(
        dialog_description(
            gpui_kit::ElementId::Name(format!("{id}.description").into()),
            cx,
        )
        .child(description),
    )
    .child(
        div()
            .flex()
            .justify_end()
            .gap(theme.space(2.))
            .pt(theme.space(2.))
            .child(
                alert_dialog_cancel(gpui_kit::ElementId::Name(format!("{id}.cancel").into()))
                    .variant(ButtonVariant::Outline)
                    .label(t!("prefs-cancel")),
            )
            .child(
                alert_dialog_action(gpui_kit::ElementId::Name(format!("{id}.confirm").into()))
                    .variant(ButtonVariant::Destructive)
                    .label(action),
            ),
    )
}

/// Says that some of a Mattermost theme's color pairs fall below WCAG AA
/// (#214, PLAN §8): a destructive tag, then how many and the three that
/// miss by most, with their ratios.
fn contrast_warning(failing: &[crate::contrast::Pair], theme: &UiTheme) -> Div {
    let mut worst: Vec<_> = failing.iter().collect();
    worst.sort_by(|a, b| (a.ratio() / a.kind.minimum()).total_cmp(&(b.ratio() / b.kind.minimum())));
    let named: Vec<String> = worst
        .iter()
        .take(3)
        .map(|pair| {
            t!(
                "prefs-contrast-pair",
                name = pair.label().to_string(),
                ratio = format!("{:.1}", pair.ratio()),
            )
            .to_string()
        })
        .collect();
    let examples = match named.as_slice() {
        [one] => one.clone(),
        [rest @ .., last] => t!(
            "prefs-list-and",
            rest = rest.join(&*t!("prefs-list-separator")),
            last = last.clone(),
        )
        .to_string(),
        [] => String::new(),
    };
    div()
        .flex()
        .flex_col()
        .items_start()
        .gap(theme.space(1.5))
        .child(
            Tag::new("preferences.colors.contrast", t!("prefs-contrast-tag"))
                .variant(TagVariant::Destructive)
                .icon(LucideIcon::TriangleAlert),
        )
        .child(
            div()
                .type_style(style::META, theme)
                .text_color(theme.colors.muted_foreground)
                .child(t!(
                    "prefs-contrast-warning",
                    count = failing.len(),
                    examples = examples,
                )),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(pairs: &[(&str, &str, &str)]) -> Vec<Preference> {
        pairs
            .iter()
            .map(|(category, name, value)| preference(category, name, value))
            .collect()
    }

    #[test]
    fn the_accounts_settings_come_into_the_local_file() {
        let mut settings = Settings::default();
        mirror(
            &server(&[
                (DISPLAY, "message_display", "compact"),
                (DISPLAY, "use_military_time", "false"),
                (DISPLAY, "name_format", "username"),
                (DISPLAY, "collapse_previews", "true"),
                (ADVANCED, "send_on_ctrl_enter", "true"),
            ]),
            &mut settings,
        );
        assert_eq!(settings.display, MessageDisplay::Compact);
        assert_eq!(settings.clock, ClockFormat::TwelveHour);
        assert_eq!(settings.name_format, NameFormat::Username);
        assert_eq!(settings.send_on, SendKey::ModEnter);
        assert!(settings.collapse_previews);
        assert_eq!(
            collapse_previews_preference(false)[0].value,
            "false",
            "the switch writes the same preference back"
        );

        mirror(
            &server(&[
                (DISPLAY, "message_display", "clean"),
                (DISPLAY, "name_format", "full_name"),
                (ADVANCED, "send_on_ctrl_enter", "false"),
                (ADVANCED, "code_block_ctrl_enter", "true"),
            ]),
            &mut settings,
        );
        assert_eq!(settings.display, MessageDisplay::Standard);
        assert_eq!(settings.name_format, NameFormat::FullName);
        assert_eq!(settings.send_on, SendKey::CodeBlocks);
        assert_eq!(
            settings.clock,
            ClockFormat::TwelveHour,
            "a preference not sent keeps the last value"
        );
    }

    #[test]
    fn settings_never_set_on_the_server_stay_local() {
        let mut settings = Settings {
            display: MessageDisplay::Compact,
            send_on: SendKey::ModEnter,
            ..Settings::default()
        };
        let before = settings.clone();
        mirror(
            &server(&[("favorite_channel", "c1", "true")]),
            &mut settings,
        );
        assert_eq!(settings, before);
        // The web app turns Ctrl+Enter on for code blocks unless told not to.
        mirror(
            &server(&[(ADVANCED, "send_on_ctrl_enter", "false")]),
            &mut settings,
        );
        assert_eq!(settings.send_on, SendKey::CodeBlocks);
    }

    #[test]
    fn every_choice_round_trips_through_the_server() {
        for display in [MessageDisplay::Standard, MessageDisplay::Compact] {
            let mut settings = Settings::default();
            mirror(&display_preference(display), &mut settings);
            assert_eq!(settings.display, display);
        }
        for clock in [ClockFormat::TwelveHour, ClockFormat::TwentyFourHour] {
            let mut settings = Settings::default();
            mirror(&clock_preference(clock), &mut settings);
            assert_eq!(settings.clock, clock);
        }
        for format in [
            NameFormat::Username,
            NameFormat::NicknameFullName,
            NameFormat::FullName,
        ] {
            let mut settings = Settings::default();
            mirror(&name_format_preference(format), &mut settings);
            assert_eq!(settings.name_format, format);
        }
        for send_on in [SendKey::Enter, SendKey::ModEnter, SendKey::CodeBlocks] {
            let mut settings = Settings::default();
            mirror(&send_key_preferences(send_on), &mut settings);
            assert_eq!(settings.send_on, send_on);
        }
    }

    #[test]
    fn the_wire_values_are_mattermosts() {
        let values = |preferences: Vec<Preference>| -> Vec<(String, String, String)> {
            preferences
                .into_iter()
                .map(|p| (p.category, p.name, p.value))
                .collect()
        };
        assert_eq!(
            values(display_preference(MessageDisplay::Standard)),
            [(DISPLAY.into(), "message_display".into(), "clean".into())]
        );
        assert_eq!(
            values(name_format_preference(NameFormat::NicknameFullName)),
            [(
                DISPLAY.into(),
                "name_format".into(),
                "nickname_full_name".into()
            )]
        );
        assert_eq!(
            values(send_key_preferences(SendKey::CodeBlocks)),
            [
                (ADVANCED.into(), "send_on_ctrl_enter".into(), "false".into()),
                (
                    ADVANCED.into(),
                    "code_block_ctrl_enter".into(),
                    "true".into()
                ),
            ]
        );
    }

    #[test]
    fn the_sections_are_the_specs_seven_in_order() {
        let labels: Vec<_> = Section::ALL.iter().map(|s| s.label()).collect();
        assert_eq!(
            labels,
            [
                "General",
                "Appearance",
                "Messages",
                "Notifications",
                "Servers",
                "Keyboard",
                "Advanced"
            ]
        );
        assert_eq!(Section::default(), Section::General);
    }

    /// Hosts the section list the way Preferences does, after a control
    /// that has focus first.
    struct Probe {
        selected: Section,
        before: gpui_kit::FocusHandle,
    }

    impl gpui_kit::Render for Probe {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let view = cx.entity().downgrade();
            div()
                .size_full()
                .flex()
                .child(
                    div()
                        .id("before")
                        .track_focus(&self.before.clone().tab_stop(true))
                        .child("Before"),
                )
                .child(
                    div()
                        .w(gpui_kit::px(200.))
                        .h(gpui_kit::px(300.))
                        .child(section_list(
                            self.selected,
                            false,
                            move |section, _, cx| {
                                view.update(cx, |probe, cx| {
                                    probe.selected = section;
                                    cx.notify();
                                })
                                .ok();
                            },
                            cx,
                        )),
                )
        }
    }

    #[gpui_kit::test]
    fn the_keyboard_walks_the_section_list(cx: &mut gpui_kit::TestAppContext) {
        cx.update(|cx| {
            crate::ui::theme::init(cx);
            crate::settings::SettingsStore::init(None, cx);
        });
        let window = cx.add_window(|window, cx| {
            window.activate_window();
            Probe {
                selected: Section::General,
                before: cx.focus_handle(),
            }
        });
        let probe = window.root(cx).unwrap();
        let mut cx = gpui_kit::VisualTestContext::from_window(window.into(), cx);
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.update(|window, cx| {
            let before = probe.read(cx).before.clone();
            before.focus(window, cx);
            window.draw(cx).clear(cx);
        });
        let mut press = |keys: &'static str| {
            for key in keys.split(' ') {
                let keystroke = gpui_kit::Keystroke::parse(key).unwrap();
                cx.simulate_event(gpui_kit::KeyDownEvent {
                    keystroke: keystroke.clone(),
                    is_held: false,
                    prefer_character_input: false,
                });
                cx.simulate_event(gpui_kit::KeyUpEvent { keystroke });
                cx.update(|window, cx| window.draw(cx).clear(cx));
            }
            cx.update(|_, cx| probe.read(cx).selected)
        };
        // Tab lands on the selected row; Down moves on and Enter opens it.
        assert_eq!(press("tab down enter"), Section::Appearance);
        assert_eq!(press("down down enter"), Section::Notifications);
        // Up from the first row wraps to the last.
        assert_eq!(press("home up enter"), Section::Advanced);
        assert_eq!(press("up up enter"), Section::Servers);
    }

    /// Hosts a keyword editor the way the Notifications section does.
    struct Keywords {
        input: gpui_kit::Entity<crate::ui::input::InputState>,
        keywords: Vec<String>,
    }

    impl gpui_kit::Render for Keywords {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let theme = UiTheme::read(cx).clone();
            let view = cx.entity().downgrade();
            div().w(gpui_kit::px(600.)).child(keyword_editor(
                "preferences.keywords",
                "Keywords".into(),
                &self.input,
                self.keywords.clone(),
                move |keyword, cx| {
                    let keyword = keyword.to_owned();
                    view.update(cx, |this, cx| {
                        this.keywords.retain(|k| *k != keyword);
                        cx.notify();
                    })
                    .ok();
                },
                &theme,
            ))
        }
    }

    #[gpui_kit::test]
    fn a_keyword_tag_removes_its_keyword(cx: &mut gpui_kit::TestAppContext) {
        cx.update(|cx| {
            crate::ui::theme::init(cx);
            crate::settings::SettingsStore::init(None, cx);
        });
        let window = cx.add_window(|window, cx| Keywords {
            input: gpui_kit::AppContext::new(cx, |cx| {
                crate::ui::input::InputState::new(window, cx)
            }),
            keywords: vec!["Deploy".into(), "on-call".into()],
        });
        let probe = window.root(cx).unwrap();
        let mut cx = gpui_kit::VisualTestContext::from_window(window.into(), cx);
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let deploy = cx
            .debug_bounds("preferences.keywords.deploy")
            .expect("a tag per keyword");
        let on_call = cx.debug_bounds("preferences.keywords.on-call").unwrap();
        assert!(deploy.origin.x < on_call.origin.x, "in the list's order");
        // The remove button ends the tag.
        let remove = gpui_kit::point(
            deploy.origin.x + deploy.size.width - gpui_kit::px(8.),
            deploy.center().y,
        );
        cx.simulate_click(remove, Default::default());
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert_eq!(
            cx.update(|_, cx| probe.read(cx).keywords.clone()),
            ["on-call"]
        );
        assert!(cx.debug_bounds("preferences.keywords.deploy").is_none());
    }
}
