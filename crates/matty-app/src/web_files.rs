//! Picking files in the browser (#1109). GPUI's web platform has no file
//! dialog, so a hidden `<input type="file">` asks the browser, and each
//! chosen file is read into memory: the page has no paths to read later.

use std::sync::Arc;

use wasm_bindgen::JsCast as _;

use crate::attachments::{PickedFile, UploadSource};

/// Reads a browser `FileList` into uploads.
async fn read(files: web_sys::FileList) -> Vec<UploadSource> {
    let mut picked = Vec::new();
    for index in 0..files.length() {
        let Some(file) = files.get(index) else {
            continue;
        };
        match wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await {
            Ok(buffer) => picked.push(UploadSource::Picked(Arc::new(PickedFile::new(
                file.name(),
                js_sys::Uint8Array::new(&buffer).to_vec(),
            )))),
            Err(error) => tracing::warn!(?error, "could not read a picked file"),
        }
    }
    picked
}

/// Files dropped on the page or pasted into it. GPUI's web platform stops
/// the browser opening a dropped file but has no way to hand files on, and
/// reads only text and images from the clipboard on request, so the page's
/// own listeners read them.
pub fn incoming() -> async_channel::Receiver<Vec<UploadSource>> {
    let (sender, receiver) = async_channel::unbounded();
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return receiver;
    };
    let forward = move |files: web_sys::FileList, event: &web_sys::Event| {
        if files.length() == 0 {
            return;
        }
        event.prevent_default();
        let sender = sender.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let files = read(files).await;
            if !files.is_empty() && sender.try_send(files).is_err() {
                tracing::warn!("nobody was waiting for the dropped or pasted files");
            }
        });
    };
    let on_drop = {
        let forward = forward.clone();
        wasm_bindgen::closure::Closure::<dyn Fn(web_sys::DragEvent)>::new(
            move |event: web_sys::DragEvent| {
                if let Some(files) = event.data_transfer().and_then(|transfer| transfer.files()) {
                    forward(files, &event);
                }
            },
        )
    };
    let on_paste = wasm_bindgen::closure::Closure::<dyn Fn(web_sys::ClipboardEvent)>::new(
        move |event: web_sys::ClipboardEvent| {
            if let Some(files) = event.clipboard_data().and_then(|data| data.files()) {
                forward(files, &event);
            }
        },
    );
    for (name, listener) in [("drop", on_drop.as_ref()), ("paste", on_paste.as_ref())] {
        if let Err(error) =
            document.add_event_listener_with_callback(name, listener.unchecked_ref())
        {
            tracing::warn!(?error, name, "could not listen for files");
        }
    }
    // The page lives as long as the app, so the listeners do too.
    on_drop.forget();
    on_paste.forget();
    receiver
}

/// Opens the browser's file chooser. The receiver gets the chosen files
/// once they are read; it gets nothing when the user cancels.
pub fn pick(multiple: bool) -> async_channel::Receiver<Vec<UploadSource>> {
    let (sender, receiver) = async_channel::bounded(1);
    let input = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.create_element("input").ok())
        .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok());
    let Some(input) = input else {
        tracing::warn!("no file input to pick files with");
        return receiver;
    };
    input.set_type("file");
    input.set_multiple(multiple);
    let chosen = input.clone();
    let on_change = wasm_bindgen::closure::Closure::once_into_js(move |_: web_sys::Event| {
        let Some(files) = chosen.files() else {
            return;
        };
        wasm_bindgen_futures::spawn_local(async move {
            let picked = read(files).await;
            if sender.try_send(picked).is_err() {
                tracing::warn!("nobody was waiting for the picked files");
            }
        });
    });
    if let Err(error) = input.add_event_listener_with_callback("change", on_change.unchecked_ref())
    {
        tracing::warn!(?error, "could not listen for picked files");
        return receiver;
    }
    input.click();
    receiver
}
