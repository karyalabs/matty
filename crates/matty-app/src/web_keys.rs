//! Option shortcuts in a Mac browser (#905). With Option held, a Mac
//! browser reports the character Option composes, not the key: Cmd+Option+1
//! arrives as `¡` and Cmd+Option+W as `∑`, and GPUI's web platform builds the
//! keystroke from `KeyboardEvent.key`, so those shortcuts never matched. A
//! listener ahead of GPUI's sends such a key on again named by its physical
//! key, as the macOS platform does with `charactersIgnoringModifiers`.
//!
//! Option without Cmd or Ctrl still types its character.

#[cfg(target_family = "wasm")]
use std::cell::Cell;

#[cfg(target_family = "wasm")]
use wasm_bindgen::JsCast as _;

#[cfg(target_family = "wasm")]
thread_local! {
    /// Set while the mended copy of a key is on its way, so the listener
    /// lets it through.
    static SENDING: Cell<bool> = const { Cell::new(false) };
}

/// The key a US layout names for `code`, for the keys shortcuts use.
#[cfg_attr(not(target_family = "wasm"), allow(dead_code))]
fn key_for_code(code: &str) -> Option<String> {
    if let Some(letter) = code.strip_prefix("Key")
        && letter.len() == 1
    {
        return Some(letter.to_lowercase());
    }
    if let Some(digit) = code.strip_prefix("Digit")
        && digit.len() == 1
    {
        return Some(digit.to_owned());
    }
    let key = match code {
        "Minus" => "-",
        "Equal" => "=",
        "BracketLeft" => "[",
        "BracketRight" => "]",
        "Backslash" => "\\",
        "Semicolon" => ";",
        "Quote" => "'",
        "Comma" => ",",
        "Period" => ".",
        "Slash" => "/",
        "Backquote" => "`",
        _ => return None,
    };
    Some(key.to_owned())
}

/// The key to send on in place of `event`'s, when Option changed it.
#[cfg(target_family = "wasm")]
fn mended_key(event: &web_sys::KeyboardEvent) -> Option<String> {
    if !event.alt_key() || !(event.meta_key() || event.ctrl_key()) {
        return None;
    }
    // Named keys (arrows, Enter) arrive as themselves.
    let key = event.key();
    if key.chars().count() != 1 {
        return None;
    }
    let mended = key_for_code(&event.code())?;
    (mended != key.to_lowercase()).then_some(mended)
}

/// Listens on the window, ahead of GPUI's listeners, on a Mac browser.
#[cfg(target_family = "wasm")]
pub fn mend_option_keys() {
    if crate::actions::Platform::current() != crate::actions::Platform::MacOs {
        return;
    }
    let Some(window) = web_sys::window() else {
        return;
    };
    let listener = wasm_bindgen::closure::Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(
        |event: web_sys::KeyboardEvent| {
            if SENDING.with(Cell::get) {
                return;
            }
            let Some(key) = mended_key(&event) else {
                return;
            };
            let Some(target) = event.target() else {
                return;
            };
            let init = web_sys::KeyboardEventInit::new();
            init.set_key(&key);
            init.set_code(&event.code());
            init.set_alt_key(true);
            init.set_meta_key(event.meta_key());
            init.set_ctrl_key(event.ctrl_key());
            init.set_shift_key(event.shift_key());
            init.set_repeat(event.repeat());
            init.set_bubbles(true);
            init.set_cancelable(true);
            let Ok(mended) =
                web_sys::KeyboardEvent::new_with_keyboard_event_init_dict(&event.type_(), &init)
            else {
                return;
            };
            event.stop_immediate_propagation();
            SENDING.with(|sending| sending.set(true));
            let kept = target.dispatch_event(&mended);
            SENDING.with(|sending| sending.set(false));
            // GPUI cancels the copy when a shortcut took it; keep the
            // browser from acting on the original then.
            if !matches!(kept, Ok(true)) {
                event.prevent_default();
            }
        },
    );
    for name in ["keydown", "keyup"] {
        if let Err(error) = window.add_event_listener_with_callback_and_bool(
            name,
            listener.as_ref().unchecked_ref(),
            true,
        ) {
            tracing::warn!(?error, name, "could not listen for Option shortcuts");
        }
    }
    // The page lives as long as the app, so the listener does too.
    listener.forget();
}

#[cfg(test)]
mod tests {
    use super::key_for_code;

    #[test]
    fn physical_keys_name_the_unshifted_us_character() {
        assert_eq!(key_for_code("Digit1").as_deref(), Some("1"));
        assert_eq!(key_for_code("KeyW").as_deref(), Some("w"));
        assert_eq!(key_for_code("Backslash").as_deref(), Some("\\"));
        assert_eq!(key_for_code("ArrowUp"), None);
        assert_eq!(key_for_code("Numpad1"), None);
    }
}
