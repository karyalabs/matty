//! Ctrl+Up and Ctrl+Down (Cmd on macOS) in a composer bring back the
//! messages and commands sent earlier in the session, as the official
//! client's message history does (#1023). One list serves every composer;
//! each composer keeps its own place in it.

use gpui_kit::{App, Global, SharedString};

/// The official client keeps this many (`Constants.MAX_PREV_MSGS`).
const LIMIT: usize = 100;

#[derive(Default)]
struct Sent(Vec<SharedString>);

impl Global for Sent {}

/// Remembers `text` as sent, newest last.
pub fn record(text: &str, cx: &mut App) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    let sent = &mut cx.default_global::<Sent>().0;
    if sent.last().is_some_and(|last| last.as_ref() == text) {
        return;
    }
    sent.push(SharedString::from(text.to_string()));
    if sent.len() > LIMIT {
        sent.remove(0);
    }
}

/// Whether the key is Ctrl+Up or Ctrl+Down (Cmd on macOS): `Some(true)`
/// for older, `Some(false)` for newer.
pub fn direction(keystroke: &gpui_kit::Keystroke) -> Option<bool> {
    let mods = &keystroke.modifiers;
    let command = if cfg!(target_os = "macos") {
        mods.platform && !mods.control
    } else {
        mods.control && !mods.platform
    };
    if !command || mods.shift || mods.alt || mods.function {
        return None;
    }
    match keystroke.key.as_str() {
        "up" => Some(true),
        "down" => Some(false),
        _ => None,
    }
}

/// A composer's place in the sent messages.
#[derive(Default)]
pub struct Browse {
    place: Option<usize>,
    /// The sent message the composer shows, until the user edits it.
    shown: Option<SharedString>,
}

impl Browse {
    /// Whether Ctrl+Up or Ctrl+Down is for the history: the composer is
    /// empty or shows a sent message. Text the user typed keeps the key.
    pub fn claims(&self, draft_is_empty: bool) -> bool {
        draft_is_empty || self.place.is_some()
    }

    /// The composer's text changed: typing over a sent message starts a
    /// new draft.
    pub fn edited(&mut self, text: &str) {
        if self
            .shown
            .as_ref()
            .is_some_and(|shown| shown.as_ref() != text)
        {
            *self = Self::default();
        }
    }

    /// A send starts over at the newest message.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// One step older or newer from a composer showing `draft`: the text
    /// to show, or `None` when there is nothing there.
    pub fn step(&mut self, draft: &str, older: bool, cx: &App) -> Option<SharedString> {
        let sent = cx
            .try_global::<Sent>()
            .map(|sent| sent.0.as_slice())
            .unwrap_or_default();
        let (place, text) = walk(sent, self.place, draft, older)?;
        self.place = place;
        self.shown = place.map(|_| text.clone());
        Some(text)
    }
}

fn walk(
    sent: &[SharedString],
    place: Option<usize>,
    draft: &str,
    older: bool,
) -> Option<(Option<usize>, SharedString)> {
    let place = if draft.trim().is_empty() {
        None
    } else {
        let at = place?;
        if sent.get(at).map(|text| text.as_ref()) != Some(draft) {
            return None;
        }
        Some(at)
    };
    let next = match (place, older) {
        (None, true) => sent.len().checked_sub(1)?,
        (Some(at), true) => at.saturating_sub(1),
        (None, false) => return None,
        (Some(at), false) if at + 1 < sent.len() => at + 1,
        (Some(_), false) => return Some((None, SharedString::default())),
    };
    Some((Some(next), sent.get(next)?.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sent() -> Vec<SharedString> {
        vec!["first".into(), "second".into(), "/away".into()]
    }

    #[test]
    fn up_walks_back_and_down_walks_forward_to_empty() {
        let sent = sent();
        let (place, text) = walk(&sent, None, "", true).unwrap();
        assert_eq!((place, text.as_ref()), (Some(2), "/away"));
        let (place, text) = walk(&sent, place, &text, true).unwrap();
        assert_eq!((place, text.as_ref()), (Some(1), "second"));
        let (place, text) = walk(&sent, place, &text, true).unwrap();
        assert_eq!((place, text.as_ref()), (Some(0), "first"));
        // The oldest stays.
        let (place, text) = walk(&sent, place, &text, true).unwrap();
        assert_eq!((place, text.as_ref()), (Some(0), "first"));
        let (place, text) = walk(&sent, place, &text, false).unwrap();
        assert_eq!((place, text.as_ref()), (Some(1), "second"));
        let (place, text) = walk(&sent, place, &text, false).unwrap();
        assert_eq!((place, text.as_ref()), (Some(2), "/away"));
        let (place, text) = walk(&sent, place, &text, false).unwrap();
        assert_eq!((place, text.as_ref()), (None, ""));
    }

    #[test]
    fn typed_text_and_an_empty_history_leave_the_key_alone() {
        let sent = sent();
        assert!(walk(&sent, None, "typing", true).is_none());
        assert!(walk(&sent, Some(1), "second, edited", true).is_none());
        assert!(walk(&sent, None, "", false).is_none());
        assert!(walk(&[], None, "", true).is_none());
    }
}
