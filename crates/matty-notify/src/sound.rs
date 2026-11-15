//! Notification sounds (#142): the Mattermost sound names the user picks
//! from, read from their `desktop_notification_sound` prop, and how each
//! one plays. macOS plays a system sound through NSSound and the web build
//! a chime through Web Audio. Linux and Windows notifications ask the system
//! to play its own new-message sound instead, so there [`play`], the preview
//! in Settings, does nothing.

/// One of the web app's notification sounds, by its `notify_props` name.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sound {
    /// Mattermost's default.
    #[default]
    Bing,
    /// Crackle.
    Crackle,
    /// Down.
    Down,
    /// Hello.
    Hello,
    /// Ripple.
    Ripple,
    /// Upstairs.
    Upstairs,
}

impl Sound {
    /// Every sound, in the order the web app lists them.
    pub const ALL: [Sound; 6] = [
        Sound::Bing,
        Sound::Crackle,
        Sound::Down,
        Sound::Hello,
        Sound::Ripple,
        Sound::Upstairs,
    ];

    /// Reads `desktop_notification_sound`: a name the web app writes, in
    /// any case. Missing or unknown is the default, Bing.
    pub fn from_prop(value: Option<&str>) -> Self {
        let value = value.unwrap_or("").trim();
        Self::ALL
            .into_iter()
            .find(|sound| sound.prop().eq_ignore_ascii_case(value))
            .unwrap_or_default()
    }

    /// The value for `desktop_notification_sound`, which is also its label.
    pub fn prop(self) -> &'static str {
        match self {
            Sound::Bing => "Bing",
            Sound::Crackle => "Crackle",
            Sound::Down => "Down",
            Sound::Hello => "Hello",
            Sound::Ripple => "Ripple",
            Sound::Upstairs => "Upstairs",
        }
    }

    /// The macOS system sound (`/System/Library/Sounds`) that stands in for
    /// it: the closest in character, since Matty does not ship the web
    /// app's audio files.
    pub fn system_name(self) -> &'static str {
        match self {
            Sound::Bing => "Glass",
            Sound::Crackle => "Frog",
            Sound::Down => "Bottle",
            Sound::Hello => "Hero",
            Sound::Ripple => "Submarine",
            Sound::Upstairs => "Purr",
        }
    }
}

/// One tone of a chime: its pitch in hertz, and when it starts and how long
/// it lasts, in seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Tone {
    pub frequency: f32,
    pub at: f64,
    pub length: f64,
}

impl Sound {
    /// The chime the web build plays for it with Web Audio, a shape of its
    /// own for each, since Matty does not ship the web app's audio files.
    #[cfg_attr(not(target_family = "wasm"), allow(dead_code))]
    pub(crate) fn chime(self) -> &'static [Tone] {
        match self {
            Sound::Bing => &[Tone {
                frequency: 1318.5,
                at: 0.0,
                length: 0.35,
            }],
            Sound::Crackle => &[
                Tone {
                    frequency: 880.0,
                    at: 0.0,
                    length: 0.05,
                },
                Tone {
                    frequency: 1046.5,
                    at: 0.07,
                    length: 0.05,
                },
                Tone {
                    frequency: 880.0,
                    at: 0.14,
                    length: 0.05,
                },
            ],
            Sound::Down => &[
                Tone {
                    frequency: 987.8,
                    at: 0.0,
                    length: 0.14,
                },
                Tone {
                    frequency: 659.3,
                    at: 0.14,
                    length: 0.25,
                },
            ],
            Sound::Hello => &[
                Tone {
                    frequency: 783.99,
                    at: 0.0,
                    length: 0.12,
                },
                Tone {
                    frequency: 1046.5,
                    at: 0.12,
                    length: 0.3,
                },
            ],
            Sound::Ripple => &[
                Tone {
                    frequency: 1046.5,
                    at: 0.0,
                    length: 0.12,
                },
                Tone {
                    frequency: 1174.7,
                    at: 0.08,
                    length: 0.12,
                },
                Tone {
                    frequency: 1318.5,
                    at: 0.16,
                    length: 0.25,
                },
            ],
            Sound::Upstairs => &[
                Tone {
                    frequency: 523.25,
                    at: 0.0,
                    length: 0.1,
                },
                Tone {
                    frequency: 659.3,
                    at: 0.1,
                    length: 0.1,
                },
                Tone {
                    frequency: 783.99,
                    at: 0.2,
                    length: 0.25,
                },
            ],
        }
    }
}

/// Plays `sound` now: a notification's sound, or a preview from the
/// preferences. macOS and the web build have a player.
pub fn play(sound: Sound) {
    #[cfg(target_os = "macos")]
    crate::macos::play(sound);
    #[cfg(target_family = "wasm")]
    crate::web::play(sound);
    #[cfg(not(any(target_os = "macos", target_family = "wasm")))]
    tracing::debug!(?sound, "no system sound API on this platform");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_web_apps_names_with_bing_by_default() {
        assert_eq!(Sound::from_prop(None), Sound::Bing);
        assert_eq!(Sound::from_prop(Some("")), Sound::Bing);
        assert_eq!(Sound::from_prop(Some("Ripple")), Sound::Ripple);
        assert_eq!(Sound::from_prop(Some("upstairs")), Sound::Upstairs);
        assert_eq!(Sound::from_prop(Some(" Hello ")), Sound::Hello);
        assert_eq!(Sound::from_prop(Some("Klaxon")), Sound::Bing);
    }

    #[test]
    fn every_sound_has_its_own_chime_that_fits_in_half_a_second() {
        let mut chimes: Vec<_> = Sound::ALL.iter().map(|sound| sound.chime()).collect();
        for chime in &chimes {
            assert!(!chime.is_empty());
            for tone in *chime {
                assert!(tone.at + tone.length <= 0.5, "{tone:?}");
                assert!((200.0..2000.0).contains(&tone.frequency), "{tone:?}");
            }
        }
        chimes.dedup();
        assert_eq!(chimes.len(), Sound::ALL.len());
    }

    #[test]
    fn every_sound_round_trips_and_has_its_own_system_sound() {
        for sound in Sound::ALL {
            assert_eq!(Sound::from_prop(Some(sound.prop())), sound);
        }
        let mut names: Vec<_> = Sound::ALL.iter().map(|s| s.system_name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Sound::ALL.len());
    }
}
