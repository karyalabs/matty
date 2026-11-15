//! Browser notifications for the web build (#910): the Notification API for
//! the banner and Web Audio for the sound, as the official web app shows a
//! notification and plays its sound itself (`notifyMe`, `ding`).
//!
//! Browsers allow both only after the person interacts with the page:
//! Firefox and Safari ask for notification permission only from a click or
//! a key press, and audio stays suspended until one. So the first click or
//! key press after Matty starts asks for permission, once, and wakes the
//! audio.

use std::cell::RefCell;
use std::collections::HashMap;

use async_channel::Sender;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{AudioContext, NotificationOptions, NotificationPermission, OscillatorType};

use crate::sound::Tone;
use crate::{Badge, Clicked, Notification, Notifier, Sound};

thread_local! {
    /// The page's audio, made on the first click or key press.
    static AUDIO: RefCell<Option<AudioContext>> = const { RefCell::new(None) };
}

/// Shows notifications through the browser.
pub struct Browser {
    clicked: Sender<Clicked>,
    /// The notifications on screen, by channel, so reading a channel can
    /// close its own.
    shown: RefCell<HashMap<String, Vec<web_sys::Notification>>>,
}

impl Browser {
    /// Waits for the person's first click or key press to ask for
    /// permission and wake the audio. Clicks arrive on `clicked`.
    pub fn new(clicked: Sender<Clicked>) -> Self {
        listen_for_first_gesture();
        Self {
            clicked,
            shown: RefCell::default(),
        }
    }
}

impl Notifier for Browser {
    fn show(&self, notification: &Notification) {
        if let Some(sound) = notification.sound {
            play(sound);
        }
        if web_sys::Notification::permission() != NotificationPermission::Granted {
            tracing::debug!("notifications are not allowed in this browser");
            return;
        }
        let options = NotificationOptions::new();
        options.set_body(&notification.body);
        // One per post, as the web app tags them; the sound is Matty's own.
        options.set_tag(&notification.post_id);
        options.set_silent(Some(true));
        let shown = match web_sys::Notification::new_with_options(&notification.title, &options) {
            Ok(shown) => shown,
            Err(error) => {
                tracing::debug!(?error, "the browser refused the notification");
                return;
            }
        };
        let target = Clicked {
            channel_id: notification.channel_id.clone(),
            post_id: notification.post_id.clone(),
            root_id: notification.root_id.clone(),
        };
        let clicked = self.clicked.clone();
        let this = shown.clone();
        let on_click = Closure::once_into_js(move || {
            if let Some(window) = web_sys::window()
                && let Err(error) = window.focus()
            {
                tracing::debug!(?error, "focusing the page");
            }
            this.close();
            if clicked.try_send(target).is_err() {
                tracing::debug!("nobody is listening for notification clicks");
            }
        });
        shown.set_onclick(Some(on_click.unchecked_ref()));
        self.shown
            .borrow_mut()
            .entry(notification.channel_id.clone())
            .or_default()
            .push(shown);
    }

    fn withdraw(&self, channel_id: &str) {
        for shown in self
            .shown
            .borrow_mut()
            .remove(channel_id)
            .unwrap_or_default()
        {
            shown.close();
        }
    }

    fn set_badge(&self, badge: Badge) {
        tracing::debug!(?badge, "no badge in the browser");
    }
}

/// Asks for permission and wakes the audio on the first click or key
/// press. The listener stays for the page's life and does nothing after.
fn listen_for_first_gesture() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let done = std::cell::Cell::new(false);
    let listener = Closure::<dyn FnMut()>::new(move || {
        if done.replace(true) {
            return;
        }
        if web_sys::Notification::permission() == NotificationPermission::Default
            && let Err(error) = web_sys::Notification::request_permission()
        {
            tracing::debug!(?error, "asking for notification permission");
        }
        wake_audio();
    });
    for event in GESTURES {
        if let Err(error) = window.add_event_listener_with_callback_and_bool(
            event,
            listener.as_ref().unchecked_ref(),
            true,
        ) {
            tracing::debug!(?error, event, "listening for the first gesture");
        }
    }
    listener.forget();
}

/// The events that count as the person interacting with the page.
const GESTURES: [&str; 2] = ["pointerdown", "keydown"];

/// Makes the audio context, or resumes it, inside a gesture.
fn wake_audio() {
    AUDIO.with(|audio| {
        let mut audio = audio.borrow_mut();
        if audio.is_none() {
            match AudioContext::new() {
                Ok(context) => *audio = Some(context),
                Err(error) => tracing::debug!(?error, "no audio in this browser"),
            }
        }
        if let Some(context) = audio.as_ref()
            && let Err(error) = context.resume()
        {
            tracing::debug!(?error, "resuming the audio");
        }
    });
}

/// Plays `sound` as a short chime of tones, Matty's stand-in for the web
/// app's audio files. Silent until the person has interacted with the page.
pub(crate) fn play(sound: Sound) {
    AUDIO.with(|audio| {
        let audio = audio.borrow();
        let Some(context) = audio.as_ref() else {
            tracing::debug!(?sound, "no audio before the first gesture");
            return;
        };
        let start = context.current_time();
        for tone in sound.chime() {
            if let Err(error) = play_tone(context, start, tone) {
                tracing::debug!(?error, ?sound, "playing the sound");
                return;
            }
        }
    });
}

fn play_tone(context: &AudioContext, start: f64, tone: &Tone) -> Result<(), wasm_bindgen::JsValue> {
    let oscillator = context.create_oscillator()?;
    oscillator.set_type(OscillatorType::Sine);
    oscillator.frequency().set_value(tone.frequency);
    let gain = context.create_gain()?;
    let begin = start + tone.at;
    let end = begin + tone.length;
    // A quick rise and a fading tail, so the tone does not click.
    gain.gain().set_value_at_time(0.0, begin)?;
    gain.gain()
        .linear_ramp_to_value_at_time(0.25, begin + 0.01)?;
    gain.gain().exponential_ramp_to_value_at_time(0.001, end)?;
    oscillator.connect_with_audio_node(&gain)?;
    gain.connect_with_audio_node(&context.destination())?;
    oscillator.start_with_when(begin)?;
    oscillator.stop_with_when(end)?;
    Ok(())
}
