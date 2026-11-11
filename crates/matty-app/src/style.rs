//! Matty's design tokens on top of gpuicn's [`UiTheme`] (docs/DESIGN.md).
//!
//! A style ([`StyleSpec`]) is a few numbers: the colors [`palette`] derives
//! every token from, and the corner radius. The built-in presets are styles
//! too, and a person's own style (#698) goes through the same path, the
//! contrast guard included. This module turns a style into a [`UiTheme`] per
//! mode, sets the type families, adds the semantic colors shadcn has no name
//! for ([`Palette`]), and names the type scale and layout metrics every view
//! shares, so a size appears once here and nowhere else.

pub mod palette;
pub mod share;

use gpui_kit::{
    App, ElementId, FontWeight, Global, InteractiveElement as _, IntoElement, ParentElement as _,
    Pixels, Rgba, SharedString, StatefulInteractiveElement as _, Styled, div,
    prelude::FluentBuilder as _, px,
};

use gpui_icons::{LucideIcon, lucide};
use serde::{Deserialize, Serialize};

use crate::ui::empty::{
    Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyMediaVariant, EmptyTitle,
};
use crate::ui::theme::{ThemeMode, UiTheme};
use crate::ui::tooltip::text_tooltip;

/// The interface and reading face (DESIGN.md §3).
pub const SANS: &str = "IBM Plex Sans";
/// Code, inline and in blocks.
pub const MONO: &str = "IBM Plex Mono";

/// A built-in theme (DESIGN.md §2). Each is one identity that works in
/// light and in dark.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Preset {
    /// Ink on paper: near-neutral greys and an ink accent. The default.
    #[default]
    Graphite,
    /// Cool greys with a lapis accent.
    Porcelain,
    /// A deep blue sidebar beside a white page, the classic team chat.
    Harbor,
    /// Warm paper neutrals with a clay accent.
    Linen,
    /// A teal accent that washes the sidebar.
    Tide,
    /// Violet, with violet-tinted neutrals.
    Dusk,
    /// A charcoal sidebar and an orange accent.
    Ember,
}

impl Preset {
    /// Every built-in theme, in the order the gallery shows them.
    pub const ALL: [Self; 7] = [
        Self::Graphite,
        Self::Porcelain,
        Self::Harbor,
        Self::Linen,
        Self::Tide,
        Self::Dusk,
        Self::Ember,
    ];

    /// The theme's name in file names and settings, such as `graphite`.
    pub fn id(self) -> &'static str {
        match self {
            Self::Graphite => "graphite",
            Self::Porcelain => "porcelain",
            Self::Harbor => "harbor",
            Self::Linen => "linen",
            Self::Tide => "tide",
            Self::Dusk => "dusk",
            Self::Ember => "ember",
        }
    }

    /// One line on what the theme is, for the CSS files.
    pub fn summary(self) -> &'static str {
        match self {
            Self::Graphite => {
                "Graphite, the default: ink on paper, near-neutral greys and an ink accent"
            }
            Self::Porcelain => "Porcelain: cool greys with a lapis accent",
            Self::Harbor => "Harbor: a deep blue sidebar beside a white page",
            Self::Linen => "Linen: warm paper neutrals with a clay accent",
            Self::Tide => "Tide: a teal accent that washes the sidebar",
            Self::Dusk => "Dusk: violet, with violet-tinted neutrals",
            Self::Ember => "Ember: a charcoal sidebar and an orange accent",
        }
    }

    /// The theme's numbers: the neutral hue and chroma, the accent's hue,
    /// chroma and lightness in light and in dark, and the sidebar.
    pub fn colors(self) -> palette::StyleColors {
        use palette::Sidebar;
        let (
            neutral_hue,
            neutral_chroma,
            accent_hue,
            accent_chroma,
            accent_light,
            accent_dark,
            sidebar,
        ) = match self {
            Self::Graphite => (260., 0.002, 260., 0.008, 0.25, 0.92, Sidebar::Soft),
            Self::Porcelain => (262., 0.007, 268., 0.13, 0.52, 0.57, Sidebar::Soft),
            Self::Harbor => (255., 0.007, 258., 0.17, 0.50, 0.555, Sidebar::Accent),
            Self::Linen => (75., 0.014, 42., 0.13, 0.56, 0.70, Sidebar::Soft),
            Self::Tide => (205., 0.008, 192., 0.10, 0.53, 0.74, Sidebar::Tinted),
            Self::Dusk => (295., 0.012, 295., 0.17, 0.52, 0.57, Sidebar::Tinted),
            Self::Ember => (55., 0.005, 48., 0.17, 0.66, 0.74, Sidebar::Dark),
        };
        palette::StyleColors {
            neutral_hue,
            neutral_chroma,
            accent_hue,
            accent_chroma,
            accent_light,
            accent_dark,
            sidebar,
            contrast: palette::Contrast::Standard,
        }
    }

    /// The preset as a whole style.
    pub fn style(self) -> StyleSpec {
        StyleSpec {
            colors: self.colors(),
            radius: Radius::default(),
        }
    }
}

/// How round corners are.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Radius {
    /// Nearly square.
    Square,
    /// The default.
    #[default]
    Soft,
    /// Round.
    Round,
}

impl Radius {
    /// The base radius in px, from which gpuicn derives its scale.
    pub fn px(self) -> f32 {
        match self {
            Self::Square => 3.,
            Self::Soft => 8.,
            Self::Round => 12.,
        }
    }
}

/// A whole style: what a built-in preset is, and what a person's own style
/// saves.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct StyleSpec {
    pub colors: palette::StyleColors,
    #[serde(default)]
    pub radius: Radius,
}

/// How the greys of a custom style are tinted (#705).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Greys {
    /// Almost pure grey.
    Neutral,
    /// A cool, blue-grey tint.
    Cool,
    /// A warm, paper-like tint.
    Warm,
    /// Tinted toward the accent.
    Tinted,
}

impl Greys {
    /// Every choice, in the order Preferences offers them.
    pub const ALL: [Self; 4] = [Self::Neutral, Self::Cool, Self::Warm, Self::Tinted];

    /// The neutral hue and chroma for an accent of `accent_hue`.
    fn tint(self, accent_hue: f32) -> (f32, f32) {
        match self {
            Self::Neutral => (accent_hue, 0.002),
            Self::Cool => (255., 0.008),
            Self::Warm => (75., 0.012),
            Self::Tinted => (accent_hue, 0.012),
        }
    }
}

impl StyleSpec {
    /// The accent as one color: its hue, chroma and light-mode lightness.
    pub fn accent(&self) -> (f32, f32, f32) {
        let c = &self.colors;
        (c.accent_light, c.accent_chroma, c.accent_hue)
    }

    /// This style with the accent `lightness`, `chroma` and `hue` (light
    /// mode). Dark mode takes its accent from it, as the presets do; tinted
    /// greys follow the hue. The contrast rules do the rest.
    pub fn with_accent(mut self, lightness: f32, chroma: f32, hue: f32) -> Self {
        let greys = self.greys();
        let c = &mut self.colors;
        c.accent_light = lightness.clamp(0.05, 0.98);
        // An ink flips to a pale ink in dark mode. A color stays the color:
        // a mid-tone that carries a white label, or the pale tone chosen.
        c.accent_dark = if chroma <= 0.05 {
            (1.17 - lightness).clamp(0.6, 0.93)
        } else {
            lightness.clamp(0.57, 0.80)
        };
        c.accent_chroma = chroma.max(0.);
        c.accent_hue = hue.rem_euclid(360.);
        self.with_greys(greys)
    }

    /// How this style's greys are tinted, the nearest choice.
    pub fn greys(&self) -> Greys {
        let c = &self.colors;
        Greys::ALL
            .into_iter()
            .min_by(|a, b| {
                let distance = |greys: Greys| {
                    let (hue, chroma) = greys.tint(c.accent_hue);
                    let hue_gap = ((hue - c.neutral_hue + 540.).rem_euclid(360.) - 180.).abs();
                    (chroma - c.neutral_chroma).abs() * 100. + hue_gap / 360.
                };
                distance(*a).total_cmp(&distance(*b))
            })
            .unwrap_or(Greys::Neutral)
    }

    /// This style with its greys tinted `greys`.
    pub fn with_greys(mut self, greys: Greys) -> Self {
        let (hue, chroma) = greys.tint(self.colors.accent_hue);
        self.colors.neutral_hue = hue;
        self.colors.neutral_chroma = chroma;
        self
    }
}

/// Builds the default preset's theme for `mode`; see [`theme_for`].
pub fn theme(mode: ThemeMode) -> UiTheme {
    theme_for(Preset::default(), mode)
}

/// Builds `preset`'s theme for `mode`.
pub fn theme_for(preset: Preset, mode: ThemeMode) -> UiTheme {
    theme_from(&preset.style(), mode)
}

/// Builds `style`'s theme for `mode`, with the contrast guard applied.
///
/// The CSS is Matty's own, so a parse failure is a bug; the app still
/// opens, on gpuicn's Neutral palette, rather than panicking.
pub fn theme_from(style: &StyleSpec, mode: ThemeMode) -> UiTheme {
    let css = palette::css(&style.colors, style.radius.px());
    let mut theme = match UiTheme::from_shadcn_css(&css, mode) {
        Ok(theme) => theme,
        Err(error) => {
            tracing::error!(
                ?error,
                "style colors did not parse; using the Neutral palette"
            );
            match mode {
                ThemeMode::Light => UiTheme::neutral_light(),
                ThemeMode::Dark => UiTheme::neutral_dark(),
            }
        }
    };
    theme.fonts.body = SANS.into();
    theme.fonts.heading = SANS.into();
    theme.fonts.mono = MONO.into();
    theme
}

/// Colors with a meaning gpuicn's tokens do not cover (DESIGN.md §2):
/// presence, and what the sidebar needs when it is not the page's mode.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    /// Presence dot: online.
    pub online: Rgba,
    /// Presence dot: away.
    pub away: Rgba,
    /// Presence dot: do not disturb.
    pub dnd: Rgba,
    /// The three presence dots as they show on the sidebar, which may be
    /// dark beside a light page.
    pub sidebar_online: Rgba,
    pub sidebar_away: Rgba,
    pub sidebar_dnd: Rgba,
    /// A muted channel's name on the sidebar: quieter than quiet text, and
    /// still 3:1.
    pub sidebar_faint: Rgba,
    /// The fill behind a field or a chip on the sidebar, such as Jump to.
    pub sidebar_field: Rgba,
    /// The selected row's fill on the sidebar, a step past a hovered row's.
    pub sidebar_selected: Rgba,
    /// Whether the sidebar is dark beside a light page, or light beside a
    /// dark one, so controls on it must use the sidebar's colors.
    pub sidebar_contrasts: bool,
}

impl Global for Palette {}

impl Palette {
    /// The default theme's semantic colors for `mode`.
    pub fn for_mode(mode: ThemeMode) -> Self {
        Self::of(&Preset::default().style(), mode)
    }

    /// `style`'s semantic colors for `mode`.
    pub fn of(style: &StyleSpec, mode: ThemeMode) -> Self {
        let tokens = palette::tokens(&style.colors, mode);
        let get = |name| tokens.get(name).unwrap_or_default();
        Self {
            online: get("online"),
            away: get("away"),
            dnd: get("dnd"),
            sidebar_online: get("sidebar-online"),
            sidebar_away: get("sidebar-away"),
            sidebar_dnd: get("sidebar-dnd"),
            sidebar_faint: get("sidebar-faint"),
            sidebar_field: tokens.sidebar_field,
            sidebar_selected: tokens.sidebar_selected,
            sidebar_contrasts: tokens.sidebar_contrasts,
        }
    }

    /// The palette for the current theme mode. Falls back to light before
    /// [`install`] has run.
    pub fn read(cx: &App) -> Self {
        cx.try_global::<Self>()
            .copied()
            .unwrap_or_else(|| Self::for_mode(ThemeMode::Light))
    }
}

/// Installs `preset` for `mode`, keeping the text scale, spacing unit and
/// motion settings the app already applied. Does nothing when the installed
/// theme is already this one, so views are not repainted.
pub fn install(preset: Preset, mode: ThemeMode, cx: &mut App) {
    install_style(&preset.style(), mode, cx);
}

/// Installs `style` for `mode`, as [`install`] does a preset.
pub fn install_style(style: &StyleSpec, mode: ThemeMode, cx: &mut App) {
    install_theme(theme_from(style, mode), Palette::of(style, mode), cx);
}

/// Installs `next` and its `palette`, such as an imported Mattermost theme
/// (#214), keeping the text scale, spacing unit, fonts (#706) and motion
/// settings the app already applied. Does nothing when both are already
/// installed.
pub fn install_theme(mut next: UiTheme, palette: Palette, cx: &mut App) {
    if cx.has_global::<UiTheme>() {
        let current = UiTheme::read(cx);
        next.text_scale = current.text_scale;
        next.fonts = current.fonts.clone();
        next.spacing = current.spacing;
        next.motion = current.motion;
        if *current == next && cx.try_global::<Palette>() == Some(&palette) {
            return;
        }
    }
    cx.set_global(palette);
    UiTheme::set(cx, next);
}

/// Applies the "Reduce motion" preference to the installed theme, so every
/// gpuicn transition snaps to its target; a no-op when nothing changes.
pub fn set_reduced_motion(reduced: bool, cx: &mut App) {
    let theme = UiTheme::read(cx);
    if theme.motion.reduced != reduced {
        let mut next = theme.clone();
        next.motion.reduced = reduced;
        UiTheme::set(cx, next);
    }
}

/// A role in the type scale (DESIGN.md §3): size and line height in px at
/// text scale 1, and the weight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeStyle {
    /// Font size at text scale 1.
    pub size: f32,
    /// Line height at text scale 1.
    pub line: f32,
    /// Font weight.
    pub weight: FontWeight,
}

/// Channel names in headers and dialog titles. A clear step above
/// everything else, so the page has one loudest line.
pub const TITLE: TypeStyle = TypeStyle {
    size: 17.,
    line: 24.,
    weight: FontWeight::SEMIBOLD,
};
/// A Settings page's title: one step above `TITLE`, since the page has no
/// other heading (#732).
pub const PAGE_TITLE: TypeStyle = TypeStyle {
    size: 20.,
    line: 28.,
    weight: FontWeight::SEMIBOLD,
};
/// A Settings group's label, in sentence case.
pub const GROUP_LABEL: TypeStyle = TypeStyle {
    size: 13.,
    line: 18.,
    weight: FontWeight::SEMIBOLD,
};
/// Post text and the composer, set for reading.
pub const BODY: TypeStyle = TypeStyle {
    size: 14.5,
    line: 22.,
    weight: FontWeight::NORMAL,
};
/// Sidebar rows, buttons, menus and the title bar.
pub const UI: TypeStyle = TypeStyle {
    size: 13.5,
    line: 20.,
    weight: FontWeight::NORMAL,
};
/// Unread rows, author names and labels. The only bold step; keep it rare.
pub const UI_STRONG: TypeStyle = TypeStyle {
    size: 13.5,
    line: 20.,
    weight: FontWeight::SEMIBOLD,
};
/// Timestamps, counts and descriptions.
pub const META: TypeStyle = TypeStyle {
    size: 12.,
    line: 16.,
    weight: FontWeight::NORMAL,
};
/// Sidebar category names, rendered uppercase.
pub const LABEL: TypeStyle = TypeStyle {
    size: 11.,
    line: 16.,
    weight: FontWeight::MEDIUM,
};
/// Inline code and code blocks, in the mono font.
pub const CODE: TypeStyle = TypeStyle {
    size: 13.,
    line: 20.,
    weight: FontWeight::NORMAL,
};

/// Applies a [`TypeStyle`] to any styled element, scaled by the theme.
pub trait TypeStyleExt: Styled + Sized {
    /// Sets font size, line height and weight from `style`.
    fn type_style(self, style: TypeStyle, theme: &UiTheme) -> Self {
        self.text_size(theme.text(style.size))
            .line_height(theme.text(style.line))
            .font_weight(style.weight)
    }
}

impl<T: Styled> TypeStyleExt for T {}

/// Layout metrics from DESIGN.md §4, in px at zoom 1. Callers multiply by
/// the theme's spacing unit through [`scaled`] so zoom applies.
pub mod metrics {
    /// Sidebar, channel and right-pane headers.
    pub const HEADER_HEIGHT: f32 = 46.;
    /// A sidebar row.
    pub const ROW_HEIGHT: f32 = 28.;
    /// Horizontal padding inside the channel view.
    pub const GUTTER: f32 = 20.;
    /// The title bar, taller than gpuicn's 34 px so its controls breathe
    /// (#625). [`super::title_bar_window_options`] centres the macOS
    /// traffic lights in it.
    pub const TITLE_BAR_HEIGHT: f32 = 40.;
    /// The quick switcher pill's widest size in the title bar.
    pub const SWITCHER_MAX_WIDTH: f32 = 560.;
    /// Chrome icons.
    pub const ICON: f32 = 16.;
    /// Icons in empty states.
    pub const ICON_LARGE: f32 = 20.;
    /// Icons inside chips, badges, menu rows and the channel header's
    /// secondary controls (DESIGN.md §5).
    pub const ICON_SMALL: f32 = 14.;
    /// Chevrons and kind glyphs beside text.
    pub const ICON_TINY: f32 = 12.;
    /// The square an icon or initial sits in on a sidebar row.
    pub const ROW_SLOT: f32 = 20.;
    /// The "… is typing" line under the transcript.
    pub const TYPING_LINE: f32 = 20.;
    /// The widest an empty state's text runs.
    pub const EMPTY_MAX_WIDTH: f32 = 420.;
    /// The Threads view's list column.
    pub const THREADS_LIST_WIDTH: f32 = 360.;
    /// The widest a Drafts card runs.
    pub const DRAFTS_MAX_WIDTH: f32 = 760.;
    /// A short field such as a time picker.
    pub const FIELD_NARROW_WIDTH: f32 = 140.;
    /// Post avatars in the channel view and threads.
    pub const AVATAR: f32 = 32.;
}

/// Scales a px metric by the theme's spacing unit (4px at zoom 1).
pub fn scaled(theme: &UiTheme, pixels: f32) -> Pixels {
    theme.spacing.unit * (pixels / 4.)
}

/// Returns `pixels` as [`Pixels`] unscaled, for one-pixel hairlines.
pub const fn hairline() -> Pixels {
    px(1.)
}

/// An empty state the way DESIGN.md §3 draws every one: a 40px icon in a
/// tinted circle, a `title` line and one `meta` sentence, all centered.
/// Callers add at most one outline button through [`Empty::content`].
pub fn empty_state(
    glyph: LucideIcon,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    cx: &App,
) -> Empty {
    let theme = UiTheme::read(cx);
    Empty::new().header(
        EmptyHeader::new()
            .media(
                EmptyMedia::new()
                    .with_variant(EmptyMediaVariant::Icon)
                    .size(theme.space(10.))
                    .rounded_full()
                    .child(
                        lucide(glyph)
                            .size(scaled(theme, metrics::ICON_LARGE))
                            .text_color(theme.colors.muted_foreground),
                    ),
            )
            .title(
                EmptyTitle::new()
                    .type_style(TITLE, theme)
                    .text_center()
                    .child(title.into()),
            )
            .description(
                EmptyDescription::new()
                    .type_style(META, theme)
                    .text_center()
                    .child(description.into()),
            ),
    )
}

/// [`empty_state`] centered in the space it fills, for a pane with nothing
/// else to show.
pub fn centered_empty(
    glyph: LucideIcon,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    cx: &App,
) -> gpui_kit::Div {
    let theme = UiTheme::read(cx);
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .items_center()
        .justify_center()
        .p(theme.space(6.))
        .child(empty_state(glyph, title, description, cx))
}

/// Wraps `child` so hovering it shows `tip` (DESIGN.md §5: every icon-only
/// button has a tooltip, with its shortcut when there is one). `id` must be
/// unique within the window; the wrapper sizes to its child.
pub fn tipped(
    id: impl Into<ElementId>,
    tip: impl Into<SharedString>,
    child: impl IntoElement,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let tip: SharedString = tip.into();
    tipped_when(true, id, tip, child)
}

/// [`tipped`] whose tooltip is attached only while `show` is true. Pass
/// `false` while a menu or popover opened from the control is showing, so
/// the tooltip does not linger over it.
pub fn tipped_when(
    show: bool,
    id: impl Into<ElementId>,
    tip: impl Into<SharedString>,
    child: impl IntoElement,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let tip: SharedString = tip.into();
    div()
        .id(id)
        .flex()
        .flex_shrink_0()
        .when(show, |this| {
            this.tooltip(move |_, cx| text_tooltip(tip.clone(), cx))
        })
        .child(child)
}

/// The tallest a composer's text box grows before it scrolls: 40% of the
/// window, read on every render so it follows resizes.
pub fn composer_cap(window: &gpui_kit::Window) -> gpui_kit::Pixels {
    window.viewport_size().height * 0.4
}

/// An avatar's accessible label with the presence its dot shows, such as
/// "Ana Lopez, away".
pub fn presence_name(name: &str, presence: crate::sidebar::Presence) -> SharedString {
    let state = crate::account::presence_label(presence).to_lowercase();
    format!("{name}, {state}").into()
}

/// The presence an avatar shows (DESIGN.md §3, #135) as gpuicn's avatar
/// status: a dot over the bottom-right corner, filled with Matty's presence
/// colors for online, away and Do not disturb and a hollow ring for
/// offline. The avatar adds the presence to its accessible name, as in
/// "Ana Lopez, away". Pair it with `ring_color` set to the surface under
/// the avatar, so the dot reads as cut out of it.
pub fn avatar_status(
    presence: crate::sidebar::Presence,
    palette: &Palette,
) -> crate::ui::avatar::AvatarStatus {
    presence_status(presence, palette.online, palette.away, palette.dnd)
}

/// [`avatar_status`] for an avatar on the sidebar, whose dots are solved
/// against the sidebar's fill rather than the page's.
pub fn sidebar_avatar_status(
    presence: crate::sidebar::Presence,
    palette: &Palette,
) -> crate::ui::avatar::AvatarStatus {
    presence_status(
        presence,
        palette.sidebar_online,
        palette.sidebar_away,
        palette.sidebar_dnd,
    )
}

fn presence_status(
    presence: crate::sidebar::Presence,
    online: Rgba,
    away: Rgba,
    dnd: Rgba,
) -> crate::ui::avatar::AvatarStatus {
    use crate::sidebar::Presence;
    use crate::ui::avatar::AvatarStatus;
    let color = match presence {
        Presence::Online => online,
        Presence::Away => away,
        Presence::Dnd => dnd,
        Presence::Offline => return AvatarStatus::Offline,
    };
    AvatarStatus::Custom {
        color,
        label: crate::account::presence_label(presence)
            .to_lowercase()
            .into(),
    }
}

/// Matty's mark, "Overlap" (`packaging/icons`), at `size`: the two arches
/// that stand for the app on the sign-in card.
pub fn mark(size: Pixels) -> gpui_kit::Img {
    static MARK: std::sync::LazyLock<std::sync::Arc<gpui_kit::Image>> =
        std::sync::LazyLock::new(|| {
            std::sync::Arc::new(gpui_kit::Image::from_bytes(
                gpui_kit::ImageFormat::Png,
                include_bytes!("../assets/brand/mark-128.png").to_vec(),
            ))
        });
    gpui_kit::img(gpui_kit::ImageSource::Image(MARK.clone())).size(size)
}

/// Window options for a window that draws Matty's [`metrics::TITLE_BAR_HEIGHT`]
/// title bar: gpuicn's, with the macOS traffic lights moved down to its
/// middle.
pub fn title_bar_window_options() -> gpui_kit::WindowOptions {
    let options = crate::ui::title_bar::title_bar_window_options();
    // The lights are 16 px tall; gpuicn puts them 9 px down a 34 px bar.
    let top = (metrics::TITLE_BAR_HEIGHT - 16.) / 2.;
    gpui_kit::WindowOptions {
        titlebar: options.titlebar.map(|titlebar| gpui_kit::TitlebarOptions {
            traffic_light_position: titlebar
                .traffic_light_position
                .map(|at| gpui_kit::point(at.x, px(top))),
            ..titlebar
        }),
        ..options
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_parse_in_both_modes() {
        for preset in Preset::ALL {
            let css = palette::css(&preset.colors(), Radius::Soft.px());
            UiTheme::from_shadcn_css(&css, ThemeMode::Light).unwrap();
            UiTheme::from_shadcn_css(&css, ThemeMode::Dark).unwrap();
        }
    }

    #[test]
    fn a_custom_accent_keeps_its_greys_and_lightens_for_dark() {
        let style = Preset::Graphite
            .style()
            .with_greys(Greys::Warm)
            .with_accent(0.55, 0.19, 262.);
        assert_eq!(style.greys(), Greys::Warm);
        assert_eq!(style.accent(), (0.55, 0.19, 262.));
        assert!(style.colors.accent_dark > style.colors.accent_light);
        let tinted = style.with_greys(Greys::Tinted).with_accent(0.6, 0.15, 30.);
        assert_eq!(tinted.colors.neutral_hue, 30.);
        assert_eq!(tinted.greys(), Greys::Tinted);
        assert_eq!(Preset::Graphite.style().greys(), Greys::Neutral);
        assert_eq!(Preset::Porcelain.style().greys(), Greys::Cool);
    }

    #[test]
    fn radius_and_style_round_trip() {
        let mut style = Preset::Porcelain.style();
        style.radius = Radius::Round;
        assert_eq!(theme_from(&style, ThemeMode::Light).radius.base, px(12.));
        let json = serde_json::to_string(&style).unwrap();
        assert_eq!(serde_json::from_str::<StyleSpec>(&json).unwrap(), style);
    }

    #[test]
    fn presets_differ_and_use_plex() {
        let graphite = theme_for(Preset::Graphite, ThemeMode::Light);
        let porcelain = theme_for(Preset::Porcelain, ThemeMode::Light);
        assert_ne!(graphite.colors.primary, porcelain.colors.primary);
        assert_eq!(graphite.fonts.body, SANS);
        assert_eq!(porcelain.fonts.mono, MONO);
    }

    #[test]
    fn theme_is_not_the_neutral_fallback() {
        assert_ne!(
            theme(ThemeMode::Light).colors.primary,
            UiTheme::neutral_light().colors.primary
        );
        assert_ne!(
            theme(ThemeMode::Dark).colors.sidebar,
            UiTheme::neutral_dark().colors.sidebar
        );
        assert_eq!(theme(ThemeMode::Light).radius.base, px(8.));
    }

    #[test]
    fn scaled_follows_the_spacing_unit() {
        let mut t = theme(ThemeMode::Light);
        assert_eq!(scaled(&t, metrics::HEADER_HEIGHT), px(46.));
        t.spacing.unit = px(5.);
        assert_eq!(scaled(&t, metrics::HEADER_HEIGHT), px(57.5));
    }

    #[test]
    fn avatar_statuses_name_the_presence() {
        use crate::sidebar::Presence;
        let palette = Palette::for_mode(ThemeMode::Light);
        let label = |presence| avatar_status(presence, &palette).label();
        assert_eq!(label(Presence::Online), "online");
        assert_eq!(label(Presence::Away), "away");
        assert_eq!(label(Presence::Dnd), "do not disturb");
        assert_eq!(label(Presence::Offline), "offline");
        assert_eq!(
            avatar_status(Presence::Offline, &palette),
            crate::ui::avatar::AvatarStatus::Offline
        );
        assert_eq!(
            presence_name("Ana Lopez", Presence::Away),
            "Ana Lopez, away"
        );
    }
}
