//! The team switcher (DESIGN.md §4, spec §2): the sidebar header's team
//! name opens a menu of the current server's teams and the other saved
//! servers, each with its unread dot or mention count and its shortcut,
//! with Join another team, Add server and Manage servers under them. The
//! header button wears a count when another team or server mentions the
//! user, and a dot when only unread messages wait there. The model also
//! feeds the dock badge, the quick switcher and Settings' Servers page.

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
}

impl EventEmitter<TeamsEvent> for TeamsModel {}

/// The menu's width (spec §2.2).
const MENU_WIDTH: f32 = 300.;

impl TeamsModel {
    pub fn new(teams: Vec<Team>, current: Option<SharedString>, cx: &mut Context<Self>) -> Self {
        let menu = cx.new(|cx| MenuState::new([], cx).width(px(MENU_WIDTH)));
        let mut this = Self {
            teams,
            current,
            servers: Vec::new(),
            current_server: None,
            menu,
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

    /// Rebuilds the menu's items from the teams and servers.
    fn refresh_menu(&mut self, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        let emit = move |event: TeamsEvent| {
            let view = view.clone();
            move |_: &gpui_kit::ClickEvent, _: &mut Window, cx: &mut App| {
                view.update(cx, |_, cx| cx.emit(event.clone())).ok();
            }
        };
        let mut items = Vec::new();
        let heading = self.server_name().map_or_else(
            || t!("teams-on-server", server = ""),
            |name| t!("teams-on-server", server = name.to_uppercase()),
        );
        items.push(MenuItem::label("teams.heading", heading));
        for (index, team) in self.teams.iter().enumerate() {
            let current = self.current.as_ref() == Some(&team.id);
            let leading = team.clone();
            let mut item = MenuItem::new(
                gpui_kit::ElementId::Name(format!("teams.team.{}", team.id).into()),
                team.name.clone(),
            )
            .leading(move |_, cx| team_icon(&leading, cx))
            .radio(current)
            .on_click(emit(TeamsEvent::Select(team.id.clone())));
            if !current && let Some(badge) = unread_mark(team.mentions, team.unread) {
                item = item.trailing(badge);
            }
            if index < 9 {
                item = item.shortcut(format!("secondary-alt-{}", index + 1));
            }
            items.push(item);
        }
        items.push(
            MenuItem::new("teams.join", t!("teams-join"))
                .icon(LucideIcon::Plus)
                .on_click(emit(TeamsEvent::Join)),
        );
        let others: Vec<_> = self
            .servers
            .iter()
            .enumerate()
            .filter(|(index, _)| self.current_server != Some(*index))
            .collect();
        items.push(MenuItem::separator());
        if self.servers.len() > 1 {
            items.push(MenuItem::label("teams.servers", t!("teams-other-servers")));
            for (index, server) in others {
                let name = server.name.clone();
                let mut item = MenuItem::new(
                    gpui_kit::ElementId::Name(format!("teams.server.{}", server.key).into()),
                    server.name.clone(),
                )
                .leading(move |_, cx| server_icon(&name, cx))
                .on_click(emit(TeamsEvent::SelectServer(index)));
                if server.host != server.name {
                    item = item.description(server.host.clone());
                }
                if let Some(badge) = unread_mark(server.mentions, server.unread) {
                    item = item.trailing(badge);
                }
                if index < 9 {
                    item = item.shortcut(format!("secondary-{}", index + 1));
                }
                items.push(item);
            }
        }
        items.push(
            MenuItem::new("teams.add-server", t!("teams-add-server"))
                .icon(LucideIcon::Plus)
                .on_click(emit(TeamsEvent::AddServer)),
        );
        items.push(MenuItem::separator());
        items.push(
            MenuItem::new("teams.manage", t!("teams-manage"))
                .icon(LucideIcon::Settings)
                .trailing(|_, cx| {
                    let theme = UiTheme::read(cx);
                    div()
                        .type_style(style::META, theme)
                        .text_color(theme.colors.muted_foreground)
                        .child(t!("teams-manage-hint"))
                        .into_any_element()
                })
                .on_click(emit(TeamsEvent::ManageServers)),
        );
        self.menu.update(cx, |menu, cx| menu.set_items(items, cx));
    }

    /// The header button: the team's name, a chevron, and what waits in
    /// other teams and servers.
    pub fn header_button(&self, name: SharedString, disabled: bool, cx: &App) -> AnyElement {
        let theme = UiTheme::read(cx).clone();
        let (mentions, unread) = self.elsewhere();
        let label = if mentions > 0 || unread {
            t!(
                "teams-switch-elsewhere",
                team = name.to_string(),
                count = mentions
            )
        } else {
            name.clone()
        };
        let badge = if mentions > 0 {
            Some(
                Badge::count(mentions, 99)
                    .h(theme.space(4.))
                    .min_w(theme.space(4.))
                    .px(theme.space(1.))
                    .text_size(theme.text(10.))
                    .line_height(theme.text(10.))
                    .bg(theme.colors.sidebar_primary)
                    .text_color(theme.colors.sidebar_primary_foreground)
                    .into_any_element(),
            )
        } else if unread {
            Some(
                div()
                    .size(px(8.))
                    .flex_shrink_0()
                    .rounded_full()
                    .bg(theme.colors.sidebar_foreground)
                    .opacity(0.72)
                    .into_any_element(),
            )
        } else {
            None
        };
        let trigger = div()
            .flex()
            .items_center()
            .gap(theme.space(1.5))
            .min_w_0()
            .child(div().truncate().child(name))
            .children(badge)
            .child(
                lucide(LucideIcon::ChevronDown)
                    .size(scaled(&theme, metrics::ICON_SMALL))
                    .text_color(theme.colors.sidebar_muted_foreground()),
            );
        let style_theme = theme.clone();
        let open = self.menu.read(cx).is_open();
        div()
            .id("sidebar.team.tip")
            .min_w_0()
            .when(!open, |tip| {
                tip.tooltip(move |_, cx| text_tooltip(t!("teams-switch"), cx))
            })
            .child(
                Menu::new(&self.menu, label)
                    .disabled(disabled)
                    .trigger(trigger)
                    .trigger_style_with(move |button| {
                        style_button(
                            button,
                            disabled,
                            ButtonVariant::Ghost,
                            ButtonSize::Sm,
                            &style_theme,
                        )
                        .px(style_theme.space(2.))
                        .type_style(style::TITLE, &style_theme)
                        .text_color(style_theme.colors.sidebar_foreground)
                    }),
            )
            .into_any_element()
    }
}

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
    fn mentions_imply_unread() {
        let team = Team::new("a", "Matty").mentions(2);
        assert!(team.unread);
        assert_eq!(team.mentions, 2);
        assert!(!Team::new("b", "Quiet").unread);
    }
}
