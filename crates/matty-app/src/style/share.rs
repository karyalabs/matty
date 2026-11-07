//! Sharing styles (#707): a style as text people paste or keep in a file,
//! a pasted Mattermost theme turned into one, and the named styles a person
//! saves to come back to.
//!
//! A shared style is a small JSON object:
//!
//! ```json
//! {
//!   "matty_style": 1,
//!   "name": "Dusk",
//!   "colors": { "neutral_hue": 260, "neutral_chroma": 0.006, … },
//!   "radius": "soft",
//!   "ui_font": "IBM Plex Sans",
//!   "code_font": "IBM Plex Mono"
//! }
//! ```
//!
//! [`import`] reads it back, or a Mattermost theme's JSON, and checks every
//! number before it becomes a [`StyleSpec`]. Fonts (#706) are a setting of
//! their own, not part of a style's numbers, but a shared or saved style
//! carries them so it looks the same elsewhere. A font the machine doesn't
//! have is kept: Matty shows Plex in its place and Preferences says so. The contrast guard in
//! [`palette::tokens`] runs on whatever comes in, so an imported style is
//! as readable as one made in the editor.

use serde::{Deserialize, Serialize};

use super::{MONO, Radius, SANS, StyleSpec, palette::StyleColors};
use crate::mattermost_theme::{self, Theme};
use crate::ui::color_picker::Oklch;
use crate::ui::theme::ThemeMode;

/// The format version [`export`] writes. [`import`] refuses later ones.
pub const VERSION: u32 = 1;

/// The most styles a person can save.
pub const SAVED_MAX: usize = 24;

/// The longest name a saved style keeps, in characters.
pub const NAME_MAX: usize = 40;

/// The longest font family name kept, in characters.
const FONT_MAX: usize = 100;

/// The fonts a shared or saved style carries; `None` is IBM Plex, as in
/// `Settings::ui_font` and `Settings::code_font`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fonts {
    #[serde(default)]
    pub ui_font: Option<String>,
    #[serde(default)]
    pub code_font: Option<String>,
}

impl Fonts {
    /// The fonts `settings` has chosen.
    pub fn of(settings: &crate::settings::Settings) -> Self {
        Self {
            ui_font: font(settings.ui_font.as_deref(), SANS),
            code_font: font(settings.code_font.as_deref(), MONO),
        }
    }
}

/// `family` tidied, or `None` when it is empty or Plex (`default`).
fn font(family: Option<&str>, default: &str) -> Option<String> {
    let family = family?;
    let family: String = family
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(FONT_MAX)
        .collect();
    let family = family.trim();
    (!family.is_empty() && !family.eq_ignore_ascii_case(default)).then(|| family.to_owned())
}

/// A style saved under a name (#707), kept in `Settings::saved_styles`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedStyle {
    pub name: String,
    pub style: StyleSpec,
    /// Its fonts; saved before fonts could be chosen means Plex.
    #[serde(flatten)]
    pub fonts: Fonts,
}

/// A style as it is written to text.
#[derive(Debug, Serialize, Deserialize)]
struct StyleFile {
    matty_style: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    colors: StyleColors,
    #[serde(default)]
    radius: Radius,
    /// Written as the family name, Plex's included, so a style made with
    /// Plex sets Plex. A style without the key keeps the reader's font.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ui_font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    code_font: Option<String>,
}

/// A style read by [`import`].
#[derive(Clone, Debug, PartialEq)]
pub struct Imported {
    /// The name it came with, if any: a shared style's own, or a built-in
    /// Mattermost theme's.
    pub name: Option<String>,
    pub style: StyleSpec,
    /// Whether it was converted from a Mattermost theme.
    pub from_mattermost: bool,
    /// The fonts it sets, or `None` to keep the person's own: a Mattermost
    /// theme, or a style shared without fonts.
    pub fonts: Option<Fonts>,
}

/// Why [`import`] read nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportError {
    /// Neither a Matty style nor a Mattermost theme.
    NotAStyle,
    /// A Matty style from a newer version of Matty.
    TooNew,
}

/// `style` with `fonts` as shareable text, under `name` when it has one.
pub fn export(style: &StyleSpec, name: Option<&str>, fonts: &Fonts) -> String {
    let round = |x: f32| (x * 1000.).round() / 1000.;
    let c = style.colors;
    let file = StyleFile {
        matty_style: VERSION,
        name: name.map(clean_name).filter(|name| !name.is_empty()),
        colors: StyleColors {
            neutral_hue: round(c.neutral_hue),
            neutral_chroma: round(c.neutral_chroma),
            accent_hue: round(c.accent_hue),
            accent_chroma: round(c.accent_chroma),
            accent_light: round(c.accent_light),
            accent_dark: round(c.accent_dark),
        },
        radius: style.radius,
        ui_font: Some(fonts.ui_font.clone().unwrap_or_else(|| SANS.to_owned())),
        code_font: Some(fonts.code_font.clone().unwrap_or_else(|| MONO.to_owned())),
    };
    serde_json::to_string_pretty(&file).expect("a style serializes")
}

/// The name a style file suggests for `name`: safe on every platform.
pub fn file_name(name: Option<&str>) -> String {
    let stem: String = name
        .unwrap_or("")
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let stem = stem.trim();
    let stem = if stem.is_empty() { "My style" } else { stem };
    format!("{stem}.matty-style.json")
}

/// Reads `text` as a Matty style or, failing that, a Mattermost theme
/// (the JSON Mattermost's "Copy theme colors" gives).
pub fn import(text: &str) -> Result<Imported, ImportError> {
    let text = text.trim();
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|_| ImportError::NotAStyle)?;
    let object = value.as_object().ok_or(ImportError::NotAStyle)?;
    if let Some(version) = object.get("matty_style") {
        if version.as_u64().is_some_and(|v| v > u64::from(VERSION)) {
            return Err(ImportError::TooNew);
        }
        let file: StyleFile =
            serde_json::from_value(value.clone()).map_err(|_| ImportError::NotAStyle)?;
        let colors = checked(file.colors).ok_or(ImportError::NotAStyle)?;
        let fonts = (file.ui_font.is_some() || file.code_font.is_some()).then(|| Fonts {
            ui_font: font(file.ui_font.as_deref(), SANS),
            code_font: font(file.code_font.as_deref(), MONO),
        });
        return Ok(Imported {
            name: file.name.map(|n| clean_name(&n)).filter(|n| !n.is_empty()),
            style: StyleSpec {
                colors,
                radius: file.radius,
            },
            from_mattermost: false,
            fonts,
        });
    }
    let built_in = object
        .get("type")
        .and_then(|v| v.as_str())
        .is_some_and(|kind| mattermost_theme::BUILT_IN.iter().any(|(n, _)| *n == kind));
    let has_colors = mattermost_theme::KEYS.iter().any(|key| {
        object
            .get(*key)
            .and_then(|v| v.as_str())
            .and_then(mattermost_theme::parse_color)
            .is_some()
    });
    if !built_in && !has_colors {
        return Err(ImportError::NotAStyle);
    }
    let theme = Theme::parse(text);
    Ok(Imported {
        name: theme.is_built_in().then(|| theme.name()),
        style: from_mattermost(&theme),
        from_mattermost: true,
        fonts: None,
    })
}

/// A Mattermost theme as a Matty style. Matty's styles are a few numbers,
/// so this keeps what makes the theme recognisable: its button color as
/// the accent, and its sidebar's or channel's tint in the greys. The
/// theme's own light or dark decides which mode the accent is matched in.
pub fn from_mattermost(theme: &Theme) -> StyleSpec {
    let m = &theme.colors;
    let accent = Oklch::from_rgba(m.button_bg);
    let center = Oklch::from_rgba(m.center_channel_bg);
    let sidebar = Oklch::from_rgba(m.sidebar_bg);
    let (neutral_hue, neutral_chroma) = {
        let from_center = center.chroma * 0.5;
        let from_sidebar = sidebar.chroma * 0.15;
        let (hue, chroma) = if from_center >= from_sidebar {
            (center.hue, from_center)
        } else {
            (sidebar.hue, from_sidebar)
        };
        if chroma < 0.002 {
            (accent.hue, 0.002)
        } else {
            (hue, chroma.min(0.015))
        }
    };
    let lightness = accent.lightness.clamp(0., 1.);
    let (accent_light, accent_dark) = match theme.mode() {
        ThemeMode::Light => (
            lightness.clamp(0.05, 0.98),
            (1.28 - lightness).clamp(0.6, 0.93),
        ),
        ThemeMode::Dark => (
            (1.28 - lightness).clamp(0.3, 0.68),
            lightness.clamp(0.6, 0.93),
        ),
    };
    StyleSpec {
        colors: StyleColors {
            neutral_hue: neutral_hue.rem_euclid(360.),
            neutral_chroma,
            accent_hue: accent.hue.rem_euclid(360.),
            accent_chroma: accent.chroma.clamp(0., MAX_CHROMA),
            accent_light,
            accent_dark,
        },
        radius: Radius::Soft,
    }
}

/// The highest chroma a style keeps; past it most hues leave sRGB.
const MAX_CHROMA: f32 = crate::ui::color_picker::MAX_CHROMA;

/// `colors` with every number finite and in its range, or `None` when one
/// is not a number at all.
fn checked(colors: StyleColors) -> Option<StyleColors> {
    let StyleColors {
        neutral_hue,
        neutral_chroma,
        accent_hue,
        accent_chroma,
        accent_light,
        accent_dark,
    } = colors;
    let all = [
        neutral_hue,
        neutral_chroma,
        accent_hue,
        accent_chroma,
        accent_light,
        accent_dark,
    ];
    if !all.iter().all(|x| x.is_finite()) {
        return None;
    }
    Some(StyleColors {
        neutral_hue: neutral_hue.rem_euclid(360.),
        // Greys stay greys: past this they read as a color.
        neutral_chroma: neutral_chroma.clamp(0., 0.03),
        accent_hue: accent_hue.rem_euclid(360.),
        accent_chroma: accent_chroma.clamp(0., MAX_CHROMA),
        accent_light: accent_light.clamp(0.05, 0.98),
        accent_dark: accent_dark.clamp(0.05, 0.98),
    })
}

/// `name` on one line, trimmed and at most [`NAME_MAX`] characters.
pub fn clean_name(name: &str) -> String {
    let one_line: String = name
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    one_line
        .trim()
        .chars()
        .take(NAME_MAX)
        .collect::<String>()
        .trim()
        .to_owned()
}

/// `saved` with `style` stored under `name`: replacing a style of the same
/// name (ignoring case), else added at the end while there is room.
/// Returns whether it was stored.
pub fn save(saved: &mut Vec<SavedStyle>, name: &str, style: StyleSpec, fonts: Fonts) -> bool {
    let name = clean_name(name);
    if name.is_empty() {
        return false;
    }
    if let Some(existing) = saved
        .iter_mut()
        .find(|s| s.name.to_lowercase() == name.to_lowercase())
    {
        existing.name = name;
        existing.style = style;
        existing.fonts = fonts;
        return true;
    }
    if saved.len() >= SAVED_MAX {
        return false;
    }
    saved.push(SavedStyle { name, style, fonts });
    true
}

/// A name for a new style that no saved style has yet: `base`, else
/// `base 2`, `base 3` and so on.
pub fn unused_name(saved: &[SavedStyle], base: &str) -> String {
    let taken = |name: &str| {
        saved
            .iter()
            .any(|s| s.name.to_lowercase() == name.to_lowercase())
    };
    if !taken(base) {
        return base.to_owned();
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|name| !taken(name))
        .expect("some number is free")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{Preset, palette};

    fn close(a: StyleSpec, b: StyleSpec) -> bool {
        let (a, b) = (a.colors, b.colors);
        [
            (a.neutral_hue, b.neutral_hue),
            (a.neutral_chroma, b.neutral_chroma),
            (a.accent_hue, b.accent_hue),
            (a.accent_chroma, b.accent_chroma),
            (a.accent_light, b.accent_light),
            (a.accent_dark, b.accent_dark),
        ]
        .iter()
        .all(|(x, y)| (x - y).abs() < 0.001)
    }

    #[test]
    fn a_style_survives_export_and_import() {
        let mut style = Preset::Porcelain.style().with_accent(0.6123, 0.15, 30.4);
        style.radius = Radius::Round;
        let text = export(&style, Some("  Dusk\n"), &Fonts::default());
        assert!(text.contains("\"matty_style\": 1"));
        let back = import(&text).unwrap();
        assert_eq!(back.name.as_deref(), Some("Dusk"));
        assert_eq!(back.style.radius, Radius::Round);
        assert!(!back.from_mattermost);
        assert!(close(back.style, style));
        // An unnamed style has no name key at all.
        assert!(!export(&style, None, &Fonts::default()).contains("name"));
    }

    #[test]
    fn bad_text_is_refused() {
        for text in [
            "",
            "hello",
            "[1, 2]",
            "{}",
            r#"{"matty_style": 1}"#,
            r#"{"matty_style": 1, "colors": {"neutral_hue": "red"}}"#,
            r#"{"sidebarBg": "not a color"}"#,
        ] {
            assert_eq!(import(text), Err(ImportError::NotAStyle), "{text:?}");
        }
        let newer = export(&Preset::Graphite.style(), None, &Fonts::default())
            .replace("\"matty_style\": 1", "\"matty_style\": 9");
        assert_eq!(import(&newer), Err(ImportError::TooNew));
    }

    #[test]
    fn numbers_out_of_range_are_brought_in() {
        let text = r#"{"matty_style": 1, "colors": {
            "neutral_hue": -20, "neutral_chroma": 4,
            "accent_hue": 725, "accent_chroma": 2,
            "accent_light": 7, "accent_dark": -1
        }}"#;
        let c = import(text).unwrap().style.colors;
        assert_eq!(c.neutral_hue, 340.);
        assert_eq!(c.neutral_chroma, 0.03);
        assert_eq!(c.accent_hue, 5.);
        assert_eq!(c.accent_chroma, MAX_CHROMA);
        assert_eq!(c.accent_light, 0.98);
        assert_eq!(c.accent_dark, 0.05);
        // The radius defaults when it is left out.
        assert_eq!(import(text).unwrap().style.radius, Radius::Soft);
    }

    #[test]
    fn an_extreme_import_still_reads() {
        // A pale accent on near-white greys: the contrast guard has to
        // step in, and does.
        let text = r#"{"matty_style": 1, "colors": {
            "neutral_hue": 90, "neutral_chroma": 0.03,
            "accent_hue": 100, "accent_chroma": 0.2,
            "accent_light": 0.97, "accent_dark": 0.1
        }}"#;
        let style = import(text).unwrap().style;
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            let tokens = palette::tokens(&style.colors, mode);
            assert!(!tokens.adjusted.is_empty());
            let fg = tokens.get("primary-foreground").unwrap();
            let bg = tokens.get("primary").unwrap();
            assert!(crate::contrast::ratio(fg, bg) >= 4.5, "{mode:?}");
        }
    }

    #[test]
    fn mattermost_themes_convert() {
        // Denim: blue buttons, a navy sidebar tinting the greys.
        let denim = import(mattermost_theme::BUILT_IN[0].1).unwrap();
        assert!(denim.from_mattermost);
        assert_eq!(denim.name.as_deref(), Some("Denim"));
        let c = denim.style.colors;
        assert!((240. ..280.).contains(&c.accent_hue), "{}", c.accent_hue);
        assert!(c.accent_chroma > 0.1);
        assert!(c.neutral_chroma > 0.005 && c.neutral_chroma <= 0.015);
        assert!(c.accent_dark > c.accent_light);
        // Onyx is dark: its button is matched in dark mode.
        let onyx = Theme::built_in("Onyx").unwrap();
        let style = from_mattermost(&onyx);
        let button = Oklch::from_rgba(onyx.colors.button_bg).lightness;
        assert!((style.colors.accent_dark - button.clamp(0.6, 0.93)).abs() < 1e-6);
        // A custom theme pasted from Mattermost has no name.
        let custom =
            r##"{"sidebarBg":"#3a1e5c","buttonBg":"#7a3fd9","centerChannelBg":"#ffffff"}"##;
        let custom = import(custom).unwrap();
        assert_eq!(custom.name, None);
        assert!((280. ..310.).contains(&custom.style.colors.accent_hue));
        // A theme named only by its type is that built-in.
        assert_eq!(
            import(r#"{"type": "Quartz"}"#).unwrap().name.as_deref(),
            Some("Quartz")
        );
    }

    #[test]
    fn saving_replaces_by_name_and_stops_at_the_limit() {
        let mut saved = Vec::new();
        let graphite = Preset::Graphite.style();
        let porcelain = Preset::Porcelain.style();
        assert!(save(&mut saved, " Work ", graphite, Fonts::default()));
        assert!(save(&mut saved, "work", porcelain, Fonts::default()));
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].name, "work");
        assert_eq!(saved[0].style, porcelain);
        assert!(!save(&mut saved, "   ", graphite, Fonts::default()));
        for n in 1..SAVED_MAX {
            assert!(save(
                &mut saved,
                &format!("Style {n}"),
                graphite,
                Fonts::default()
            ));
        }
        assert!(!save(
            &mut saved,
            "One too many",
            graphite,
            Fonts::default()
        ));
        assert_eq!(saved.len(), SAVED_MAX);
        assert_eq!(unused_name(&saved, "Style"), "Style");
        assert_eq!(unused_name(&saved, "Style 1"), "Style 1 2");
        assert_eq!(unused_name(&saved, "work"), "work 2");
    }

    #[test]
    fn fonts_travel_with_a_style() {
        let style = Preset::Graphite.style();
        let inter = Fonts {
            ui_font: Some("Inter".into()),
            code_font: None,
        };
        let text = export(&style, None, &inter);
        // Plex is written by name, so it sets Plex where it is pasted.
        assert!(text.contains("\"code_font\": \"IBM Plex Mono\""));
        assert_eq!(import(&text).unwrap().fonts, Some(inter));
        // A family this machine lacks is kept as written.
        let missing = text.replace("Inter", "Comic Neue");
        assert_eq!(
            import(&missing).unwrap().fonts.unwrap().ui_font.as_deref(),
            Some("Comic Neue")
        );
        // No font keys, or a Mattermost theme: keep the person's own.
        let bare = r#"{"matty_style": 1, "colors": {
            "neutral_hue": 0, "neutral_chroma": 0, "accent_hue": 0,
            "accent_chroma": 0, "accent_light": 0.3, "accent_dark": 0.9
        }}"#;
        assert_eq!(import(bare).unwrap().fonts, None);
        assert_eq!(import(mattermost_theme::BUILT_IN[0].1).unwrap().fonts, None);
        // Saved styles from before fonts read as Plex.
        let old: SavedStyle = serde_json::from_str(&format!(
            r#"{{"name": "Old", "style": {}}}"#,
            serde_json::to_string(&style).unwrap()
        ))
        .unwrap();
        assert_eq!(old.fonts, Fonts::default());
        assert_eq!(font(Some(" ibm plex sans "), SANS), None);
        assert_eq!(font(Some("  "), SANS), None);
    }

    #[test]
    fn names_and_file_names_are_tidy() {
        assert_eq!(clean_name("  a\tb\n"), "a b");
        assert_eq!(clean_name(&"x".repeat(60)).chars().count(), NAME_MAX);
        assert_eq!(
            file_name(Some("Dusk / Night")),
            "Dusk - Night.matty-style.json"
        );
        assert_eq!(file_name(None), "My style.matty-style.json");
    }
}
