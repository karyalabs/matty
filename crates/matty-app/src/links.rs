//! Where a link in a message goes (#85, #103): `~channel`, `@user` and
//! `#hashtag` (rewritten to `matty://` links by `matty-markdown`), this
//! server's permalinks and channel URLs, and everything else in the
//! browser. [`route`] decides; the window acts on the answer.

use gpui_kit::{
    App, ElementId, IntoElement, ParentElement as _, SharedString, Styled as _, Window, div,
};
use std::sync::Arc;

use crate::t;
use crate::ui::{
    alert_dialog::{alert_dialog, alert_dialog_action, alert_dialog_cancel},
    button::ButtonVariant,
    collapsible::ShowMore,
    dialog::{DialogHandle, dialog_description, dialog_popup, dialog_title},
    markdown::{
        CustomEmoji as CustomEmojiImages, EntityLinks, MarkdownBlocks, MarkdownImages,
        MarkdownView, markdown, markdown_with,
    },
    theme::UiTheme,
};

/// Where a clicked link goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkTarget {
    /// A channel by URL name: in `team` (a team name) from a channel URL,
    /// or in the team on screen from `~name`.
    Channel { team: Option<String>, name: String },
    /// A person by username.
    User(String),
    /// A hashtag, without the `#`: a message search for it.
    Hashtag(String),
    /// A post on this server: its channel, at the post.
    Permalink { team: String, post_id: String },
    /// Add these people to the channel: the server's "did not get
    /// notified" message offers it (#1018).
    AddToChannel {
        channel_id: String,
        user_ids: Vec<String>,
    },
    /// Nothing to open: `@here`, an emoji, or a `matty://` link Matty
    /// does not know.
    Nothing,
    /// Anywhere else: the system browser.
    External(String),
}

/// Where `url` goes. `server` is the signed-in server's address, and
/// `teams` the names of the user's teams: a channel URL in another team
/// opens in the browser, since Matty cannot show it.
pub fn route(url: &str, server: Option<&str>, teams: &[&str]) -> LinkTarget {
    if let Some(entity) = url.strip_prefix("matty://") {
        return entity_target(entity);
    }
    server
        .and_then(|server| server_target(url, server, Some(teams)))
        .unwrap_or_else(|| LinkTarget::External(url.to_owned()))
}

/// `matty://user/ana` and the rest of `matty-markdown`'s entity links.
fn entity_target(entity: &str) -> LinkTarget {
    let (kind, value) = entity.split_once('/').unwrap_or((entity, ""));
    if value.is_empty() {
        return LinkTarget::Nothing;
    }
    match kind {
        "user" => LinkTarget::User(value.to_owned()),
        "channel" => LinkTarget::Channel {
            team: None,
            name: value.to_owned(),
        },
        "hashtag" => LinkTarget::Hashtag(value.to_owned()),
        "add-to-channel" => match value.split_once('/') {
            Some((channel_id, ids)) if !channel_id.is_empty() && !ids.is_empty() => {
                LinkTarget::AddToChannel {
                    channel_id: channel_id.to_owned(),
                    user_ids: ids.split(',').map(str::to_owned).collect(),
                }
            }
            _ => LinkTarget::Nothing,
        },
        _ => LinkTarget::Nothing,
    }
}

/// Whether `url` is a post, channel or direct message page of `server`,
/// whichever team it names: a link Matty can open once it shows that
/// server (#206), whose teams it does not know until then.
pub fn opens_on(url: &str, server: &str) -> bool {
    server_target(url, server, None).is_some()
}

/// A link into this server's web app: `{server}/{team}/pl/{post_id}`,
/// `{server}/{team}/channels/{name}` or `{server}/{team}/messages/@{user}`.
/// With `teams`, a channel or direct message link opens only through one
/// of them; with `None`, through any.
fn server_target(url: &str, server: &str, teams: Option<&[&str]>) -> Option<LinkTarget> {
    let url = url::Url::parse(url).ok()?;
    let server = url::Url::parse(server).ok()?;
    if url.origin() != server.origin() {
        return None;
    }
    let base: Vec<&str> = server.path_segments()?.filter(|s| !s.is_empty()).collect();
    let segments: Vec<&str> = url.path_segments()?.filter(|s| !s.is_empty()).collect();
    let rest = segments.strip_prefix(base.as_slice())?;
    let [team, kind, value] = rest else {
        return None;
    };
    let id_like = |value: &str| {
        !value.is_empty()
            && value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    };
    let known =
        teams.is_none_or(|teams| teams.iter().any(|known| known.eq_ignore_ascii_case(team)));
    match *kind {
        // A post id names one post on the server, whichever team the link
        // went through.
        "pl" if id_like(value) => Some(LinkTarget::Permalink {
            team: (*team).to_owned(),
            post_id: (*value).to_owned(),
        }),
        "channels" if known && id_like(value) => Some(LinkTarget::Channel {
            team: Some((*team).to_owned()),
            name: (*value).to_owned(),
        }),
        "messages" if known => {
            let user = value.strip_prefix('@')?;
            id_like(user).then(|| LinkTarget::User(user.to_owned()))
        }
        _ => None,
    }
}

/// The mentions in a body that are the viewer's: `@username`, and
/// `@here`, `@channel` and `@all` when those notify them. Mattermost fills
/// these and shows every other mention as a link.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OwnMentions {
    /// The viewer's username, without the `@`; empty matches nothing.
    pub username: SharedString,
    /// Whether `@here`, `@channel` and `@all` count.
    pub channel_wide: bool,
    /// The user groups the viewer is in, by `@name` (#1057).
    pub groups: Arc<[String]>,
    /// Words the body highlights, as Mattermost does: the viewer's first
    /// name when it mentions them, their keywords and highlight-only
    /// keywords, and among search results the search's terms. See
    /// [`matty_markdown::mark_words_in_source`] for how they match.
    pub marks: Arc<[String]>,
}

impl OwnMentions {
    pub fn of(keys: &matty_notify::MentionKeys) -> Self {
        Self {
            username: keys.username.clone().into(),
            channel_wide: keys.channel_wide,
            groups: keys.groups.iter().cloned().collect(),
            marks: keys
                .first_name
                .iter()
                .chain(&keys.keywords)
                .chain(&keys.highlight)
                // `@name` keywords are mentions, which show as chips.
                .filter(|word| !word.starts_with('@') && !word.trim().is_empty())
                .cloned()
                .collect(),
        }
    }

    /// These mentions, also highlighting `words`.
    pub fn with_marks(mut self, words: impl IntoIterator<Item = String>) -> Self {
        let mut marks = self.marks.to_vec();
        marks.extend(words);
        self.marks = marks.into();
        self
    }

    /// Whether the entity link `target` (the part after `matty:`, such as
    /// `//user/arya`) is one of the viewer's mentions.
    pub fn matches(&self, target: &str) -> bool {
        match target.strip_prefix("//") {
            Some(rest) => match rest.split_once('/') {
                Some(("user", name)) => {
                    (!self.username.is_empty() && name.eq_ignore_ascii_case(&self.username))
                        || self
                            .groups
                            .iter()
                            .any(|group| name.eq_ignore_ascii_case(group))
                }
                Some(("special", _)) => self.channel_wide,
                _ => false,
            },
            None => false,
        }
    }
}

/// A message body: Markdown with `~channel`, `@user` and `#hashtag` as
/// gpuicn entity chips, and every click, chip or link, handed to
/// `on_link` with its URL. A body taller than about 600 px collapses
/// behind Show more, as in Mattermost (#682).
///
/// `body` is a post's stored text: its Markdown source is worked out once
/// per body and custom emoji list (see [`cached_source`]), not every frame.
pub fn message_body(
    id: impl Into<ElementId>,
    body: &SharedString,
    image_sizes: &[(SharedString, u32, u32)],
    on_link: impl Fn(&SharedString, &mut Window, &mut App) + Clone + Send + Sync + 'static,
    on_link_menu: Option<LinkMenu>,
    own: &OwnMentions,
    cx: &App,
) -> ShowMore {
    let id = id.into();
    let source = cached_source(body, cx);
    let source = if own.marks.is_empty() {
        source
    } else {
        marked_source(source, &own.marks)
    };
    let emoticons = crate::settings::SettingsStore::try_global(cx)
        .is_none_or(|store| store.read(cx).get().emoticons);
    let mut view = markdown_from_source(
        id.clone(),
        source,
        image_sizes,
        on_link,
        on_link_menu,
        own,
        cx,
    );
    // As in Mattermost, a message of nothing but emoji shows them large.
    let custom = cx.try_global::<crate::emoji::CustomEmoji>();
    let body = if emoticons {
        matty_markdown::emoticons_as_emoji_codes(body)
    } else {
        body.to_string()
    };
    if only_emoji(&body, &|name| {
        crate::emoji::lookup(name).is_some() || custom.is_some_and(|known| known.contains(name))
    }) {
        view = view.text_size(UiTheme::read(cx).text(30.));
    }
    ShowMore::new((id, "more"), view)
}

/// Whether `body` is only emoji: glyphs or `:name:` codes `known` knows,
/// with nothing but spaces between them.
fn only_emoji(body: &str, known: &dyn Fn(&str) -> bool) -> bool {
    let body = body.trim();
    if body.is_empty() || body.len() > 400 {
        return false;
    }
    body.split_whitespace().all(|word| {
        let mut rest = word;
        while !rest.is_empty() {
            if let Some(after) = rest.strip_prefix(':')
                && let Some(end) = after.find(':')
                && end > 0
                && known(&after[..end])
            {
                rest = &after[end + 1..];
                continue;
            }
            let glyphs = rest.find(':').unwrap_or(rest.len());
            if glyphs == 0 || !emoji_glyphs(&rest[..glyphs]) {
                return false;
            }
            rest = &rest[glyphs..];
        }
        true
    })
}

/// Whether `text` is made of emoji characters and their joiners alone. A
/// digit, `#` or `*` counts only as part of a keycap.
fn emoji_glyphs(text: &str) -> bool {
    let keycap = text.contains('\u{20E3}');
    text.chars().all(|c| match u32::from(c) {
        0x30..=0x39 | 0x23 | 0x2A => keycap,
        0x200D | 0xFE0F | 0x20E3 | 0xE0020..=0xE007F => true,
        0xA9 | 0xAE | 0x203C | 0x2049 | 0x2122 | 0x2139 | 0x2194..=0x21AA => true,
        0x231A..=0x23FF | 0x24C2 | 0x25AA..=0x25FE | 0x2600..=0x27BF => true,
        0x2934 | 0x2935 | 0x2B05..=0x2B55 | 0x3030 | 0x303D | 0x3297 | 0x3299 => true,
        0x1F000..=0x1FAFF => true,
        _ => false,
    })
}

/// How many bodies' sources [`cached_source`] keeps before starting over:
/// a few screens of every open view.
const SOURCE_CACHE_LIMIT: usize = 4096;

/// Message bodies' Markdown sources, by body text.
#[derive(Default)]
struct SourceCache {
    /// The custom emoji list the sources were made with: its identity and
    /// generation.
    emoji: (usize, u64),
    /// The channel names the sources were made with, likewise.
    channels: (usize, u64),
    /// Whether the sources were made with Markdown on.
    formatting: bool,
    /// Whether the sources were made with emoticons as emoji.
    emoticons: bool,
    sources: std::collections::HashMap<SharedString, SharedString>,
}

thread_local! {
    static SOURCES: std::cell::RefCell<SourceCache> = std::cell::RefCell::default();
    static MARKED: std::cell::RefCell<std::collections::HashMap<(SharedString, u64), SharedString>> =
        std::cell::RefCell::default();
}

/// A body's Markdown `source` with `marks` highlighted, worked out once per
/// source and list of words.
fn marked_source(source: SharedString, marks: &Arc<[String]>) -> SharedString {
    use std::hash::{Hash as _, Hasher as _};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    marks.hash(&mut hasher);
    let key = (source.clone(), hasher.finish());
    MARKED.with_borrow_mut(|cache| {
        if let Some(marked) = cache.get(&key) {
            return marked.clone();
        }
        if cache.len() >= SOURCE_CACHE_LIMIT {
            cache.clear();
        }
        let marked: SharedString = matty_markdown::mark_words_in_source(&source, marks).into();
        cache.insert(key, marked.clone());
        marked
    })
}

/// [`body_source`] for a stored body, made once per body text and custom
/// emoji list (#788): a hash lookup a frame instead of the fence, emoji
/// and line-break passes. The same `SharedString` comes back each frame,
/// so the text view recognizes a long one by pointer.
fn cached_source(body: &SharedString, cx: &App) -> SharedString {
    let custom = cx.try_global::<crate::emoji::CustomEmoji>();
    let emoji = custom.map_or((0, 0), crate::emoji::CustomEmoji::identity);
    let channel_names = cx.try_global::<ChannelNames>();
    let channels = channel_names.map_or((0, 0), ChannelNames::identity);
    let (formatting, emoticons) =
        crate::settings::SettingsStore::try_global(cx).map_or((true, true), |store| {
            let settings = store.read(cx).get();
            (settings.formatting, settings.emoticons)
        });
    SOURCES.with_borrow_mut(|cache| {
        if cache.emoji != emoji
            || cache.channels != channels
            || cache.formatting != formatting
            || cache.emoticons != emoticons
            || cache.sources.len() >= SOURCE_CACHE_LIMIT
        {
            cache.emoji = emoji;
            cache.channels = channels;
            cache.formatting = formatting;
            cache.emoticons = emoticons;
            cache.sources.clear();
        }
        if let Some(source) = cache.sources.get(body) {
            return source.clone();
        }
        let is_custom = |name: &str| custom.is_some_and(|known| known.contains(name));
        let source: SharedString = if formatting {
            let source = body_source(body, emoticons, &is_custom);
            match channel_names {
                Some(names) => names.label(&source),
                None => source,
            }
        } else {
            plain_source(body)
        }
        .into();
        cache.sources.insert(body.clone(), source.clone());
        source
    })
}

/// The display names of the channels the user is in, by URL name, so a
/// `~town-square` mention reads `~Town Square` as in Mattermost.
#[derive(Clone, Default)]
pub struct ChannelNames(
    std::rc::Rc<std::cell::RefCell<std::collections::HashMap<String, String>>>,
    /// Bumped by each change, for the caches made from the names.
    std::rc::Rc<std::cell::Cell<u64>>,
);

impl gpui_kit::Global for ChannelNames {}

impl ChannelNames {
    /// Replaces the names, keeping the caches when nothing changed.
    pub fn set(&self, names: std::collections::HashMap<String, String>) {
        if *self.0.borrow() != names {
            *self.0.borrow_mut() = names;
            self.1.set(self.1.get() + 1);
        }
    }

    /// Which list this is and how often it changed.
    pub fn identity(&self) -> (usize, u64) {
        (std::rc::Rc::as_ptr(&self.0) as usize, self.1.get())
    }

    /// `source` with its channel mentions labeled by display name.
    pub(crate) fn label(&self, source: &str) -> String {
        let names = self.0.borrow();
        if names.is_empty() {
            return source.to_owned();
        }
        matty_markdown::label_channel_links(source, &|name| names.get(name).cloned())
    }
}

/// Opens a menu for a link right-clicked in a message, at the pointer.
pub type LinkMenu = std::sync::Arc<
    dyn Fn(&SharedString, gpui_kit::Point<gpui_kit::Pixels>, &mut Window, &mut App) + Send + Sync,
>;

/// The Markdown view of [`message_body`], without the Show more clip, for
/// text that changes as it is typed (the composer's preview).
pub fn markdown_body(
    id: impl Into<ElementId>,
    body: &str,
    image_sizes: &[(SharedString, u32, u32)],
    on_link: impl Fn(&SharedString, &mut Window, &mut App) + Clone + Send + Sync + 'static,
    on_link_menu: Option<LinkMenu>,
    cx: &App,
) -> MarkdownView {
    let custom = cx.try_global::<crate::emoji::CustomEmoji>();
    let is_custom = |name: &str| custom.is_some_and(|known| known.contains(name));
    let emoticons = crate::settings::SettingsStore::try_global(cx)
        .is_none_or(|store| store.read(cx).get().emoticons);
    let source = body_source(body, emoticons, &is_custom);
    let source = match cx.try_global::<ChannelNames>() {
        Some(names) => names.label(&source),
        None => source,
    }
    .into();
    markdown_from_source(
        id,
        source,
        image_sizes,
        on_link,
        on_link_menu,
        &OwnMentions::default(),
        cx,
    )
}

/// The Markdown view of a message whose source [`body_source`] made.
fn markdown_from_source(
    id: impl Into<ElementId>,
    source: SharedString,
    image_sizes: &[(SharedString, u32, u32)],
    on_link: impl Fn(&SharedString, &mut Window, &mut App) + Clone + Send + Sync + 'static,
    on_link_menu: Option<LinkMenu>,
    own: &OwnMentions,
    cx: &App,
) -> MarkdownView {
    // Table cells and headings are views of their own; they get the same
    // plugins and link handling as the message around them.
    let blocks = {
        let image_sizes = image_sizes.to_vec();
        let on_link = on_link.clone();
        let on_link_menu = on_link_menu.clone();
        let own = own.clone();
        MarkdownBlocks::new().inline(move |id, source, _, cx| {
            with_message_plugins(
                markdown(id, source, cx),
                &image_sizes,
                on_link.clone(),
                on_link_menu.clone(),
                &own,
            )
        })
    };
    with_message_plugins(
        markdown_with(id, source, blocks, cx),
        image_sizes,
        on_link,
        on_link_menu,
        own,
    )
}

/// `view` with a message's emoji, images, entity chips and link handling.
fn with_message_plugins(
    view: MarkdownView,
    image_sizes: &[(SharedString, u32, u32)],
    on_link: impl Fn(&SharedString, &mut Window, &mut App) + Clone + Send + Sync + 'static,
    on_link_menu: Option<LinkMenu>,
    own: &OwnMentions,
) -> MarkdownView {
    let chip = on_link.clone();
    let own = own.clone();
    let image_link = on_link.clone();
    view
        // Custom emoji as images sized to the text, from sync's media cache
        // (#153); until one lands, and for names the server does not know,
        // the text stays `:name:`.
        .plugin(CustomEmojiImages::new("emoji", |name, _, cx| {
            crate::emoji::custom_source(name, cx)
        }))
        // Inline images come from outside the server through sync and
        // the media cache (#159), in the box the server measured. One
        // still waiting under "Load remote images: Ask" shows a
        // click-to-load placeholder; a loaded one opens its link.
        .plugin(
            MarkdownImages::new(|url, _, cx| crate::remote_images::source(url, cx))
                .image_sizes(image_sizes.iter().cloned())
                .placeholder(|url, _, cx| crate::remote_images::placeholder(url, cx))
                .on_click(move |url, window, cx| image_link(url, window, cx)),
        )
        // Highlighted words: plain text with the mention fill, no link.
        .plugin(EntityLinks::new("mark").highlight(|_| true))
        .plugin(
            EntityLinks::new("matty")
                .on_click(move |target, window, cx| {
                    chip(&format!("matty:{target}").into(), window, cx)
                })
                .highlight(move |target| own.matches(target)),
        )
        // A right-click on a link opens its menu, never the link (#757);
        // a left or middle click, Enter or a tap opens it.
        .on_link_click(move |url, event, window, cx| match event {
            gpui_kit::ClickEvent::Mouse(click)
                if click.up.button == gpui_kit::MouseButton::Right =>
            {
                if let Some(menu) = &on_link_menu {
                    menu(url, click.up.position, window, cx);
                }
            }
            gpui_kit::ClickEvent::Mouse(click)
                if !matches!(
                    click.up.button,
                    gpui_kit::MouseButton::Left | gpui_kit::MouseButton::Middle
                ) => {}
            _ => on_link(url, window, cx),
        })
}

/// `body` with its entities as links, read the way Mattermost shows it:
/// each typed line break stays a line break, and a code fence with text
/// after it (```` ``` #783 ````) closes an otherwise unclosed block, and
/// `**` with spaces just inside still makes bold. Other
/// clients send emoji as `:name:` text (#677): system ones become their
/// glyph, the custom ones `is_custom` knows an image, and unknown names
/// stay text. With `emoticons`, `:)` and the like show as emoji too.
fn body_source(body: &str, emoticons: bool, is_custom: &dyn Fn(&str) -> bool) -> String {
    let body = matty_markdown::close_fences_with_trailing_text(body);
    let body = if emoticons {
        matty_markdown::emoticons_as_emoji_codes(&body)
    } else {
        body
    };
    let body = matty_markdown::mattermost_strong(&body);
    let linked = matty_markdown::link_entities_and_emoji_in_source(
        &body,
        &|name| crate::emoji::lookup(name).is_some() || is_custom(name),
        &|name| crate::emoji::lookup(name).map(str::to_owned),
    );
    matty_markdown::hard_line_breaks(&linked)
}

/// A body shown as typed, for the web app's "Enable post formatting" off:
/// Markdown punctuation is escaped so it reads literally, every line break
/// stays, and links still link.
fn plain_source(body: &str) -> String {
    let mut out = String::with_capacity(body.len() + body.len() / 8);
    for (index, line) in body.split('\n').enumerate() {
        if index > 0 {
            out.push_str("\\\n");
        }
        // Leading spaces would make a code block: they stay as spaces that
        // Markdown does not read.
        let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
        let (lead, line) = line.split_at(indent);
        for c in lead.chars() {
            out.push_str(if c == '\t' {
                "\u{a0}\u{a0}\u{a0}\u{a0}"
            } else {
                "\u{a0}"
            });
        }
        for (at, word) in line.split(' ').enumerate() {
            if at > 0 {
                out.push(' ');
            }
            if word.starts_with("http://") || word.starts_with("https://") {
                out.push_str(word);
                continue;
            }
            for c in word.chars() {
                // Colons stay, so :emoji: still shows.
                if c.is_ascii_punctuation() && c != ':' && c != '@' {
                    out.push('\\');
                }
                out.push(c);
            }
        }
    }
    out
}

/// What to do with the channel a link named, once found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Opening {
    /// The user is in it: open it.
    Open,
    /// A public channel the user is not in: offer to join.
    Join,
    /// A channel of a team the user is not on.
    OtherTeam,
    /// Private, archived, or otherwise not the user's to open.
    Closed,
}

/// Whether the user can open `channel` from a link: `member` says it is
/// in one of the user's teams' sidebars, `on_team` that the user is on its
/// team. A channel of another of the user's teams opens there, its team
/// shown first (#89).
pub fn opening(channel: &matty_api::models::Channel, member: bool, on_team: bool) -> Opening {
    if member {
        return Opening::Open;
    }
    if !on_team {
        return Opening::OtherTeam;
    }
    if channel.kind == "O" && channel.delete_at == 0 {
        Opening::Join
    } else {
        Opening::Closed
    }
}

/// A public channel a link named that the user is not in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JoinOffer {
    pub channel_id: SharedString,
    pub name: SharedString,
    /// The permalink's post, to jump to once joined.
    pub post: Option<SharedString>,
}

/// "You're not in #name": gpuicn's alert dialog, kept mounted while closed. `on_join`
/// runs when the user chooses Join.
pub fn join_dialog(
    handle: &DialogHandle,
    offer: Option<&JoinOffer>,
    on_join: impl Fn(&mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let theme = UiTheme::read(cx).clone();
    let title = offer.map_or_else(
        || t!("link-join-title-unknown"),
        |offer| t!("link-join-title", name = offer.name.to_string()),
    );
    let what = if offer.is_some_and(|offer| offer.post.is_some()) {
        t!("link-join-post")
    } else {
        t!("link-join-channel")
    };
    let popup = dialog_popup("link.join.popup", t!("link-join"), cx)
        .child(dialog_title("link.join.title", cx).child(title))
        .child(dialog_description("link.join.description", cx).child(what))
        .child(
            div()
                .flex()
                .justify_end()
                .gap(theme.space(2.))
                .pt(theme.space(2.))
                .child(
                    alert_dialog_cancel("link.join.cancel")
                        .variant(ButtonVariant::Outline)
                        .label(t!("link-join-cancel")),
                )
                .child(alert_dialog_action("link.join.confirm").label(t!("link-join"))),
        );
    alert_dialog("link.join", handle, popup, window, cx).on_ok(move |_, window, cx| {
        on_join(window, cx);
        true
    })
}

/// Whether the switcher's query is a link worth offering to open.
pub fn looks_like_link(query: &str) -> bool {
    let query = query.trim();
    !query.contains(char::is_whitespace)
        && url::Url::parse(query).is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_message_of_only_emoji_is_told_apart() {
        let known = |name: &str| ["tada", "partyparrot", "+1"].contains(&name);
        for body in [
            "🎉",
            " 🎉 🎉 ",
            ":tada:",
            ":tada::+1: 👍🏽",
            "❤️",
            "👨‍👩‍👧",
            "1️⃣",
            ":partyparrot:",
        ] {
            assert!(only_emoji(body, &known), "{body:?}");
        }
        for body in ["", "hi 🎉", "🎉!", ":nope:", "1", ":tada: ok"] {
            assert!(!only_emoji(body, &known), "{body:?}");
        }
    }

    use super::*;

    const SERVER: &str = "https://chat.example.com";

    /// Only the viewer's own mentions get the filled chip (as in
    /// Mattermost); `@here` and friends only when they notify the viewer.
    #[test]
    fn own_mentions_match_the_viewer_only() {
        let own = OwnMentions {
            username: "ada".into(),
            channel_wide: true,
            groups: Arc::from(["developers".to_owned()]),
            marks: Arc::from([]),
        };
        assert!(own.matches("//user/ada"));
        assert!(own.matches("//user/Developers"), "a group the viewer is in");
        assert!(!own.matches("//user/design-team"));
        assert!(own.matches("//user/Ada"));
        assert!(!own.matches("//user/sam"));
        assert!(!own.matches("//user/adam"));
        assert!(own.matches("//special/here"));
        assert!(!own.matches("//channel/ada"));
        let quiet = OwnMentions {
            channel_wide: false,
            ..own.clone()
        };
        assert!(!quiet.matches("//special/channel"));
        assert!(!OwnMentions::default().matches("//user/"));
    }

    #[test]
    fn keywords_and_the_first_name_are_marked_but_at_keywords_are_not() {
        let keys = matty_notify::MentionKeys {
            username: "ada".into(),
            first_name: Some("Ada".into()),
            keywords: vec!["deploy".into(), "@oncall".into()],
            highlight: vec!["incident".into()],
            ..Default::default()
        };
        let own = OwnMentions::of(&keys).with_marks(["staging*".to_owned()]);
        assert_eq!(&*own.marks, ["Ada", "deploy", "incident", "staging*"]);
    }
    const TEAMS: &[&str] = &["matty", "design-guild"];

    fn go(url: &str) -> LinkTarget {
        route(url, Some(SERVER), TEAMS)
    }

    #[test]
    fn entities() {
        assert_eq!(
            go("matty://channel/town-square"),
            LinkTarget::Channel {
                team: None,
                name: "town-square".into()
            }
        );
        assert_eq!(go("matty://user/ana"), LinkTarget::User("ana".into()));
        assert_eq!(
            go("matty://hashtag/release"),
            LinkTarget::Hashtag("release".into())
        );
        assert_eq!(go("matty://special/here"), LinkTarget::Nothing);
        assert_eq!(go("matty://emoji/tada"), LinkTarget::Nothing);
        assert_eq!(go("matty://user/"), LinkTarget::Nothing);
        assert_eq!(
            go("matty://add-to-channel/c1/u1,u2"),
            LinkTarget::AddToChannel {
                channel_id: "c1".into(),
                user_ids: vec!["u1".into(), "u2".into()],
            }
        );
        assert_eq!(go("matty://add-to-channel/c1/"), LinkTarget::Nothing);
    }

    #[test]
    fn permalinks_on_this_server() {
        let post = "8xk4f1qjtbyzjm7ffwz3ipk3ya";
        assert_eq!(
            go(&format!("{SERVER}/matty/pl/{post}")),
            LinkTarget::Permalink {
                team: "matty".into(),
                post_id: post.into()
            }
        );
        // Any team: the post id alone finds the post.
        assert_eq!(
            go(&format!("{SERVER}/elsewhere/pl/{post}?x=1#y")),
            LinkTarget::Permalink {
                team: "elsewhere".into(),
                post_id: post.into()
            }
        );
    }

    #[test]
    fn channel_and_message_urls() {
        assert_eq!(
            go(&format!("{SERVER}/design-guild/channels/reviews")),
            LinkTarget::Channel {
                team: Some("design-guild".into()),
                name: "reviews".into()
            }
        );
        assert_eq!(
            go(&format!("{SERVER}/matty/messages/@ana")),
            LinkTarget::User("ana".into())
        );
        // A team the user is not in: the browser shows it.
        let other = format!("{SERVER}/other/channels/reviews");
        assert_eq!(go(&other), LinkTarget::External(other.clone()));
    }

    #[test]
    fn a_server_under_a_path() {
        let server = "https://example.com/mattermost/";
        assert_eq!(
            route(
                "https://example.com/mattermost/matty/pl/abc123",
                Some(server),
                TEAMS
            ),
            LinkTarget::Permalink {
                team: "matty".into(),
                post_id: "abc123".into()
            }
        );
        let outside = "https://example.com/matty/pl/abc123";
        assert_eq!(
            route(outside, Some(server), TEAMS),
            LinkTarget::External(outside.into())
        );
    }

    #[test]
    fn everything_else_opens_in_the_browser() {
        for url in [
            "https://github.com/karyalabs/matty",
            "http://chat.example.com/matty/pl/abc",
            "https://chat.example.com:8443/matty/pl/abc",
            "https://chat.example.com/matty/pl/abc/extra",
            "https://chat.example.com/matty/pl/a%20b",
            "https://chat.example.com/matty",
            "mailto:ana@example.com",
        ] {
            assert_eq!(go(url), LinkTarget::External(url.into()), "{url}");
        }
        // Signed out: no server to compare with.
        let link = "https://chat.example.com/matty/pl/abc";
        assert_eq!(route(link, None, TEAMS), LinkTarget::External(link.into()));
    }

    #[test]
    fn openings() {
        let channel = |kind: &str, team: &str, delete_at: i64| matty_api::models::Channel {
            kind: kind.into(),
            team_id: team.into(),
            delete_at,
            ..Default::default()
        };
        let shown = true;
        assert_eq!(opening(&channel("O", "t1", 0), true, shown), Opening::Open);
        assert_eq!(opening(&channel("O", "t1", 0), false, shown), Opening::Join);
        // Another of the user's teams: open or join there.
        assert_eq!(opening(&channel("O", "t2", 0), true, shown), Opening::Open);
        assert_eq!(opening(&channel("O", "t2", 0), false, shown), Opening::Join);
        assert_eq!(
            opening(&channel("O", "t3", 0), false, false),
            Opening::OtherTeam
        );
        assert_eq!(
            opening(&channel("P", "t1", 0), false, shown),
            Opening::Closed
        );
        assert_eq!(
            opening(&channel("O", "t1", 5), false, shown),
            Opening::Closed
        );
        assert_eq!(opening(&channel("D", "", 0), false, shown), Opening::Closed);
    }

    #[test]
    fn plain_bodies_read_as_typed() {
        let parsed =
            |body: &str| matty_markdown::to_plain(&matty_markdown::parse(&plain_source(body)));
        assert_eq!(
            parsed("**bold** and `code`\n- not a list"),
            "**bold** and `code`\n- not a list"
        );
        assert_eq!(
            plain_source("see https://a.com/x_y"),
            "see https://a.com/x_y"
        );
        assert!(!plain_source("    indented").starts_with(' '));
    }

    #[test]
    fn switcher_links() {
        assert!(looks_like_link("  https://chat.example.com/matty/pl/abc "));
        assert!(!looks_like_link("design"));
        assert!(!looks_like_link("https://a.com b"));
        assert!(!looks_like_link("ftp://a.com"));
    }

    #[test]
    fn system_emoji_shortcodes_show_as_emoji() {
        let custom = |name: &str| name == "partyparrot";
        assert_eq!(
            body_source(
                ":clown_face: :+1_medium_skin_tone: :thumbsup: :partyparrot: :nope:",
                true,
                &custom
            ),
            "🤡 👍🏽 👍 ![:partyparrot:](emoji:partyparrot) :nope:"
        );
    }

    #[test]
    fn emoticons_show_as_emoji_unless_turned_off() {
        assert_eq!(
            body_source("nice :) <3 `:)`", true, &|_| false),
            "nice 🙂 ❤️ `:)`"
        );
        assert_eq!(body_source("nice :) <3", false, &|_| false), "nice :) <3");
    }

    #[test]
    fn bold_closed_on_the_next_line_is_bold() {
        assert_eq!(
            body_source("**Release notes\n** 1. Faster search", true, &|_| false),
            "**Release notes**\\\n 1\\. Faster search"
        );
        assert_eq!(
            crate::channel::plain_text("**Release notes\n** 1. Faster search"),
            "Release notes\n1. Faster search"
        );
    }

    /// #788: a stored body's source is made once and handed back as the
    /// same string, until the custom emoji list changes.
    #[gpui_kit::test]
    fn a_bodys_source_is_made_once(cx: &mut gpui_kit::TestAppContext) {
        let emoji = crate::emoji::CustomEmoji::default();
        cx.update(|cx| cx.set_global(emoji.clone()));
        // Long enough to live on the heap rather than inline.
        let body: SharedString = format!("hi :partyparrot: {}", "and more ".repeat(8)).into();
        let (first, second) = cx.update(|cx| (cached_source(&body, cx), cached_source(&body, cx)));
        assert!(first.starts_with("hi :partyparrot: and more"));
        assert_eq!(
            first.as_ptr(),
            second.as_ptr(),
            "the same string, not a copy"
        );
        emoji.set(&[matty_api::models::Emoji {
            name: "partyparrot".into(),
            ..Default::default()
        }]);
        let third = cx.update(|cx| cached_source(&body, cx));
        assert!(third.starts_with("hi ![:partyparrot:](emoji:partyparrot) and more"));
    }

    /// A message body drawing one Markdown image, for the tests below.
    struct Body;

    impl gpui_kit::Render for Body {
        fn render(
            &mut self,
            _: &mut gpui_kit::Window,
            cx: &mut gpui_kit::Context<Self>,
        ) -> impl gpui_kit::IntoElement {
            use gpui_kit::{ParentElement as _, Styled as _};
            gpui_kit::div().w(gpui_kit::px(800.)).child(message_body(
                "body",
                &"![chart](https://cdn.example.com/chart.png)".into(),
                &[("https://cdn.example.com/chart.png".into(), 1200, 600)],
                |_, _, _| {},
                None,
                &OwnMentions::default(),
                cx,
            ))
        }
    }

    /// #159: under "Load remote images: Ask" an inline image is a
    /// click-to-load placeholder in the box the server measured, and a
    /// click loads it.
    #[gpui_kit::test]
    fn an_inline_image_waits_in_its_own_box(cx: &mut gpui_kit::TestAppContext) {
        let fetched = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        let log = fetched.clone();
        cx.update(|cx| {
            crate::ui::theme::init(cx);
            crate::settings::SettingsStore::init(None, cx);
            cx.set_global(crate::remote_images::RemoteImages::new(
                std::rc::Rc::new(move |url: &str| log.borrow_mut().push(url.to_owned())),
                true,
            ));
        });
        let window = cx.add_window(|_, _| Body);
        let mut cx = gpui_kit::VisualTestContext::from_window(window.into(), cx);
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let placeholder = cx
            .debug_bounds("markdown-image-placeholder")
            .expect("a placeholder, not the alt text");
        // 1200 by 600, scaled to fit 360 by 200.
        assert_eq!(
            (placeholder.size.width, placeholder.size.height),
            (gpui_kit::px(360.), gpui_kit::px(180.))
        );
        assert!(fetched.borrow().is_empty(), "nothing loads on its own");
        cx.simulate_click(placeholder.center(), Default::default());
        assert_eq!(*fetched.borrow(), ["https://cdn.example.com/chart.png"]);
    }
}
