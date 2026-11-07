//! The `matty-web` binary: Matty in a browser tab. `scripts/web build`
//! turns it into the page in `crates/matty-web/web/`.
//!
//! The page's query string takes `preview=1` (or `login`, `login-error`) for
//! the design fixtures `MATTY_PREVIEW` shows on the desktop, and
//! `backend=webgl` to skip WebGPU, `threads=1` to run background work on
//! web workers (off by default, see `matty_app::run_web`), and `demo=1` or
//! `demo=0` to open, or not, the sample workspace in `demo/demo.sqlite` (made
//! by `scripts/demo/run`) while no account is signed in. `web/index.html`
//! decides that (by default only on a `demo.` host and locally), downloads
//! the file next to the app, in parallel, and hands it over as
//! `window.mattyDemo`, so the app's own download stays small. On a `demo.`
//! host it also sets `window.mattyConnectUrl`, where the demo's "Connect
//! your server" goes. Off the web this binary does nothing.

fn main() {
    #[cfg(target_family = "wasm")]
    {
        let params = web_sys::window()
            .and_then(|window| window.location().search().ok())
            .and_then(|search| web_sys::UrlSearchParams::new_with_str(&search).ok());
        let get = |key: &str| params.as_ref().and_then(|params| params.get(key));
        if let Some(preview) = get("preview") {
            matty_app::preview::set(preview);
        }
        let webgl = get("backend").as_deref() == Some("webgl");
        let threads = get("threads").as_deref() == Some("1");
        matty_app::run_web(
            include_bytes!("../assets/IBMPlexSans-Regular.ttf"),
            webgl,
            threads,
            demo_snapshot(),
            connect_url(),
        );
    }
}

/// The demo snapshot the page fetched, kept for the life of the page.
#[cfg(target_family = "wasm")]
fn demo_snapshot() -> Option<&'static [u8]> {
    use wasm_bindgen::JsCast as _;
    let window = web_sys::window()?;
    let value = js_sys::Reflect::get(&window, &"mattyDemo".into()).ok()?;
    let bytes = value.dyn_into::<js_sys::ArrayBuffer>().ok()?;
    Some(js_sys::Uint8Array::new(&bytes).to_vec().leak())
}

/// Where the demo's "Connect your server" goes, when the page set one.
#[cfg(target_family = "wasm")]
fn connect_url() -> Option<String> {
    let window = web_sys::window()?;
    js_sys::Reflect::get(&window, &"mattyConnectUrl".into())
        .ok()?
        .as_string()
}
