//! Settings' Customise group (#705, #733): each style's tiny window in its
//! own colors, for the gallery, and the controls that change the style on
//! screen (accent, greys and corners).

use gpui_kit::{
    App, AppContext as _, Div, Entity, Global, ParentElement as _, Rgba, Styled as _, div,
    prelude::FluentBuilder as _, relative,
};

use crate::settings::{Settings, SettingsStore, ThemeSource};
use crate::style::{self, Greys, Preset, Radius, StyleSpec, TypeStyleExt as _, palette};
use crate::t;
use crate::ui::{
    color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState, ColorSwatch, Oklch},
    theme::{ThemeMode, UiTheme},
    toggle_group::ToggleGroup,
};

/// The accents offered as swatches: (name key, lightness, chroma, hue).
const SWATCHES: [(&str, f32, f32, f32); 8] = [
    ("prefs-accent-ink", 0.25, 0.008, 260.),
    ("prefs-accent-lapis", 0.52, 0.12, 268.),
    ("prefs-accent-blue", 0.55, 0.19, 258.),
    ("prefs-accent-teal", 0.55, 0.1, 195.),
    ("prefs-accent-green", 0.55, 0.14, 150.),
    ("prefs-accent-amber", 0.62, 0.14, 65.),
    ("prefs-accent-rose", 0.57, 0.19, 10.),
    ("prefs-accent-violet", 0.52, 0.19, 300.),
];

/// The custom style now, or the one a person starts from: the preset they
/// had chosen.
pub(super) fn custom_or_seed(settings: &Settings) -> StyleSpec {
    settings
        .custom_style
        .unwrap_or_else(|| match settings.theme_source {
            ThemeSource::Porcelain => Preset::Porcelain.style(),
            _ => Preset::Graphite.style(),
        })
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

/// Forgets the accent picker, so the next render makes it again from the
/// custom style: for when the whole style was replaced, by an import or a
/// saved style (#707).
pub(super) fn reset_accent_picker(cx: &mut App) {
    if cx.has_global::<AccentPicker>() {
        cx.remove_global::<AccentPicker>();
    }
}

/// The colors a style shows in its gallery card, and its corners.
#[derive(Clone, Copy)]
pub(super) struct StyleCard {
    sidebar: Rgba,
    page: Rgba,
    text: Rgba,
    accent: Rgba,
    border: Rgba,
    radius: Radius,
}

impl StyleCard {
    /// `style` as it shows in `mode`.
    pub(super) fn of(style: &StyleSpec, mode: ThemeMode) -> Self {
        let tokens = palette::tokens(&style.colors, mode);
        let get = |name| tokens.get(name).unwrap_or_default();
        Self {
            sidebar: get("sidebar"),
            page: get("background"),
            text: get("muted-foreground"),
            accent: get("primary"),
            border: get("border"),
            radius: style.radius,
        }
    }

    /// A Mattermost theme, in its own light or dark.
    pub(super) fn mattermost(theme: &crate::mattermost_theme::Theme) -> Self {
        let (ui, _) = theme.ui();
        let c = ui.colors;
        Self {
            sidebar: c.sidebar,
            page: c.background,
            text: c.muted_foreground,
            accent: c.primary,
            border: c.border,
            radius: Radius::Soft,
        }
    }

    /// A tiny window, 64px tall: a sidebar and lines of text, one with an
    /// accent dot.
    pub(super) fn render(self, theme: &UiTheme) -> Div {
        let corner = gpui_kit::px(self.radius.px() * 0.75);
        let line = |width: f32, color: Rgba| {
            div()
                .h(theme.space(1.))
                .w(relative(width))
                .rounded_full()
                .bg(color)
        };
        div()
            .w_full()
            .h(theme.space(16.))
            .flex()
            .overflow_hidden()
            .rounded(corner)
            .border_1()
            .border_color(self.border)
            .bg(self.page)
            .child(
                div()
                    .w(relative(0.3))
                    .h_full()
                    .bg(self.sidebar)
                    .p(theme.space(1.5))
                    .flex()
                    .flex_col()
                    .gap(theme.space(1.5))
                    .child(line(0.8, self.text))
                    .child(line(0.6, self.text))
                    .child(line(0.7, self.text)),
            )
            .child(
                div()
                    .flex_1()
                    .p(theme.space(2.))
                    .flex()
                    .flex_col()
                    .gap(theme.space(2.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(theme.space(1.))
                            .child(div().size(theme.space(2.)).rounded_full().bg(self.accent))
                            .child(line(0.6, self.text)),
                    )
                    .child(line(0.8, self.text))
                    .child(line(0.5, self.text)),
            )
    }
}

/// Changes the style on screen, saving a copy first when it is a preset
/// or the Mattermost theme.
fn edit(store: &Entity<SettingsStore>, change: impl FnOnce(StyleSpec) -> StyleSpec, cx: &mut App) {
    super::style_library::edit_style(store, change, cx);
}

/// The Customise group's controls for the style on screen: the accent
/// picker (with a line when the contrast guard adjusted a color), the
/// greys and the corners.
pub(super) struct Editor {
    pub(super) accent: Div,
    pub(super) greys: ToggleGroup,
    pub(super) corners: ToggleGroup,
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
    let greys = super::settings_pages::choices(
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
    let adjusted = [ThemeMode::Light, ThemeMode::Dark]
        .into_iter()
        .any(|mode| !palette::tokens(&spec.colors, mode).adjusted.is_empty());
    let accent = div()
        .w_full()
        .flex()
        .flex_col()
        .gap(theme.space(2.))
        .child(ColorPicker::new(&picker).swatches(swatches))
        .when(adjusted, |accent| {
            accent.child(
                div()
                    .type_style(style::META, &theme)
                    .text_color(theme.colors.muted_foreground)
                    .child(t!("prefs-accent-adjusted")),
            )
        });
    Editor {
        accent,
        greys,
        corners,
    }
}
