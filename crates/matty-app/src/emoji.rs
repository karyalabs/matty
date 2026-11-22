//! Matty's emoji catalog for gpuicn's picker (PLAN §3 M1 "Reactions"):
//! the Mattermost webapp's own list (`emoji.tsv`, from
//! `scripts/import-emoji.py`), so every name, alias and skin tone the
//! official client sends means the same emoji here. Custom emoji join once
//! sync loads them from the server.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

use gpui_icons::LucideIcon;
use gpui_kit::{App, Context, Global, Image, ImageSource, SharedString};

use crate::avatars::EmojiImages;
use crate::settings::SettingsStore;
use crate::t;
use crate::ui::emoji_picker::{Emoji, EmojiCategory, EmojiPickerState};
use crate::ui::message::Reaction;

/// How many recently picked emoji the picker remembers.
pub const RECENT_LIMIT: usize = 27;

/// One emoji of the picker, as the webapp lists it.
struct Entry {
    category: &'static str,
    glyph: &'static str,
    /// Mattermost's names for it; the first is the one reactions use.
    names: Vec<&'static str>,
    /// Its Unicode name, which the picker's search also matches.
    label: &'static str,
    /// Its five single-tone versions' names, lightest first, or none.
    tones: Vec<&'static str>,
}

struct Catalog {
    entries: Vec<Entry>,
    /// Every name, toned ones included, to its glyph.
    glyphs: HashMap<&'static str, &'static str>,
    /// Every glyph, without its variation selectors, to its first name.
    names: HashMap<String, &'static str>,
    /// A toned version's name to the name of its default-tone emoji.
    untoned: HashMap<&'static str, &'static str>,
}

fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| parse(include_str!("emoji.tsv")))
}

fn parse(source: &'static str) -> Catalog {
    let mut catalog = Catalog {
        entries: Vec::new(),
        glyphs: HashMap::new(),
        names: HashMap::new(),
        untoned: HashMap::new(),
    };
    for line in source.lines().filter(|line| !line.starts_with('#')) {
        let fields: Vec<&'static str> = line.split('\t').collect();
        let (glyph, names) = match fields.as_slice() {
            ["E", category, glyph, names, label, tones] => {
                let names: Vec<&'static str> = names.split(' ').collect();
                let tones: Vec<&'static str> = tones.split(' ').filter(|t| !t.is_empty()).collect();
                if let Some(first) = names.first() {
                    for tone in &tones {
                        catalog.untoned.insert(tone, first);
                    }
                }
                catalog.entries.push(Entry {
                    category,
                    glyph,
                    names: names.clone(),
                    label,
                    tones,
                });
                (*glyph, names)
            }
            ["T", glyph, names] => (*glyph, names.split(' ').collect()),
            _ => continue,
        };
        for name in &names {
            catalog.glyphs.entry(name).or_insert(glyph);
        }
        if let Some(first) = names.first() {
            catalog.names.entry(plain(glyph)).or_insert(first);
        }
    }
    catalog
}

/// `glyph` without variation selectors, which some clients send and
/// others leave out (❤️ and ❤ are the same heart).
fn plain(glyph: &str) -> String {
    glyph
        .chars()
        .filter(|c| *c != '\u{fe0f}' && *c != '\u{fe0e}')
        .collect()
}

/// The picker's categories, in tab order.
pub fn categories() -> Vec<EmojiCategory> {
    let groups = [
        (
            "smileys-emotion",
            t!("emoji-group-smileys"),
            LucideIcon::FaceSlightlySmiling,
        ),
        ("people-body", t!("emoji-group-people"), LucideIcon::Hand),
        (
            "animals-nature",
            t!("emoji-group-animals"),
            LucideIcon::Leaf,
        ),
        ("food-drink", t!("emoji-group-food"), LucideIcon::Coffee),
        ("travel-places", t!("emoji-group-travel"), LucideIcon::Plane),
        (
            "activities",
            t!("emoji-group-activities"),
            LucideIcon::Trophy,
        ),
        ("objects", t!("emoji-group-objects"), LucideIcon::Lightbulb),
        ("symbols", t!("emoji-group-symbols"), LucideIcon::Heart),
        ("flags", t!("emoji-group-flags"), LucideIcon::Flag),
    ];
    let catalog = catalog();
    groups
        .into_iter()
        .map(|(group, name, icon)| {
            let emoji = catalog
                .entries
                .iter()
                .filter(|entry| entry.category == group)
                .filter_map(convert);
            EmojiCategory::new(name, icon, emoji)
        })
        .collect()
}

/// Shortcodes for the composer's `:` autocomplete, as `(name, glyph)`
/// pairs: the `frequent` ones that match first, in the picker's order,
/// then those starting with `query`, then those containing it.
pub fn suggest(
    query: &str,
    limit: usize,
    frequent: &[SharedString],
) -> Vec<(SharedString, SharedString)> {
    let query = query.to_lowercase();
    if query.len() < 2 {
        return Vec::new();
    }
    let used: Vec<(SharedString, SharedString)> = frequent
        .iter()
        .filter(|name| name.contains(&query))
        .filter_map(|name| Some((name.clone(), glyph(name)?)))
        .collect();
    let mut starts = used.clone();
    let mut contains = Vec::new();
    for entry in &catalog().entries {
        let Some(name) = entry.names.iter().find(|name| name.contains(&query)) else {
            continue;
        };
        if used.iter().any(|(seen, _)| seen == name) {
            continue;
        }
        let pair = (SharedString::from(*name), SharedString::from(entry.glyph));
        if name.starts_with(&query) {
            starts.push(pair);
        } else {
            contains.push(pair);
        }
        if starts.len() >= limit {
            break;
        }
    }
    starts.extend(contains);
    starts.truncate(limit);
    starts
}

/// The glyph behind a shortcode, for reactions and the composer.
pub fn glyph(name: &str) -> Option<SharedString> {
    lookup(name).map(SharedString::from)
}

/// `text` with each `:shortcode:` it knows replaced by its glyph, as a
/// notification shows a message; unknown names, such as custom emoji, stay.
pub fn with_glyphs(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(':') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let name_end = after.find(|c: char| !(c.is_alphanumeric() || "_+-".contains(c)));
        match name_end {
            Some(end) if end > 0 && after[end..].starts_with(':') => match lookup(&after[..end]) {
                Some(glyph) => {
                    out.push_str(glyph);
                    rest = &after[end + 1..];
                }
                None => {
                    out.push(':');
                    rest = after;
                }
            },
            _ => {
                out.push(':');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The Mattermost name behind a reaction value: a glyph's shortcode, or
/// `name` for a custom emoji's `:name:`.
pub fn name_for(emoji: &str) -> Option<String> {
    if let Some(name) = emoji.strip_prefix(':').and_then(|e| e.strip_suffix(':')) {
        return Some(name.to_owned());
    }
    name_of(emoji).map(str::to_owned)
}

/// A reaction value as Mattermost writes it in "reacted with :tada:".
pub fn shortcode(emoji: &str) -> String {
    match name_for(emoji) {
        Some(name) => format!(":{name}:"),
        None => emoji.to_owned(),
    }
}

/// The glyph a Mattermost name stands for, `+1_medium_skin_tone` and
/// aliases such as `thumbsup` included.
pub fn lookup(name: &str) -> Option<&'static str> {
    catalog().glyphs.get(name).copied()
}

/// The Mattermost name for a glyph, with its skin tone's suffix when it
/// has one.
pub fn name_of(glyph: &str) -> Option<&'static str> {
    catalog().names.get(&plain(glyph)).copied()
}

/// The emoji picked most, for the picker's first row and the top of the
/// `:` autocomplete (#209).
pub fn frequent(cx: &App) -> Vec<SharedString> {
    SettingsStore::global(cx)
        .read(cx)
        .get()
        .emoji_usage
        .iter()
        .take(RECENT_LIMIT)
        .map(|seen| SharedString::from(seen.name.clone()))
        .collect()
}

/// Counts a pick of `name` for [`frequent`], under its default tone.
pub fn record_pick(name: &str, cx: &mut App) {
    let name = base_name(name);
    SettingsStore::global(cx).update(cx, |store, cx| {
        store.update(cx, |settings| settings.count_emoji(&name))
    });
}

/// Keeps the tone the user chose in a picker, for every picker.
pub fn save_skin_tone(tone: Option<usize>, cx: &mut App) {
    SettingsStore::global(cx).update(cx, |store, cx| {
        store.update(cx, |settings| settings.skin_tone = tone)
    });
}

/// Brings a picker up to date as it opens: the saved skin tone, the most
/// used emoji first and the server's custom emoji.
pub fn prepare_picker(picker: &mut EmojiPickerState, cx: &mut Context<EmojiPickerState>) {
    let tone = SettingsStore::global(cx).read(cx).get().skin_tone;
    picker.set_skin_tone(tone, cx);
    picker.set_recents(frequent(cx), cx);
    custom_tab(picker, cx);
}

/// `name` in the default tone: `+1_medium_skin_tone` counts as `+1`.
fn base_name(name: &str) -> String {
    // An alias counts as the name reactions use.
    let name = lookup(name).and_then(name_of).unwrap_or(name);
    catalog()
        .untoned
        .get(name)
        .copied()
        .unwrap_or(name)
        .to_owned()
}

/// The server's custom emoji, name to id and creator (#153), as an app
/// global the views read while rendering. [`crate::live::Live`] fills it
/// from sync.
#[derive(Clone, Default)]
pub struct CustomEmoji(
    Rc<RefCell<HashMap<String, (String, String)>>>,
    /// Bumped by each [`Self::set`], for caches made from the list.
    Rc<std::cell::Cell<u64>>,
);

impl Global for CustomEmoji {}

impl CustomEmoji {
    /// Replaces the list with `emoji`.
    pub fn set(&self, emoji: &[matty_api::models::Emoji]) {
        *self.0.borrow_mut() = emoji
            .iter()
            .map(|one| (one.name.clone(), (one.id.clone(), one.creator_id.clone())))
            .collect();
        self.1.set(self.1.get() + 1);
    }

    /// Which list this is and how often it was replaced: two equal
    /// identities saw the same emoji.
    pub fn identity(&self) -> (usize, u64) {
        (Rc::as_ptr(&self.0) as usize, self.1.get())
    }

    /// The id of the custom emoji called `name`.
    pub fn id(&self, name: &str) -> Option<String> {
        self.0.borrow().get(name).map(|(id, _)| id.clone())
    }

    /// Who added the custom emoji called `name`.
    pub fn creator(&self, name: &str) -> Option<String> {
        self.0
            .borrow()
            .get(name)
            .map(|(_, creator)| creator.clone())
    }

    /// Every custom emoji's name, A to Z.
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.0.borrow().keys().cloned().collect();
        names.sort_unstable();
        names
    }

    /// Whether `name` is one of the server's custom emoji.
    pub fn contains(&self, name: &str) -> bool {
        self.0.borrow().contains_key(name)
    }
}

/// The image for the custom emoji called `name`, asking sync for it the
/// first time. `None` until it lands, and for names that are not custom
/// emoji, so the view shows `:name:` meanwhile.
pub fn custom_image(name: &str, cx: &App) -> Option<Arc<Image>> {
    let id = cx.try_global::<CustomEmoji>()?.id(name)?;
    cx.try_global::<EmojiImages>()?.image(&id)
}

/// The custom emoji called `name` as a source that draws once its image
/// lands, for the picker's grid, which is built before the images are.
pub fn lazy_source(name: &str) -> ImageSource {
    let name = name.to_owned();
    ImageSource::Custom(Arc::new(move |window, cx| {
        custom_image(&name, cx)?
            .use_render_image(window, cx)
            .map(Ok)
    }))
}

/// Puts the server's custom emoji in `picker`'s Custom tab (#209).
pub fn custom_tab(picker: &mut EmojiPickerState, cx: &mut Context<EmojiPickerState>) {
    let names = cx
        .try_global::<CustomEmoji>()
        .map(CustomEmoji::names)
        .unwrap_or_default();
    picker.set_custom(
        names
            .into_iter()
            .map(|name| (SharedString::from(name.clone()), lazy_source(&name))),
        cx,
    );
}

/// [`custom_image`] as a source for gpuicn's views.
pub fn custom_source(name: &str, cx: &App) -> Option<ImageSource> {
    custom_image(name, cx).map(ImageSource::Image)
}

/// A reaction chip for `emoji` (a glyph, or `:name:` for a custom emoji),
/// with the custom emoji's image once it has one.
pub fn reaction(emoji: &SharedString, count: usize, mine: bool, cx: &App) -> Reaction {
    let chip = Reaction::new(emoji.clone(), count, mine);
    let custom = emoji
        .strip_prefix(':')
        .and_then(|name| name.strip_suffix(':'))
        .and_then(|name| custom_image(name, cx));
    match custom {
        Some(image) => chip.image(image),
        None => chip,
    }
}

fn convert(entry: &Entry) -> Option<Emoji> {
    let (name, aliases) = entry.names.split_first()?;
    let keywords = aliases
        .iter()
        .map(|alias| alias.to_string())
        .chain(std::iter::once(entry.label.to_string()));
    let tones = entry
        .tones
        .iter()
        .filter_map(|tone| Some(Emoji::new(*tone, lookup(tone)?)));
    Some(
        Emoji::new(*name, entry.glyph)
            .keywords(keywords)
            .skin_tones(tones),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tones_count_as_their_default_and_used_emoji_suggest_first() {
        assert_eq!(base_name("+1_medium_skin_tone"), "+1");
        assert_eq!(glyph("+1_dark_skin_tone").as_deref(), Some("👍🏿"));
        assert_eq!(name_of("👍🏼"), Some("+1_medium_light_skin_tone"));
        assert_eq!(name_of("👍"), Some("+1"));
        let hand = categories()
            .into_iter()
            .flat_map(|category| category.emoji)
            .find(|emoji| emoji.name == "wave")
            .unwrap();
        assert_eq!(hand.in_tone(Some(0)).name, "wave_light_skin_tone");
        assert_eq!(base_name("joy"), "joy");
        assert_eq!(base_name("not_an_emoji"), "not_an_emoji");
        let names = |frequent: &[SharedString]| {
            suggest("smil", 3, frequent)
                .into_iter()
                .map(|(name, _)| name.to_string())
                .collect::<Vec<_>>()
        };
        let plain = names(&[]);
        assert!(!plain.contains(&"smiley_cat".to_string()));
        let ranked = names(&["tada".into(), "smiley_cat".into()]);
        assert_eq!(ranked[0], "smiley_cat");
        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked.iter().filter(|n| *n == "smiley_cat").count(), 1);
    }

    #[test]
    fn mattermost_names_find_their_emoji() {
        for (name, glyph) in [
            ("clown_face", "🤡"),
            ("slightly_smiling_face", "🙂"),
            ("thumbsup", "👍"),
            ("+1", "👍"),
            ("flag-us", "🇺🇸"),
            ("us", "🇺🇸"),
        ] {
            assert_eq!(lookup(name), Some(glyph), "{name}");
        }
        assert!(lookup("not_an_emoji").is_none());
        // The names the official client gives reactions (#973).
        for (name, glyph) in [
            ("badminton_racquet_and_shuttlecock", "🏸"),
            ("table_tennis_paddle_and_ball", "🏓"),
            ("man-biking", "🚴\u{200d}♂\u{fe0f}"),
            (
                "people_holding_hands_medium_skin_tone_medium_skin_tone",
                "🧑🏽\u{200d}🤝\u{200d}🧑🏽",
            ),
        ] {
            assert_eq!(lookup(name), Some(glyph), "{name}");
        }
        assert_eq!(name_of("🏸"), Some("badminton_racquet_and_shuttlecock"));
        assert_eq!(name_of("❤"), Some("heart"), "with or without U+FE0F");
        assert_eq!(base_name("thumbsup_dark_skin_tone"), "+1");
    }

    #[test]
    fn catalog_has_the_quick_reactions() {
        let categories = categories();
        assert_eq!(categories.len(), 9);
        let names: Vec<String> = categories
            .iter()
            .flat_map(|category| category.emoji.iter().map(|emoji| emoji.name.to_string()))
            .collect();
        for name in ["+1", "heart", "tada", "eyes", "rocket", "joy"] {
            assert!(names.iter().any(|n| n == name), "missing {name}");
        }
        assert_eq!(glyph("tada").as_deref(), Some("🎉"));
        let names: Vec<_> = suggest("tad", 8, &[]).into_iter().map(|(n, _)| n).collect();
        assert_eq!(names.first().map(|n| n.as_ref()), Some("tada"));
        assert!(
            suggest("t", 8, &[]).is_empty(),
            "one letter is too little to go on"
        );
        assert_eq!(glyph("not-an-emoji"), None);
    }

    #[test]
    fn shortcodes_become_glyphs_and_the_rest_stays() {
        assert_eq!(with_glyphs("ship it :tada: now"), "ship it 🎉 now");
        assert_eq!(with_glyphs(":+1::+1:"), "👍👍");
        assert_eq!(
            with_glyphs("at 10:30: :party_parrot:"),
            "at 10:30: :party_parrot:"
        );
        assert_eq!(with_glyphs("a: b"), "a: b");
        assert_eq!(with_glyphs("ends with :"), "ends with :");
    }
}
