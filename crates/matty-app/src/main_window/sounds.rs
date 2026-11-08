//! Preferences' notification sound (#142): gpuicn's select over the web
//! app's sound names, saved to the user's `desktop_notification_sound` so
//! the web app and Matty agree. Choosing one plays it. Preferences'
//! Notifications section shows it under the Sound switch.

use std::collections::HashMap;

use gpui_kit::{Context, SharedString};
use matty_notify::Sound;

use super::MainWindow;

/// The prop that names the sound.
const PROP: &str = "desktop_notification_sound";

/// The sound `props` names, as the select's value.
pub(super) fn chosen(props: Option<&HashMap<String, String>>) -> SharedString {
    let value = props.and_then(|props| props.get(PROP)).map(String::as_str);
    Sound::from_prop(value).prop().into()
}

/// The sounds, as select options: Mattermost's names.
pub(super) fn options() -> Vec<(&'static str, SharedString)> {
    Sound::ALL
        .iter()
        .map(|sound| (sound.prop(), SharedString::from(sound.prop())))
        .collect()
}

impl MainWindow {
    /// Saves and plays the sound picked in Settings.
    pub(super) fn choose_sound(&mut self, value: &str, cx: &mut Context<Self>) {
        // The server's own value coming back is not a choice.
        if chosen(self.notify_props.as_ref()) == value {
            return;
        }
        let sound = Sound::from_prop(Some(value));
        matty_notify::play_sound(sound);
        self.set_notify_prop(PROP, sound.prop(), cx);
    }
}
