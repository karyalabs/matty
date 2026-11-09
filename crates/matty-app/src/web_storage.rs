//! The browser's `localStorage`, standing in for the files Matty keeps on
//! the desktop (settings) and the credential store (the session token).
//!
//! Keys are the paths the desktop would use, so the code above stays the
//! same. Everything here is per origin: `https://karyalabs.dev` sees only its
//! own entries, and the demo's `https://demo.karyalabs.dev` its own.
//!
//! Chrome writes `localStorage` to disk lazily: after an origin's first
//! commit, later ones can wait about a minute, and a browser that quits or is
//! killed in that time forgets them, such as the session just signed in to.
//! So every change is also handed to the page's `mattyKept` (`web/index.html`),
//! which keeps a copy in IndexedDB with strict durability and puts it back in
//! `localStorage` before the app starts.

use std::io;

fn storage() -> io::Result<web_sys::Storage> {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .ok_or_else(|| io::Error::new(io::ErrorKind::Unsupported, "no localStorage"))
}

fn js_error(_: wasm_bindgen::JsValue) -> io::Error {
    io::Error::other("localStorage refused the change (full or disabled)")
}

/// The text kept under `key`.
pub fn get(key: &str) -> io::Result<String> {
    storage()?
        .get_item(key)
        .map_err(js_error)?
        .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
}

/// Keeps `value` under `key`, replacing what was there.
pub fn set(key: &str, value: &str) -> io::Result<()> {
    storage()?.set_item(key, value).map_err(js_error)?;
    keep("put", &[key, value]);
    Ok(())
}

/// Forgets `key`.
pub fn remove(key: &str) -> io::Result<()> {
    storage()?.remove_item(key).map_err(js_error)?;
    keep("remove", &[key]);
    Ok(())
}

/// Calls `mattyKept[method](...args)` on the page, when it has one.
fn keep(method: &str, args: &[&str]) {
    use wasm_bindgen::{JsCast as _, JsValue};

    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(function) = js_sys::Reflect::get(&window, &"mattyKept".into())
        .and_then(|kept| js_sys::Reflect::get(&kept, &method.into()))
        .ok()
        .and_then(|function| function.dyn_into::<js_sys::Function>().ok())
    else {
        return;
    };
    let args = args
        .iter()
        .map(|arg| JsValue::from_str(arg))
        .collect::<js_sys::Array>();
    if function.apply(&JsValue::NULL, &args).is_err() {
        tracing::warn!(method, "could not keep a copy of localStorage");
    }
}
