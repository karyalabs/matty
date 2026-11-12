//! The server switcher and team tabs
//! (docs/specs/sidebar-places-and-switching.md §6, §7): the sidebar header
//! is the current server and opens a menu of the saved servers, each with
//! its unread dot or mention count, then Add a server and Manage servers.
//! It wears a count when another server mentions the user, and a dot when
//! only unread messages wait there. The current server's teams are named
//! tabs under it, each with its own count; tabs that do not fit go under
//! More. The model also feeds the dock badge, the quick switcher and
//! Settings' Servers page.

use gpui_icons::{LucideIcon, lucide};
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, InteractiveElement as _,
    IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _,
    StyledImage as _, Window, div, prelude::FluentBuilder as _, px,
};

use crate::style::{self, TypeStyleExt as _, metrics, scaled};
use crate::t;
use crate::ui::{
    badge::Badge,
    button::{ButtonSize, ButtonVariant, style_button},
    menu::{Menu, MenuItem, MenuState},
    theme::UiTheme,
    tooltip::text_tooltip,
};

/// A saved server (#206), as the team menu and Settings list it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Server {
    /// What names the server for good, its data key; its name until set.
    pub key: SharedString,
    /// Its `SiteName`, or its host.
    pub name: SharedString,
    /// Its host, as the menu's description shows it.
    pub host: SharedString,
    /// Some unmuted channel on the server is unread.
    pub unread: bool,
    /// Mentions of the user across the server.
    pub mentions: usize,
}

impl Server {
    pub fn new(name: impl Into<SharedString>) -> Self {
        let name = name.into();
        Self {
            key: name.clone(),
            host: name.clone(),
            name,
            unread: false,
            mentions: 0,
        }
    }

    /// The server's data key, which keeps its place in the dragged list.
    pub fn key(mut self, key: impl Into<SharedString>) -> Self {
        self.key = key.into();
        self
    }

    /// The server's host, when its name is its `SiteName`.
    pub fn host(mut self, host: impl Into<SharedString>) -> Self {
        self.host = host.into();
        self
    }

    pub fn unread(mut self, unread: bool) -> Self {
        self.unread |= unread;
        self
    }

    pub fn mentions(mut self, count: usize) -> Self {
        self.mentions = count;
        self.unread |= count > 0;
        self
    }
}

/// Up to two initials from `name`'s words, as a square's label.
pub(crate) fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|word| word.chars().next())
        .take(2)
        .collect()
}

/// A team of the current server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Team {
    pub id: SharedString,
    pub name: SharedString,
    /// Some channel in the team is unread.
    pub unread: bool,
    /// Mentions of the user across the team's channels.
    pub mentions: usize,
    /// The team's `last_team_icon_update`; 0 when it has no icon, and the
    /// square shows initials.
    pub icon: i64,
}

impl Team {
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            unread: false,
            mentions: 0,
            icon: 0,
        }
    }

    /// The team's icon version, from its `last_team_icon_update`.
    pub fn icon(mut self, last_team_icon_update: i64) -> Self {
        self.icon = last_team_icon_update;
        self
    }

    /// The team's icon, once sync has it (#153). Learning the version here
    /// asks for a changed icon the first time it draws.
    pub(crate) fn icon_image(&self, cx: &gpui_kit::App) -> Option<std::sync::Arc<gpui_kit::Image>> {
        if self.icon <= 0 {
            return None;
        }
        let icons = cx.try_global::<crate::avatars::TeamIcons>()?;
        icons.learn(&self.id, self.icon);
        icons.image(&self.id)
    }

    pub fn unread(mut self) -> Self {
        self.unread = true;
        self
    }

    pub fn mentions(mut self, count: usize) -> Self {
        self.mentions = count;
        self.unread |= count > 0;
        self
    }

    /// Up to two initials from the display name, as a team's label.
    pub fn initials(&self) -> String {
        initials(&self.name)
    }

    /// The team's accessible name: the team's name and what is unread
    /// in it, as the badge shows (#89).
    pub fn label(&self) -> SharedString {
        match (self.mentions, self.unread) {
            (0, false) => self.name.clone(),
            (0, true) => t!("teams-team-unread", team = self.name.to_string()),
            (n, _) => t!(
                "teams-team-mentions",
                team = self.name.to_string(),
                count = n
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TeamsEvent {
    /// The user picked a team; the window switches to it and marks it
    /// current (#89).
    Select(SharedString),
    /// The user asked to join another team.
    Join,
    /// The user picked the `n`th saved server; the window switches to it.
    SelectServer(usize),
    /// The user asked to add a server.
    AddServer,
    /// The user asked for Settings' Servers page.
    ManageServers,
}

/// The teams and saved servers with their unread state, and the header
/// button that switches between them. Sync fills the teams and their
/// counts, the window the saved servers.
pub struct TeamsModel {
    teams: Vec<Team>,
    current: Option<SharedString>,
    servers: Vec<Server>,
    current_server: Option<usize>,
    menu: Entity<MenuState>,
    /// The teams whose tabs did not fit, and their menu.
    more: Entity<MenuState>,
    hidden: Vec<SharedString>,
}

impl EventEmitter<TeamsEvent> for TeamsModel {}

impl TeamsModel {
    /// Whether the server menu or the More menu is open.
    pub fn menu_open(&self, cx: &App) -> bool {
        self.menu.read(cx).is_open() || self.more.read(cx).is_open()
    }
}

/// The server menu's width (spec §6).
const MENU_WIDTH: f32 = 290.;
/// The More menu's width (spec §7).
const MORE_WIDTH: f32 = 220.;

impl TeamsModel {
    pub fn new(teams: Vec<Team>, current: Option<SharedString>, cx: &mut Context<Self>) -> Self {
        let menu = cx.new(|cx| MenuState::new([], cx).width(px(MENU_WIDTH)));
        let more = cx.new(|cx| MenuState::new([], cx).width(px(MORE_WIDTH)));
        let mut this = Self {
            teams,
            current,
            servers: Vec::new(),
            current_server: None,
            menu,
            more,
            hidden: Vec::new(),
        };
        this.refresh_menu(cx);
        this
    }

    pub fn servers(&self) -> &[Server] {
        &self.servers
    }

    pub fn current_server(&self) -> Option<usize> {
        self.current_server
    }

    /// Replaces the saved servers and which one is shown.
    pub fn set_servers(
        &mut self,
        servers: Vec<Server>,
        current: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        if self.servers != servers || self.current_server != current {
            self.servers = servers;
            self.current_server = current;
            self.changed(cx);
        }
    }

    pub fn teams(&self) -> &[Team] {
        &self.teams
    }

    pub fn current(&self) -> Option<&SharedString> {
        self.current.as_ref()
    }

    pub fn set_teams(
        &mut self,
        teams: Vec<Team>,
        current: Option<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.teams = teams;
        self.current = current;
        self.changed(cx);
    }

    /// Updates one team's unread mark and mention count.
    pub fn set_unread(&mut self, id: &str, unread: bool, mentions: usize, cx: &mut Context<Self>) {
        if let Some(team) = self.teams.iter_mut().find(|team| team.id.as_ref() == id) {
            team.unread = unread || mentions > 0;
            team.mentions = mentions;
            self.changed(cx);
        }
    }

    /// Asks the window to switch to `id`; the window marks it current
    /// once it shows the team (#89). The team's own counts stay until sync
    /// clears them channel by channel, as viewing one channel does not
    /// read the others.
    pub fn select(&mut self, id: SharedString, cx: &mut Context<Self>) {
        if self.current.as_ref() != Some(&id) {
            cx.emit(TeamsEvent::Select(id));
        }
    }

    /// Marks `id` as the team on screen.
    pub fn set_current(&mut self, id: Option<SharedString>, cx: &mut Context<Self>) {
        if self.current != id {
            self.current = id;
            self.changed(cx);
        }
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.refresh_menu(cx);
        cx.notify();
    }

    /// The current server's name, as the menu's first label names it.
    fn server_name(&self) -> Option<SharedString> {
        self.current_server
            .and_then(|index| self.servers.get(index))
            .map(|server| server.name.clone())
    }

    /// Mentions in other teams of this server and on other servers, and
    /// whether anything is unread there: what the header button wears.
    pub fn elsewhere(&self) -> (usize, bool) {
        let teams = self
            .teams
            .iter()
            .filter(|team| self.current.as_ref() != Some(&team.id));
        let servers = self
            .servers
            .iter()
            .enumerate()
            .filter(|(index, _)| self.current_server != Some(*index))
            .map(|(_, server)| server);
        let mut mentions = 0;
        let mut unread = false;
        for (m, u) in teams
            .map(|t| (t.mentions, t.unread))
            .chain(servers.map(|s| (s.mentions, s.unread)))
        {
            mentions += m;
            unread |= u;
        }
        (mentions, unread)
    }

    /// Mentions and unread state on the other saved servers: what the
    /// server button wears (docs/specs/sidebar-places-and-switching.md §6).
    /// Other teams of this server show theirs on their tabs.
    pub fn other_servers(&self) -> (usize, bool) {
        self.servers
            .iter()
            .enumerate()
            .filter(|(index, _)| self.current_server != Some(*index))
            .fold((0, false), |(mentions, unread), (_, server)| {
                (mentions + server.mentions, unread || server.unread)
            })
    }

    /// Rebuilds the menus: the server menu lists servers only, then adding
    /// a server, joining a team (when the server has one team, so there are
    /// no tabs to join from) and managing servers (spec §6); the More menu
    /// lists the teams whose tabs do not fit (spec §7).
    fn refresh_menu(&mut self, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        let emit = move |event: TeamsEvent| {
            let view = view.clone();
            move |_: &gpui_kit::ClickEvent, _: &mut Window, cx: &mut App| {
                view.update(cx, |_, cx| cx.emit(event.clone())).ok();
            }
        };
        let mut items = Vec::new();
        for (index, server) in self.servers.iter().enumerate() {
            let current = self.current_server == Some(index);
            let name = server.name.clone();
            let mut item = MenuItem::new(
                gpui_kit::ElementId::Name(format!("teams.server.{}", server.key).into()),
                server.name.clone(),
            )
            .leading(move |_, cx| server_tile(&name, MENU_TILE, current, cx))
            .radio(current)
            .on_click(emit(TeamsEvent::SelectServer(index)));
            if server.host != server.name {
                item = item.description(server.host.clone());
            }
            if !current && let Some(badge) = unread_mark(server.mentions, server.unread) {
                item = item.trailing(badge);
            }
            items.push(item);
        }
        if !items.is_empty() {
            items.push(MenuItem::separator());
        }
        items.push(
            MenuItem::new("teams.add-server", t!("teams-add-server"))
                .icon(LucideIcon::Plus)
                .on_click(emit(TeamsEvent::AddServer)),
        );
        if self.teams.len() < 2 {
            let label = match self.server_name() {
                Some(server) => t!("teams-join-on", server = server.to_string()),
                None => t!("teams-join"),
            };
            items.push(
                MenuItem::new("teams.join", label)
                    .icon(LucideIcon::Users)
                    .on_click(emit(TeamsEvent::Join)),
            );
        }
        items.push(
            MenuItem::new("teams.manage", t!("teams-manage"))
                .icon(LucideIcon::Server)
                .on_click(emit(TeamsEvent::ManageServers)),
        );
        self.menu.update(cx, |menu, cx| menu.set_items(items, cx));
        self.refresh_more(cx);
    }

    /// The More menu: the teams whose tabs did not fit, then joining.
    fn refresh_more(&mut self, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        let mut items = Vec::new();
        for team in self
            .teams
            .iter()
            .filter(|team| self.hidden.contains(&team.id))
        {
            let id = team.id.clone();
            let view = view.clone();
            let mut item = MenuItem::new(
                gpui_kit::ElementId::Name(format!("teams.more.{}", team.id).into()),
                team.name.clone(),
            )
            .on_click(move |_, _, cx| {
                view.update(cx, |this, cx| this.select(id.clone(), cx)).ok();
            });
            if let Some(badge) = unread_mark(team.mentions, team.unread) {
                item = item.trailing(badge);
            }
            items.push(item);
        }
        items.push(MenuItem::separator());
        let view = view.clone();
        items.push(
            MenuItem::new("teams.more.join", t!("teams-join"))
                .icon(LucideIcon::Plus)
                .on_click(move |_, _, cx| {
                    view.update(cx, |_, cx| cx.emit(TeamsEvent::Join)).ok();
                }),
        );
        self.more.update(cx, |menu, cx| menu.set_items(items, cx));
    }

    /// The header button: the current server's tile, name and a chevron,
    /// then what waits on other servers (spec §6). It opens the server
    /// menu. Before the servers are known it names the team instead.
    pub fn header_button(
        &self,
        fallback: SharedString,
        disabled: bool,
        cx: &mut App,
    ) -> AnyElement {
        let theme = UiTheme::read(cx).clone();
        let name = self.server_name().unwrap_or(fallback);
        let (mentions, unread) = self.other_servers();
        let elsewhere = (mentions > 0 || unread).then(|| {
            div()
                .ml_auto()
                .pl(theme.space(1.))
                .flex_shrink_0()
                .children(sidebar_mark(mentions, unread, &theme))
        });
        let trigger = div()
            .flex()
            .flex_1()
            .items_center()
            .gap(theme.space(2.))
            .min_w_0()
            .child(server_tile(&name, TILE, true, cx))
            .child(div().min_w_0().truncate().child(name.clone()))
            .child(
                lucide(LucideIcon::ChevronDown)
                    .flex_shrink_0()
                    .size(scaled(&theme, 14.))
                    .text_color(theme.colors.sidebar_muted_foreground()),
            )
            .children(elsewhere);
        let label = match (mentions, unread) {
            (0, false) => name.clone(),
            (0, true) => t!("teams-elsewhere-unread", server = name.to_string()),
            (n, _) => t!(
                "teams-elsewhere-mentions",
                server = name.to_string(),
                count = n
            ),
        };
        let style_theme = theme.clone();
        let open = self.menu.read(cx).is_open();
        div()
            .id("sidebar.server.tip")
            .flex_1()
            .min_w_0()
            .flex()
            .when(!open, |tip| {
                tip.tooltip(move |_, cx| text_tooltip(t!("teams-switch"), cx))
            })
            .child(
                Menu::new(&self.menu, label)
                    .flex_1()
                    .min_w_0()
                    .disabled(disabled)
                    .trigger(trigger)
                    .trigger_style_with(move |button| {
                        style_button(
                            button,
                            disabled,
                            ButtonVariant::Sidebar,
                            ButtonSize::Sm,
                            &style_theme,
                        )
                        .w_full()
                        .h(scaled(&style_theme, 36.))
                        .pl(style_theme.space(1.5))
                        .pr(style_theme.space(2.))
                        .min_w_0()
                        .justify_start()
                        .type_style(HEADER_NAME, &style_theme)
                        .text_color(style_theme.colors.sidebar_foreground)
                    }),
            )
            .into_any_element()
    }

    /// The team tabs under the header (spec §7), or `None` with one team.
    /// `width` is the row's width: tabs that do not fit go under More, and
    /// the current team always keeps a tab.
    pub fn team_tabs(
        &mut self,
        this: &Entity<Self>,
        width: f32,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        use crate::ui::tabs::{Tab, Tabs, TabsSize, TabsVariant};
        if self.teams.len() < 2 {
            if !self.hidden.is_empty() {
                self.hidden.clear();
                self.refresh_more(cx);
            }
            return None;
        }
        let theme = UiTheme::read(cx).clone();
        let (shown, hidden) = fit_tabs(&self.teams, self.current.as_ref(), width);
        if hidden != self.hidden {
            self.hidden = hidden.clone();
            self.refresh_more(cx);
        }
        let mut tabs = Tabs::new("sidebar.teams")
            .variant(TabsVariant::Line)
            .size(TabsSize::Sm)
            .aria_label(t!("teams-tabs"))
            .text_colors(
                theme.colors.sidebar_foreground,
                theme.colors.sidebar_muted_foreground(),
            )
            .w_full()
            .px(theme.space(2.))
            .border_color(theme.colors.sidebar_border)
            .on_change({
                let this = this.downgrade();
                move |id, _, cx| {
                    let id = id.clone();
                    this.update(cx, |this, cx| this.select(id, cx)).ok();
                }
            });
        if let Some(current) = &self.current {
            tabs = tabs.selected(current.clone());
        }
        for team in self.teams.iter().filter(|team| shown.contains(&team.id)) {
            let mut tab = Tab::new(
                gpui_kit::ElementId::Name(format!("sidebar.team-tab.{}", team.id).into()),
                team.id.clone(),
                team.name.clone(),
            )
            .aria_label(team.label());
            if let Some(mark) = sidebar_mark(team.mentions, team.unread, &theme) {
                tab = tab.trailing(mark);
            }
            tabs = tabs.item(tab);
        }
        let end = if hidden.is_empty() {
            let this = this.downgrade();
            style::tipped(
                "sidebar.team-join.tip",
                t!("teams-join-short"),
                crate::ui::button::Button::new("sidebar.team-join")
                    .variant(ButtonVariant::Sidebar)
                    .size(ButtonSize::IconSm)
                    .w(scaled(&theme, 24.))
                    .h(scaled(&theme, 24.))
                    .aria_label(t!("teams-join-short"))
                    .on_click(move |_, _, cx| {
                        this.update(cx, |_, cx| cx.emit(TeamsEvent::Join)).ok();
                    })
                    .child(
                        lucide(LucideIcon::Plus)
                            .size(scaled(&theme, metrics::ICON_SMALL))
                            .text_color(theme.colors.sidebar_muted_foreground()),
                    ),
            )
            .ml_auto()
            .into_any_element()
        } else {
            let (mentions, unread) = self
                .teams
                .iter()
                .filter(|team| hidden.contains(&team.id))
                .fold((0, false), |(m, u), team| {
                    (m + team.mentions, u || team.unread)
                });
            let trigger_theme = theme.clone();
            Menu::new(&self.more, t!("teams-more-label"))
                .trigger(
                    div()
                        .flex()
                        .items_center()
                        .gap(theme.space(1.))
                        .child(t!("teams-more-tab"))
                        .children(sidebar_mark(mentions, unread, &theme)),
                )
                .trigger_style_with(move |button| {
                    style_button(
                        button,
                        false,
                        ButtonVariant::Sidebar,
                        ButtonSize::Sm,
                        &trigger_theme,
                    )
                    .h(scaled(&trigger_theme, 24.))
                    .px(trigger_theme.space(1.))
                    .text_size(trigger_theme.text(13.))
                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                    .text_color(trigger_theme.colors.sidebar_muted_foreground())
                })
                .into_any_element()
        };
        Some(tabs.end(end).into_any_element())
    }
}

/// Which teams get a tab in `width` px and which go under More: tabs keep
/// the menu's order, More takes the rest, and the current team takes the
/// last shown place when it would not fit. Widths are estimated from the
/// name's length at 13px (no text measuring in the sidebar's layout).
fn fit_tabs(
    teams: &[Team],
    current: Option<&SharedString>,
    width: f32,
) -> (Vec<SharedString>, Vec<SharedString>) {
    const PER_CHAR: f32 = 6.3;
    const PADDING: f32 = 12. + 4.;
    const MARK: f32 = 22.;
    const MORE: f32 = 58.;
    const JOIN: f32 = 28.;
    let tab_width = |team: &Team| {
        team.name.chars().count() as f32 * PER_CHAR
            + PADDING
            + if team.mentions > 0 || team.unread {
                MARK
            } else {
                0.
            }
    };
    let total: f32 = teams.iter().map(tab_width).sum();
    if total + JOIN <= width {
        return (teams.iter().map(|t| t.id.clone()).collect(), Vec::new());
    }
    let mut shown = Vec::new();
    let mut used = 0.;
    for team in teams {
        let next = tab_width(team);
        if used + next + MORE > width {
            break;
        }
        used += next;
        shown.push(team.clone());
    }
    if let Some(current) = current
        && !shown.iter().any(|team| &team.id == current)
        && let Some(team) = teams.iter().find(|team| &team.id == current)
    {
        while !shown.is_empty() && used + tab_width(team) + MORE > width {
            if let Some(last) = shown.pop() {
                used -= tab_width(&last);
            }
        }
        shown.push(team.clone());
    }
    if shown.is_empty()
        && let Some(first) = teams.first()
    {
        shown.push(first.clone());
    }
    let shown: Vec<SharedString> = shown.into_iter().map(|team| team.id).collect();
    let hidden = teams
        .iter()
        .filter(|team| !shown.contains(&team.id))
        .map(|team| team.id.clone())
        .collect();
    (shown, hidden)
}

/// A mention pill or unread dot in the sidebar's colours, for the server
/// button and the team tabs.
fn sidebar_mark(mentions: usize, unread: bool, theme: &UiTheme) -> Option<AnyElement> {
    if mentions > 0 {
        Some(
            Badge::count(mentions, 99)
                .h(scaled(theme, 16.))
                .min_w(scaled(theme, 16.))
                .px(px(4.))
                .text_size(theme.text(10.5))
                .line_height(theme.text(10.5))
                .bg(theme.colors.sidebar_primary)
                .text_color(theme.colors.sidebar_primary_foreground)
                .into_any_element(),
        )
    } else if unread {
        Some(
            div()
                .size(scaled(theme, 7.))
                .flex_shrink_0()
                .rounded_full()
                .bg(theme.colors.sidebar_foreground)
                .into_any_element(),
        )
    } else {
        None
    }
}

/// A server's tile of `side` px: its initials on `primary` for the current
/// server and `secondary` for the others, a rounded square like a team's.
pub(crate) fn server_tile(name: &str, side: f32, current: bool, cx: &mut App) -> AnyElement {
    let theme = UiTheme::read(cx).clone();
    let (bg, fg) = if current {
        (theme.colors.primary, theme.colors.primary_foreground)
    } else {
        (theme.colors.secondary, theme.colors.secondary_foreground)
    };
    div()
        .size(scaled(&theme, side))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(scaled(&theme, side * 0.28))
        .bg(bg)
        .text_color(fg)
        .text_size(theme.text(side * 0.42))
        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
        .child(initials(name).to_uppercase())
        .into_any_element()
}

/// A server tile's side in the server menu (spec §6).
const MENU_TILE: f32 = 30.;

/// The server tile's side in the header (spec §6).
const TILE: f32 = 24.;

/// The server name in the sidebar header: 14.5px SemiBold (spec §6).
const HEADER_NAME: style::TypeStyle = style::TypeStyle {
    size: 14.5,
    line: 20.,
    weight: gpui_kit::FontWeight::SEMIBOLD,
};

/// A team's 24px square: its icon, or its initials.
pub(crate) fn team_icon(team: &Team, cx: &mut App) -> AnyElement {
    let theme = UiTheme::read(cx).clone();
    let square = div()
        .size(px(24.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme.radius.md)
        .bg(theme.colors.secondary)
        .text_color(theme.colors.secondary_foreground)
        .text_size(theme.text(11.))
        .font_weight(gpui_kit::FontWeight::SEMIBOLD);
    match team.icon_image(cx) {
        Some(icon) => square
            .child(
                gpui_kit::img(icon)
                    .size_full()
                    .rounded(theme.radius.md)
                    .object_fit(gpui_kit::ObjectFit::Cover),
            )
            .into_any_element(),
        None => square
            .child(team.initials().to_uppercase())
            .into_any_element(),
    }
}

/// A server's 24px circle with its initials.
pub(crate) fn server_icon(name: &str, cx: &mut App) -> AnyElement {
    let theme = UiTheme::read(cx).clone();
    div()
        .size(px(24.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(theme.colors.secondary)
        .text_color(theme.colors.secondary_foreground)
        .text_size(theme.text(11.))
        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
        .child(initials(name).to_uppercase())
        .into_any_element()
}

/// A mention count or unread dot, as a menu row's trailing slot.
fn unread_mark(
    mentions: usize,
    unread: bool,
) -> Option<impl Fn(&mut Window, &mut App) -> AnyElement + 'static> {
    (mentions > 0 || unread).then_some(move |_: &mut Window, cx: &mut App| {
        unread_badge(mentions, unread, cx).unwrap_or_else(|| div().into_any_element())
    })
}

/// A mention pill, or an 8px dot when only unread.
pub(crate) fn unread_badge(mentions: usize, unread: bool, cx: &App) -> Option<AnyElement> {
    let theme = UiTheme::read(cx);
    if mentions > 0 {
        Some(
            Badge::count(mentions, 99)
                .h(theme.space(4.))
                .min_w(theme.space(4.))
                .px(theme.space(1.))
                .text_size(theme.text(10.))
                .line_height(theme.text(10.))
                .into_any_element(),
        )
    } else if unread {
        Some(
            div()
                .size(px(8.))
                .rounded_full()
                .bg(theme.colors.foreground)
                .into_any_element(),
        )
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initials_take_two_words() {
        assert_eq!(Team::new("a", "Matty").initials(), "M");
        assert_eq!(Team::new("b", "Design Guild").initials(), "DG");
        assert_eq!(Team::new("c", "Open Source Friends").initials(), "OS");
        assert_eq!(initials("Matty Community"), "MC");
    }

    #[test]
    fn a_teams_name_says_what_is_unread() {
        assert_eq!(Team::new("a", "Matty").label().as_ref(), "Matty");
        assert_eq!(
            Team::new("a", "Matty").unread().label().as_ref(),
            "Matty, unread messages"
        );
        assert_eq!(
            Team::new("a", "Matty").mentions(1).label().as_ref(),
            "Matty, 1 mention"
        );
        assert_eq!(
            Team::new("a", "Matty").mentions(3).label().as_ref(),
            "Matty, 3 mentions"
        );
    }

    #[gpui_kit::test]
    fn the_header_counts_other_teams_and_servers(cx: &mut gpui_kit::TestAppContext) {
        let teams = vec![
            Team::new("a", "Matty").mentions(5),
            Team::new("b", "Design").mentions(2),
            Team::new("c", "Quiet").unread(),
        ];
        let model = cx.new(|cx| TeamsModel::new(teams, Some("a".into()), cx));
        // The current team's own mentions are in the sidebar.
        model.read_with(cx, |model, _| assert_eq!(model.elsewhere(), (2, true)));
        model.update(cx, |model, cx| {
            model.set_servers(
                vec![Server::new("Acme"), Server::new("Other").mentions(4)],
                Some(0),
                cx,
            )
        });
        model.read_with(cx, |model, _| assert_eq!(model.elsewhere(), (6, true)));
        let quiet = cx.new(|cx| TeamsModel::new(vec![Team::new("a", "Matty")], None, cx));
        quiet.read_with(cx, |model, _| assert_eq!(model.elsewhere(), (0, false)));
    }

    #[gpui_kit::test]
    fn picking_a_team_asks_the_window_which_marks_it(cx: &mut gpui_kit::TestAppContext) {
        let teams = vec![Team::new("a", "Matty"), Team::new("b", "Design")];
        let model = cx.new(|cx| TeamsModel::new(teams, Some("a".into()), cx));
        let picked = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        cx.update(|cx| {
            let picked = picked.clone();
            cx.subscribe(&model, move |_, event: &TeamsEvent, _| {
                picked.borrow_mut().push(event.clone())
            })
            .detach();
        });
        model.update(cx, |model, cx| model.select("a".into(), cx));
        model.update(cx, |model, cx| model.select("b".into(), cx));
        assert_eq!(*picked.borrow(), vec![TeamsEvent::Select("b".into())]);
        // Still the shown team until the window switches.
        model.read_with(cx, |model, _| {
            assert_eq!(model.current().map(|id| id.as_ref()), Some("a"))
        });
        model.update(cx, |model, cx| model.set_current(Some("b".into()), cx));
        model.read_with(cx, |model, _| {
            assert_eq!(model.current().map(|id| id.as_ref()), Some("b"))
        });
    }

    #[test]
    fn tabs_that_do_not_fit_go_under_more_and_the_current_team_keeps_one() {
        let teams = vec![
            Team::new("a", "Matty"),
            Team::new("b", "Design Guild").mentions(3),
            Team::new("c", "Open Source").unread(),
        ];
        let ids = |ids: &[&str]| {
            ids.iter()
                .map(|id| SharedString::from(*id))
                .collect::<Vec<_>>()
        };
        // Everything fits in a wide sidebar.
        assert_eq!(
            fit_tabs(&teams, None, 400.),
            (ids(&["a", "b", "c"]), vec![])
        );
        // At the default width the last team goes under More.
        assert_eq!(
            fit_tabs(&teams, Some(&"a".into()), 244.),
            (ids(&["a", "b"]), ids(&["c"]))
        );
        // The current team takes the last place when it would not fit.
        assert_eq!(
            fit_tabs(&teams, Some(&"c".into()), 244.),
            (ids(&["a", "c"]), ids(&["b"]))
        );
        // However narrow, one tab stays.
        assert_eq!(fit_tabs(&teams, None, 10.).0, ids(&["a"]));
    }

    #[test]
    fn mentions_imply_unread() {
        let team = Team::new("a", "Matty").mentions(2);
        assert!(team.unread);
        assert_eq!(team.mentions, 2);
        assert!(!Team::new("b", "Quiet").unread);
    }
}
