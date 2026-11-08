//! Preferences' Colors group (#705): a gallery of styles, each shown as a
//! tiny window in its own colors, and the editor for the person's own style
//! (accent, greys and corners) under it.

use gpui_kit::{
    App, AppContext as _, Div, Entity, Global, ParentElement as _, Rgba, SharedString, Styled as _,
    div, prelude::FluentBuilder as _, relative,
};

use crate::settings::{Settings, SettingsStore, ThemeSource};
use crate::style::{self, Greys, Preset, Radius, StyleSpec, TypeStyleExt as _, palette};
use crate::t;
use crate::ui::{
    color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState, ColorSwatch, Oklch},
    theme::{ThemeMode, UiTheme},
    toggle_group::{ToggleGroup, ToggleGroupItem},
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
    let (lightness, chroma, hue) = custom_or_seed(store.read(cx).get()).accent();
    let picker = cx.new(|cx| ColorPickerState::new(Oklch::new(lightness, chroma, hue), cx));
    let store = store.clone();
    cx.subscribe(&picker, move |_, event, cx| {
        let ColorPickerEvent::Change(color) = *event;
        store.update(cx, |store, cx| {
            store.update(cx, |s| {
                let spec = custom_or_seed(s).with_accent(color.lightness, color.chroma, color.hue);
                if s.custom_style != Some(spec) {
                    s.custom_style = Some(spec);
                }
            })
        });
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

/// The colors a style shows in its gallery card.
struct Swatch {
    sidebar: Rgba,
    page: Rgba,
    text: Rgba,
    accent: Rgba,
    border: Rgba,
}

fn swatch_of(style: &StyleSpec, mode: ThemeMode) -> Swatch {
    let tokens = palette::tokens(&style.colors, mode);
    let get = |name| tokens.get(name).unwrap_or_default();
    Swatch {
        sidebar: get("sidebar"),
        page: get("background"),
        text: get("muted-foreground"),
        accent: get("primary"),
        border: get("border"),
    }
}

/// A tiny window in `swatch`'s colors: a sidebar and two lines of text,
/// one with an accent dot.
fn card(swatch: Swatch, radius: Radius, theme: &UiTheme) -> Div {
    let corner = gpui_kit::px(radius.px() * 0.75);
    let line = |width: f32, color: Rgba| {
        div()
            .h(theme.space(1.))
            .w(relative(width))
            .rounded_full()
            .bg(color)
    };
    div()
        .w_full()
        .h(theme.space(12.))
        .flex()
        .overflow_hidden()
        .rounded(corner)
        .border_1()
        .border_color(swatch.border)
        .bg(swatch.page)
        .child(
            div()
                .w(relative(0.3))
                .h_full()
                .bg(swatch.sidebar)
                .p(theme.space(1.5))
                .flex()
                .flex_col()
                .gap(theme.space(1.))
                .child(line(0.8, swatch.text))
                .child(line(0.6, swatch.text)),
        )
        .child(
            div()
                .flex_1()
                .p(theme.space(2.))
                .flex()
                .flex_col()
                .gap(theme.space(1.5))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(theme.space(1.))
                        .child(div().size(theme.space(2.)).rounded_full().bg(swatch.accent))
                        .child(line(0.6, swatch.text)),
                )
                .child(line(0.8, swatch.text)),
        )
}

/// The style gallery: Graphite, Porcelain, the person's own and their
/// Mattermost theme, each a card in its own colors for the mode on screen.
pub(super) fn gallery(store: &Entity<SettingsStore>, cx: &mut App) -> ToggleGroup {
    let theme = UiTheme::read(cx).clone();
    let settings = store.read(cx).get().clone();
    let mode = theme.mode;
    let value = match settings.theme_source {
        ThemeSource::Matty => "matty",
        ThemeSource::Porcelain => "porcelain",
        ThemeSource::Custom => "custom",
        ThemeSource::Mattermost => "mattermost",
    };
    let custom = custom_or_seed(&settings);
    let mattermost = {
        let imported = crate::mattermost_theme::shown(&settings, cx)
            .unwrap_or_else(crate::mattermost_theme::Theme::denim);
        let (ui, _) = imported.ui();
        let c = ui.colors;
        Swatch {
            sidebar: c.sidebar,
            page: c.background,
            text: c.muted_foreground,
            accent: c.primary,
            border: c.border,
        }
    };
    let options: [(&'static str, SharedString, Swatch, Radius); 4] = [
        (
            "matty",
            t!("prefs-colors-graphite"),
            swatch_of(&Preset::Graphite.style(), mode),
            Radius::Soft,
        ),
        (
            "porcelain",
            t!("prefs-colors-porcelain"),
            swatch_of(&Preset::Porcelain.style(), mode),
            Radius::Soft,
        ),
        (
            "custom",
            t!("prefs-colors-own"),
            swatch_of(&custom, mode),
            custom.radius,
        ),
        (
            "mattermost",
            t!("prefs-colors-mattermost"),
            mattermost,
            Radius::Soft,
        ),
    ];
    let mut group = ToggleGroup::new("preferences.colors")
        .aria_label(t!("prefs-colors"))
        .w_full()
        .value([SharedString::from(value)]);
    for (option, name, swatch, radius) in options {
        group = group.item(
            ToggleGroupItem::new(
                gpui_kit::ElementId::Name(format!("preferences.colors.{option}").into()),
                option,
            )
            .aria_label(name.clone())
            .flex_1()
            .min_w_0()
            .h_auto()
            .p(theme.space(1.5))
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(theme.space(1.5))
                    .child(card(swatch, radius, &theme))
                    .child(
                        div()
                            .type_style(style::META, &theme)
                            .text_center()
                            .truncate()
                            .child(name),
                    ),
            ),
        );
    }
    let store = store.clone();
    group.on_change(move |next, window, cx| {
        let Some(value) = next.first() else { return };
        let source = match value.as_ref() {
            "porcelain" => ThemeSource::Porcelain,
            "custom" => ThemeSource::Custom,
            "mattermost" => ThemeSource::Mattermost,
            _ => ThemeSource::Matty,
        };
        store.update(cx, |store, cx| {
            store.update(cx, |s| {
                if source == ThemeSource::Custom && s.custom_style.is_none() {
                    s.custom_style = Some(custom_or_seed(s));
                }
                s.theme_source = source;
            })
        });
        if source == ThemeSource::Custom {
            let (lightness, chroma, hue) = custom_or_seed(store.read(cx).get()).accent();
            let picker = accent_picker(&store, cx);
            picker.update(cx, |picker, cx| {
                picker.set_value(Oklch::new(lightness, chroma, hue), window, cx)
            });
        }
    })
}

/// Writes `change` into the custom style.
fn edit(store: &Entity<SettingsStore>, change: impl FnOnce(StyleSpec) -> StyleSpec, cx: &mut App) {
    store.update(cx, |store, cx| {
        store.update(cx, |s| s.custom_style = Some(change(custom_or_seed(s))))
    });
}

/// The editor for the person's own style: accent, greys and corners, and a
/// line when the contrast guard had to adjust a color.
pub(super) fn editor(store: &Entity<SettingsStore>, cx: &mut App) -> Div {
    let theme = UiTheme::read(cx).clone();
    let spec = custom_or_seed(store.read(cx).get());
    let picker = accent_picker(store, cx);
    let swatches = SWATCHES.map(|(key, lightness, chroma, hue)| {
        ColorSwatch::new(
            crate::i18n::text(key, &[]),
            Oklch::new(lightness, chroma, hue),
        )
    });
    let label = |text: SharedString| {
        div()
            .type_style(style::UI, &theme)
            .text_color(theme.colors.foreground)
            .child(text)
    };
    let greys_value = match spec.greys() {
        Greys::Neutral => "neutral",
        Greys::Cool => "cool",
        Greys::Warm => "warm",
        Greys::Tinted => "tinted",
    };
    let greys_store = store.clone();
    let greys = super::preferences::choices(
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
    let corners = super::preferences::choices(
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
    div()
        .flex()
        .flex_col()
        .gap(theme.space(4.))
        .p(theme.space(4.))
        .rounded(theme.radius.lg)
        .border_1()
        .border_color(theme.colors.border)
        .child(
            div()
                .flex()
                .flex_col()
                .gap(theme.space(2.))
                .child(label(t!("prefs-accent")))
                .child(ColorPicker::new(&picker).swatches(swatches))
                .when(adjusted, |accent| {
                    accent.child(
                        div()
                            .type_style(style::META, &theme)
                            .text_color(theme.colors.muted_foreground)
                            .child(t!("prefs-accent-adjusted")),
                    )
                }),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(theme.space(2.))
                .child(label(t!("prefs-greys")))
                .child(greys),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(theme.space(2.))
                .child(label(t!("prefs-corners")))
                .child(corners),
        )
}
