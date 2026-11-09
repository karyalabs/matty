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
//! decodes again from its bytes.
//!
//! The encoded bytes of what sync fetched (file images, outside images) do
//! not stay with the app either (#789): [`hold`] keeps them in a cache of
//! [`ENCODED_BUDGET`], least recently used first out, and hands back a
//! handle without bytes for the models and views to carry. When a picture
//! must decode again after its bytes left, it asks for them once more, and
//! sync reads them from its media cache on disk.
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
/// Encoded bytes [`hold`] keeps, so a picture scrolled back to decodes
/// without asking sync for its bytes again.
pub const ENCODED_BUDGET: usize = 48 << 20;
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
    /// The lightbox: the whole window, at the window's own size in
    /// pixels when it first draws the picture (#794), and never past
    /// [`Fit::max_side`] so a huge original cannot take hundreds of
    /// megabytes. The size holds until the picture is let go, so a window
    /// grown afterwards draws it scaled up; [`FULL_MIN_SIDE`] keeps that
    /// sharp enough.
    Full,
}

/// The lightbox decodes at least this many pixels on its longest side,
/// so a small window grown afterwards still shows a sharp picture.
const FULL_MIN_SIDE: u32 = 1536;
/// The lightbox's size in pixels is rounded up to a multiple of this.
const FULL_STEP: u32 = 256;

impl Fit {
    /// The longest side, in pixels.
    pub fn max_side(self) -> u32 {
        match self {
            Fit::Tile => 768,
            Fit::Inline => 2048,
            Fit::Full => 4096,
        }
    }

    /// The longest side to decode at for drawing in `window`: the
    /// lightbox's is the window's longest side in pixels, so a 1440 by
    /// 900 window at 2x decodes at 3,072 rather than 4,096 (about half
    /// the memory); the others are fixed.
    fn side_in(self, window: &Window) -> u32 {
        match self {
            Fit::Full => {
                let size = window.viewport_size();
                let points = f32::from(size.width).max(f32::from(size.height));
                full_side(points * window.scale_factor())
            }
            _ => self.max_side(),
        }
    }
}

/// [`Fit::Full`]'s side for a window `pixels` long on its longest side.
fn full_side(pixels: f32) -> u32 {
    // Saturates: a NaN or negative size is 0, a huge one the cap.
    let pixels = pixels.ceil() as u32;
    pixels
        .div_ceil(FULL_STEP)
        .saturating_mul(FULL_STEP)
        .clamp(FULL_MIN_SIDE, Fit::Full.max_side())
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

/// Asks for a held picture's bytes again; they come back through [`hold`].
pub type Refetch = std::rc::Rc<dyn Fn()>;

/// Keeps `image`'s encoded bytes here, within [`ENCODED_BUDGET`], and
/// returns a handle without them: the same id and format, for [`source`]
/// and anything that compares images. When the bytes have left and the
/// picture must decode again, `refetch` asks for them; passing them to
/// `hold` again brings them back. SVGs keep their bytes: GPUI draws them.
pub fn hold(image: Arc<Image>, refetch: Refetch) -> Arc<Image> {
    if image.format == ImageFormat::Svg || image.bytes.is_empty() {
        return image;
    }
    let handle = Arc::new(Image {
        format: image.format,
        bytes: Vec::new(),
        id: image.id,
    });
    let woken = STORE.with(|store| {
        let mut store = store.borrow_mut();
        store.refetch.insert(image.id, refetch);
        store.hold(image)
    });
    if let Some(woken) = woken {
        let _ = woken.try_send(());
    }
    handle
}

/// The encoded bytes behind `image`: its own, or held ones for a handle.
pub fn bytes(image: &Arc<Image>) -> Option<Arc<Image>> {
    if !image.bytes.is_empty() {
        return Some(image.clone());
    }
    STORE.with(|store| store.borrow_mut().held_bytes(image.id))
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
    /// Encoded bytes by image id, with when each was last used.
    held: HashMap<u64, (Arc<Image>, Instant)>,
    /// Their size in bytes.
    held_size: usize,
    /// How to ask for each held picture's bytes again.
    refetch: HashMap<u64, Refetch>,
    /// Pictures whose bytes were asked for again: their arrival wakes the
    /// task that decodes them.
    awaited: HashMap<u64, async_channel::Sender<()>>,
    /// Bytes of decoded pixels in `Ready` entries.
    kept: usize,
    /// A pass is on its way.
    passing: bool,
    /// Idle passes are scheduled.
    idling: bool,
    /// The system is short of memory: the next pass lets go of every
    /// picture off screen, whatever the budget.
    pressed: bool,
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

impl Store {
    /// Keeps `image`'s bytes, letting the least recently used go past
    /// [`ENCODED_BUDGET`]. Returns who waits for them.
    fn hold(&mut self, image: Arc<Image>) -> Option<async_channel::Sender<()>> {
        let id = image.id;
        let size = image.bytes.len();
        if let Some((old, _)) = self.held.insert(id, (image, Instant::now())) {
            self.held_size -= old.bytes.len();
        }
        self.held_size += size;
        if self.held_size > ENCODED_BUDGET {
            let mut by_age: Vec<(Instant, u64)> = self
                .held
                .iter()
                .map(|(id, (_, used))| (*used, *id))
                .collect();
            by_age.sort_unstable();
            for (_, old) in by_age {
                if self.held_size <= ENCODED_BUDGET || old == id {
                    break;
                }
                if let Some((image, _)) = self.held.remove(&old) {
                    self.held_size -= image.bytes.len();
                }
            }
        }
        // Handles nobody holds any more need no way back.
        if self.refetch.len() > 4 * self.held.len().max(256) {
            let live: HashSet<u64> = self.entries.keys().map(|(id, _)| *id).collect();
            let held = &self.held;
            self.refetch
                .retain(|id, _| held.contains_key(id) || live.contains(id));
        }
        self.awaited.remove(&id)
    }

    fn held_bytes(&mut self, id: u64) -> Option<Arc<Image>> {
        let (image, used) = self.held.get_mut(&id)?;
        *used = Instant::now();
        Some(image.clone())
    }
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
    /// Its bytes were let go and are asked for again.
    Fetching,
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
            State::Decoding | State::Fetching => {
                entry.waiting.insert(window.current_view());
                return None;
            }
            State::Idle => {}
        }
        let Some(image) = entry.image.upgrade() else {
            entry.state = State::Failed;
            return None;
        };
        entry.waiting.insert(window.current_view());
        if !image.bytes.is_empty() {
            entry.state = State::Decoding;
            return Some(Err(Start::Decode(image)));
        }
        if let Some(held) = store.held_bytes(key.0) {
            store.entries.get_mut(&key)?.state = State::Decoding;
            return Some(Err(Start::Decode(held)));
        }
        let Some(refetch) = store.refetch.get(&key.0).cloned() else {
            store.entries.get_mut(&key)?.state = State::Failed;
            return None;
        };
        store.entries.get_mut(&key)?.state = State::Fetching;
        if store.awaited.contains_key(&key.0) {
            return None;
        }
        let (woken, wake) = async_channel::bounded(1);
        store.awaited.insert(key.0, woken);
        Some(Err(Start::Fetch(refetch, wake)))
    });
    match start? {
        Ok(result) => Some(result),
        Err(Start::Decode(image)) => {
            decode_in_background(key, image, key.1.side_in(window), cx);
            None
        }
        Err(Start::Fetch(refetch, wake)) => {
            refetch();
            cx.spawn(async move |cx| {
                if wake.recv().await.is_ok() {
                    cx.update(|cx| fetched(key.0, cx));
                }
            })
            .detach();
            None
        }
    }
}

/// What [`load`] starts once the store is let go.
enum Start {
    Decode(Arc<Image>),
    Fetch(Refetch, async_channel::Receiver<()>),
}

/// Bytes asked for again arrived: every fit of that picture waiting for
/// them may decode now, so the views that drew it draw again.
fn fetched(id: u64, cx: &mut App) {
    let waiting: Vec<EntityId> = STORE.with(|store| {
        let mut store = store.borrow_mut();
        store
            .entries
            .iter_mut()
            .filter(|((key_id, _), entry)| *key_id == id && matches!(entry.state, State::Fetching))
            .flat_map(|(_, entry)| {
                entry.state = State::Idle;
                entry.waiting.drain().collect::<Vec<_>>()
            })
            .collect()
    });
    for view in waiting {
        cx.notify(view);
    }
}

fn decode_in_background(key: Key, image: Arc<Image>, side: u32, cx: &mut App) {
    // In the browser "background" is the page's only thread: the browser's
    // own decoder, on a worker, keeps scrolling smooth.
    #[cfg(target_family = "wasm")]
    if crate::browser_decode::handles(image.format)
        && let Some(answer) = crate::browser_decode::decode(&image.bytes, image.format, side)
    {
        cx.spawn(async move |cx| {
            let decoded = match answer.recv().await {
                Ok(Ok(picture)) => RgbaImage::from_raw(picture.width, picture.height, picture.bgra)
                    .map(|buffer| RenderImage::new(vec![Frame::new(buffer)]))
                    .ok_or_else(|| "the decoder sent back too few pixels".to_string()), // i18n-ignore: a log message
                Ok(Err(error)) => Err(error),
                Err(error) => Err(error.to_string()),
            };
            cx.update(|cx| landed(key, decoded, cx));
        })
        .detach();
        return;
    }
    let decoding = cx
        .background_executor()
        .spawn(async move { decode(&image, side) });
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

/// The system is short of memory (#794): lets go of the held encoded
/// bytes, which sync can read from disk again, and starts a pass that
/// lets go of every decoded picture off screen.
pub fn relieve(cx: &mut App) {
    STORE.with(|store| {
        let mut store = store.borrow_mut();
        store.pressed = true;
        store.held.clear();
        store.held_size = 0;
    });
    start_pass(cx);
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
        let dropped = evict(&mut store, mark, now);
        store.pressed = false;
        dropped
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
/// the oldest first while over [`LOW_WATER`] (all of them under memory
/// pressure), and any not drawn for [`IDLE`] before `now`. Forgets
/// entries whose bytes are gone.
fn evict(store: &mut Store, mark: Instant, now: Instant) -> Vec<Arc<RenderImage>> {
    let mut stale: Vec<(Instant, Key)> = store
        .entries
        .iter()
        .filter(|(_, entry)| matches!(entry.state, State::Ready(..)) && entry.touched < mark)
        .map(|(key, entry)| (entry.touched, *key))
        .collect();
    stale.sort_by_key(|(touched, _)| *touched);
    let mut dropped = Vec::new();
    let floor = if store.pressed { 0 } else { LOW_WATER };
    for (touched, key) in stale {
        let idle = now.saturating_duration_since(touched) >= IDLE;
        if store.kept <= floor && !idle {
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
    fn the_lightbox_decodes_at_the_windows_size_within_bounds() {
        // A 1440 point window at 2x.
        assert_eq!(super::full_side(2880.0), 3072);
        assert_eq!(super::full_side(2816.0), 2816);
        assert_eq!(super::full_side(800.0), super::FULL_MIN_SIDE);
        assert_eq!(super::full_side(12_000.0), Fit::Full.max_side());
        assert_eq!(super::full_side(f32::NAN), super::FULL_MIN_SIDE);
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
    fn under_memory_pressure_every_picture_off_screen_goes() {
        let image = png(1, 1);
        let mut store = Store::default();
        let start = Instant::now();
        ready(&mut store, 1, 1 << 20, start, &image);
        ready(
            &mut store,
            2,
            1 << 20,
            start + Duration::from_secs(2),
            &image,
        );
        store.pressed = true;
        // Picture 2 was drawn after the pass began: it is on screen.
        let mark = start + Duration::from_secs(1);
        let dropped = evict(&mut store, mark, mark);
        assert_eq!(dropped.len(), 1);
        assert!(matches!(store.entries[&(1, Fit::Tile)].state, State::Idle));
        assert!(matches!(
            store.entries[&(2, Fit::Tile)].state,
            State::Ready(..)
        ));
    }

    #[test]
    fn held_pictures_hand_out_handles_without_bytes() {
        let image = png(20, 20);
        let handle = hold(image.clone(), std::rc::Rc::new(|| {}));
        assert!(handle.bytes.is_empty());
        assert_eq!((handle.id, handle.format), (image.id, image.format));
        assert_eq!(bytes(&handle).unwrap().bytes, image.bytes);
    }

    #[test]
    fn held_bytes_stay_within_their_budget_oldest_out_first() {
        let mut store = Store::default();
        let big = |n: u8| {
            Arc::new(Image::from_bytes(
                ImageFormat::Png,
                vec![n; ENCODED_BUDGET / 3 + 1],
            ))
        };
        let (a, b, c) = (big(1), big(2), big(3));
        store.hold(a.clone());
        store.hold(b.clone());
        // Using `a` makes `b` the oldest.
        assert!(store.held_bytes(a.id).is_some());
        store.hold(c.clone());
        assert!(store.held_size <= ENCODED_BUDGET);
        assert!(store.held_bytes(b.id).is_none());
        assert!(store.held_bytes(a.id).is_some());
        assert!(store.held_bytes(c.id).is_some());
    }

    #[test]
    fn bytes_asked_for_again_wake_who_waits() {
        let mut store = Store::default();
        let image = png(2, 2);
        let (woken, wake) = async_channel::bounded(1);
        store.awaited.insert(image.id, woken);
        store.hold(image).unwrap().try_send(()).unwrap();
        assert!(wake.try_recv().is_ok());
        assert!(store.awaited.is_empty());
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
