//! Picture decoding in the browser, off the main thread (#818).
//!
//! On the web every GPUI "background" task runs on the page's one thread
//! (`matty_app::run_web`), so decoding a 4000 by 2500 JPEG in WebAssembly
//! froze scrolling for as long as it took. Here a small Web Worker hands the
//! bytes to the browser's own decoder (`createImageBitmap`), scales the
//! picture down and sends back its pixels. The main thread only copies them
//! into memory.
//!
//! The worker is made from a script in this file, so the page needs no extra
//! file, and it runs on plain `ArrayBuffer`s, not the app's shared memory.

use std::cell::RefCell;
use std::collections::HashMap;

use gpui_kit::ImageFormat;
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};

/// The worker: decode, scale to fit `max` on the longest side (EXIF
/// orientation applied, as everywhere in the browser), then the pixels as
/// GPUI's BGRA.
const SCRIPT: &str = r#"
onmessage = async ({ data: { id, bytes, type, max } }) => {
  try {
    let bitmap = await createImageBitmap(new Blob([bytes], { type }));
    const scale = Math.min(1, max / Math.max(bitmap.width, bitmap.height));
    const width = Math.max(1, Math.round(bitmap.width * scale));
    const height = Math.max(1, Math.round(bitmap.height * scale));
    if (scale < 1) {
      const scaled = await createImageBitmap(bitmap, {
        resizeWidth: width, resizeHeight: height, resizeQuality: "high",
      });
      bitmap.close();
      bitmap = scaled;
    }
    const canvas = new OffscreenCanvas(width, height);
    const context = canvas.getContext("2d", { willReadFrequently: true });
    context.drawImage(bitmap, 0, 0);
    bitmap.close();
    const pixels = context.getImageData(0, 0, width, height).data;
    // GPUI draws BGRA.
    for (let i = 0; i < pixels.length; i += 4) {
      const red = pixels[i];
      pixels[i] = pixels[i + 2];
      pixels[i + 2] = red;
    }
    postMessage({ id, width, height, pixels: pixels.buffer }, [pixels.buffer]);
  } catch (error) {
    postMessage({ id, error: String(error) });
  }
};
"#;

/// A decoded picture: its size and BGRA pixels.
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

type Reply = async_channel::Sender<Result<Decoded, String>>;

struct Decoder {
    worker: web_sys::Worker,
    waiting: HashMap<u32, Reply>,
    next: u32,
    _on_message: Closure<dyn FnMut(web_sys::MessageEvent)>,
}

thread_local! {
    /// `None` inside once making the worker failed: decode in WebAssembly.
    static DECODER: RefCell<Option<Option<Decoder>>> = const { RefCell::new(None) };
}

/// Whether the browser decodes `format` (Chrome, Firefox and Safari all do
/// these). GIFs are not sent: their frames are decoded in WebAssembly.
pub fn handles(format: ImageFormat) -> bool {
    matches!(
        format,
        ImageFormat::Png
            | ImageFormat::Jpeg
            | ImageFormat::Webp
            | ImageFormat::Bmp
            | ImageFormat::Ico
    )
}

/// Decodes `bytes` in the worker, scaled to fit `max_side`. `None` when
/// there is no worker; the caller decodes in WebAssembly then.
pub fn decode(
    bytes: &[u8],
    format: ImageFormat,
    max_side: u32,
) -> Option<async_channel::Receiver<Result<Decoded, String>>> {
    DECODER.with(|decoder| {
        let mut decoder = decoder.borrow_mut();
        let decoder = decoder.get_or_insert_with(start).as_mut()?;
        let id = decoder.next;
        decoder.next = decoder.next.wrapping_add(1);
        // The app's memory is shared, which postMessage cannot transfer:
        // copy the bytes into a buffer of their own and hand that over.
        let buffer = js_sys::Uint8Array::new_with_length(bytes.len() as u32);
        buffer.copy_from(bytes);
        let message = js_sys::Object::new();
        let set = |key: &str, value: &JsValue| {
            let _ = js_sys::Reflect::set(&message, &key.into(), value);
        };
        set("id", &id.into());
        set("bytes", &buffer.buffer());
        set("type", &format.mime_type().into());
        set("max", &max_side.into());
        let transfer = js_sys::Array::of1(&buffer.buffer());
        if let Err(error) = decoder
            .worker
            .post_message_with_transfer(&message, &transfer)
        {
            tracing::debug!(?error, "could not send a picture to the decoder");
            return None;
        }
        let (reply, answer) = async_channel::bounded(1);
        decoder.waiting.insert(id, reply);
        Some(answer)
    })
}

fn start() -> Option<Decoder> {
    let script = js_sys::Array::of1(&SCRIPT.into());
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("text/javascript");
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&script, &options).ok()?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).ok()?;
    let worker = web_sys::Worker::new(&url);
    let _ = web_sys::Url::revoke_object_url(&url);
    let worker = match worker {
        Ok(worker) => worker,
        Err(error) => {
            tracing::info!(
                ?error,
                "no picture decoder worker; decoding on the main thread"
            );
            return None;
        }
    };
    let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(answered);
    worker.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    Some(Decoder {
        worker,
        waiting: HashMap::new(),
        next: 0,
        _on_message: on_message,
    })
}

/// The worker answered one picture.
fn answered(event: web_sys::MessageEvent) {
    let data = event.data();
    let get = |key: &str| js_sys::Reflect::get(&data, &key.into()).unwrap_or(JsValue::UNDEFINED);
    let Some(id) = get("id").as_f64().map(|id| id as u32) else {
        return;
    };
    let Some(reply) = DECODER.with(|decoder| {
        decoder
            .borrow_mut()
            .as_mut()
            .and_then(Option::as_mut)
            .and_then(|decoder| decoder.waiting.remove(&id))
    }) else {
        return;
    };
    let result = match get("error").as_string() {
        Some(error) => Err(error),
        None => {
            let width = get("width").as_f64().unwrap_or(0.) as u32;
            let height = get("height").as_f64().unwrap_or(0.) as u32;
            let bgra = js_sys::Uint8Array::new(&get("pixels")).to_vec();
            if width == 0 || height == 0 || bgra.len() != (width * height * 4) as usize {
                Err("the decoder sent back no pixels".into()) // i18n-ignore: a log message
            } else {
                Ok(Decoded {
                    width,
                    height,
                    bgra,
                })
            }
        }
    };
    let _ = reply.try_send(result);
}
