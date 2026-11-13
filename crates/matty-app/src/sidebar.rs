//! The channel sidebar: the view model sync fills and the view that draws it
//! (DESIGN.md §4). Categories collapse, unread rows are stronger, mentions
//! are `primary` pills, muted rows fade, and direct messages show presence.

use gpui_icons::{LucideIcon, lucide};
use gpui_kit::base::ElementExt as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, AppContext as _, Context, Entity, EventEmitter, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    div, px,
};

use crate::style::{self, Palette, TypeStyleExt as _, metrics, scaled};
use crate::t;
use crate::ui::{
    avatar::{Avatar, AvatarSize},
    badge::{Badge, BadgeVariant},
    button::{Button, ButtonSize, ButtonVariant, style_button},
    collapsible::collapsible_trigger,
    item::Reveal,
    menu::{Menu, MenuItem, MenuState},
    sidebar::{Sidebar, SidebarItem, SidebarItemSize, sidebar_menu_action, sidebar_menu_skeleton},
    sortable::{Sortable, SortableGroup, SortableMove, SortableRow, move_group},
    theme::{UiTheme, transition_value},
    toggle::{Toggle, ToggleSize},
};

/// The hover group of a category header: its + and options show while
/// the pointer is over it or keyboard focus is on it.
const SECTION_GROUP: &str = "sidebar-section";

/// Section names: sentence case, 12px Medium (spec §2.4).
const SECTION: style::TypeStyle = style::TypeStyle {
    size: 12.,
    line: 16.,
    weight: gpui_kit::FontWeight::MEDIUM,
};

/// "· 2 unread" after a collapsed section's name, and the unread pill.
const SECTION_NOTE: style::TypeStyle = style::TypeStyle {
    size: 11.5,
    line: 16.,
    weight: gpui_kit::FontWeight::NORMAL,
};

/// An unread row's place in the tree's scrolling content, in px from its
/// top, as last painted: what the "N unread below" pill counts.
#[derive(Clone, Debug)]
struct PaintedRow {
    top: f32,
    bottom: f32,
}

/// What the pill points at: how many unread rows are out of view that way
/// and the nearest one's place.
#[derive(Clone, Copy, Debug, PartialEq)]
struct OutOfView {
    count: usize,
    /// The content offset to scroll to so the nearest such row shows.
    target: f32,
}

/// A user's presence, as Mattermost reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presence {
    Online,
    Away,
    Dnd,
    Offline,
}

/// What a sidebar row points at, which decides its icon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChannelKind {
    Public,
    Private,
    Direct { presence: Presence },
    Group { members: usize },
}

/// One row in a category.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelRow {
    pub id: SharedString,
    pub name: SharedString,
    pub kind: ChannelKind,
    pub unread: bool,
    pub mentions: usize,
    pub muted: bool,
    /// A direct message's other person, whose picture the row shows.
    pub person: Option<SharedString>,
    /// A direct message's other person's username, so the quick switcher
    /// finds the row by it whatever the name format shows (#156); for a
    /// channel, its URL name, which `~` mentions use.
    pub handle: Option<SharedString>,
    /// A direct message's other person's custom status: its emoji after
    /// the name (#221), its text in the emoji's tooltip (#681).
    pub status: Option<crate::custom_status::StatusLine>,
}

impl ChannelRow {
    pub fn new(
        id: impl Into<SharedString>,
        name: impl Into<SharedString>,
        kind: ChannelKind,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind,
            unread: false,
            mentions: 0,
            muted: false,
            person: None,
            handle: None,
            status: None,
        }
    }

    /// The direct message's other person, by user id.
    pub fn person(mut self, user_id: impl Into<SharedString>) -> Self {
        self.person = Some(user_id.into());
        self
    }

    /// The other person's custom status.
    pub fn status(mut self, status: crate::custom_status::StatusLine) -> Self {
        self.status = Some(status);
        self
    }

    /// The direct message's other person's username.
    pub fn handle(mut self, username: impl Into<SharedString>) -> Self {
        self.handle = Some(username.into());
        self
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

    pub fn muted(mut self) -> Self {
        self.muted = true;
        self
    }
}

/// A server-side category (Favorites, Channels, Direct messages, custom).
/// Mattermost's category types: which channels each may hold.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CategoryKind {
    Favorites,
    /// Public and private channels only.
    #[default]
    Channels,
    /// Direct and group messages only.
    Direct,
    /// Made by the user; holds anything.
    Custom,
}

impl CategoryKind {
    /// The kind for the server's `type`.
    pub fn from_server(kind: &str) -> Self {
        match kind {
            "favorites" => Self::Favorites,
            "direct_messages" => Self::Direct,
            "custom" => Self::Custom,
            _ => Self::Channels,
        }
    }

    /// Whether a row of `kind` may move into a category of this kind.
    pub fn takes(self, kind: &ChannelKind) -> bool {
        let direct = matches!(kind, ChannelKind::Direct { .. } | ChannelKind::Group { .. });
        match self {
            Self::Favorites | Self::Custom => true,
            Self::Channels => !direct,
            Self::Direct => direct,
        }
    }
}

/// How a category orders its channels, as its Sort menu picks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sorting {
    /// By name.
    Alphabetical,
    /// By recent activity, newest first.
    Recent,
    /// As the user placed them; the server's default.
    #[default]
    Manual,
}

impl Sorting {
    /// A category's `sorting` as the server sends it.
    pub fn from_server(sorting: &str) -> Self {
        match sorting {
            "alpha" => Self::Alphabetical,
            "recent" => Self::Recent,
            _ => Self::Manual,
        }
    }

    /// The server's name for it.
    pub fn server_name(self) -> &'static str {
        match self {
            Self::Alphabetical => "alpha",
            Self::Recent => "recent",
            Self::Manual => "manual",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Category {
    pub id: SharedString,
    pub name: SharedString,
    pub collapsed: bool,
    /// Which of Mattermost's kinds it is; only custom ones can be renamed
    /// and deleted (#208).
    pub kind: CategoryKind,
    /// How its rows are ordered; they arrive in that order.
    pub sorting: Sorting,
    pub rows: Vec<ChannelRow>,
}

impl Category {
    pub fn new(
        id: impl Into<SharedString>,
        name: impl Into<SharedString>,
        rows: Vec<ChannelRow>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            collapsed: false,
            kind: CategoryKind::Channels,
            sorting: Sorting::Manual,
            rows,
        }
    }
}

/// Everything the sidebar draws. Sync replaces it whole; the view keeps no
/// other state than which categories the user collapsed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SidebarModel {
    /// True before the first snapshot; the view shows loading rows.
    pub loading: bool,
    pub selected: Option<SharedString>,
    /// Followed threads with replies the user has not read.
    pub unread_threads: usize,
    /// Mentions of the user in those threads' unread replies (#124).
    pub thread_mentions: usize,
    /// Unsent drafts, channels' and threads', for the Drafts row's count.
    pub drafts: usize,
    /// Channels whose composer holds a draft; their rows show a pencil.
    pub drafted: std::collections::HashSet<SharedString>,
    pub categories: Vec<Category>,
}

/// Which way Alt+Up/Down walks the sidebar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Previous,
    Next,
}

impl SidebarModel {
    /// The model before anything has loaded.
    pub fn loading() -> Self {
        Self {
            loading: true,
            ..Self::default()
        }
    }

    /// Every row in sidebar order, collapsed categories included, as
    /// Mattermost's Alt+Up/Down does.
    fn rows(&self) -> impl Iterator<Item = &ChannelRow> {
        self.categories
            .iter()
            .flat_map(|category| category.rows.iter())
    }

    /// The row one `step` away from the selection, wrapping at the ends;
    /// with `unread_only`, the nearest unread row in that direction, or
    /// `None` when nothing is unread. Muted channels never count as unread.
    pub fn neighbor(&self, step: Step, unread_only: bool) -> Option<SharedString> {
        let rows: Vec<&ChannelRow> = self
            .rows()
            .filter(|row| !unread_only || (row.unread && !row.muted))
            .collect();
        if rows.is_empty() {
            return None;
        }
        let current = self
            .selected
            .as_ref()
            .and_then(|id| self.rows().position(|row| &row.id == id));
        let order: Vec<SharedString> = self.rows().map(|row| row.id.clone()).collect();
        let candidates: Vec<usize> = rows
            .iter()
            .filter_map(|row| order.iter().position(|id| id == &row.id))
            .collect();
        let next = match (current, step) {
            (None, Step::Next) => candidates[0],
            (None, Step::Previous) => candidates[candidates.len() - 1],
            (Some(at), Step::Next) => candidates
                .iter()
                .copied()
                .find(|&index| index > at)
                .unwrap_or(candidates[0]),
            (Some(at), Step::Previous) => candidates
                .iter()
                .rev()
                .copied()
                .find(|&index| index < at)
                .unwrap_or(candidates[candidates.len() - 1]),
        };
        Some(order[next].clone())
    }

    /// The nearest unread row past the selection in `step`'s direction,
    /// without wrapping at the ends; from no selection, the first (or
    /// last) unread row. Muted channels never count as unread.
    pub fn unread_ahead(&self, step: Step) -> Option<SharedString> {
        let order: Vec<&ChannelRow> = self.rows().collect();
        let at = self
            .selected
            .as_ref()
            .and_then(|id| order.iter().position(|row| &row.id == id));
        let unread = |row: &&&ChannelRow| row.unread && !row.muted;
        match (at, step) {
            (None, Step::Next) => order.iter().find(unread),
            (None, Step::Previous) => order.iter().rev().find(unread),
            (Some(at), Step::Next) => order[at + 1..].iter().find(unread),
            (Some(at), Step::Previous) => order[..at].iter().rev().find(unread),
        }
        .map(|row| row.id.clone())
    }

    /// What the Unreads filter shows (PLAN §4.4): rows with unread posts
    /// (muted ones only for a mention) and the open channel, in their
    /// categories; categories left empty drop out.
    pub fn unreads(&self) -> SidebarModel {
        let mut model = self.clone();
        for category in &mut model.categories {
            category.rows.retain(|row| {
                (row.unread && !row.muted)
                    || row.mentions > 0
                    || self.selected.as_ref() == Some(&row.id)
            });
        }
        model
            .categories
            .retain(|category| !category.rows.is_empty());
        model
    }

    /// No row needs reading; the open channel does not count.
    pub fn caught_up(&self) -> bool {
        self.rows().all(|row| {
            self.selected.as_ref() == Some(&row.id)
                || (!row.unread || row.muted) && row.mentions == 0
        })
    }

    fn row_mut(&mut self, id: &str) -> Option<&mut ChannelRow> {
        self.categories
            .iter_mut()
            .flat_map(|category| category.rows.iter_mut())
            .find(|row| row.id.as_ref() == id)
    }

    /// Whether `id` sits in the Favorites category.
    pub fn is_favorite(&self, id: &str) -> bool {
        self.categories.iter().any(|category| {
            category.id == FAVORITES_ID && category.rows.iter().any(|row| row.id.as_ref() == id)
        })
    }

    /// Takes `id` out of whatever category holds it.
    pub fn remove(&mut self, id: &str) -> Option<ChannelRow> {
        for category in &mut self.categories {
            if let Some(index) = category.rows.iter().position(|row| row.id.as_ref() == id) {
                return Some(category.rows.remove(index));
            }
        }
        None
    }

    /// Moves `id` into Favorites, or back to the category its kind lives
    /// in (`channels` or `direct`), appending at the end as Mattermost does.
    /// Missing categories are created so the row never disappears.
    pub fn set_favorite(&mut self, id: &str, favorite: bool) {
        let Some(row) = self.remove(id) else { return };
        let (target_id, target_name) = if favorite {
            (FAVORITES_ID, t!("sidebar-favorites"))
        } else if matches!(
            row.kind,
            ChannelKind::Direct { .. } | ChannelKind::Group { .. }
        ) {
            (DIRECT_ID, t!("sidebar-direct-messages"))
        } else {
            (CHANNELS_ID, t!("sidebar-channels"))
        };
        match self.categories.iter_mut().find(|c| c.id == target_id) {
            Some(category) => category.rows.push(row),
            None => {
                let category = Category::new(target_id, target_name, vec![row]);
                if favorite {
                    self.categories.insert(0, category);
                } else {
                    self.categories.push(category);
                }
            }
        }
    }

    /// The channels with unread messages or mentions, in sidebar order,
    /// for Mark all read to view on the server.
    pub fn unread_ids(&self) -> Vec<SharedString> {
        self.rows()
            .filter(|row| row.unread || row.mentions > 0)
            .map(|row| row.id.clone())
            .collect()
    }

    /// Unread channels that are not muted: the Unreads row's count.
    pub fn unread_count(&self) -> usize {
        self.rows().filter(|row| row.unread && !row.muted).count()
    }

    /// The channels the unread digest lists, in sidebar order: unread
    /// ones that are not muted, and any with a mention (#223).
    pub fn digest_ids(&self) -> Vec<SharedString> {
        self.rows()
            .filter(|row| (row.unread && !row.muted) || row.mentions > 0)
            .map(|row| row.id.clone())
            .collect()
    }

    /// Clears every unread mark and mention count.
    pub fn mark_all_read(&mut self) {
        for row in self
            .categories
            .iter_mut()
            .flat_map(|category| category.rows.iter_mut())
        {
            row.unread = false;
            row.mentions = 0;
        }
        self.unread_threads = 0;
        self.thread_mentions = 0;
    }

    /// The Threads entry's count: its mentions as a channel row shows
    /// them, otherwise how many threads are unread (#124).
    pub fn threads_count(&self) -> EntryCount {
        if self.thread_mentions > 0 {
            EntryCount::Mentions(self.thread_mentions)
        } else if self.unread_threads > 0 {
            EntryCount::Count(self.unread_threads)
        } else {
            EntryCount::None
        }
    }
}

/// The badge on a fixed sidebar entry (DESIGN.md §4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryCount {
    /// Nothing to read or finish.
    None,
    /// Things to read or finish, as a `secondary` badge.
    Count(usize),
    /// Mentions, as the `primary` pill a channel row's mentions use.
    Mentions(usize),
}

/// The channel a peek card shows, its newest posts, and the authors whose
/// picture had loaded when they arrived.
struct Peek {
    channel: SharedString,
    posts: Vec<crate::channel::Post>,
    pictured: std::collections::HashSet<SharedString>,
}

/// How many posts a peek card shows (spec §2.5).
const PEEK_POSTS: usize = 3;
/// How long the pointer rests on a row before its card opens, and how long
/// the card stays after the pointer leaves it (spec §2.5).
const PEEK_OPEN_DELAY: std::time::Duration = std::time::Duration::from_millis(380);
const PEEK_CLOSE_DELAY: std::time::Duration = std::time::Duration::from_millis(220);

/// The sidebar header's height (spec §2.1).
const SIDEBAR_HEADER: f32 = 46.;
/// What the bottom of the tree keeps clear for the unread pill.
const PILL_ROOM: f32 = 30.;

/// The unread rows painted out of view above and below the tree's
/// visible part, from last frame's places and this frame's offset.
fn out_of_view(
    painted: &[PaintedRow],
    scroll: &gpui_kit::ScrollHandle,
) -> (Option<OutOfView>, Option<OutOfView>) {
    let viewport = scroll.bounds().size.height;
    let height = f32::from(viewport);
    if height <= 0. {
        return (None, None);
    }
    let top = -f32::from(scroll.offset().y);
    let bottom = top + height;
    let margin = 8.;
    let mut above: Option<OutOfView> = None;
    let mut below: Option<OutOfView> = None;
    for row in painted.iter() {
        if row.bottom <= top + 0.5 {
            let target = (row.top - margin).max(0.);
            above = Some(match above {
                // Rows come in tree order: the last one above is nearest.
                Some(seen) => OutOfView {
                    count: seen.count + 1,
                    target,
                },
                None => OutOfView { count: 1, target },
            });
        } else if row.top >= bottom - 0.5 {
            // The pill and the bottom padding sit over the last 30px.
            let target = row.bottom + margin + PILL_ROOM - height;
            below = Some(match below {
                Some(seen) => OutOfView {
                    count: seen.count + 1,
                    ..seen
                },
                None => OutOfView { count: 1, target },
            });
        }
    }
    (above, below)
}

/// A stable element id from a prefix and a server id.
fn element_id(prefix: &str, id: &SharedString) -> gpui_kit::ElementId {
    gpui_kit::ElementId::Name(format!("{prefix}.{id}").into())
}

/// What the sidebar asks the window to do.
/// The `selected` id that marks the Threads row, which is not a channel.
pub const THREADS_ID: &str = "~threads";
/// The hover group of a channel row: its options button shows and its
/// count hides while the pointer is over the row.
const ROW_GROUP: &str = "sidebar-row";
/// The Favorites category, as Mattermost names it.
pub const FAVORITES_ID: &str = "favorites";
/// The default category for channels.
pub const CHANNELS_ID: &str = "channels";
/// The default category for direct and group messages.
pub const DIRECT_ID: &str = "direct";
/// The `selected` id that marks the Drafts row.
pub const DRAFTS_ID: &str = "~drafts";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SidebarEvent {
    /// The user opened the Threads entry above the categories.
    OpenThreads,
    /// The user opened the Drafts entry above the categories.
    OpenDrafts,
    /// The user activated a channel row.
    Select(SharedString),
    /// "Copy link" from a row's menu (#754).
    CopyLink(SharedString),
    /// "New message" from the sidebar's plus menu.
    NewMessage,
    /// "Browse channels" from the sidebar's plus menu.
    BrowseChannels,
    /// "Create channel" from the sidebar's plus menu.
    CreateChannel,
    /// "Mark as read" from a row's menu or its peek card; the row is
    /// already clear.
    MarkRead(SharedString),
    /// A row's peek card opened (spec §2.5): the window answers with
    /// [`SidebarView::set_peek`].
    Peek(SharedString),
    /// Favorite or unfavorite a channel from its row menu; the row has
    /// already moved between Favorites and its home category.
    SetFavorite { id: SharedString, favorite: bool },
    /// Mute or unmute a channel from its row menu; the row already shows it.
    SetMuted { id: SharedString, muted: bool },
    /// "Open in new window" from a row's menu (#222).
    PopOut(SharedString),
    /// "Open in split view" from a row's menu (#222).
    OpenInSplit(SharedString),
    /// "Leave channel" from a channel row's menu.
    Leave(SharedString),
    /// "Close conversation" from a direct or group message row's menu.
    Close(SharedString),
    /// "Create new category" from the plus menu, or "New category…" in a
    /// row's Move to menu, which moves that channel into it (#208).
    NewCategory { channel_id: Option<SharedString> },
    /// "Move to" a category from a row's menu.
    MoveToCategory {
        channel_id: SharedString,
        category_id: SharedString,
    },
    /// "Rename category" from a custom category's menu.
    RenameCategory {
        id: SharedString,
        name: SharedString,
    },
    /// "Delete category" from a custom category's menu.
    DeleteCategory {
        id: SharedString,
        name: SharedString,
    },
    /// "Move category up" or "down" from a category's menu.
    MoveCategory { id: SharedString, up: bool },
    /// A choice in a category's Sort menu.
    SortCategory { id: SharedString, sorting: Sorting },
    /// "Move up" or "Move down" in a row's Move to menu: within its
    /// category, which then sorts by hand.
    MoveChannel {
        category_id: SharedString,
        channel_id: SharedString,
        up: bool,
    },
    /// A category dragged, or moved with Alt+Shift+Up/Down, to `index`;
    /// the sidebar already shows it there.
    PlaceCategory { id: SharedString, index: usize },
    /// A row dragged, or moved with Alt+Shift+Up/Down, to `index` among
    /// all of `category_id`'s rows; the sidebar already shows it there.
    PlaceChannel {
        channel_id: SharedString,
        category_id: SharedString,
        index: usize,
    },
}

pub struct SidebarView {
    model: SidebarModel,
    team_name: SharedString,
    /// The teams and servers the header's team name switches between.
    switcher: Option<Entity<crate::teams::TeamsModel>>,
    /// The plus menu: new message, browse, create.
    menu: Entity<MenuState>,
    /// The per-row options menu, refilled for the row it opens on.
    row_menu: Entity<MenuState>,
    /// The Unreads filter is on: only rows that need reading show.
    unreads_only: bool,
    /// The signed-in user, drawn in the footer.
    account: Option<Entity<crate::account::AccountView>>,
    /// The tree's scroll position, for the unread pills.
    scroll: gpui_kit::ScrollHandle,
    /// The unread rows as last painted, keyed by their tree order.
    painted: std::rc::Rc<std::cell::RefCell<Vec<PaintedRow>>>,
    /// The pills this frame drew; a paint that finds them stale redraws.
    pills: std::rc::Rc<std::cell::Cell<(Option<OutOfView>, Option<OutOfView>)>>,
    /// The newest posts of the channel last peeked at (spec §2.5): the
    /// card shows only once they are here.
    peek: Option<Peek>,
    /// The row whose card Space asked for, opened in the next frame.
    peek_request: std::cell::RefCell<Option<SharedString>>,
    /// The sidebar floats over the content under the pane headers, peeked
    /// at while hidden: it is not the window's top edge then.
    floating: bool,
}

impl EventEmitter<SidebarEvent> for SidebarView {}

impl SidebarView {
    pub fn new(
        team_name: impl Into<SharedString>,
        model: SidebarModel,
        cx: &mut Context<Self>,
    ) -> Self {
        let view = cx.weak_entity();
        let item = |id: &'static str, label: SharedString, event: SidebarEvent| {
            let view = view.clone();
            MenuItem::new(id, label).on_click(move |_, _, cx| {
                view.update(cx, |_, cx| cx.emit(event.clone())).ok();
            })
        };
        let menu = cx.new(|cx| {
            MenuState::new(
                [
                    item(
                        "sidebar.new-message",
                        t!("sidebar-new-message"),
                        SidebarEvent::NewMessage,
                    ),
                    MenuItem::separator(),
                    item(
                        "sidebar.browse-channels",
                        t!("sidebar-browse-channels"),
                        SidebarEvent::BrowseChannels,
                    ),
                    item(
                        "sidebar.create-channel",
                        t!("sidebar-create-channel"),
                        SidebarEvent::CreateChannel,
                    ),
                    MenuItem::separator(),
                    item(
                        "sidebar.create-category",
                        t!("sidebar-create-category"),
                        SidebarEvent::NewCategory { channel_id: None },
                    ),
                ],
                cx,
            )
        });
        let row_menu = cx.new(|cx| MenuState::new([], cx));
        Self {
            model,
            team_name: team_name.into(),
            switcher: None,
            menu,
            row_menu,
            unreads_only: false,
            account: None,
            scroll: gpui_kit::ScrollHandle::new(),
            painted: Default::default(),
            pills: Default::default(),
            peek: None,
            peek_request: Default::default(),
            floating: false,
        }
    }

    /// Whether the sidebar floats over the content, as it does while the
    /// hidden sidebar is peeked at.
    pub fn set_floating(&mut self, floating: bool, cx: &mut Context<Self>) {
        if self.floating != floating {
            self.floating = floating;
            cx.notify();
        }
    }

    /// Whether one of the sidebar's menus is open or a row is being
    /// dragged: the peeked sidebar stays while it is.
    pub fn busy(&self, cx: &App) -> bool {
        cx.has_active_drag()
            || self.menu.read(cx).is_open()
            || self.row_menu.read(cx).is_open()
            || self
                .switcher
                .as_ref()
                .is_some_and(|switcher| switcher.read(cx).menu_open(cx))
    }

    /// The newest posts of `id` for its peek card, oldest first.
    pub fn set_peek(
        &mut self,
        id: SharedString,
        mut posts: Vec<crate::channel::Post>,
        cx: &mut Context<Self>,
    ) {
        let skip = posts.len().saturating_sub(PEEK_POSTS);
        posts.drain(..skip);
        // Faces whose picture is here now; the rest keep their initial
        // while the card is open rather than swap in under the pointer.
        let pictured = posts
            .iter()
            .filter(|post| crate::avatars::image(&post.author_id, cx).is_some())
            .map(|post| post.author_id.clone())
            .collect();
        self.peek = Some(Peek {
            channel: id,
            posts,
            pictured,
        });
        cx.notify();
    }

    /// The signed-in user the footer shows; the footer redraws as they
    /// change presence or status.
    pub fn set_account(
        &mut self,
        account: Entity<crate::account::AccountView>,
        cx: &mut Context<Self>,
    ) {
        cx.observe(&account, |_, _, cx| cx.notify()).detach();
        self.account = Some(account);
        cx.notify();
    }

    /// Whether the Unreads filter is on.
    pub fn unreads_only(&self) -> bool {
        self.unreads_only
    }

    /// Turns the Unreads filter on or off (`Cmd+Shift+U` or the header
    /// toggle).
    pub fn toggle_unreads(&mut self, cx: &mut Context<Self>) {
        self.set_unreads_only(!self.unreads_only, cx);
    }

    fn set_unreads_only(&mut self, on: bool, cx: &mut Context<Self>) {
        self.unreads_only = on;
        cx.notify();
    }

    /// The rows as drawn: the whole model, or what the Unreads filter
    /// leaves, so Alt+Up/Down walks what the user sees.
    pub fn shown(&self) -> std::borrow::Cow<'_, SidebarModel> {
        if self.unreads_only {
            std::borrow::Cow::Owned(self.model.unreads())
        } else {
            std::borrow::Cow::Borrowed(&self.model)
        }
    }

    /// Clears one row's unread mark, as "Mark as read" does.
    pub fn mark_read(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(row) = self.model.row_mut(id) {
            row.unread = false;
            row.mentions = 0;
            cx.notify();
        }
    }

    pub fn set_muted(&mut self, id: &str, muted: bool, cx: &mut Context<Self>) {
        if let Some(row) = self.model.row_mut(id) {
            row.muted = muted;
            cx.notify();
        }
    }

    pub fn set_favorite(&mut self, id: &str, favorite: bool, cx: &mut Context<Self>) {
        self.model.set_favorite(id, favorite);
        cx.notify();
    }

    /// The row menu's items for `row`: mark read while unread, favorite,
    /// mute, move, open in a new window, then leave or close.
    fn row_items(&self, row: &ChannelRow, cx: &Context<Self>) -> Vec<MenuItem> {
        let view = cx.weak_entity();
        let emit = |id: &'static str, label: SharedString, event: SidebarEvent| {
            let view = view.clone();
            MenuItem::new(id, label).on_click(move |_, _, cx| {
                view.update(cx, |_, cx| cx.emit(event.clone())).ok();
            })
        };
        let direct = matches!(
            row.kind,
            ChannelKind::Direct { .. } | ChannelKind::Group { .. }
        );
        let favorite = self.model.is_favorite(&row.id);
        let mut items = Vec::new();
        if row.unread {
            items.push(emit(
                "sidebar.row.read",
                t!("sidebar-mark-read"),
                SidebarEvent::MarkRead(row.id.clone()),
            ));
        }
        items.push(emit(
            "sidebar.row.favorite",
            if favorite {
                t!("sidebar-favorite-remove")
            } else {
                t!("sidebar-favorite-add")
            },
            SidebarEvent::SetFavorite {
                id: row.id.clone(),
                favorite: !favorite,
            },
        ));
        items.push(emit(
            "sidebar.row.mute",
            match (row.muted, direct) {
                (true, true) => t!("sidebar-unmute-conversation"),
                (true, false) => t!("sidebar-unmute-channel"),
                (false, true) => t!("sidebar-mute-conversation"),
                (false, false) => t!("sidebar-mute-channel"),
            },
            SidebarEvent::SetMuted {
                id: row.id.clone(),
                muted: !row.muted,
            },
        ));
        items.push(self.move_to(row, cx));
        items.push(emit(
            "sidebar.row.copy-link",
            t!("sidebar-copy-link"),
            SidebarEvent::CopyLink(row.id.clone()),
        ));
        items.push(emit(
            "sidebar.row.split",
            t!("sidebar-open-split"),
            SidebarEvent::OpenInSplit(row.id.clone()),
        ));
        items.push(emit(
            "sidebar.row.pop-out",
            t!("sidebar-open-window"),
            SidebarEvent::PopOut(row.id.clone()),
        ));
        items.push(MenuItem::separator());
        items.push(if direct {
            emit(
                "sidebar.row.close",
                t!("sidebar-close-conversation"),
                SidebarEvent::Close(row.id.clone()),
            )
        } else {
            emit(
                "sidebar.row.leave",
                t!("sidebar-leave-channel"),
                SidebarEvent::Leave(row.id.clone()),
            )
        });
        items
    }

    /// "Move to": every other category, then a new one.
    fn move_to(&self, row: &ChannelRow, cx: &Context<Self>) -> MenuItem {
        let view = cx.weak_entity();
        let emit = |id: String, label: SharedString, event: SidebarEvent| {
            let view = view.clone();
            MenuItem::new(SharedString::from(id), label).on_click(move |_, _, cx| {
                view.update(cx, |_, cx| cx.emit(event.clone())).ok();
            })
        };
        let mut items: Vec<MenuItem> = Vec::new();
        if let Some(category) = self
            .model
            .categories
            .iter()
            .find(|category| category.rows.iter().any(|other| other.id == row.id))
        {
            let at = category
                .rows
                .iter()
                .position(|other| other.id == row.id)
                .unwrap_or_default();
            for (up, label, end) in [
                (true, t!("sidebar-move-up"), at == 0),
                (
                    false,
                    t!("sidebar-move-down"),
                    at + 1 >= category.rows.len(),
                ),
            ] {
                items.push(
                    emit(
                        format!("sidebar.row.move.{}", if up { "up" } else { "down" }),
                        label,
                        SidebarEvent::MoveChannel {
                            category_id: category.id.clone(),
                            channel_id: row.id.clone(),
                            up,
                        },
                    )
                    .disabled(end),
                );
            }
            items.push(MenuItem::separator());
        }
        let before = items.len();
        items.extend(
            self.model
                .categories
                .iter()
                .filter(|category| {
                    category.kind.takes(&row.kind)
                        && !category.rows.iter().any(|other| other.id == row.id)
                })
                .map(|category| {
                    emit(
                        format!("sidebar.row.move.{}", category.id),
                        category.name.clone(),
                        SidebarEvent::MoveToCategory {
                            channel_id: row.id.clone(),
                            category_id: category.id.clone(),
                        },
                    )
                }),
        );
        if items.len() > before {
            items.push(MenuItem::separator());
        }
        items.push(emit(
            "sidebar.row.move.new".into(),
            t!("sidebar-new-category"),
            SidebarEvent::NewCategory {
                channel_id: Some(row.id.clone()),
            },
        ));
        MenuItem::submenu("sidebar.row.move", t!("sidebar-move-to"), items)
    }

    /// A category's menu: move it, and for a custom one rename or delete it.
    fn open_category_menu(
        &mut self,
        id: &SharedString,
        at: gpui_kit::Point<gpui_kit::Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let categories = &self.model.categories;
        let Some(index) = categories.iter().position(|category| &category.id == id) else {
            return;
        };
        let category = &categories[index];
        let custom = category.kind == CategoryKind::Custom;
        let view = cx.weak_entity();
        let emit = |id: &'static str, label: SharedString, event: SidebarEvent| {
            let view = view.clone();
            MenuItem::new(id, label).on_click(move |_, _, cx| {
                view.update(cx, |_, cx| cx.emit(event.clone())).ok();
            })
        };
        let (id, name) = (category.id.clone(), category.name.clone());
        let mut items = vec![
            MenuItem::new("sidebar.category.unreads", t!("sidebar-show-unread-only"))
                .checked(self.unreads_only)
                .on_click(|_, window, cx| {
                    window.dispatch_action(Box::new(crate::actions::UnreadsFilter), cx)
                }),
            MenuItem::separator(),
        ];
        if custom {
            items.push(emit(
                "sidebar.category.rename",
                t!("sidebar-rename-category"),
                SidebarEvent::RenameCategory {
                    id: id.clone(),
                    name: name.clone(),
                },
            ));
        }
        // As in the web app, Direct messages sort only by name or activity.
        let choices: &[Sorting] = if category.kind == CategoryKind::Direct {
            &[Sorting::Alphabetical, Sorting::Recent]
        } else {
            &[Sorting::Alphabetical, Sorting::Recent, Sorting::Manual]
        };
        let sorts: Vec<MenuItem> = choices
            .iter()
            .map(|&sorting| {
                let (item, label) = match sorting {
                    Sorting::Alphabetical => {
                        ("sidebar.category.sort.alpha", t!("sidebar-sort-alpha"))
                    }
                    Sorting::Recent => ("sidebar.category.sort.recent", t!("sidebar-sort-recent")),
                    Sorting::Manual => ("sidebar.category.sort.manual", t!("sidebar-sort-manual")),
                };
                emit(
                    item,
                    label,
                    SidebarEvent::SortCategory {
                        id: id.clone(),
                        sorting,
                    },
                )
                .checked(category.sorting == sorting)
            })
            .collect();
        items.push(MenuItem::submenu(
            "sidebar.category.sort",
            t!("sidebar-sort"),
            sorts,
        ));
        items.push(MenuItem::separator());
        items.push(emit(
            "sidebar.category.new",
            t!("sidebar-create-category"),
            SidebarEvent::NewCategory { channel_id: None },
        ));
        items.push(MenuItem::separator());
        items.push(
            emit(
                "sidebar.category.up",
                t!("sidebar-category-up"),
                SidebarEvent::MoveCategory {
                    id: id.clone(),
                    up: true,
                },
            )
            .disabled(index == 0),
        );
        items.push(
            emit(
                "sidebar.category.down",
                t!("sidebar-category-down"),
                SidebarEvent::MoveCategory {
                    id: id.clone(),
                    up: false,
                },
            )
            .disabled(index + 1 >= categories.len()),
        );
        if custom {
            items.push(MenuItem::separator());
            items.push(emit(
                "sidebar.category.delete",
                t!("sidebar-delete-category"),
                SidebarEvent::DeleteCategory { id, name },
            ));
        }
        self.row_menu.update(cx, |menu, cx| {
            menu.set_items(items, cx);
            menu.open_at(Some(at), window, cx);
        });
    }

    fn open_row_menu(
        &mut self,
        id: &SharedString,
        at: gpui_kit::Point<gpui_kit::Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(row) = self
            .model
            .categories
            .iter()
            .flat_map(|category| category.rows.iter())
            .find(|row| &row.id == id)
            .cloned()
        else {
            return;
        };
        let items = self.row_items(&row, cx);
        self.row_menu.update(cx, |menu, cx| {
            menu.set_items(items, cx);
            menu.open_at(Some(at), window, cx);
        });
    }

    pub fn model(&self) -> &SidebarModel {
        &self.model
    }

    /// Makes the header's team name the team and server switcher.
    pub fn set_switcher(
        &mut self,
        model: Entity<crate::teams::TeamsModel>,
        cx: &mut Context<Self>,
    ) {
        cx.observe(&model, |_, _, cx| cx.notify()).detach();
        self.switcher = Some(model);
        cx.notify();
    }

    /// The team name in the header.
    pub fn set_team_name(&mut self, name: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.team_name = name.into();
        cx.notify();
    }

    /// Replaces the model, keeping the user's collapsed categories. Says
    /// whether anything changed; an equal model does not redraw (#787).
    pub fn set_model(&mut self, mut model: SidebarModel, cx: &mut Context<Self>) -> bool {
        for category in &mut model.categories {
            if let Some(old) = self.model.categories.iter().find(|c| c.id == category.id) {
                category.collapsed = old.collapsed;
            }
        }
        if model == self.model {
            return false;
        }
        self.model = model;
        cx.notify();
        true
    }

    /// Selects `id` and clears its unread mark, as viewing a channel does;
    /// sync confirms the view with the server.
    pub fn select(&mut self, id: Option<SharedString>, cx: &mut Context<Self>) {
        if let Some(row) = id.as_ref().and_then(|id| {
            self.model
                .categories
                .iter_mut()
                .flat_map(|category| category.rows.iter_mut())
                .find(|row| &row.id == id)
        }) {
            row.unread = false;
            row.mentions = 0;
        }
        self.model.selected = id;
        cx.notify();
    }

    /// Marks `id` as the one shown, leaving its unread mark: split view's
    /// focus moved to a pane that already shows it (#222).
    pub fn highlight(&mut self, id: Option<SharedString>, cx: &mut Context<Self>) {
        if self.model.selected != id {
            self.model.selected = id;
            cx.notify();
        }
    }

    /// Marks one row unread, as U on a selected post does.
    pub fn mark_unread(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(row) = self.model.row_mut(id) {
            row.unread = true;
            cx.notify();
        }
    }

    /// Whether `id` shows unread messages or mentions.
    /// Whether the sidebar on screen lists the channel.
    pub fn has_row(&self, id: &str) -> bool {
        self.model.rows().any(|row| row.id.as_ref() == id)
    }

    pub fn is_unread(&self, id: &str) -> bool {
        self.model
            .rows()
            .any(|row| row.id.as_ref() == id && (row.unread || row.mentions > 0))
    }

    /// The channels Mark all read clears.
    pub fn unread_ids(&self) -> Vec<SharedString> {
        self.model.unread_ids()
    }

    pub fn mark_all_read(&mut self, cx: &mut Context<Self>) {
        self.model.mark_all_read();
        cx.notify();
    }

    fn toggle_category(&mut self, id: &SharedString, cx: &mut Context<Self>) {
        if let Some(category) = self.model.categories.iter_mut().find(|c| &c.id == id) {
            category.collapsed = !category.collapsed;
            cx.notify();
        }
    }

    /// The team name without a switcher, as in a test's sidebar.
    fn plain_team_button(&self, theme: &UiTheme) -> Button {
        let theme = theme.clone();
        Button::new("sidebar.team")
            .variant(ButtonVariant::Sidebar)
            .size(ButtonSize::Sm)
            .aria_label(t!("sidebar-team-menu"))
            .px(theme.space(2.))
            .type_style(style::TITLE, &theme)
            .text_color(theme.colors.sidebar_foreground)
            .child(self.team_name.clone())
            .child(
                lucide(LucideIcon::ChevronDown)
                    .size(scaled(&theme, metrics::ICON_SMALL))
                    .text_color(theme.colors.sidebar_muted_foreground()),
            )
    }

    /// The server button (docs/specs/sidebar-places-and-switching.md §6),
    /// then the team tabs when the server has more than one team (§7).
    fn header(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = UiTheme::read(cx).clone();
        let width = crate::settings::SettingsStore::try_global(cx)
            .map_or(260., |store| store.read(cx).get().sidebar_width);
        let (server, tabs) = match &self.switcher {
            Some(switcher) => {
                let name = self.team_name.clone();
                let entity = switcher.clone();
                switcher.update(cx, |model, cx| {
                    (
                        model.header_button(name, false, cx),
                        model.team_tabs(&entity, width - 16., cx),
                    )
                })
            }
            None => (self.plain_team_button(&theme).into_any_element(), None),
        };
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(scaled(&theme, SIDEBAR_HEADER))
                    .px(theme.space(2.))
                    .flex()
                    .items_center()
                    .child(div().flex_1().min_w_0().flex().child(server)),
            )
            .children(tabs.map(|tabs| div().pb(theme.space(2.5)).child(tabs)))
    }

    /// Spec §2.2: a search field look-alike that opens the quick switcher,
    /// and the New menu.
    fn jump_row(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = UiTheme::read(cx).clone();
        let loading = self.model.loading;
        let platform = crate::actions::Platform::current();
        let height = scaled(&theme, 30.);
        let keys = crate::actions::shortcut_for("Quick switcher").unwrap_or_default(); // i18n-ignore
        // A field on the sidebar: the page's color where the sidebar is the
        // page's mode, else a wash of the sidebar's own text color (#855).
        let palette = Palette::read(cx);
        let quiet = theme.colors.sidebar_muted_foreground();
        let jump = Button::new("sidebar.jump")
            .variant(ButtonVariant::Sidebar)
            .border_color(theme.colors.sidebar_border)
            .size(ButtonSize::Sm)
            .aria_label(t!("sidebar-jump-label", keys = keys.to_string()))
            .on_click(|_, window, cx| {
                window.dispatch_action(Box::new(crate::actions::QuickSwitcher), cx)
            })
            .flex_1()
            .min_w_0()
            .h(height)
            .pl(theme.space(2.5))
            .pr(theme.space(1.5))
            .gap(theme.space(2.))
            .rounded(scaled(&theme, 7.))
            .justify_start()
            .bg(palette.sidebar_field)
            .text_color(quiet)
            .font_weight(gpui_kit::FontWeight::NORMAL)
            .child(
                lucide(LucideIcon::Search)
                    .size(scaled(&theme, metrics::ICON_SMALL))
                    .text_color(quiet),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_left()
                    .child(t!("sidebar-jump")),
            )
            .child(
                crate::ui::kbd::Kbd::new(match platform {
                    crate::actions::Platform::MacOs => "⌘K",     // i18n-ignore
                    crate::actions::Platform::Other => "Ctrl K", // i18n-ignore
                })
                .bg(theme.colors.sidebar_accent)
                .text_color(quiet),
            );
        // The unread filter as a visible toggle (it was the Unreads row and
        // a section menu item; docs/specs/sidebar-places-and-switching.md §4).
        let on = self.unreads_only;
        let filter = Toggle::new("sidebar.filter")
            .variant(crate::ui::toggle::ToggleVariant::Sidebar)
            .size(ToggleSize::Sm)
            .pressed(on)
            .px_0()
            .disabled(loading)
            .aria_label(t!("sidebar-show-unread-only"))
            .w(height)
            .h(height)
            .rounded(scaled(&theme, 7.))
            .border_1()
            .map(|toggle| {
                if on {
                    toggle
                        .bg(theme.colors.sidebar_primary)
                        .border_color(theme.colors.sidebar_primary)
                } else {
                    toggle
                        .bg(palette.sidebar_field)
                        .border_color(theme.colors.sidebar_border)
                }
            })
            .on_change(|_, window, cx| {
                window.dispatch_action(Box::new(crate::actions::UnreadsFilter), cx)
            })
            .child(
                lucide(LucideIcon::ListFilter)
                    .size(scaled(&theme, metrics::ICON))
                    .text_color(if on {
                        theme.colors.sidebar_primary_foreground
                    } else {
                        quiet
                    }),
            );
        let button_theme = theme.clone();
        let compose = Menu::new(&self.menu, t!("sidebar-new"))
            .trigger(
                lucide(LucideIcon::SquarePen)
                    .size(scaled(&theme, metrics::ICON))
                    .text_color(quiet),
            )
            .trigger_style_with(move |button| {
                style_button(
                    button.aria_label(t!("sidebar-new-label")).disabled(loading),
                    false,
                    ButtonVariant::Sidebar,
                    ButtonSize::IconSm,
                    &button_theme,
                )
                .size(scaled(&button_theme, 30.))
                .rounded(scaled(&button_theme, 7.))
                .border_color(button_theme.colors.sidebar_border)
                .bg(palette.sidebar_field)
            });
        div()
            .px(scaled(&theme, 10.))
            .pb(theme.space(2.))
            .flex()
            .items_center()
            .gap(theme.space(1.5))
            .child(jump)
            .child(style::tipped(
                "sidebar.filter.tip",
                crate::actions::tooltip_text(&t!("sidebar-show-unread-only"), "Unreads filter"),
                filter,
            ))
            .child(style::tipped(
                "sidebar.compose.tip",
                t!("sidebar-new-label"),
                compose,
            ))
    }

    /// The account button and Settings
    /// (`docs/specs/sidebar-places-and-switching.md` §5); Do not disturb and
    /// Help live in the account menu.
    fn footer(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = UiTheme::read(cx).clone();
        let settings = Button::new("sidebar.settings")
            .variant(ButtonVariant::Sidebar)
            .size(ButtonSize::IconSm)
            .w(scaled(&theme, 28.))
            .h(scaled(&theme, 28.))
            .aria_label(t!("sidebar-settings"))
            .on_click(|_, window, cx| {
                window.dispatch_action(Box::new(crate::actions::OpenSettings), cx)
            })
            .child(
                lucide(LucideIcon::SlidersHorizontal)
                    .size(scaled(&theme, metrics::ICON))
                    .text_color(theme.colors.sidebar_muted_foreground()),
            );
        div()
            .h(scaled(&theme, 48.))
            .w_full()
            .flex()
            .items_center()
            .gap(theme.space(0.5))
            .pl(theme.space(2.))
            .pr(theme.space(2.))
            .border_t_1()
            .border_color(theme.colors.sidebar_border)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .children(self.account.clone()),
            )
            .child(style::tipped(
                "sidebar.settings.tip",
                crate::actions::tooltip_text("Settings", &t!("sidebar-settings")),
                settings,
            ))
    }

    /// The "N unread above" or "below" pill (spec §2.4).
    fn unread_pill(&self, up: bool, at: OutOfView, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = UiTheme::read(cx).clone();
        let label = if up {
            t!("sidebar-unread-above", count = at.count)
        } else {
            t!("sidebar-unread-below", count = at.count)
        };
        let scroll = self.scroll.clone();
        let target = at.target;
        let pill = Button::new(if up {
            "sidebar.unread-above"
        } else {
            "sidebar.unread-below"
        })
        .variant(ButtonVariant::Default)
        .size(ButtonSize::Xs)
        .aria_label(label.clone())
        .h(scaled(&theme, 24.))
        .px(theme.space(2.5))
        .gap(theme.space(1.))
        .rounded_full()
        .type_style(
            style::TypeStyle {
                weight: gpui_kit::FontWeight::MEDIUM,
                ..SECTION_NOTE
            },
            &theme,
        )
        .shadow_md()
        .on_click(cx.listener(move |_, _, _, cx| {
            let max = f32::from(scroll.max_offset().y).max(0.);
            let mut offset = scroll.offset();
            offset.y = px(-target.clamp(0., max));
            scroll.set_offset(offset);
            cx.notify();
        }))
        .child(
            lucide(if up {
                LucideIcon::ArrowUp
            } else {
                LucideIcon::ArrowDown
            })
            .size(scaled(&theme, metrics::ICON_TINY))
            .text_color(theme.colors.primary_foreground),
        )
        .child(label);
        let place = div().absolute().left_0().right_0().flex().justify_center();
        // The pill takes the pointer, so a click on it never reaches the
        // row it floats over.
        let pill = div().occlude().child(pill);
        if up {
            place.top(px(2.)).child(pill)
        } else {
            place.bottom(px(2.)).child(pill)
        }
    }

    /// Records where an unread row painted, in the tree's content space.
    /// `row`, with its peek card when it is unread and not open (spec
    /// §2.5): resting the pointer on it or pressing Space opens the card.
    fn peekable(&self, row: &ChannelRow, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let drawn = self.row(row, cx);
        let selected = self.model.selected.as_ref() == Some(&row.id);
        if !(row.unread && !row.muted) || selected {
            return drawn.into_any_element();
        }
        let requested = {
            let mut request = self.peek_request.borrow_mut();
            let asked = request.as_ref() == Some(&row.id);
            if asked {
                *request = None;
            }
            asked
        };
        let id = row.id.clone();
        let view = cx.entity().downgrade();
        let card = {
            let (view, row) = (view.clone(), row.clone());
            move |window: &mut Window, cx: &mut App| {
                view.update(cx, |this, cx| this.peek_card(&row, window, cx))
                    .unwrap_or_else(|_| div().into_any_element())
            }
        };
        let space = {
            let (view, id) = (view.clone(), id.clone());
            move |event: &gpui_kit::KeyDownEvent, _: &mut Window, cx: &mut App| {
                if event.keystroke.key == "space" && !event.keystroke.modifiers.modified() {
                    cx.stop_propagation();
                    view.update(cx, |this, cx| {
                        *this.peek_request.borrow_mut() = Some(id.clone());
                        cx.emit(SidebarEvent::Peek(id.clone()));
                        cx.notify();
                    })
                    .ok();
                }
            }
        };
        let opened = {
            let (view, id) = (view, id.clone());
            move |open: &bool, _: &mut Window, cx: &mut App| {
                if *open {
                    view.update(cx, |_, cx| cx.emit(SidebarEvent::Peek(id.clone())))
                        .ok();
                }
            }
        };
        // No card pops up over the rows while a sidebar menu is open or a
        // row is being dragged (spec §2.5).
        let busy = {
            let menus = [self.menu.clone(), self.row_menu.clone()];
            let switcher = self.switcher.clone();
            move |_: &Window, cx: &App| {
                cx.has_active_drag()
                    || menus.iter().any(|menu| menu.read(cx).is_open())
                    || switcher
                        .as_ref()
                        .is_some_and(|switcher| switcher.read(cx).menu_open(cx))
            }
        };
        crate::ui::popover::hover_popover(element_id("sidebar.peek", &id), row.name.clone())
            .passive()
            .open_requested(requested)
            .suppress_hover_when(busy)
            .open_on_hover(PEEK_OPEN_DELAY, PEEK_CLOSE_DELAY)
            .placement(gpui_kit::base::Placement::Right)
            .align(gpui_kit::base::Align::Start)
            .on_open_change(opened)
            .w_full()
            .trigger(div().w_full().capture_key_down(space).child(drawn))
            .content(card)
            .into_any_element()
    }

    /// The card a peek opens: the channel, its newest posts as plain text,
    /// and Mark read and Open. Nothing until the posts are here.
    fn peek_card(
        &mut self,
        row: &ChannelRow,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let Some(peek) = self.peek.as_ref().filter(|peek| peek.channel == row.id) else {
            return div().into_any_element();
        };
        let (posts, pictured) = (&peek.posts, &peek.pictured);
        let theme = UiTheme::read(cx).clone();
        let palette = Palette::read(cx);
        let clock = crate::settings::ClockFormat::current(cx);
        let note = if row.mentions > 0 {
            t!("sidebar-peek-mentions", count = row.mentions)
        } else {
            t!("sidebar-peek-unread")
        };
        let header = div()
            .flex()
            .items_center()
            .gap(theme.space(2.))
            .child(Self::row_icon(row, &theme, &palette, cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .type_style(style::UI_STRONG, &theme)
                    .text_color(theme.colors.popover_foreground)
                    .child(row.name.clone()),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .type_style(style::META, &theme)
                    .text_color(theme.colors.muted_foreground)
                    .child(note),
            );
        let posts = posts.iter().map(|post| {
            let initial: String = post.author.chars().take(1).collect();
            div()
                .flex()
                .gap(theme.space(2.))
                .child({
                    let face = Avatar::new(element_id("sidebar.peek.avatar", &post.id))
                        .size(AvatarSize::Sm)
                        .w(scaled(&theme, 20.))
                        .h(scaled(&theme, 20.))
                        .fallback(initial);
                    if pictured.contains(&post.author_id) {
                        crate::avatars::with_picture(face, &post.author_id, cx)
                    } else {
                        face
                    }
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .flex()
                                .items_baseline()
                                .gap(theme.space(1.5))
                                .child(
                                    div()
                                        .truncate()
                                        .type_style(style::UI, &theme)
                                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                                        .text_color(theme.colors.popover_foreground)
                                        .child(post.author.clone()),
                                )
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .type_style(style::META, &theme)
                                        .text_color(theme.colors.muted_foreground)
                                        .child(clock.format(post.time)),
                                ),
                        )
                        .child(
                            div()
                                .type_style(style::UI, &theme)
                                .text_color(theme.colors.popover_foreground)
                                .line_clamp(3)
                                // One paragraph: the clamp counts lines, not
                                // the gaps between them.
                                .child(
                                    crate::channel::plain_text(&post.body)
                                        .split_whitespace()
                                        .collect::<Vec<_>>()
                                        .join(" "),
                                ),
                        ),
                )
        });
        let (read, open) = (row.id.clone(), row.id.clone());
        let footer = div()
            .flex()
            .justify_end()
            .gap(theme.space(2.))
            .child(
                Button::new("sidebar.peek.read")
                    .variant(ButtonVariant::Outline)
                    .size(ButtonSize::Sm)
                    .label(t!("sidebar-peek-read"))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.emit(SidebarEvent::MarkRead(read.clone()))
                    })),
            )
            .child(
                Button::new("sidebar.peek.open")
                    .variant(ButtonVariant::Default)
                    .size(ButtonSize::Sm)
                    .label(t!("sidebar-peek-open"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.model.selected = Some(open.clone());
                        cx.emit(SidebarEvent::Select(open.clone()));
                        window.dispatch_action(Box::new(crate::actions::FocusComposer), cx);
                        cx.notify();
                    })),
            );
        crate::ui::preview_card::preview_card_popup(element_id("sidebar.peek.card", &row.id), cx)
            .w(scaled(&theme, 300.))
            // The popover keeps 4px from the row; the card wants 10.
            .ml(px(6.))
            .flex()
            .flex_col()
            .gap(theme.space(3.))
            .child(header)
            .children(posts)
            .child(footer)
            .into_any_element()
    }

    fn track_unread(&self, row: impl IntoElement) -> impl IntoElement {
        let (painted, scroll) = (self.painted.clone(), self.scroll.clone());
        div()
            .w_full()
            .on_prepaint(move |bounds, _, _| {
                let origin = f32::from(scroll.bounds().top()) + f32::from(scroll.offset().y);
                painted.borrow_mut().push(PaintedRow {
                    top: f32::from(bounds.top()) - origin,
                    bottom: f32::from(bounds.bottom()) - origin,
                });
            })
            .child(row)
    }

    /// The chip that says the Unreads filter is on and turns it off.
    fn filter_chip(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = UiTheme::read(cx).clone();
        div()
            .h(scaled(&theme, 24.))
            .mb(theme.space(2.))
            .px(theme.space(1.))
            .flex()
            .items_center()
            .child(
                Button::new("sidebar.filter-chip")
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Xs)
                    .aria_label(t!("sidebar-filter-off"))
                    .h(scaled(&theme, 22.))
                    .px(theme.space(2.))
                    .gap(theme.space(1.))
                    .rounded_full()
                    .type_style(style::META, &theme)
                    .on_click(cx.listener(|this, _, _, cx| this.set_unreads_only(false, cx)))
                    .child(t!("sidebar-unread-only"))
                    .child(
                        lucide(LucideIcon::X)
                            .size(scaled(&theme, metrics::ICON_TINY))
                            .text_color(theme.colors.secondary_foreground),
                    ),
            )
    }

    fn draft_icon(theme: &UiTheme) -> impl IntoElement + use<> {
        div()
            .size(scaled(theme, metrics::ROW_SLOT))
            .flex()
            .items_center()
            .justify_center()
            .child(
                lucide(LucideIcon::Pencil)
                    .size(scaled(theme, metrics::ICON))
                    .text_color(theme.colors.sidebar_muted_foreground()),
            )
    }

    /// The icon slot: `#` and lock for channels, an avatar with a presence
    /// dot for people, a member count for group messages.
    fn row_icon(
        row: &ChannelRow,
        theme: &UiTheme,
        palette: &Palette,
        cx: &App,
    ) -> impl IntoElement + use<> {
        let icon_size = scaled(theme, metrics::ICON);
        let slot = scaled(theme, metrics::ROW_SLOT);
        let color = theme.colors.sidebar_muted_foreground();
        let boxed = |inner: gpui_kit::AnyElement| {
            div()
                .size(slot)
                .flex()
                .items_center()
                .justify_center()
                .child(inner)
                .into_any_element()
        };
        match &row.kind {
            ChannelKind::Public => boxed(
                lucide(LucideIcon::Hash)
                    .size(icon_size)
                    .text_color(color)
                    .into_any_element(),
            ),
            ChannelKind::Private => boxed(
                lucide(LucideIcon::Lock)
                    .size(icon_size)
                    .text_color(color)
                    .into_any_element(),
            ),
            ChannelKind::Group { members } => boxed(
                div()
                    .size(icon_size)
                    .rounded(theme.radius.sm)
                    .bg(theme.colors.sidebar_accent)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(theme.text(10.))
                    .line_height(theme.text(10.))
                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                    .text_color(color)
                    .child(members.to_string())
                    .into_any_element(),
            ),
            ChannelKind::Direct { presence } => {
                let initial: String = row.name.chars().take(1).collect();
                div()
                    .relative()
                    .size(slot)
                    .child(
                        crate::avatars::with_picture(
                            Avatar::new(element_id("sidebar.avatar", &row.id))
                                .size(AvatarSize::Sm)
                                .fallback(initial)
                                .aria_label(row.name.clone())
                                .status(crate::style::sidebar_avatar_status(*presence, palette))
                                .status_size(scaled(theme, 6.))
                                .ring_color(theme.colors.sidebar),
                            row.person.as_deref().unwrap_or_default(),
                            cx,
                        )
                        .w(slot)
                        .h(slot)
                        .text_size(theme.text(9.)),
                    )
                    .into_any_element()
            }
        }
    }

    fn row(&self, row: &ChannelRow, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = UiTheme::read(cx).clone();
        let palette = Palette::read(cx);
        let selected = self.model.selected.as_ref() == Some(&row.id);
        let strong = row.unread && !row.muted;
        let id = row.id.clone();
        let mut item = SidebarItem::new(element_id("sidebar.channel", &row.id), row.name.clone())
            .size(SidebarItemSize::Small)
            .selected(selected)
            .icon(if self.model.drafted.contains(&row.id) && !selected {
                // An unsent draft takes the icon's place, as in Mattermost
                // (#114).
                Self::draft_icon(&theme).into_any_element()
            } else {
                Self::row_icon(row, &theme, &palette, cx).into_any_element()
            })
            .type_style(if strong { style::UI_STRONG } else { style::UI }, &theme)
            .on_activate(
                cx.listener(move |this, event: &gpui_kit::ClickEvent, window, cx| {
                    this.model.selected = Some(id.clone());
                    cx.emit(SidebarEvent::Select(id.clone()));
                    // A click lands in the composer, as in Mattermost, so the
                    // row keeps focus (and Alt+Shift+Up/Down moves it) only
                    // when the list was reached from the keyboard.
                    if !event.is_keyboard() {
                        window.dispatch_action(Box::new(crate::actions::FocusComposer), cx);
                    }
                    cx.notify();
                }),
            );
        if !strong && !selected {
            item = item.text_color(theme.colors.sidebar_muted_foreground());
        }
        // A muted channel is the quietest text on the sidebar, at a color
        // that still reads (3:1 or more), not an opacity that may not.
        if row.muted && !selected {
            item = item.text_color(palette.sidebar_faint);
        }
        if selected {
            item = item.bg(palette.sidebar_selected);
        }
        // The status emoji follows the name, where the options button never
        // covers it, and its tooltip reads the status (#681).
        if let Some(status) = &row.status {
            let mut tooltip = status.label();
            let until = status.until_label(
                &crate::clock::now(),
                crate::settings::ClockFormat::current(cx),
            );
            if let Some(until) = &until {
                tooltip = format!("{tooltip} · {until}");
            }
            // The tooltip is a card like a reaction's (#753): the emoji
            // large, the status under it, when it clears muted.
            let name = status.emoji.clone();
            let text: SharedString = if status.text.is_empty() {
                status.label().into()
            } else {
                status.text.clone()
            };
            let hint: Option<SharedString> = until;
            item = item
                .label_suffix(crate::custom_status::emoji(
                    &status.emoji,
                    scaled(&theme, metrics::ICON_SMALL),
                    cx,
                ))
                .label_suffix_tooltip(tooltip)
                .label_suffix_tooltip_view(move |_, cx| {
                    let glyph =
                        crate::emoji::glyph(&name).unwrap_or_else(|| format!(":{name}:").into());
                    let image = crate::emoji::custom_source(&name, cx);
                    crate::ui::message::emoji_tooltip(glyph, image, text.clone(), hint.clone(), cx)
                });
        }
        let mut trailing: Vec<gpui_kit::AnyElement> = Vec::new();
        if row.mentions > 0 {
            trailing.push(
                Badge::count(row.mentions, 99)
                    .bg(theme.colors.sidebar_badge())
                    .text_color(theme.colors.sidebar_badge_foreground())
                    .h(theme.space(4.5))
                    .min_w(theme.space(4.5))
                    .px(theme.space(1.5))
                    .text_size(theme.text(11.))
                    .line_height(theme.text(11.))
                    .into_any_element(),
            );
        } else if row.unread && !row.muted {
            trailing.push(
                div()
                    .size(theme.space(1.5))
                    .rounded_full()
                    .bg(theme.colors.sidebar_foreground)
                    .mr(theme.space(1.))
                    .into_any_element(),
            );
        }
        // The count or dot hides while the row is hovered,
        // where the options button takes their place, as in Mattermost.
        if !trailing.is_empty() {
            item = item.trailing(
                div()
                    .flex()
                    .items_center()
                    .gap(theme.space(1.5))
                    .group_hover(ROW_GROUP, |style| style.invisible())
                    .children(trailing),
            );
        }
        let menu_id = row.id.clone();
        let context_id = row.id.clone();
        let options = sidebar_menu_action(
            element_id("sidebar.channel.options", &row.id),
            t!("sidebar-row-options", name = row.name.to_string()),
            cx,
        )
        .child(
            lucide(LucideIcon::Ellipsis)
                .size(scaled(&theme, metrics::ICON_SMALL))
                .text_color(theme.colors.sidebar_muted_foreground()),
        )
        .on_click(
            cx.listener(move |this, event: &gpui_kit::ClickEvent, window, cx| {
                // The button floats over the row, whose own click would
                // otherwise fire too and open the channel.
                cx.stop_propagation();
                this.open_row_menu(&menu_id, event.position(), window, cx)
            }),
        );
        div()
            .id(element_id("sidebar.channel.row", &row.id))
            .group(ROW_GROUP)
            .relative()
            .w_full()
            .on_mouse_down(
                gpui_kit::MouseButton::Right,
                cx.listener(move |this, event: &gpui_kit::MouseDownEvent, window, cx| {
                    this.open_row_menu(&context_id, event.position, window, cx)
                }),
            )
            .child(item)
            .child(
                div()
                    .absolute()
                    .right(theme.space(1.))
                    .top_0()
                    .bottom_0()
                    .flex()
                    .items_center()
                    .invisible()
                    .group_hover(ROW_GROUP, |style| style.visible())
                    .child(options),
            )
    }

    /// The places above the tree (`docs/specs/sidebar-places-and-switching.md`
    /// §2): Threads always, Drafts while there are drafts, drawn like
    /// channel rows with a count badge where there is something to read or
    /// finish. Listings (mentions, saved) are panels, not rows.
    fn fixed_entries(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = UiTheme::read(cx).clone();
        let entry = |id: &'static str,
                     label: SharedString,
                     glyph: LucideIcon,
                     count: EntryCount,
                     selected: bool,
                     opens: SidebarEvent,
                     cx: &mut Context<Self>| {
            let strong = count != EntryCount::None;
            let mut item = SidebarItem::new(id, label)
                .size(SidebarItemSize::Small)
                .selected(selected)
                .icon(
                    div()
                        .size(scaled(&theme, metrics::ROW_SLOT))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            lucide(glyph)
                                .size(scaled(&theme, metrics::ICON))
                                .text_color(theme.colors.sidebar_muted_foreground()),
                        ),
                )
                .type_style(if strong { style::UI_STRONG } else { style::UI }, &theme);
            if selected {
                item = item.bg(Palette::read(cx).sidebar_selected);
            }
            item = item.on_activate(cx.listener(move |_, _, _, cx| cx.emit(opens.clone())));
            let badge = match count {
                EntryCount::None => None,
                // A plain count is a quiet chip in the sidebar's own colors,
                // so it reads on a dark sidebar beside a light page too.
                EntryCount::Count(count) => Some(
                    Badge::new(count.to_string())
                        .variant(BadgeVariant::Secondary)
                        .bg(theme.colors.sidebar_accent)
                        .text_color(theme.colors.sidebar_accent_foreground),
                ),
                EntryCount::Mentions(count) => Some(
                    Badge::count(count, 99)
                        .bg(theme.colors.sidebar_badge())
                        .text_color(theme.colors.sidebar_badge_foreground()),
                ),
            };
            if let Some(badge) = badge {
                item = item.trailing(
                    div().debug_selector(move || format!("{id}.count")).child(
                        badge
                            .h(theme.space(4.5))
                            .min_w(theme.space(4.5))
                            .px(theme.space(1.5))
                            .text_size(theme.text(11.))
                            .line_height(theme.text(11.)),
                    ),
                );
            } else if !selected {
                item = item.text_color(theme.colors.sidebar_muted_foreground());
            }
            item
        };
        let selected = |id: &str| self.model.selected.as_deref() == Some(id);
        let drafts = match self.model.drafts {
            0 => EntryCount::None,
            count => EntryCount::Count(count),
        };
        let mut rows = vec![entry(
            THREADS_ID,
            t!("sidebar-threads"),
            LucideIcon::MessageSquareText,
            self.model.threads_count(),
            selected(THREADS_ID),
            SidebarEvent::OpenThreads,
            cx,
        )];
        // Drafts is a place only while there is something to finish
        // (spec §2), or while its view is open.
        if self.model.drafts > 0 || selected(DRAFTS_ID) {
            rows.push(entry(
                DRAFTS_ID,
                t!("sidebar-drafts"),
                LucideIcon::FilePenLine,
                drafts,
                selected(DRAFTS_ID),
                SidebarEvent::OpenDrafts,
                cx,
            ));
        }
        div()
            .flex()
            .flex_col()
            .gap(px(1.))
            .px(theme.space(2.))
            .pb(theme.space(3.))
            .children(rows)
    }

    /// A category's header: the collapsible trigger, which also opens the
    /// category's menu and is the handle for dragging the category.
    fn category_header(
        &self,
        category: &Category,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let theme = UiTheme::read(cx).clone();
        let open = !category.collapsed;
        let id = category.id.clone();
        let menu_id = category.id.clone();
        let turn = transition_value(
            element_id("sidebar.category.chevron", &category.id),
            if open { 1. } else { 0. },
            theme.motion.fast,
            window,
            cx,
        );
        let unread = if open {
            0
        } else {
            category
                .rows
                .iter()
                .filter(|row| row.unread && !row.muted)
                .count()
        };
        let trigger = collapsible_trigger(element_id("sidebar.category", &category.id), open, cx)
            .aria_label(category.name.clone())
            .w_full()
            .h(theme.space(6.))
            .px(theme.space(2.))
            .py(px(0.))
            .flex()
            .items_center()
            .justify_start()
            .gap(theme.space(1.))
            .type_style(SECTION, &theme)
            .text_color(theme.colors.sidebar_muted_foreground())
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_category(&id, cx)))
            .on_mouse_down(
                gpui_kit::MouseButton::Right,
                cx.listener(move |this, event: &gpui_kit::MouseDownEvent, window, cx| {
                    this.open_category_menu(&menu_id, event.position, window, cx)
                }),
            )
            .child(
                // The chevron turns from right to down as the category opens
                // (DESIGN.md §1 motion), snapping under reduced motion.
                lucide(LucideIcon::ChevronRight)
                    .size(scaled(&theme, metrics::ICON_TINY))
                    .text_color(theme.colors.sidebar_muted_foreground())
                    .with_transformation(gpui_kit::Transformation::rotate(gpui_kit::radians(
                        turn * std::f32::consts::FRAC_PI_2,
                    ))),
            )
            .child(div().truncate().child(category.name.clone()))
            .when(unread > 0, |trigger| {
                trigger.child(
                    div()
                        .flex_shrink_0()
                        .type_style(SECTION_NOTE, &theme)
                        .child(t!("sidebar-section-unread", count = unread)),
                )
            });
        // + and options show on hover, and on focus for the keyboard
        // (spec §2.4); Favorites takes nothing new.
        let add = match category.kind {
            CategoryKind::Favorites => None,
            CategoryKind::Direct => Some((t!("sidebar-new-message"), SidebarEvent::NewMessage)),
            CategoryKind::Channels | CategoryKind::Custom => {
                Some((t!("sidebar-browse-channels"), SidebarEvent::BrowseChannels))
            }
        };
        let ghost = |id: gpui_kit::ElementId, label: SharedString, glyph: LucideIcon| {
            sidebar_menu_action(id, label, cx)
                .w(scaled(&theme, 22.))
                .h(scaled(&theme, 22.))
                .child(
                    lucide(glyph)
                        .size(scaled(&theme, metrics::ICON_SMALL))
                        .text_color(theme.colors.sidebar_muted_foreground()),
                )
        };
        let options_id = category.id.clone();
        let options = ghost(
            element_id("sidebar.category.options", &category.id),
            t!("sidebar-section-options", name = category.name.to_string()),
            LucideIcon::Ellipsis,
        )
        .on_click(
            cx.listener(move |this, event: &gpui_kit::ClickEvent, window, cx| {
                cx.stop_propagation();
                this.open_category_menu(&options_id, event.position(), window, cx)
            }),
        );
        let add = add.map(|(label, event)| {
            ghost(
                element_id("sidebar.category.add", &category.id),
                t!(
                    "sidebar-section-add",
                    name = category.name.to_string(),
                    action = label.to_string()
                ),
                LucideIcon::Plus,
            )
            .on_click(cx.listener(move |_, _, _, cx| {
                cx.stop_propagation();
                cx.emit(event.clone())
            }))
        });
        // Shift+F10 or the context-menu key on the focused header opens the
        // same menu. The + and options show on hover, and while keyboard
        // focus is on the header or on them (gpuicn#187).
        let focus = window
            .use_keyed_state(
                element_id("sidebar.category.focus", &category.id),
                cx,
                |_, cx| cx.focus_handle(),
            )
            .read(cx)
            .clone();
        let place = std::rc::Rc::new(std::cell::Cell::new(gpui_kit::Bounds::default()));
        let (painted_at, key_id) = (place.clone(), category.id.clone());
        div()
            .id(element_id("sidebar.category.head", &category.id))
            .group(SECTION_GROUP)
            .track_focus(&focus)
            .relative()
            .w_full()
            .on_prepaint(move |bounds, _, _| painted_at.set(bounds))
            .on_key_down(
                cx.listener(move |this, event: &gpui_kit::KeyDownEvent, window, cx| {
                    let keys = &event.keystroke;
                    let menu_key = keys.key == "menu"
                        || (keys.key == "f10" && keys.modifiers.shift && !keys.modifiers.control);
                    if menu_key {
                        let at = place.get();
                        cx.stop_propagation();
                        this.open_category_menu(&key_id, at.bottom_left(), window, cx);
                    }
                }),
            )
            .child(trigger)
            .child(
                Reveal::new(
                    element_id("sidebar.category.actions", &category.id),
                    SECTION_GROUP,
                )
                .focus_within(&focus)
                .absolute()
                .right(theme.space(1.))
                .top_0()
                .bottom_0()
                .gap(px(2.))
                .children(add)
                .child(options),
            )
    }

    /// The rows a category shows: all of them when open, and only unread
    /// rows and the open channel when collapsed, as Mattermost does.
    fn shown_rows<'a>(&self, category: &'a Category) -> Vec<&'a ChannelRow> {
        category
            .rows
            .iter()
            .filter(|row| {
                !category.collapsed || row.unread || self.model.selected.as_ref() == Some(&row.id)
            })
            .collect()
    }

    /// The categories as one list whose rows and headers drag, or move
    /// with Alt+Shift+Up and Down (#121).
    fn sortable(&self, cx: &mut Context<Self>) -> Sortable {
        let view = cx.entity();
        let groups = self.model.categories.iter().map(|category| {
            let header = {
                let (view, category) = (view.clone(), category.clone());
                move |window: &mut Window, cx: &mut App| {
                    view.update(cx, |this, cx| {
                        this.category_header(&category, window, cx)
                            .into_any_element()
                    })
                }
            };
            SortableGroup::new(
                element_id("sidebar.group", &category.id),
                category.name.clone(),
            )
            .header(header)
            .rows(self.shown_rows(category).into_iter().map(|row| {
                let (view, row) = (view.clone(), row.clone());
                SortableRow::new(
                    element_id("sidebar.drag", &row.id),
                    row.name.clone(),
                    move |_: &mut Window, cx: &mut App| {
                        // The 1px under each row keeps the static list's spacing.
                        let strong = row.unread && !row.muted;
                        view.update(cx, |this, cx| {
                            let drawn = div().pb(px(1.)).child(this.peekable(&row, cx));
                            if strong {
                                this.track_unread(drawn).into_any_element()
                            } else {
                                drawn.into_any_element()
                            }
                        })
                    },
                )
            }))
        });
        let (rows, groups_moved) = (view.downgrade(), view.downgrade());
        let theme = UiTheme::read(cx);
        Sortable::new("sidebar.categories", t!("sidebar-categories"))
            .gap(theme.space(3.))
            .reorder_groups(true)
            .groups(groups.collect::<Vec<_>>())
            .on_reorder(move |movement, _, cx| {
                rows.update(cx, |this, cx| this.place_channel(movement, cx))
                    .ok();
            })
            .on_reorder_group(move |from, to, _, cx| {
                groups_moved
                    .update(cx, |this, cx| this.place_category(from, to, cx))
                    .ok();
            })
    }

    /// Applies a dragged or Alt+Shift-moved row to the model and asks the
    /// window to save it. Rows a category may not hold stay put.
    fn place_channel(&mut self, movement: &SortableMove, cx: &mut Context<Self>) {
        let categories = &self.model.categories;
        let (Some(from), Some(to)) = (
            categories.get(movement.from.group),
            categories.get(movement.to.group),
        ) else {
            return;
        };
        let Some(row) = self
            .shown_rows(from)
            .get(movement.from.index)
            .map(|row| (*row).clone())
        else {
            return;
        };
        if from.id != to.id && !to.kind.takes(&row.kind) {
            return;
        }
        let full: Vec<SharedString> = to
            .rows
            .iter()
            .filter(|other| other.id != row.id)
            .map(|other| other.id.clone())
            .collect();
        let shown: Vec<SharedString> = self
            .shown_rows(to)
            .into_iter()
            .filter(|other| other.id != row.id)
            .map(|other| other.id.clone())
            .collect();
        let index = placed_index(&full, &shown, movement.to.index);
        let category_id = to.id.clone();
        self.model.remove(&row.id);
        let channel_id = row.id.clone();
        if let Some(category) = self.model.categories.get_mut(movement.to.group) {
            let index = index.min(category.rows.len());
            category.rows.insert(index, row);
        }
        cx.emit(SidebarEvent::PlaceChannel {
            channel_id,
            category_id,
            index,
        });
        cx.notify();
    }

    /// Applies a dragged or Alt+Shift-moved category header.
    fn place_category(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if from >= self.model.categories.len() {
            return;
        }
        let id = self.model.categories[from].id.clone();
        move_group(&mut self.model.categories, from, to);
        cx.emit(SidebarEvent::PlaceCategory { id, index: to });
        cx.notify();
    }
}

/// Where a row dropped at `index` among a category's `shown` rows goes in
/// all of its rows, `full`; both leave the moved row out. Past the last
/// shown row, as on a collapsed category, it goes to the end.
fn placed_index(full: &[SharedString], shown: &[SharedString], index: usize) -> usize {
    shown
        .get(index)
        .and_then(|id| full.iter().position(|other| other == id))
        .unwrap_or(full.len())
}

impl Render for SidebarView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = UiTheme::read(cx).clone();
        // Last frame's unread rows give this frame's pills; this frame
        // records them again as they paint.
        let (above, below) = out_of_view(&self.painted.borrow(), &self.scroll);
        self.pills.set((above, below));
        self.painted.borrow_mut().clear();
        // The header is the window's top edge and drags it; on macOS a strip
        // above it keeps the traffic lights clear of the team button. Peeked
        // at, the sidebar floats under the pane headers and needs no strip.
        let lights =
            !self.floating && f32::from(crate::ui::title_bar::traffic_light_inset(window)) > 0.;
        let strip = lights.then(|| {
            crate::ui::title_bar::window_drag_area(
                "sidebar.lights.drag",
                div().id("sidebar.lights").h(scaled(&theme, 28.)).w_full(),
                window,
                cx,
            )
        });
        let header = self.header(cx);
        let header = crate::ui::title_bar::window_drag_area(
            "sidebar.header.drag",
            div().id("sidebar.header").child(header),
            window,
            cx,
        );
        let top = div()
            .flex()
            .flex_col()
            .children(strip)
            .child(header)
            .child(self.jump_row(cx))
            .when(!self.model.loading, |top| top.child(self.fixed_entries(cx)));
        let mut sidebar = Sidebar::new("sidebar", t!("sidebar-channels"))
            .roving_focus(true)
            .track_scroll(&self.scroll)
            .header_padding(px(0.))
            .footer_padding(px(0.))
            .header(top)
            .footer(self.footer(cx));
        if self.model.loading {
            for (label, rows) in [
                (t!("sidebar-favorites"), 2),
                (t!("sidebar-channels"), 4),
                (t!("sidebar-direct-messages"), 3),
            ] {
                sidebar = sidebar.child(
                    div()
                        .flex()
                        .flex_col()
                        .pb(theme.space(3.))
                        .child(
                            div()
                                .h(theme.space(6.))
                                .px(theme.space(2.))
                                .flex()
                                .items_center()
                                .type_style(SECTION, &theme)
                                .text_color(theme.colors.sidebar_muted_foreground())
                                .child(label),
                        )
                        .children((0..rows).map(|row| sidebar_menu_skeleton(row % 2 == 0, cx))),
                );
            }
            return sidebar;
        }
        if self.unreads_only {
            sidebar = sidebar.child(self.filter_chip(cx));
            // The filtered list leaves rows out, so it does not reorder.
            for category in &self.shown().categories {
                let header = self.category_header(category, window, cx);
                let rows: Vec<_> = self
                    .shown_rows(category)
                    .into_iter()
                    .map(|row| {
                        let strong = row.unread && !row.muted;
                        let drawn = self.peekable(row, cx);
                        if strong {
                            self.track_unread(drawn).into_any_element()
                        } else {
                            drawn.into_any_element()
                        }
                    })
                    .collect();
                sidebar = sidebar.child(
                    div()
                        .flex()
                        .flex_col()
                        .pb(theme.space(3.))
                        .child(header)
                        .child(div().flex().flex_col().gap(px(1.)).children(rows)),
                );
            }
        } else {
            sidebar = sidebar.child(
                div()
                    .key_context(crate::actions::SIDEBAR_SORTABLE)
                    .child(self.sortable(cx)),
            );
        }
        // Under the filter with nothing to read, only the open channel is
        // left; say so rather than look broken.
        if self.unreads_only && self.model.caught_up() {
            sidebar = sidebar.child(div().pt(theme.space(4.)).child(style::empty_state(
                LucideIcon::CircleCheck,
                t!("sidebar-caught-up"),
                t!("sidebar-caught-up-hint"),
                cx,
            )));
        }
        // Room under the last row, so the pill never covers it.
        // It paints after every row: when the rows just painted call for
        // other pills than this frame drew (the first frame, a resize, a
        // row turning unread), draw again.
        let (painted, scroll, pills, view) = (
            self.painted.clone(),
            self.scroll.clone(),
            self.pills.clone(),
            cx.weak_entity(),
        );
        sidebar = sidebar.child(
            div()
                .flex_shrink_0()
                .h(scaled(&theme, PILL_ROOM))
                .on_prepaint(move |_, _, cx| {
                    let now = out_of_view(&painted.borrow(), &scroll);
                    if now != pills.get() {
                        pills.set(now);
                        // Not during the paint itself: that frame is done.
                        let view = view.clone();
                        cx.defer(move |cx| {
                            view.update(cx, |_, cx| cx.notify()).ok();
                        });
                    }
                }),
        );
        let pills = div()
            .absolute()
            .inset_0()
            .children(above.map(|at| self.unread_pill(true, at, cx).into_any_element()))
            .children(below.map(|at| self.unread_pill(false, at, cx).into_any_element()));
        let pills_shown = above.is_some() || below.is_some();
        sidebar
            .when(pills_shown, |sidebar| sidebar.overlay(pills))
            .child(
                Menu::new(&self.row_menu, t!("sidebar-channel-options"))
                    .context_area(div())
                    .disabled(true),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_dropped_among_shown_rows_land_between_the_same_rows_in_the_full_list() {
        let ids = |ids: &[&str]| {
            ids.iter()
                .map(|id| SharedString::from(*id))
                .collect::<Vec<_>>()
        };
        let full = ids(&["a", "b", "c", "d"]);
        // An open category shows every row: the index is unchanged.
        assert_eq!(placed_index(&full, &full, 0), 0);
        assert_eq!(placed_index(&full, &full, 2), 2);
        assert_eq!(placed_index(&full, &full, 4), 4);
        // A collapsed one shows only b and d: before d is before d.
        let shown = ids(&["b", "d"]);
        assert_eq!(placed_index(&full, &shown, 0), 1);
        assert_eq!(placed_index(&full, &shown, 1), 3);
        // Past the last shown row, or into an empty group, is the end.
        assert_eq!(placed_index(&full, &shown, 2), 4);
        assert_eq!(placed_index(&full, &[], 0), 4);
    }

    #[test]
    fn favorite_moves_rows_between_categories() {
        let mut model = SidebarModel {
            loading: false,
            selected: None,
            unread_threads: 0,
            thread_mentions: 0,
            drafts: 0,
            drafted: Default::default(),
            categories: vec![
                Category::new(FAVORITES_ID, "Favorites", vec![]),
                Category::new(
                    CHANNELS_ID,
                    "Channels",
                    vec![
                        ChannelRow::new("a", "a", ChannelKind::Public),
                        ChannelRow::new("b", "b", ChannelKind::Public),
                    ],
                ),
                Category::new(
                    DIRECT_ID,
                    "Direct messages",
                    vec![ChannelRow::new(
                        "d",
                        "Dana",
                        ChannelKind::Direct {
                            presence: Presence::Online,
                        },
                    )],
                ),
            ],
        };
        assert!(!model.is_favorite("a"));
        model.set_favorite("a", true);
        assert!(model.is_favorite("a"));
        assert_eq!(model.categories[1].rows.len(), 1);
        model.set_favorite("d", true);
        assert_eq!(model.categories[0].rows.len(), 2);
        model.set_favorite("a", false);
        model.set_favorite("d", false);
        assert_eq!(model.categories[0].rows.len(), 0);
        // Back home, appended after the rows that stayed.
        assert_eq!(model.categories[1].rows[1].id, "a");
        assert_eq!(model.categories[2].rows[0].id, "d");
        assert_eq!(model.remove("b").map(|row| row.id), Some("b".into()));
        assert!(model.remove("b").is_none());
    }

    fn ids(model: &SidebarModel) -> Vec<Vec<&str>> {
        model
            .categories
            .iter()
            .map(|category| category.rows.iter().map(|row| row.id.as_ref()).collect())
            .collect()
    }

    #[test]
    fn the_unreads_filter_keeps_unread_rows_and_the_open_channel() {
        // Muted c is left out; the open channel stays though it is read.
        assert_eq!(ids(&model("a").unreads()), [vec!["a", "b"], vec!["e"]]);
        assert_eq!(ids(&model("d").unreads()), [vec!["b"], vec!["d", "e"]]);
        // Alt+Down walks the filtered rows: from b straight to e.
        assert_eq!(
            model("b").unreads().neighbor(Step::Next, false),
            Some("e".into())
        );
    }

    #[test]
    fn the_unreads_filter_is_empty_when_everything_is_read() {
        let mut all_read = model("a");
        assert!(!all_read.caught_up());
        // Shift+Escape views every unread row on the server, muted ones too.
        assert_eq!(all_read.unread_ids(), ["b", "c", "e"]);
        all_read.mark_all_read();
        assert!(all_read.unread_ids().is_empty());
        // Only the open channel is left, and that is all caught up.
        assert_eq!(ids(&all_read.unreads()), [vec!["a"]]);
        assert!(all_read.caught_up());
        all_read.selected = None;
        assert!(all_read.unreads().categories.is_empty());
    }

    fn model(selected: &str) -> SidebarModel {
        SidebarModel {
            loading: false,
            selected: Some(selected.into()),
            unread_threads: 0,
            thread_mentions: 0,
            drafts: 0,
            drafted: Default::default(),
            categories: vec![
                Category::new(
                    "fav",
                    "Favorites",
                    vec![
                        ChannelRow::new("a", "a", ChannelKind::Public),
                        ChannelRow::new("b", "b", ChannelKind::Public).unread(),
                    ],
                ),
                Category::new(
                    "ch",
                    "Channels",
                    vec![
                        ChannelRow::new("c", "c", ChannelKind::Public)
                            .unread()
                            .muted(),
                        ChannelRow::new("d", "d", ChannelKind::Public),
                        ChannelRow::new("e", "e", ChannelKind::Public).mentions(1),
                    ],
                ),
            ],
        }
    }

    #[test]
    fn neighbor_walks_across_categories_and_wraps() {
        let m = model("b");
        assert_eq!(m.neighbor(Step::Next, false).as_deref(), Some("c"));
        assert_eq!(m.neighbor(Step::Previous, false).as_deref(), Some("a"));
        assert_eq!(model("e").neighbor(Step::Next, false).as_deref(), Some("a"));
        assert_eq!(
            model("a").neighbor(Step::Previous, false).as_deref(),
            Some("e")
        );
    }

    #[test]
    fn unread_neighbor_skips_read_and_muted_rows() {
        let m = model("a");
        assert_eq!(m.neighbor(Step::Next, true).as_deref(), Some("b"));
        assert_eq!(model("b").neighbor(Step::Next, true).as_deref(), Some("e"));
        assert_eq!(
            model("b").neighbor(Step::Previous, true).as_deref(),
            Some("e")
        );
        let mut read = model("a");
        read.mark_all_read();
        assert_eq!(read.neighbor(Step::Next, true), None);
        assert_eq!(read.neighbor(Step::Next, false).as_deref(), Some("b"));
    }

    #[test]
    fn unread_ahead_stops_at_the_ends() {
        // b and e are unread; the others are read or muted.
        assert_eq!(model("a").unread_ahead(Step::Next).as_deref(), Some("b"));
        assert_eq!(model("b").unread_ahead(Step::Next).as_deref(), Some("e"));
        assert_eq!(model("e").unread_ahead(Step::Next), None);
        assert_eq!(model("b").unread_ahead(Step::Previous), None);
        let mut fresh = model("a");
        fresh.selected = None;
        assert_eq!(fresh.unread_ahead(Step::Next).as_deref(), Some("b"));
        assert_eq!(fresh.unread_ahead(Step::Previous).as_deref(), Some("e"));
    }

    /// #124: the Threads entry shows its mentions as channel rows do,
    /// otherwise how many threads are unread.
    #[test]
    fn the_threads_entry_counts_mentions_first() {
        let mut model = model("a");
        assert_eq!(model.threads_count(), EntryCount::None);
        model.unread_threads = 2;
        assert_eq!(model.threads_count(), EntryCount::Count(2));
        model.thread_mentions = 3;
        assert_eq!(model.threads_count(), EntryCount::Mentions(3));
        model.mark_all_read();
        assert_eq!(model.threads_count(), EntryCount::None);
    }

    #[test]
    fn mentions_imply_unread_and_set_model_keeps_collapse() {
        let row = ChannelRow::new("a", "a", ChannelKind::Public).mentions(3);
        assert!(row.unread);
        let mut old = SidebarModel::default();
        old.categories.push(Category {
            collapsed: true,
            ..Category::new("c", "C", vec![])
        });
        let mut next = SidebarModel::default();
        next.categories.push(Category::new("c", "C", vec![row]));
        // The same merge the view does.
        for category in &mut next.categories {
            if let Some(prev) = old.categories.iter().find(|c| c.id == category.id) {
                category.collapsed = prev.collapsed;
            }
        }
        assert!(next.categories[0].collapsed);
    }

    #[test]
    fn categories_take_only_the_rows_mattermost_allows() {
        let channel = ChannelKind::Public;
        let direct = ChannelKind::Direct {
            presence: Presence::Online,
        };
        let group = ChannelKind::Group { members: 3 };
        assert!(CategoryKind::Channels.takes(&channel));
        assert!(!CategoryKind::Channels.takes(&direct));
        assert!(CategoryKind::Direct.takes(&group));
        assert!(!CategoryKind::Direct.takes(&ChannelKind::Private));
        for any in [CategoryKind::Favorites, CategoryKind::Custom] {
            assert!(any.takes(&channel) && any.takes(&direct));
        }
        assert_eq!(CategoryKind::from_server("custom"), CategoryKind::Custom);
        assert_eq!(
            CategoryKind::from_server("direct_messages"),
            CategoryKind::Direct
        );
    }
}
