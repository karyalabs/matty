//! The web build's address bar follows the open channel, as the web app's
//! does: `#/{team}/channels/{name}` after the page's own address, so a
//! reload, a bookmark or a link a teammate pastes opens that channel, and
//! the browser's Back and Forward step between channels.

/// The page path a location hash names, such as `/team/channels/town-square`
/// for `#/team/channels/town-square`; `None` for any other hash.
pub fn path_of_hash(hash: &str) -> Option<&str> {
    let path = hash.strip_prefix('#')?;
    (path.starts_with('/') && path.len() > 1).then_some(path)
}

/// The hash for `link`, a channel's link on `server`; `None` when the link
/// is on another server.
pub fn hash_for(link: &str, server: &str) -> Option<String> {
    let path = link.strip_prefix(server.trim_end_matches('/'))?;
    path.starts_with('/').then(|| format!("#{path}"))
}

#[cfg(target_family = "wasm")]
mod browser {
    use wasm_bindgen::{JsCast as _, closure::Closure};

    fn location_hash() -> Option<String> {
        web_sys::window()?.location().hash().ok()
    }

    /// The page path the address had when the page loaded.
    pub fn initial() -> Option<String> {
        super::path_of_hash(&location_hash()?).map(str::to_owned)
    }

    /// The page paths Back and Forward, or an edited address, go to.
    pub fn changes() -> async_channel::Receiver<String> {
        let (sender, receiver) = async_channel::unbounded();
        let Some(window) = web_sys::window() else {
            return receiver;
        };
        let listener = Closure::<dyn Fn()>::new(move || {
            if let Some(path) = location_hash().as_deref().and_then(super::path_of_hash) {
                if let Err(error) = sender.try_send(path.to_owned()) {
                    tracing::warn!(%error, "dropped an address change");
                }
            }
        });
        for event in ["popstate", "hashchange"] {
            if let Err(error) =
                window.add_event_listener_with_callback(event, listener.as_ref().unchecked_ref())
            {
                tracing::warn!(?error, event, "could not follow the address bar");
            }
        }
        // Lives as long as the page.
        listener.forget();
        receiver
    }

    /// Puts `hash` in the address bar, when it is not there already: as a
    /// new history entry, or in place of an address that named no channel,
    /// so Back does not stop at the bare page.
    pub fn show(hash: &str) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let current = window.location().hash().unwrap_or_default();
        if current == hash {
            return;
        }
        let state = wasm_bindgen::JsValue::NULL;
        let pushed = window.history().and_then(|history| {
            if super::path_of_hash(&current).is_some() {
                history.push_state_with_url(&state, "", Some(hash))
            } else {
                history.replace_state_with_url(&state, "", Some(hash))
            }
        });
        if let Err(error) = pushed {
            tracing::warn!(?error, "could not update the address bar");
        }
    }
}

#[cfg(target_family = "wasm")]
pub use browser::{changes, initial, show};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hash_names_a_page_path() {
        assert_eq!(
            path_of_hash("#/book-club/channels/town-square"),
            Some("/book-club/channels/town-square")
        );
        assert_eq!(path_of_hash("#/"), None);
        assert_eq!(path_of_hash("#section"), None);
        assert_eq!(path_of_hash(""), None);
    }

    #[test]
    fn a_channel_link_becomes_a_hash_on_its_own_server() {
        assert_eq!(
            hash_for(
                "https://chat.example.com/book-club/messages/@ada",
                "https://chat.example.com/"
            )
            .as_deref(),
            Some("#/book-club/messages/@ada")
        );
        assert_eq!(
            hash_for(
                "https://other.example.com/t/channels/x",
                "https://chat.example.com"
            ),
            None
        );
    }
}
