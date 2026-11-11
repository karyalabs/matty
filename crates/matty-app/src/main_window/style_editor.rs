//! Settings' theme editor (#705, #733, #855): the controls that change the
//! theme on screen (accent, background, sidebar, contrast and corners), its
//! preview in light and dark side by side, and what the contrast rules
//! found.

use gpui_kit::{
    App, AppContext as _, Div, Entity, Global, ParentElement as _, SharedString, Styled as _, div,
    prelude::FluentBuilder as _,
};

use super::style_preview::{self, Look};
use crate::contrast::{self, Kind};
use crate::settings::{Settings, SettingsStore};
use crate::style::{
    self, Greys, Radius, StyleSpec, TypeStyleExt as _,
    palette::{self, Contrast, Sidebar},
};
use crate::t;
use crate::ui::{
    color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState, ColorSwatch, Oklch},
    theme::{ThemeMode, UiTheme},
    toggle_group::ToggleGroup,
};

/// The accents offered as swatches: (name key, lightness, chroma, hue).
/// Each is what a built-in theme uses or a color that needs no adjusting
/// to carry a label on a white page.
const SWATCHES: [(&str, f32, f32, f32); 10] = [
    ("prefs-accent-ink", 0.25, 0.008, 260.),
    ("prefs-accent-lapis", 0.52, 0.13, 268.),
    ("prefs-accent-blue", 0.50, 0.17, 258.),
    ("prefs-accent-teal", 0.53, 0.10, 192.),
    ("prefs-accent-green", 0.55, 0.14, 150.),
    ("prefs-accent-amber", 0.66, 0.15, 75.),
    ("prefs-accent-orange", 0.66, 0.17, 48.),
    ("prefs-accent-clay", 0.56, 0.13, 42.),
    ("prefs-accent-rose", 0.57, 0.19, 10.),
    ("prefs-accent-violet", 0.52, 0.17, 295.),
];

/// The custom style now, or the one a person starts from: the preset they
/// had chosen.
pub(super) fn custom_or_seed(settings: &Settings) -> StyleSpec {
    settings
        .custom_style
        .unwrap_or_else(|| settings.theme_source.preset().unwrap_or_default().style())
}

/// The accent picker, one for the app, kept so a drag survives re-renders.
struct AccentPicker(Entity<ColorPickerState>);

impl Global for AccentPicker {}

fn accent_picker(store: &Entity<SettingsStore>, cx: &mut App) -> Entity<ColorPickerState> {
    if let Some(picker) = cx.try_global::<AccentPicker>() {
        return picker.0.clone();
    }
    let (lightness, chroma, hue) = super::style_library::shown(store.read(cx).get(), cx)
        .0
        .accent();
    let picker = cx.new(|cx| ColorPickerState::new(Oklch::new(lightness, chroma, hue), cx));
    let store = store.clone();
    cx.subscribe(&picker, move |_, event, cx| {
        let ColorPickerEvent::Change(color) = *event;
        let (now, _) = super::style_library::shown(store.read(cx).get(), cx);
        if now.accent() == (color.lightness, color.chroma, color.hue) {
            return;
        }
        super::style_library::edit_style(
            &store,
            |spec| spec.with_accent(color.lightness, color.chroma, color.hue),
            cx,
        );
    })
    .detach();
    cx.set_global(AccentPicker(picker.clone()));
    picker
}

/// The accent of the style on screen as a hex color, such as `#1f5fd0`.
pub(super) fn accent_hex(settings: &Settings, cx: &App) -> String {
    let (lightness, chroma, hue) = super::style_library::shown(settings, cx).0.accent();
    palette::css_hex(Oklch::new(lightness, chroma, hue).to_rgba())
}

/// Makes `typed` the accent when it is a whole hex color, such as
/// `#1f5fd0` or `1F5FD0`; anything else, such as a color half typed, is
/// left alone.
pub(super) fn set_accent_hex(store: &Entity<SettingsStore>, typed: &str, cx: &mut App) {
    let digits = typed.trim().trim_start_matches('#').to_ascii_lowercase();
    if digits.len() != 6 {
        return;
    }
    let hex = format!("#{digits}");
    let Some(color) = crate::mattermost_theme::parse_color(&hex) else {
        return;
    };
    if accent_hex(store.read(cx).get(), cx) == hex {
        return;
    }
    let picked = Oklch::from_rgba(color);
    edit(
        store,
        |spec| spec.with_accent(picked.lightness, picked.chroma, picked.hue),
        cx,
    );
    reset_accent_picker(cx);
}

/// Forgets the accent picker, so the next render makes it again from the
/// custom style: for when the whole style was replaced, by an import or a
/// saved style (#707).
pub(super) fn reset_accent_picker(cx: &mut App) {
    if cx.has_global::<AccentPicker>() {
        cx.remove_global::<AccentPicker>();
    }
}

/// A theme's thumbnail for the gallery.
#[derive(Clone, Copy)]
pub(super) struct StyleCard(Look);

impl StyleCard {
    /// `style` as it shows in `mode`.
    pub(super) fn of(style: &StyleSpec, mode: ThemeMode) -> Self {
        Self(Look::of(style, mode))
    }

    /// A Mattermost theme, in its own light or dark.
    pub(super) fn mattermost(theme: &crate::mattermost_theme::Theme) -> Self {
        Self(Look::mattermost(theme))
    }

    /// A tiny window, 92px tall, in the theme's colors.
    pub(super) fn render(self) -> Div {
        style_preview::thumbnail(&self.0, 92.)
    }
}

/// Changes the style on screen, saving a copy first when it is a preset
/// or the Mattermost theme.
fn edit(store: &Entity<SettingsStore>, change: impl FnOnce(StyleSpec) -> StyleSpec, cx: &mut App) {
    super::style_library::edit_style(store, change, cx);
}

/// What the contrast rules found for a theme, over light and dark.
pub(super) struct Readability {
    /// How many color pairs were checked.
    pub(super) checked: usize,
    /// The pairs under their minimum, as "name (ratio:1)", worst first.
    pub(super) failing: Vec<SharedString>,
    /// The text pair nearest its minimum: its name and ratio.
    pub(super) closest: Option<(SharedString, f32)>,
    /// Whether a color a person chose was moved so a label reads.
    pub(super) adjusted: bool,
    /// Whether quiet text reaches 7:1 everywhere.
    pub(super) high: bool,
}

impl Readability {
    /// Checks every pair Matty draws with `spec`, in light and in dark.
    pub(super) fn of(spec: &StyleSpec) -> Self {
        let mut checked = 0;
        let mut failing: Vec<(f32, SharedString)> = Vec::new();
        let mut closest: Option<(SharedString, f32)> = None;
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            let look = Look::of(spec, mode);
            for pair in contrast::pairs_of(&look.colors, &look.palette) {
                checked += 1;
                let ratio = pair.ratio();
                if !pair.passes() {
                    failing.push((ratio, format!("{} ({ratio:.1}:1)", pair.label()).into()));
                }
                if pair.kind == Kind::Text
                    && closest.as_ref().is_none_or(|(_, least)| ratio < *least)
                {
                    closest = Some((pair.label(), ratio));
                }
            }
        }
        failing.sort_by(|a, b| a.0.total_cmp(&b.0));
        let adjusted = [ThemeMode::Light, ThemeMode::Dark]
            .into_iter()
            .any(|mode| !palette::tokens(&spec.colors, mode).adjusted.is_empty());
        Self {
            checked,
            failing: failing.into_iter().map(|(_, name)| name).collect(),
            closest,
            adjusted,
            high: spec.colors.contrast == Contrast::High,
        }
    }

    /// One sentence on the result.
    pub(super) fn summary(&self) -> SharedString {
        if let Some(first) = self.failing.first() {
            return t!(
                "prefs-readability-failing",
                count = self.failing.len(),
                pair = first.to_string()
            );
        }
        let (pair, ratio) = self
            .closest
            .clone()
            .unwrap_or_else(|| (SharedString::default(), 0.));
        let ratio = format!("{ratio:.1}");
        if self.high {
            t!(
                "prefs-readability-high",
                count = self.checked,
                pair = pair.to_string(),
                ratio = ratio
            )
        } else {
            t!(
                "prefs-readability-passes",
                count = self.checked,
                pair = pair.to_string(),
                ratio = ratio
            )
        }
    }
}

/// The theme editor's controls for the style on screen.
pub(super) struct Editor {
    /// The theme in light and in dark, side by side.
    pub(super) preview: Div,
    /// The accent picker, with a line when a color was adjusted.
    pub(super) accent: Div,
    pub(super) background: ToggleGroup,
    pub(super) sidebar: ToggleGroup,
    pub(super) contrast: ToggleGroup,
    pub(super) corners: ToggleGroup,
    pub(super) readability: Readability,
}

pub(super) fn editor(store: &Entity<SettingsStore>, cx: &mut App) -> Editor {
    let theme = UiTheme::read(cx).clone();
    let spec = super::style_library::shown(store.read(cx).get(), cx).0;
    let picker = accent_picker(store, cx);
    let swatches = SWATCHES.map(|(key, lightness, chroma, hue)| {
        ColorSwatch::new(
            crate::i18n::text(key, &[]),
            Oklch::new(lightness, chroma, hue),
        )
    });
    let greys_value = match spec.greys() {
        Greys::Neutral => "neutral",
        Greys::Cool => "cool",
        Greys::Warm => "warm",
        Greys::Tinted => "tinted",
    };
    let greys_store = store.clone();
    let background = super::settings_pages::choices(
        "preferences.greys",
        t!("prefs-greys"),
        greys_value,
        &[
            ("neutral", t!("prefs-greys-neutral")),
            ("cool", t!("prefs-greys-cool")),
            ("warm", t!("prefs-greys-warm")),
            ("tinted", t!("prefs-greys-tinted")),
        ],
        move |value, cx| {
            let greys = match value {
                "cool" => Greys::Cool,
                "warm" => Greys::Warm,
                "tinted" => Greys::Tinted,
                _ => Greys::Neutral,
            };
            edit(&greys_store, |spec| spec.with_greys(greys), cx);
        },
    );
    let sidebar_store = store.clone();
    let sidebar = super::settings_pages::choices(
        "preferences.sidebar-fill",
        t!("prefs-sidebar-fill"),
        match spec.colors.sidebar {
            Sidebar::Soft => "soft",
            Sidebar::Tinted => "tinted",
            Sidebar::Dark => "dark",
            Sidebar::Accent => "accent",
        },
        &[
            ("soft", t!("prefs-sidebar-fill-soft")),
            ("tinted", t!("prefs-sidebar-fill-tinted")),
            ("dark", t!("prefs-sidebar-fill-dark")),
            ("accent", t!("prefs-sidebar-fill-accent")),
        ],
        move |value, cx| {
            let sidebar = match value {
                "tinted" => Sidebar::Tinted,
                "dark" => Sidebar::Dark,
                "accent" => Sidebar::Accent,
                _ => Sidebar::Soft,
            };
            edit(
                &sidebar_store,
                |mut spec| {
                    spec.colors.sidebar = sidebar;
                    spec
                },
                cx,
            );
        },
    );
    let contrast_store = store.clone();
    let contrast = super::settings_pages::choices(
        "preferences.contrast",
        t!("prefs-contrast"),
        match spec.colors.contrast {
            Contrast::Standard => "standard",
            Contrast::High => "high",
        },
        &[
            ("standard", t!("prefs-contrast-standard")),
            ("high", t!("prefs-contrast-high")),
        ],
        move |value, cx| {
            let contrast = match value {
                "high" => Contrast::High,
                _ => Contrast::Standard,
            };
            edit(
                &contrast_store,
                |mut spec| {
                    spec.colors.contrast = contrast;
                    spec
                },
                cx,
            );
        },
    );
    let corners_store = store.clone();
    let corners = super::settings_pages::choices(
        "preferences.corners",
        t!("prefs-corners"),
        match spec.radius {
            Radius::Square => "square",
            Radius::Soft => "soft",
            Radius::Round => "round",
        },
        &[
            ("square", t!("prefs-corners-square")),
            ("soft", t!("prefs-corners-soft")),
            ("round", t!("prefs-corners-round")),
        ],
        move |value, cx| {
            let radius = match value {
                "square" => Radius::Square,
                "round" => Radius::Round,
                _ => Radius::Soft,
            };
            edit(&corners_store, |spec| StyleSpec { radius, ..spec }, cx);
        },
    );
    let readability = Readability::of(&spec);
    let accent = div()
        .w_full()
        .flex()
        .flex_col()
        .gap(theme.space(2.))
        .child(ColorPicker::new(&picker).swatches(swatches))
        .when(readability.adjusted, |accent| {
            accent.child(
                div()
                    .type_style(style::META, &theme)
                    .text_color(theme.colors.muted_foreground)
                    .child(t!("prefs-accent-adjusted")),
            )
        });
    let preview = div()
        .w_full()
        .flex()
        .gap(theme.space(3.))
        .child(style_preview::scene(
            &Look::of(&spec, ThemeMode::Light),
            t!("prefs-theme-light"),
            &theme,
        ))
        .child(style_preview::scene(
            &Look::of(&spec, ThemeMode::Dark),
            t!("prefs-theme-dark"),
            &theme,
        ));
    Editor {
        preview,
        accent,
        background,
        sidebar,
        contrast,
        corners,
        readability,
    }
}
