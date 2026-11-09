//! Decoded images, kept within a memory budget.
//!
//! GPUI decodes an `ImageSource::Image` at full size, keeps the pixels in
//! its asset cache and the GPU atlas for as long as the app runs, and never
//! lets either go. A channel of photos then holds gigabytes: a 1920 by 1080
//! preview is 8 MB of pixels twice over, for a tile drawn 360 points wide.
//!
//! The pictures messages show (file tiles, link previews, inline Markdown
//! images and the lightbox) come through [`source`] instead. It decodes each
//! one off the main thread, scaled down to the size its [`Fit`] draws it at,
//! and keeps the pixels here, where they can be let go:
//!
//! - over [`BUDGET`], the least recently drawn pictures go until the cache
//!   is back under [`LOW_WATER`];
//! - a picture not drawn for [`IDLE`] goes too, so memory comes back after
//!   scrolling away from a gallery.
//!
//! Only pictures off screen go. A pass first redraws every window, so each
//! picture on screen, in any pane, says it is still wanted; what did not
//! is dropped from the cache and the GPU atlas. A picture scrolled back to
//! decodes again from its bytes, which the app still holds.
//!
//! Avatars and emoji are small and stay with GPUI.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::sync::{Arc, Weak};
use std::time::Duration;

use gpui_kit::{
    App, EntityId, Image, ImageCacheError, ImageFormat, ImageSource, RenderImage, Window,
};
use image::{AnimationDecoder as _, DynamicImage, Frame, ImageDecoder as _, RgbaImage, imageops};
use web_time::Instant;

/// Decoded pixels kept before a pass lets the oldest go. The GPU atlas
/// holds a second copy of what is drawn, so this is about half the cost.
pub const BUDGET: usize = 128 << 20;
/// Where a pass over budget stops.
pub const LOW_WATER: usize = 80 << 20;
/// A picture not drawn for this long goes at the next idle pass.
pub const IDLE: Duration = Duration::from_secs(60);
/// How often idle passes run while pictures are kept.
const IDLE_PASS: Duration = Duration::from_secs(30);
/// How long a pass waits after redrawing for the windows to draw.
const SETTLE: Duration = Duration::from_millis(250);

/// The size a picture is drawn at, which bounds its longest side in
/// pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Fit {
    /// A file tile or a link preview's image: at most 360 by 240 points,
    /// so 768 pixels is sharp at 2x.
    Tile,
    /// An inline Markdown image, as wide as the message column.
    Inline,
    /// The lightbox: the whole window. Capped so a huge original cannot
    /// take hundreds of megabytes.
    Full,
}

impl Fit {
    /// The longest side, in pixels.
    pub fn max_side(self) -> u32 {
        match self {
            Fit::Tile => 768,
            Fit::Inline => 2048,
            Fit::Full => 4096,
        }
    }
}

/// `image` for drawing at `fit`: decoded off the main thread, scaled down,
/// and let go when off screen. The same image and fit always give the same
/// source, so an animated picture keeps its frame clock across frames.
pub fn source(image: &Arc<Image>, fit: Fit) -> ImageSource {
    // SVGs are drawn by GPUI's renderer at their own size.
    if image.format == ImageFormat::Svg {
        return ImageSource::Image(image.clone());
    }
    let key = (image.id, fit);
    STORE.with(|store| {
        let mut store = store.borrow_mut();
        let entry = store.entries.entry(key).or_insert_with(|| Entry {
            loader: ImageSource::Custom(Arc::new(move |window, cx| load(key, window, cx))),
            image: Weak::new(),
            state: State::Idle,
            touched: Instant::now(),
            waiting: HashSet::new(),
        });
        if entry.image.strong_count() == 0 {
            entry.image = Arc::downgrade(image);
            if matches!(entry.state, State::Failed) {
                entry.state = State::Idle;
            }
        }
        entry.loader.clone()
    })
}

/// How many bytes of decoded pixels are kept now.
pub fn kept_bytes() -> usize {
    STORE.with(|store| store.borrow().kept)
}

/// How many pictures are decoded now.
pub fn kept_images() -> usize {
    STORE.with(|store| {
        store
            .borrow()
            .entries
            .values()
            .filter(|entry| matches!(entry.state, State::Ready(..)))
            .count()
    })
}

type Key = (u64, Fit);

thread_local! {
    static STORE: RefCell<Store> = RefCell::new(Store::default());
}

#[derive(Default)]
struct Store {
    entries: HashMap<Key, Entry>,
    /// Bytes of decoded pixels in `Ready` entries.
    kept: usize,
    /// A pass is on its way.
    passing: bool,
    /// Idle passes are scheduled.
    idling: bool,
    /// When a window that shows pictures last drew a frame.
    drawn: Option<Instant>,
    /// When the last pass finished.
    passed: Option<Instant>,
}

/// A window that shows pictures drew a frame. Until one does, what is on
/// screen is what the last pass saw, so an idle pass has nothing to learn
/// and does not redraw every window to find out (#795).
pub fn frame_drawn() {
    STORE.with(|store| store.borrow_mut().drawn = Some(Instant::now()));
}

struct Entry {
    /// The source views draw; it asks [`load`] each layout.
    loader: ImageSource,
    /// The encoded bytes, held by whoever showed the picture.
    image: Weak<Image>,
    state: State,
    /// When a window last laid the picture out.
    touched: Instant,
    /// The views to redraw when the decode lands.
    waiting: HashSet<EntityId>,
}

enum State {
    /// Not decoded: never drawn yet, or let go.
    Idle,
    Decoding,
    Ready(Arc<RenderImage>, usize),
    Failed,
}

/// What an image element asks each time it lays the picture out.
fn load(
    key: Key,
    window: &mut Window,
    cx: &mut App,
) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
    let start = STORE.with(|store| {
        let mut store = store.borrow_mut();
        let entry = store.entries.get_mut(&key)?;
        entry.touched = Instant::now();
        match &entry.state {
            State::Ready(image, _) => return Some(Ok(Ok(image.clone()))),
            State::Failed => {
                return Some(Ok(Err(ImageCacheError::Asset(
                    "the image could not be decoded".into(), // i18n-ignore: a log message
                ))));
            }
            State::Decoding => {
                entry.waiting.insert(window.current_view());
                return None;
            }
            State::Idle => {}
        }
        let Some(image) = entry.image.upgrade() else {
            entry.state = State::Failed;
            return None;
        };
        entry.state = State::Decoding;
        entry.waiting.insert(window.current_view());
        Some(Err(image))
    });
    match start? {
        Ok(result) => Some(result),
        Err(image) => {
            decode_in_background(key, image, cx);
            None
        }
    }
}

fn decode_in_background(key: Key, image: Arc<Image>, cx: &mut App) {
    let decoding = cx
        .background_executor()
        .spawn(async move { decode(&image, key.1.max_side()) });
    cx.spawn(async move |cx| {
        let decoded = decoding.await;
        cx.update(|cx| landed(key, decoded, cx));
    })
    .detach();
}

/// A decode finished: keep it, redraw who asked, and start a pass when the
/// cache went over budget.
fn landed(key: Key, decoded: Result<RenderImage, String>, cx: &mut App) {
    let (waiting, over) = STORE.with(|store| {
        let mut store = store.borrow_mut();
        let Some(entry) = store.entries.get_mut(&key) else {
            return (Vec::new(), false);
        };
        let mut added = 0;
        entry.state = match decoded {
            Ok(image) => {
                added = bytes_of(&image);
                State::Ready(Arc::new(image), added)
            }
            Err(error) => {
                tracing::debug!(%error, "could not decode an image");
                State::Failed
            }
        };
        let waiting: Vec<_> = entry.waiting.drain().collect();
        store.kept += added;
        (waiting, store.kept > BUDGET)
    });
    for view in waiting {
        cx.notify(view);
    }
    if over {
        start_pass(cx);
    }
    start_idling(cx);
}

/// Redraws every window so each picture on screen marks itself drawn, then
/// lets go of what did not: the oldest while over [`LOW_WATER`], and any
/// not drawn for [`IDLE`].
fn start_pass(cx: &mut App) {
    let already = STORE.with(|store| std::mem::replace(&mut store.borrow_mut().passing, true));
    if already {
        return;
    }
    let mark = Instant::now();
    cx.refresh_windows();
    let settle = cx.background_executor().timer(SETTLE);
    cx.spawn(async move |cx| {
        settle.await;
        cx.update(|cx| finish_pass(mark, cx));
    })
    .detach();
}

fn finish_pass(mark: Instant, cx: &mut App) {
    let dropped = STORE.with(|store| {
        let mut store = store.borrow_mut();
        store.passing = false;
        let now = Instant::now();
        store.passed = Some(now);
        evict(&mut store, mark, now)
    });
    if dropped.is_empty() {
        return;
    }
    tracing::debug!(
        count = dropped.len(),
        kept_bytes = kept_bytes(),
        "let decoded images go"
    );
    for image in dropped {
        cx.drop_image(image, None);
    }
    // No frame may draw a tile the atlas no longer has.
    cx.refresh_windows();
}

/// Takes from `store` what goes: of the pictures not drawn since `mark`,
/// the oldest first while over [`LOW_WATER`], and any not drawn for
/// [`IDLE`] before `now`. Forgets entries whose bytes are gone.
fn evict(store: &mut Store, mark: Instant, now: Instant) -> Vec<Arc<RenderImage>> {
    let mut stale: Vec<(Instant, Key)> = store
        .entries
        .iter()
        .filter(|(_, entry)| matches!(entry.state, State::Ready(..)) && entry.touched < mark)
        .map(|(key, entry)| (entry.touched, *key))
        .collect();
    stale.sort_by_key(|(touched, _)| *touched);
    let mut dropped = Vec::new();
    for (touched, key) in stale {
        let idle = now.saturating_duration_since(touched) >= IDLE;
        if store.kept <= LOW_WATER && !idle {
            continue;
        }
        let Some(entry) = store.entries.get_mut(&key) else {
            continue;
        };
        if let State::Ready(image, bytes) = std::mem::replace(&mut entry.state, State::Idle) {
            store.kept -= bytes;
            dropped.push(image);
        }
    }
    store.entries.retain(|_, entry| {
        !(matches!(entry.state, State::Idle | State::Failed) && entry.image.strong_count() == 0)
    });
    dropped
}

/// Whether a frame was drawn after the last pass finished: only then can a
/// picture the pass kept have left the screen.
fn redrawn_since_pass(store: &Store) -> bool {
    match (store.passed, store.drawn) {
        (Some(passed), Some(drawn)) => drawn > passed,
        (Some(_), None) => false,
        (None, _) => true,
    }
}

/// Runs a pass every [`IDLE_PASS`] while any picture is kept.
fn start_idling(cx: &mut App) {
    let already = STORE.with(|store| std::mem::replace(&mut store.borrow_mut().idling, true));
    if already {
        return;
    }
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(IDLE_PASS).await;
            let go_on = cx.update(|cx| {
                let (any, due) = STORE.with(|store| {
                    let store = store.borrow();
                    let now = Instant::now();
                    let ready = store
                        .entries
                        .values()
                        .filter(|entry| matches!(entry.state, State::Ready(..)));
                    let mut any = false;
                    let mut due = false;
                    for entry in ready {
                        any = true;
                        due |= now.saturating_duration_since(entry.touched) >= IDLE;
                    }
                    (any, due && redrawn_since_pass(&store))
                });
                if due {
                    start_pass(cx);
                }
                if !any {
                    STORE.with(|store| store.borrow_mut().idling = false);
                }
                any
            });
            if !go_on {
                break;
            }
        }
    })
    .detach();
}

fn bytes_of(image: &RenderImage) -> usize {
    (0..image.frame_count())
        .filter_map(|frame| image.as_bytes(frame))
        .map(<[u8]>::len)
        .sum()
}

/// Decodes `image`, scaled so its longest side is at most `max_side`, into
/// GPUI's BGRA frames.
fn decode(image: &Image, max_side: u32) -> Result<RenderImage, String> {
    let format = match image.format {
        ImageFormat::Png => image::ImageFormat::Png,
        ImageFormat::Jpeg => image::ImageFormat::Jpeg,
        ImageFormat::Webp => image::ImageFormat::WebP,
        ImageFormat::Gif => image::ImageFormat::Gif,
        ImageFormat::Bmp => image::ImageFormat::Bmp,
        ImageFormat::Tiff => image::ImageFormat::Tiff,
        ImageFormat::Ico => image::ImageFormat::Ico,
        ImageFormat::Pnm => image::ImageFormat::Pnm,
        ImageFormat::Svg => return Err("SVGs are not decoded here".into()), // i18n-ignore: a log message
    };
    let frames = if format == image::ImageFormat::Gif {
        let decoder =
            image::codecs::gif::GifDecoder::new(Cursor::new(&image.bytes)).map_err(err)?;
        let frames: Vec<Frame> = decoder
            .into_frames()
            .filter_map(Result::ok)
            .map(|frame| {
                // Each frame comes composited onto the whole canvas.
                let delay = frame.delay();
                let buffer = bgra(fit(frame.into_buffer(), max_side));
                Frame::from_parts(buffer, 0, 0, delay)
            })
            .collect();
        if frames.is_empty() {
            return Err("the GIF has no frames".into()); // i18n-ignore: a log message
        }
        frames
    } else {
        let mut decoder = image::ImageReader::with_format(Cursor::new(&image.bytes), format)
            .into_decoder()
            .map_err(err)?;
        let orientation = decoder.orientation().map_err(err)?;
        let mut picture = DynamicImage::from_decoder(decoder).map_err(err)?;
        picture.apply_orientation(orientation);
        let picture = if picture.width().max(picture.height()) > max_side {
            picture.resize(max_side, max_side, imageops::FilterType::Triangle)
        } else {
            picture
        };
        vec![Frame::new(bgra(picture.into_rgba8()))]
    };
    Ok(RenderImage::new(frames))
}

/// `buffer` scaled to fit within `max_side`, as a GIF frame.
fn fit(buffer: RgbaImage, max_side: u32) -> RgbaImage {
    let (width, height) = buffer.dimensions();
    if width.max(height) <= max_side {
        return buffer;
    }
    let scale = max_side as f32 / width.max(height) as f32;
    let width = ((width as f32 * scale).round() as u32).max(1);
    let height = ((height as f32 * scale).round() as u32).max(1);
    imageops::resize(&buffer, width, height, imageops::FilterType::Triangle)
}

/// GPUI draws BGRA.
fn bgra(mut buffer: RgbaImage) -> RgbaImage {
    for pixel in buffer.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    buffer
}

fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32) -> Arc<Image> {
        let picture = RgbaImage::from_pixel(width, height, image::Rgba([255, 0, 0, 255]));
        let mut bytes = Vec::new();
        DynamicImage::ImageRgba8(picture)
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        Arc::new(Image::from_bytes(ImageFormat::Png, bytes))
    }

    #[test]
    fn large_pictures_decode_scaled_down() {
        let image = png(3000, 1500);
        let decoded = decode(&image, Fit::Tile.max_side()).unwrap();
        let size = decoded.size(0);
        assert_eq!((size.width.0, size.height.0), (768, 384));
        assert_eq!(bytes_of(&decoded), 768 * 384 * 4);
    }

    #[test]
    fn small_pictures_keep_their_size() {
        let image = png(200, 100);
        let decoded = decode(&image, Fit::Tile.max_side()).unwrap();
        let size = decoded.size(0);
        assert_eq!((size.width.0, size.height.0), (200, 100));
        // Red in RGBA is blue's slot in BGRA.
        assert_eq!(&decoded.as_bytes(0).unwrap()[..4], &[0, 0, 255, 255]);
    }

    #[test]
    fn the_same_image_and_fit_give_the_same_source() {
        let image = png(10, 10);
        let id = |source: ImageSource| match source {
            ImageSource::Custom(load) => Arc::as_ptr(&load) as *const () as usize,
            _ => panic!("expected a custom source"),
        };
        assert_eq!(id(source(&image, Fit::Tile)), id(source(&image, Fit::Tile)));
        assert_ne!(id(source(&image, Fit::Tile)), id(source(&image, Fit::Full)));
    }

    fn ready(store: &mut Store, id: u64, bytes: usize, touched: Instant, image: &Arc<Image>) {
        let key = (id, Fit::Tile);
        store.entries.insert(
            key,
            Entry {
                loader: ImageSource::Render(Arc::new(RenderImage::new(Vec::new()))),
                image: Arc::downgrade(image),
                state: State::Ready(Arc::new(RenderImage::new(Vec::new())), bytes),
                touched,
                waiting: HashSet::new(),
            },
        );
        store.kept += bytes;
    }

    #[test]
    fn over_budget_the_oldest_unseen_go_first() {
        let image = png(1, 1);
        let mut store = Store::default();
        let start = Instant::now();
        let mb = 1 << 20;
        ready(&mut store, 1, 50 * mb, start, &image);
        ready(
            &mut store,
            2,
            50 * mb,
            start + Duration::from_secs(1),
            &image,
        );
        ready(
            &mut store,
            3,
            50 * mb,
            start + Duration::from_secs(2),
            &image,
        );
        // Drawn after the pass began: on screen, so it stays.
        ready(
            &mut store,
            4,
            50 * mb,
            start + Duration::from_secs(4),
            &image,
        );
        let mark = start + Duration::from_secs(3);
        let dropped = evict(&mut store, mark, mark);
        assert_eq!(dropped.len(), 3, "down to the low water mark");
        assert_eq!(store.kept, 50 * mb);
        assert!(matches!(
            store.entries[&(4, Fit::Tile)].state,
            State::Ready(..)
        ));
        assert!(matches!(store.entries[&(1, Fit::Tile)].state, State::Idle));
    }

    #[test]
    fn under_budget_only_idle_pictures_go() {
        let image = png(1, 1);
        let mut store = Store::default();
        let start = Instant::now();
        ready(&mut store, 1, 1 << 20, start, &image);
        ready(&mut store, 2, 1 << 20, start + IDLE, &image);
        let now = start + IDLE + Duration::from_secs(1);
        let dropped = evict(&mut store, now, now);
        assert_eq!(dropped.len(), 1);
        assert!(matches!(store.entries[&(1, Fit::Tile)].state, State::Idle));
        assert!(matches!(
            store.entries[&(2, Fit::Tile)].state,
            State::Ready(..)
        ));
    }

    #[test]
    fn entries_whose_bytes_are_gone_are_forgotten_once_let_go() {
        let mut store = Store::default();
        let start = Instant::now();
        {
            let image = png(1, 1);
            ready(&mut store, 1, 1 << 20, start, &image);
        }
        let now = start + IDLE;
        evict(&mut store, now, now);
        assert!(store.entries.is_empty());
    }

    /// #795: with nothing drawn since the last pass, an idle pass would only
    /// redraw what is already known to be on screen.
    #[test]
    fn an_idle_pass_waits_for_a_frame() {
        let mut store = Store::default();
        assert!(redrawn_since_pass(&store), "no pass yet");
        let start = Instant::now();
        store.passed = Some(start);
        assert!(!redrawn_since_pass(&store), "nothing drawn at all");
        store.drawn = Some(start - Duration::from_secs(1));
        assert!(!redrawn_since_pass(&store), "drawn before the pass");
        store.drawn = Some(start + Duration::from_secs(1));
        assert!(redrawn_since_pass(&store), "drawn after it");
    }
}
