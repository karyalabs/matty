//! Fonts (#706): IBM Plex, bundled; the font files a person adds, kept in
//! the data folder and loaded at start; and the families Preferences offers
//! for the interface and for code.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::{App, Global, SharedString, TextSystem, font, px};

use crate::settings::Settings;
use crate::style::{MONO, SANS};

/// Where the font files a person added are kept.
pub fn dir() -> Option<PathBuf> {
    #[cfg(target_family = "wasm")]
    return None;
    #[cfg(not(target_family = "wasm"))]
    dirs::data_local_dir().map(|dir| dir.join("Matty").join("fonts"))
}

/// Whether `path` names a font file Matty loads.
fn is_font_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("ttf") || extension.eq_ignore_ascii_case("otf")
        })
}

/// Draws text the way Mattermost's own apps do on macOS: antialiased, with
/// no font smoothing. GPUI copies CoreGraphics' smoothing, which thickens
/// every stroke (most of all for light text on dark), so Matty read as bold
/// next to the official client, whose CSS sets `-webkit-font-smoothing:
/// antialiased`. GPUI reads `AppleFontSmoothing` from Matty's own defaults
/// once, at the first glyph, so this has to run before anything is drawn.
#[cfg(target_os = "macos")]
fn match_mattermost_text_rendering() {
    use objc2_foundation::{NSString, NSUserDefaults};
    NSUserDefaults::standardUserDefaults()
        .setInteger_forKey(0, &NSString::from_str("AppleFontSmoothing"));
}

/// Loads IBM Plex and the person's own font files, then lists the families
/// this machine has.
pub fn load(cx: &mut App) {
    #[cfg(target_os = "macos")]
    match_mattermost_text_rendering();
    cx.text_system()
        .add_fonts(vec![
            Cow::Borrowed(include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf")),
            Cow::Borrowed(include_bytes!("../assets/fonts/IBMPlexSans-Italic.ttf")),
            Cow::Borrowed(include_bytes!("../assets/fonts/IBMPlexSans-Medium.ttf")),
            Cow::Borrowed(include_bytes!("../assets/fonts/IBMPlexSans-SemiBold.ttf")),
            Cow::Borrowed(include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf")),
            Cow::Borrowed(include_bytes!("../assets/fonts/IBMPlexMono-Medium.ttf")),
        ])
        .expect("load bundled IBM Plex fonts");
    let own = dir()
        .and_then(|dir| std::fs::read_dir(dir).ok())
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| is_font_file(path));
    for path in own {
        // One by one, so a broken file doesn't keep the others out.
        let loaded = std::fs::read(&path)
            .map_err(|error| error.to_string())
            .and_then(|bytes| {
                cx.text_system()
                    .add_fonts(vec![bytes.into()])
                    .map_err(|error| error.to_string())
            });
        if let Err(error) = loaded {
            tracing::warn!(path = %path.display(), %error, "could not load a font file");
        }
    }
    let names = list(cx.text_system());
    cx.set_global(Families {
        names,
        mono: None,
        scanning: false,
    });
}

fn list(text_system: &TextSystem) -> Vec<SharedString> {
    text_system
        .all_font_names()
        .into_iter()
        // Families the system keeps for itself start with a dot (macOS).
        .filter(|name| !name.starts_with('.'))
        .map(SharedString::from)
        .collect()
}

/// The font families this machine has, sorted, and which of them are
/// monospace once [`scan`] has looked.
#[derive(Default)]
pub struct Families {
    names: Vec<SharedString>,
    mono: Option<BTreeSet<SharedString>>,
    scanning: bool,
}

impl Global for Families {}

impl Families {
    /// Every family, sorted.
    pub fn names(&self) -> &[SharedString] {
        &self.names
    }

    /// The monospace families, or `None` until [`scan`] has looked.
    pub fn mono(&self) -> Option<&BTreeSet<SharedString>> {
        self.mono.as_ref()
    }

    /// Whether [`scan`] is looking now.
    pub fn scanning(&self) -> bool {
        self.scanning
    }

    /// Whether this machine has `family`.
    pub fn has(&self, family: &str) -> bool {
        self.names
            .binary_search_by(|name| name.as_ref().cmp(family))
            .is_ok()
    }
}

/// The families on this machine; empty before [`load`].
pub fn families(cx: &App) -> &Families {
    static EMPTY: Families = Families {
        names: Vec::new(),
        mono: None,
        scanning: false,
    };
    cx.try_global::<Families>().unwrap_or(&EMPTY)
}

/// Whether `family` sets every character the same width: `i`, `m` and `W`
/// advance alike.
fn is_monospace(text_system: &TextSystem, family: &SharedString) -> bool {
    let id = text_system.resolve_font(&font(family.clone()));
    // A family that can't be loaded resolves to a fallback.
    if text_system
        .get_font_for_id(id)
        .is_some_and(|resolved| resolved.family != *family)
    {
        return false;
    }
    let size = px(16.);
    let advance = |ch| {
        text_system
            .advance(id, size, ch)
            .ok()
            .map(|size| size.width)
    };
    match (advance('i'), advance('m'), advance('W')) {
        (Some(i), Some(m), Some(w)) => {
            i > px(0.) && (i - m).abs() < px(0.01) && (i - w).abs() < px(0.01)
        }
        _ => false,
    }
}

/// Finds the monospace families in the background, once, and calls `done`
/// when it knows. Loading every family takes a moment on a machine with
/// many, so the code font menu shows "Loading…" until then.
pub fn scan(cx: &mut App, done: impl FnOnce(&mut App) + 'static) {
    let families = families(cx);
    if families.mono.is_some() || families.scanning || !cx.has_global::<Families>() {
        return;
    }
    let names = families.names.clone();
    cx.global_mut::<Families>().scanning = true;
    let text_system: Arc<TextSystem> = cx.text_system().clone();
    cx.spawn(async move |cx| {
        let mono = cx
            .background_executor()
            .spawn(async move {
                names
                    .into_iter()
                    .filter(|name| is_monospace(&text_system, name))
                    .collect::<BTreeSet<_>>()
            })
            .await;
        cx.update(|cx| {
            if cx.has_global::<Families>() {
                let families = cx.global_mut::<Families>();
                families.mono = Some(mono);
                families.scanning = false;
            }
            done(cx);
        });
    })
    .detach();
}

/// The families the theme uses for `settings`: the person's choices when
/// this machine has them, otherwise IBM Plex.
#[derive(Clone, Debug, PartialEq)]
pub struct Chosen {
    pub sans: SharedString,
    pub mono: SharedString,
    /// The chosen families this machine doesn't have.
    pub missing: Vec<SharedString>,
}

/// What [`Chosen`] `settings` asks for.
pub fn chosen(settings: &Settings, cx: &App) -> Chosen {
    let families = families(cx);
    let mut missing = Vec::new();
    let mut pick = |wanted: &Option<String>, default: &'static str| match wanted {
        Some(family) if families.has(family) => SharedString::from(family.clone()),
        Some(family) => {
            missing.push(SharedString::from(family.clone()));
            default.into()
        }
        None => default.into(),
    };
    let sans = pick(&settings.ui_font, SANS);
    let mono = pick(&settings.code_font, MONO);
    Chosen {
        sans,
        mono,
        missing,
    }
}

/// Why a font file could not be added.
#[derive(Debug)]
pub enum AddError {
    /// Not a TrueType or OpenType font, or one without a family name.
    NotAFont,
    /// Reading the file or keeping the copy failed.
    Io(std::io::Error),
    /// The text system refused it.
    Rejected(String),
}

/// A font file's family name and whether it is monospace.
fn describe(bytes: &[u8]) -> Option<(String, bool)> {
    use skrifa::MetadataProvider as _;
    use skrifa::raw::TableProvider as _;
    use skrifa::string::StringId;
    let font = skrifa::FontRef::new(bytes).ok()?;
    let find = |id| {
        font.localized_strings(id)
            .english_or_first()
            .map(|name| name.to_string())
            .filter(|name| !name.trim().is_empty())
    };
    let family = find(StringId::TYPOGRAPHIC_FAMILY_NAME).or_else(|| find(StringId::FAMILY_NAME))?;
    let monospace = font.post().is_ok_and(|post| post.is_fixed_pitch() != 0);
    Some((family, monospace))
}

/// Copies the font file at `path` into [`dir`], loads it, and returns its
/// family name and whether it is monospace.
pub fn add_file(path: &Path, cx: &mut App) -> Result<(SharedString, bool), AddError> {
    let bytes = std::fs::read(path).map_err(AddError::Io)?;
    let (family, monospace) = describe(&bytes).ok_or(AddError::NotAFont)?;
    let dir =
        dir().ok_or_else(|| AddError::Io(std::io::Error::other("there is no data folder")))?;
    let name = path.file_name().ok_or(AddError::NotAFont)?;
    std::fs::create_dir_all(&dir).map_err(AddError::Io)?;
    std::fs::write(dir.join(name), &bytes).map_err(AddError::Io)?;
    cx.text_system()
        .add_fonts(vec![bytes.into()])
        .map_err(|error| AddError::Rejected(error.to_string()))?;
    let names = list(cx.text_system());
    let family = SharedString::from(family);
    let current = families(cx);
    let mut mono = current.mono.clone();
    let scanning = current.scanning;
    if monospace && let Some(mono) = &mut mono {
        mono.insert(family.clone());
    }
    cx.set_global(Families {
        names,
        mono,
        scanning,
    });
    Ok((family, monospace))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plex_files_name_their_families() {
        let sans = include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf");
        let mono = include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf");
        assert_eq!(describe(sans), Some((SANS.to_owned(), false)));
        assert_eq!(describe(mono), Some((MONO.to_owned(), true)));
        assert_eq!(describe(b"not a font"), None);
    }

    #[test]
    fn only_ttf_and_otf_files_load() {
        assert!(is_font_file(Path::new("Inter.ttf")));
        assert!(is_font_file(Path::new("Inter.OTF")));
        assert!(!is_font_file(Path::new("Inter.woff2")));
        assert!(!is_font_file(Path::new("notes.txt")));
    }

    #[test]
    fn a_missing_family_falls_back_to_plex() {
        let families = Families {
            names: vec!["Fira Code".into(), SANS.into()],
            mono: None,
            scanning: false,
        };
        assert!(families.has("Fira Code"));
        assert!(!families.has("Comic Sans"));
    }
}
