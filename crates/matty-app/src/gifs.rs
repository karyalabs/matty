//! The composer's GIF picker (#958), as the official client offers it:
//! with the server's `EnableGifPicker` on, the composer's emoji picker has
//! an Emojis and a GIFs tab. GIFs come from GIPHY with the server's key:
//! trending ones until the person types, then GIPHY's search, a page at a
//! time as the grid scrolls. Picking one puts `![title](url)` in the
//! composer, which every Mattermost client draws as the animated GIF.
//!
//! One [`GifSearch`] serves every composer, as only one picker is open at
//! a time; each composer keeps a [`ComposerGifs`] with its own picker.

use std::rc::Rc;
use std::time::Duration;

use gpui_kit::{
    App, AppContext as _, Context, Entity, EventEmitter, Global, IntoElement, ParentElement as _,
    SharedString, Styled as _, Subscription, Task, Window, div,
};
use matty_api::giphy::{Gif, GifPage};

use crate::t;
use crate::ui::{
    emoji_picker::EmojiPicker,
    gif_picker::{GifPicker, GifPickerEvent, GifPickerState},
    tabs::{Tab, Tabs, TabsVariant},
    theme::UiTheme,
};

/// How long typing pauses before GIPHY is asked, as GIPHY's grid waits.
const SETTLE: Duration = Duration::from_millis(300);

/// What a search is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    Loading,
    Failed,
}

/// Sent to sync, which asks GIPHY (`Command::SearchGifs`).
#[derive(Clone, Debug, PartialEq)]
pub struct Ask {
    /// The search text; empty for trending GIFs.
    pub query: String,
    /// Where the page starts.
    pub offset: u32,
}

/// The GIFs for the open picker: one search, its pages so far.
pub struct GifSearch {
    query: Option<String>,
    gifs: Vec<Gif>,
    next: Option<u32>,
    phase: Phase,
    /// Counts searches, so a composer knows when the list was replaced
    /// rather than added to.
    generation: u64,
    settle: Option<Task<()>>,
}

impl EventEmitter<Ask> for GifSearch {}

struct Shared(Entity<GifSearch>);

impl Global for Shared {}

impl GifSearch {
    /// The app's search, made on first use.
    pub fn global(cx: &mut App) -> Entity<GifSearch> {
        if let Some(shared) = cx.try_global::<Shared>() {
            return shared.0.clone();
        }
        let search = cx.new(|_| GifSearch {
            query: None,
            gifs: Vec::new(),
            next: None,
            phase: Phase::Idle,
            generation: 0,
            settle: None,
        });
        cx.set_global(Shared(search.clone()));
        search
    }

    /// Looks `query` up once typing settles; the same text again keeps
    /// what is already here.
    pub fn search(&mut self, query: &str, cx: &mut Context<Self>) {
        let query = query.trim().to_string();
        if self.query.as_deref() == Some(query.as_str()) && self.phase != Phase::Failed {
            return;
        }
        self.query = Some(query.clone());
        self.gifs.clear();
        self.next = None;
        self.phase = Phase::Loading;
        self.generation += 1;
        let wait = !query.is_empty();
        cx.notify();
        if !wait {
            self.settle = None;
            cx.emit(Ask { query, offset: 0 });
            return;
        }
        self.settle = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SETTLE).await;
            this.update(cx, |this, cx| {
                if this.query.as_deref() == Some(query.as_str()) {
                    cx.emit(Ask { query, offset: 0 });
                }
            })
            .ok();
        }));
    }

    /// Asks for the next page, if there is one.
    pub fn more(&mut self, cx: &mut Context<Self>) {
        let (Some(query), Some(offset)) = (self.query.clone(), self.next) else {
            return;
        };
        if self.phase == Phase::Loading {
            return;
        }
        self.phase = Phase::Loading;
        cx.emit(Ask { query, offset });
        cx.notify();
    }

    /// GIPHY's answer to an [`Ask`]; one for an older search is dropped.
    pub fn answered(
        &mut self,
        query: &str,
        offset: u32,
        page: Option<&GifPage>,
        cx: &mut Context<Self>,
    ) {
        if self.query.as_deref() != Some(query) {
            return;
        }
        match page {
            Some(page) => {
                if offset == 0 {
                    self.gifs.clear();
                }
                let known: std::collections::HashSet<String> =
                    self.gifs.iter().map(|gif| gif.id.clone()).collect();
                self.gifs.extend(
                    page.gifs
                        .iter()
                        .filter(|gif| !known.contains(&gif.id))
                        .cloned(),
                );
                self.next = page.next;
                self.phase = Phase::Idle;
            }
            None => {
                self.next = None;
                self.phase = Phase::Failed;
            }
        }
        cx.notify();
    }

    /// Whether `url` is one of the previews shown, so its arrival redraws
    /// the picker.
    pub fn shows(&self, url: &str) -> bool {
        self.gifs.iter().any(|gif| gif.preview == url)
    }

    /// Forgets everything, for another server or signing out.
    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.query = None;
        self.gifs.clear();
        self.next = None;
        self.phase = Phase::Idle;
        self.generation += 1;
        self.settle = None;
        cx.notify();
    }

    fn gif(&self, id: &str) -> Option<&Gif> {
        self.gifs.iter().find(|gif| gif.id == id)
    }
}

/// The picker's tile for `gif`: its preview once it has landed. The person
/// asked for GIFs by opening the tab, so "Load remote images: Ask" does
/// not hold the previews back.
fn tile(gif: &Gif, cx: &App) -> crate::ui::gif_picker::Gif {
    let tile = crate::ui::gif_picker::Gif::new(
        gif.id.clone(),
        gif.title.clone(),
        gif.width as f32,
        gif.height as f32,
    );
    match preview(&gif.preview, cx) {
        Some(image) => tile.image(image),
        None => tile,
    }
}

fn preview(url: &str, cx: &App) -> Option<gpui_kit::ImageSource> {
    let images = cx.try_global::<crate::remote_images::RemoteImages>()?;
    match images.image(url) {
        Some(image) => Some(crate::decoded_images::source(
            &image,
            crate::decoded_images::Fit::Tile,
        )),
        None => {
            images.load(url);
            None
        }
    }
}

/// What a composer's GIF picker tells its view.
#[derive(Clone, Debug, PartialEq)]
pub enum ComposerGifsEvent {
    /// A GIF was picked: the Markdown to put in the composer.
    Pick(SharedString),
    /// Escape with an empty search.
    Cancel,
}

/// One composer's tabs and GIF picker.
pub struct ComposerGifs {
    picker: Entity<GifPickerState>,
    search: Entity<GifSearch>,
    /// The GIFs tab is showing.
    showing: bool,
    /// The search's generation and how many GIFs the picker has of it.
    synced: (u64, usize),
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ComposerGifsEvent> for ComposerGifs {}

impl ComposerGifs {
    /// A picker over the app's search.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = GifSearch::global(cx);
        let picker = cx.new(|cx| {
            let mut picker = GifPickerState::new(window, cx);
            picker.set_placeholder(t!("gif-search"), window, cx);
            picker.set_attribution(Some(t!("gif-attribution")), cx);
            picker
        });
        let subscriptions = vec![
            cx.subscribe(&picker, |this, _, event: &GifPickerEvent, cx| match event {
                GifPickerEvent::Search(query) => {
                    let query = query.to_string();
                    this.search
                        .update(cx, |search, cx| search.search(strip_colons(&query), cx));
                }
                GifPickerEvent::LoadMore => this.search.update(cx, GifSearch::more),
                GifPickerEvent::Pick(id) => {
                    if let Some(gif) = this.search.read(cx).gif(id) {
                        cx.emit(ComposerGifsEvent::Pick(gif.markdown().into()));
                    }
                }
                GifPickerEvent::Cancel => cx.emit(ComposerGifsEvent::Cancel),
            }),
            cx.observe(&search, |this, _, cx| this.sync(cx)),
        ];
        Self {
            picker,
            search,
            showing: false,
            synced: (u64::MAX, 0),
            _subscriptions: subscriptions,
        }
    }

    /// Whether the GIFs tab shows.
    pub fn showing(&self) -> bool {
        self.showing
    }

    /// Shows the Emojis tab (`false`) or the GIFs tab, carrying the search
    /// text across as the official client's tabs share one filter.
    pub fn show(
        &mut self,
        gifs: bool,
        emoji: &Entity<crate::ui::emoji_picker::EmojiPickerState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.showing == gifs {
            return;
        }
        self.showing = gifs;
        if gifs {
            let query = emoji.read(cx).query(cx);
            self.picker
                .update(cx, |picker, cx| picker.set_query(query.clone(), window, cx));
            self.search
                .update(cx, |search, cx| search.search(strip_colons(&query), cx));
            self.sync(cx);
            self.picker.update(cx, |picker, cx| picker.open(window, cx));
        } else {
            let query = self.picker.read(cx).query(cx);
            emoji.update(cx, |emoji, cx| {
                emoji.set_query(query, window, cx);
                emoji.open(window, cx);
            });
        }
        cx.notify();
    }

    /// Back to the Emojis tab for the next time the picker opens, as the
    /// official client opens it.
    pub fn closed(&mut self, cx: &mut Context<Self>) {
        self.showing = false;
        cx.notify();
    }

    /// Focuses the GIF search when its tab shows.
    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.showing {
            self.picker.update(cx, |picker, cx| picker.open(window, cx));
        }
    }

    /// Brings the picker up to the search.
    fn sync(&mut self, cx: &mut Context<Self>) {
        let search = self.search.read(cx);
        let generation = search.generation;
        let phase = search.phase;
        let more = search.next.is_some();
        let tiles: Vec<_> = search.gifs.iter().map(|gif| tile(gif, cx)).collect();
        let images: std::collections::HashMap<SharedString, gpui_kit::ImageSource> = tiles
            .iter()
            .filter_map(|tile| Some((tile.id.clone(), tile.image.clone()?)))
            .collect();
        let image = move |gif: &crate::ui::gif_picker::Gif| images.get(&gif.id).cloned();
        let (synced_generation, synced_count) = self.synced;
        self.synced = (generation, tiles.len());
        self.picker.update(cx, |picker, cx| {
            if phase == Phase::Failed {
                picker.set_message(Some(t!("gif-failed")), cx);
                return;
            }
            if synced_generation != generation || tiles.len() < synced_count {
                if phase == Phase::Loading && tiles.is_empty() {
                    picker.set_gifs([], false, cx);
                    picker.set_loading(true, cx);
                } else {
                    picker.set_gifs(tiles, more, cx);
                }
            } else if tiles.len() > synced_count {
                let added: Vec<_> = tiles.into_iter().skip(synced_count).collect();
                picker.extend_gifs(added, more, cx);
                // Previews of the earlier ones may have landed too.
                picker.set_images(image, cx);
            } else {
                picker.set_images(image, cx);
                picker.set_loading(phase == Phase::Loading, cx);
            }
        });
    }

    /// The popover's content: the tabs over whichever picker is showing.
    pub fn popup(
        this: &Entity<Self>,
        emoji: &Entity<crate::ui::emoji_picker::EmojiPickerState>,
        cx: &App,
    ) -> gpui_kit::AnyElement {
        let theme = UiTheme::read(cx);
        let showing = this.read(cx).showing;
        let picker = this.read(cx).picker.clone();
        let (gifs, emoji_state) = (this.clone(), emoji.clone());
        let tabs = Tabs::new("composer.emoji.tabs")
            .variant(TabsVariant::Line)
            .aria_label(t!("chan-emoji"))
            .selected(if showing { "gifs" } else { "emojis" })
            .item(Tab::new(
                "composer.emoji.tabs.emojis",
                "emojis",
                t!("gif-tab-emojis"),
            ))
            .item(Tab::new(
                "composer.emoji.tabs.gifs",
                "gifs",
                t!("gif-tab-gifs"),
            ))
            .on_change(move |value, window, cx| {
                let show = value == "gifs";
                gifs.update(cx, |gifs, cx| gifs.show(show, &emoji_state, window, cx));
            });
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .px(theme.space(2.))
                    .border_b_1()
                    .border_color(theme.colors.border)
                    .child(tabs),
            )
            .child(if showing {
                GifPicker::new(&picker).bordered(false).into_any_element()
            } else {
                EmojiPicker::new(emoji).bordered(false).into_any_element()
            })
            .into_any_element()
    }
}

/// The official client drops colons around the search, so `:party:`
/// typed for an emoji still finds GIFs.
fn strip_colons(query: &str) -> &str {
    let query = query.strip_prefix(':').unwrap_or(query);
    query.strip_suffix(':').unwrap_or(query)
}

/// Whether the server offers GIFs: the composer's picker then has tabs.
pub fn enabled(cx: &App) -> bool {
    crate::priority::Features::read(cx).gifs
}

/// Forwards the app's GIF searches to sync for as long as the
/// subscription lives.
pub fn forward(cx: &mut App, send: Rc<dyn Fn(Ask)>) -> Subscription {
    let search = GifSearch::global(cx);
    cx.subscribe(&search, move |_, ask: &Ask, _| send(ask.clone()))
}

/// Sync's answer, for the app's search.
pub fn answered(query: &str, offset: u32, page: Option<&GifPage>, cx: &mut App) {
    let search = GifSearch::global(cx);
    search.update(cx, |search, cx| search.answered(query, offset, page, cx));
}

/// An outside image landed: redraw the picker if it shows it.
pub fn image_landed(url: &str, cx: &mut App) {
    let Some(search) = cx.try_global::<Shared>().map(|shared| shared.0.clone()) else {
        return;
    };
    if search.read(cx).shows(url) {
        search.update(cx, |_, cx| cx.notify());
    }
}

/// Signing out or switching servers drops the GIFs.
pub fn reset(cx: &mut App) {
    if let Some(search) = cx.try_global::<Shared>().map(|shared| shared.0.clone()) {
        search.update(cx, GifSearch::reset);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gif(id: &str) -> Gif {
        Gif {
            id: id.into(),
            title: format!("{id} GIF"),
            url: format!("https://media.giphy.com/media/{id}/200.gif"),
            preview: format!("https://media.giphy.com/media/{id}/200w_d.gif"),
            width: 200,
            height: 150,
        }
    }

    fn page(ids: &[&str], next: Option<u32>) -> GifPage {
        GifPage {
            gifs: ids.iter().map(|id| gif(id)).collect(),
            next,
        }
    }

    #[gpui_kit::test]
    fn trending_asks_at_once_and_pages_add_up(cx: &mut gpui_kit::TestAppContext) {
        let asked = Rc::new(std::cell::RefCell::new(Vec::new()));
        let search = cx.update(GifSearch::global);
        let _forward = cx.update(|cx| {
            let asked = asked.clone();
            forward(cx, Rc::new(move |ask| asked.borrow_mut().push(ask)))
        });
        search.update(cx, |search, cx| search.search("", cx));
        assert_eq!(
            *asked.borrow(),
            vec![Ask {
                query: String::new(),
                offset: 0
            }]
        );
        cx.update(|cx| answered("", 0, Some(&page(&["a", "b"], Some(2))), cx));
        search.update(cx, |search, cx| search.more(cx));
        assert_eq!(asked.borrow().last().map(|ask| ask.offset), Some(2));
        // "b" again on the next page is not shown twice.
        cx.update(|cx| answered("", 2, Some(&page(&["b", "c"], None)), cx));
        search.read_with(cx, |search, _| {
            let ids: Vec<_> = search.gifs.iter().map(|gif| gif.id.as_str()).collect();
            assert_eq!(ids, ["a", "b", "c"]);
            assert!(search.shows("https://media.giphy.com/media/c/200w_d.gif"));
        });
        // The last page: nothing more to ask.
        search.update(cx, |search, cx| search.more(cx));
        assert_eq!(asked.borrow().len(), 2);
    }

    #[gpui_kit::test]
    fn typing_waits_and_stale_answers_are_dropped(cx: &mut gpui_kit::TestAppContext) {
        let asked = Rc::new(std::cell::RefCell::new(Vec::new()));
        let search = cx.update(GifSearch::global);
        let _forward = cx.update(|cx| {
            let asked = asked.clone();
            forward(cx, Rc::new(move |ask| asked.borrow_mut().push(ask)))
        });
        search.update(cx, |search, cx| search.search("ca", cx));
        search.update(cx, |search, cx| search.search("cat", cx));
        assert!(asked.borrow().is_empty());
        cx.executor().advance_clock(SETTLE);
        cx.run_until_parked();
        assert_eq!(
            *asked.borrow(),
            vec![Ask {
                query: "cat".into(),
                offset: 0
            }]
        );
        cx.update(|cx| answered("ca", 0, Some(&page(&["old"], None)), cx));
        search.read_with(cx, |search, _| assert!(search.gifs.is_empty()));
        cx.update(|cx| answered("cat", 0, None, cx));
        search.read_with(cx, |search, _| assert_eq!(search.phase, Phase::Failed));
    }

    #[test]
    fn colons_around_the_search_are_dropped() {
        assert_eq!(strip_colons(":party:"), "party");
        assert_eq!(strip_colons("party"), "party");
        assert_eq!(strip_colons(":"), "");
    }
}
