//! Colors from a few choices (#704, #855, DESIGN.md §2).
//!
//! A theme's colors are the hue and chroma that tint every neutral, the
//! accent's hue, chroma and lightness in each mode, how the sidebar is
//! filled and how much contrast text gets. Every token is derived from
//! them in OKLCH by rule, not by hand:
//!
//! - **Surfaces** are a ladder of lightness steps from the page, so a
//!   hover, a selected row and a raised panel are each one clear step from
//!   what they sit on, in light and in dark.
//! - **Text** is solved, not picked: each text color moves in lightness
//!   until it reaches its contrast target against every surface it is
//!   drawn on. Nothing depends on a preset happening to pass.
//! - **The accent** keeps the color a person chose. Its label is white or
//!   ink, whichever reads, and links take the accent's hue at a lightness
//!   that reads as text.
//! - **Meaning has fixed hues**: green, amber, red and blue for status, and
//!   amber for a mention of you, whatever the accent is.
//!
//! The committed `assets/theme-*.css` are this module's output for the
//! built-in themes; a test keeps them in step.

use std::fmt::Write as _;

use gpui_kit::Rgba;
use serde::{Deserialize, Serialize};

use crate::contrast::{over, ratio};
use crate::ui::theme::ThemeMode;

/// How the sidebar is filled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sidebar {
    /// A step from the page, in the theme's neutral.
    #[default]
    Soft,
    /// A light wash of the accent's hue.
    Tinted,
    /// Dark in both modes: charcoal beside a light page.
    Dark,
    /// The accent's hue, deep, in both modes.
    Accent,
}

impl Sidebar {
    /// Every choice, in the order Settings offers them.
    pub const ALL: [Self; 4] = [Self::Soft, Self::Tinted, Self::Dark, Self::Accent];
}

/// How much contrast text and control outlines get.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Contrast {
    /// Body text 12:1 or more, quiet text 4.5:1 or more (WCAG AA).
    #[default]
    Standard,
    /// Quiet text and links 7:1 or more (WCAG AAA), control outlines 3:1.
    High,
}

/// The choices a theme's colors come from.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct StyleColors {
    /// Hue of the neutral tint, in degrees.
    pub neutral_hue: f32,
    /// How tinted the neutrals are; 0 is pure grey, 0.01 is clearly tinted.
    pub neutral_chroma: f32,
    /// Hue of the accent, in degrees.
    pub accent_hue: f32,
    /// Accent chroma; under 0.05 the accent is an ink, not a color.
    pub accent_chroma: f32,
    /// Accent lightness in light mode, 0 to 1.
    pub accent_light: f32,
    /// Accent lightness in dark mode, 0 to 1.
    pub accent_dark: f32,
    /// How the sidebar is filled.
    #[serde(default)]
    pub sidebar: Sidebar,
    /// How much contrast text gets.
    #[serde(default)]
    pub contrast: Contrast,
}

impl StyleColors {
    /// Whether the accent is a neutral ink rather than a color.
    pub fn ink(&self) -> bool {
        self.accent_chroma <= 0.05
    }
}

/// A color in OKLCH: lightness 0 to 1, chroma, hue in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklch {
    pub l: f32,
    pub c: f32,
    pub h: f32,
}

const fn lch(l: f32, c: f32, h: f32) -> Oklch {
    Oklch { l, c, h }
}

impl Oklch {
    /// Linear sRGB, unclipped.
    fn linear(self) -> [f32; 3] {
        let h = self.h.to_radians();
        let (a, b) = (self.c * h.cos(), self.c * h.sin());
        let l_ = self.l + 0.396_337_78 * a + 0.215_803_76 * b;
        let m_ = self.l - 0.105_561_346 * a - 0.063_854_17 * b;
        let s_ = self.l - 0.089_484_18 * a - 1.291_485_5 * b;
        let (l, m, s) = (l_.powi(3), m_.powi(3), s_.powi(3));
        [
            4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s,
            -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s,
            -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
        ]
    }

    fn in_gamut(self) -> bool {
        self.linear().iter().all(|x| (-0.0005..=1.0005).contains(x))
    }

    /// This color inside sRGB: chroma comes down until it fits, so the
    /// lightness and hue a rule asked for are the ones it gets.
    fn fit(self) -> Self {
        let color = Self {
            l: self.l.clamp(0., 1.),
            ..self
        };
        if color.in_gamut() {
            return color;
        }
        let (mut low, mut high) = (0., color.c);
        for _ in 0..16 {
            let mid = (low + high) / 2.;
            if (Self { c: mid, ..color }).in_gamut() {
                low = mid;
            } else {
                high = mid;
            }
        }
        Self { c: low, ..color }
    }

    /// As sRGB, fitted to the gamut and rounded to 8 bits a channel, the
    /// way the CSS files store it.
    pub fn rgba(self) -> Rgba {
        let [r, g, b] = self.fit().linear().map(|x| {
            let x = x.clamp(0., 1.);
            let encoded = if x <= 0.003_130_8 {
                12.92 * x
            } else {
                1.055 * x.powf(1. / 2.4) - 0.055
            };
            (encoded * 255.).round() / 255.
        });
        Rgba { r, g, b, a: 1. }
    }
}

/// A token that failed its contrast minimum and was moved until it passed.
#[derive(Clone, Debug, PartialEq)]
pub struct Adjustment {
    /// The shadcn token name, such as `primary`.
    pub token: &'static str,
    /// The lightness it would have had.
    pub from: f32,
    /// The lightness it has now.
    pub to: f32,
}

/// Every token a style sets as CSS, in the order the CSS files list them.
pub const ORDER: [&str; 52] = [
    "background",
    "foreground",
    "card",
    "card-foreground",
    "popover",
    "popover-foreground",
    "primary",
    "primary-foreground",
    "secondary",
    "secondary-foreground",
    "muted",
    "muted-foreground",
    "accent",
    "accent-foreground",
    "destructive",
    "destructive-foreground",
    "success",
    "success-foreground",
    "warning",
    "warning-foreground",
    "info",
    "info-foreground",
    "border",
    "input",
    "ring",
    "chart-1",
    "chart-2",
    "chart-3",
    "chart-4",
    "chart-5",
    "sidebar",
    "sidebar-foreground",
    "sidebar-primary",
    "sidebar-primary-foreground",
    "sidebar-accent",
    "sidebar-accent-foreground",
    "sidebar-border",
    "sidebar-ring",
    "sidebar-muted-foreground",
    "sidebar-badge",
    "sidebar-badge-foreground",
    "link",
    "mention-highlight",
    "mention-highlight-foreground",
    "new-messages",
    // Matty's own, which gpuicn has no token for (`style::Palette`).
    "online",
    "away",
    "dnd",
    "sidebar-online",
    "sidebar-away",
    "sidebar-dnd",
    "sidebar-faint",
];

/// A style's tokens for one mode.
#[derive(Clone, Debug, PartialEq)]
pub struct Tokens {
    colors: Vec<(&'static str, Rgba)>,
    /// The fill behind a field on the sidebar, such as Jump to.
    pub sidebar_field: Rgba,
    /// The selected row's fill on the sidebar, a step past a hovered row's
    /// (`sidebar-accent`).
    pub sidebar_selected: Rgba,
    /// Whether the sidebar is dark while the page is light, or the other
    /// way round, so controls on it cannot use the page's colors.
    pub sidebar_contrasts: bool,
    /// What the contrast guard moved from what the person chose, empty for
    /// a style that passes as is.
    pub adjusted: Vec<Adjustment>,
}

impl Tokens {
    /// The color of `token`, one of [`ORDER`].
    pub fn get(&self, token: &str) -> Option<Rgba> {
        self.colors
            .iter()
            .find(|(name, _)| *name == token)
            .map(|(_, color)| *color)
    }

    /// Every token and its color, in [`ORDER`].
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, Rgba)> + '_ {
        self.colors.iter().copied()
    }
}

fn fixed(value: u32) -> Rgba {
    gpui_kit::rgb(value)
}

fn luminance(color: Rgba) -> f32 {
    // ratio(c, black) = (L + 0.05) / 0.05
    ratio(
        color,
        Rgba {
            r: 0.,
            g: 0.,
            b: 0.,
            a: 1.,
        },
    ) * 0.05
        - 0.05
}

/// Which way [`solve`] moves a color.
#[derive(Clone, Copy, PartialEq)]
enum Toward {
    Darker,
    Lighter,
    /// Away from the surfaces: darker on light ones, lighter on dark.
    Away,
}

/// `color` with its lightness moved, a step at a time, until it reaches
/// `minimum` against every one of `against`.
fn solve(color: Oklch, against: &[Rgba], minimum: f32, toward: Toward) -> Oklch {
    let darker = match toward {
        Toward::Darker => true,
        Toward::Lighter => false,
        Toward::Away => {
            let mean = against.iter().map(|bg| luminance(*bg)).sum::<f32>() / against.len() as f32;
            mean > 0.18
        }
    };
    solve_by(color, darker, |c| {
        against.iter().all(|bg| ratio(c.rgba(), *bg) >= minimum)
    })
}

/// `color` with its lightness moved, a step at a time, until `passes`.
fn solve_by(color: Oklch, darker: bool, passes: impl Fn(Oklch) -> bool) -> Oklch {
    let step = if darker { -0.004 } else { 0.004 };
    let mut next = color;
    while !passes(next) && (0.0..=1.0).contains(&(next.l + step)) {
        next.l += step;
    }
    next
}

/// The label for a solid `fill`: near white when that reads at 4.5:1, else
/// ink, else whichever of the two reads better.
fn label_on(fill: Rgba) -> Oklch {
    let (white, ink) = (lch(0.985, 0., 0.), lch(0.16, 0., 0.));
    let (on_white, on_ink) = (ratio(white.rgba(), fill), ratio(ink.rgba(), fill));
    if on_white >= 4.5 || on_white >= on_ink {
        white
    } else {
        ink
    }
}

/// A solid fill of the accent that shows on `surface` at 3:1, with the
/// label that reads on it at 4.5:1.
fn fill(color: Oklch, surface: Rgba) -> (Oklch, Oklch) {
    let mut fill = solve(color, &[surface], 3., Toward::Away);
    let mut label = label_on(fill.rgba());
    if ratio(label.rgba(), fill.rgba()) < 4.5 {
        let toward = if label.l > 0.5 {
            Toward::Darker
        } else {
            Toward::Lighter
        };
        fill = solve(fill, &[label.rgba()], 4.5, toward);
        label = label_on(fill.rgba());
    }
    (fill, label)
}

/// Derives `colors`' tokens for `mode`. Every text color reaches its
/// contrast target on every surface it is drawn on, and every solid fill
/// shows on its surface and carries a label that reads.
pub fn tokens(colors: &StyleColors, mode: ThemeMode) -> Tokens {
    let StyleColors {
        neutral_hue: nh,
        neutral_chroma: nc,
        accent_hue: ah,
        accent_chroma: ac,
        accent_light: al,
        accent_dark: ald,
        sidebar,
        contrast,
    } = *colors;
    let light = mode == ThemeMode::Light;
    let high = contrast == Contrast::High;
    let ink = colors.ink();
    let mut adjusted = Vec::new();
    // A neutral: lightness `l`, the theme's tint scaled by `k`.
    let n = |l: f32, k: f32| lch(l, nc * k, nh);
    let pick = |standard: f32, high_value: f32| if high { high_value } else { standard };

    // The surface ladder. Light: the page is the brightest plane and fills
    // step down from it. Dark: the page is dim and fills step up, so what
    // is raised or pressed is always nearer the text color.
    let (page, raised, muted, hover, secondary, border, control, foreground, quiet) = if light {
        (
            n(0.993, 0.5),
            n(0.999, 0.3),
            n(0.952, 1.2),
            n(0.94, 1.3),
            n(0.932, 1.3),
            n(pick(0.898, 0.85), 1.3),
            n(pick(0.80, 0.64), 1.6),
            n(pick(0.235, 0.16), 2.5),
            n(pick(0.52, 0.42), 2.5),
        )
    } else {
        (
            n(0.215, 1.6),
            n(0.25, 1.6),
            n(0.262, 1.5),
            n(0.285, 1.5),
            n(0.30, 1.5),
            n(pick(0.31, 0.37), 1.4),
            n(pick(0.41, 0.56), 1.4),
            n(pick(0.915, 0.97), 0.8),
            n(pick(0.71, 0.80), 1.4),
        )
    };
    let page_rgb = page.rgba();
    let content = [
        page_rgb,
        raised.rgba(),
        muted.rgba(),
        hover.rgba(),
        secondary.rgba(),
    ];
    let text_minimum = pick(4.5, 7.);
    let foreground = solve(foreground, &content, 7., Toward::Away);
    let quiet = solve(quiet, &content, text_minimum, Toward::Away);
    let control = if high {
        solve(control, &[page_rgb, raised.rgba()], 3., Toward::Away)
    } else {
        control
    };

    // The sidebar: its fill, then its own ladder and text, solved against
    // it the same way, so a dark sidebar beside a light page works.
    let tint_hue = if ink { nh } else { ah };
    let side = match (sidebar, light) {
        (Sidebar::Soft, true) => n(0.966, 1.3),
        (Sidebar::Soft, false) => n(0.185, 1.6),
        (Sidebar::Tinted, true) => lch(0.958, (nc * 1.3).max(0.016), tint_hue),
        (Sidebar::Tinted, false) => lch(0.195, (nc * 1.8).max(0.022), tint_hue),
        (Sidebar::Dark, true) => n(0.255, if nc < 0.004 { 3. } else { 2. }),
        (Sidebar::Dark, false) => n(0.155, 1.6),
        (Sidebar::Accent, true) => lch(0.31, (ac * 0.55).clamp(0.03, 0.10), tint_hue),
        (Sidebar::Accent, false) => lch(0.235, (ac * 0.4).clamp(0.03, 0.07), tint_hue),
    };
    let side_rgb = side.rgba();
    let side_dark = luminance(side_rgb) < 0.18;
    let sidebar_contrasts = side_dark == light;
    // A hovered row is one step from the sidebar and the selected row two,
    // so selection never reads as a hover that stuck.
    let side_step = |hover: f32, selected: f32, chroma: f32| {
        (
            lch(side.l + hover, side.c * chroma, side.h),
            lch(side.l + selected, side.c * chroma, side.h),
        )
    };
    let (side_hover, side_selected) = if side_dark {
        side_step(0.045, 0.085, 1.15)
    } else {
        side_step(-0.035, -0.065, 1.2)
    };
    let side_border = if side_dark {
        lch(side.l + 0.07, side.c * 1.1, side.h)
    } else {
        lch(side.l - 0.068, side.c * 1.1, side.h)
    };
    let side_surfaces = [side_rgb, side_selected.rgba()];
    let side_foreground = if !sidebar_contrasts {
        foreground
    } else if side_dark {
        lch(0.95, side.c.min(0.01), side.h)
    } else {
        lch(0.235, (side.c * 2.).min(0.02), side.h)
    };
    let side_foreground = solve(side_foreground, &side_surfaces, 7., Toward::Away);
    let side_quiet = lch(
        match (side_dark, high) {
            (true, false) => 0.74,
            (true, true) => 0.84,
            (false, false) => 0.50,
            (false, true) => 0.40,
        },
        (side.c * 1.5).min(0.03),
        side.h,
    );
    let side_quiet = solve(side_quiet, &side_surfaces, text_minimum, Toward::Away);
    // Muted channels: the one text below 4.5:1, since a person asked not
    // to see them; still 3:1, or 4.5:1 with high contrast.
    let side_faint = solve(
        lch(
            if side_dark { 0.60 } else { 0.62 },
            (side.c * 1.5).min(0.03),
            side.h,
        ),
        &[side_rgb],
        pick(3.2, 4.5),
        Toward::Away,
    );
    let sidebar_field = if sidebar_contrasts {
        over(side_foreground.rgba().opacity(0.1), side_rgb)
    } else {
        page_rgb
    };

    // The accent as a fill, keeping the color chosen where it can.
    let chosen = lch(if light { al } else { ald }, ac, ah);
    let (primary, primary_label) = fill(chosen, page_rgb);
    if (primary.l - chosen.l).abs() > 0.012 {
        adjusted.push(Adjustment {
            token: "primary",
            from: chosen.l,
            to: primary.l,
        });
    }
    // The accent as text: its hue at a lightness that reads. An ink accent
    // would read as plain text, so links take a quiet blue there.
    let link = if ink {
        if light {
            lch(0.50, 0.14, 262.)
        } else {
            lch(0.78, 0.11, 262.)
        }
    } else if light {
        lch(al.min(0.55), ac, ah)
    } else {
        lch(ald.max(0.74), ac * 0.85, ah)
    };
    let link = solve(link, &content, text_minimum, Toward::Away);
    let ring = solve(primary, &[page_rgb, raised.rgba()], 3., Toward::Away);
    let side_ring = solve(primary, &[side_rgb], 3., Toward::Away);
    // Mention counts on the sidebar: the accent where it shows there.
    let (badge, badge_label) = fill(primary, side_rgb);

    // Status: fixed hues, solved as text on the page, on cards and on
    // their own tint (an error banner is the error color at 8%).
    let status = |l_light: f32, l_dark: f32, c: f32, h: f32| {
        let minimum = pick(4.5, 6.);
        solve_by(
            lch(if light { l_light } else { l_dark }, c, h),
            light,
            |x| {
                let color = x.rgba();
                let banner = over(color.opacity(0.08), page_rgb);
                [page_rgb, raised.rgba(), banner]
                    .iter()
                    .all(|bg| ratio(color, *bg) >= minimum)
            },
        )
    };
    let destructive = status(0.55, 0.70, 0.20, 27.);
    let success = status(0.52, 0.74, 0.15, 150.);
    let warning = status(0.56, 0.80, 0.13, 75.);
    let info = status(0.52, 0.74, 0.14, 250.);
    // Presence dots: 3:1 on what they sit on. The sidebar has its own, so
    // a dark sidebar beside a light page gets dots that show on each.
    let dot = |l_light: f32, l_dark: f32, c: f32, h: f32| {
        let on_page = solve(
            lch(if light { l_light } else { l_dark }, c, h),
            &[page_rgb, raised.rgba()],
            3.,
            Toward::Away,
        );
        let on_side = solve(
            lch(if side_dark { l_dark } else { l_light }, c, h),
            &side_surfaces,
            3.,
            Toward::Away,
        );
        (on_page, on_side)
    };
    let (online, side_online) = dot(0.62, 0.74, 0.17, 150.);
    let (away, side_away) = dot(0.68, 0.80, 0.15, 75.);
    let (dnd, side_dnd) = dot(0.58, 0.68, 0.20, 27.);
    // A mention of you is amber whatever the accent is, so it is never
    // mistaken for a link or a hovered row.
    let mention = if light {
        lch(0.82, 0.155, 82.)
    } else {
        lch(0.80, 0.14, 82.)
    };
    let mention_label = lch(0.22, 0.03, 82.);

    let chart_1 = if !ink {
        primary
    } else if light {
        lch(0.52, 0.12, 268.)
    } else {
        lch(0.72, 0.12, 268.)
    };
    let charts = if light {
        [0x16a34a, 0xcc7a00, 0xd42020, 0x8b5cf6]
    } else {
        [0x3fb950, 0xf2b038, 0xf0564f, 0xa78bfa]
    };

    let fg = foreground.rgba();
    let side_fg = side_foreground.rgba();
    let on = |fill: Oklch| label_on(fill.rgba()).rgba();
    let colors = vec![
        ("background", page_rgb),
        ("foreground", fg),
        ("card", raised.rgba()),
        ("card-foreground", fg),
        ("popover", raised.rgba()),
        ("popover-foreground", fg),
        ("primary", primary.rgba()),
        ("primary-foreground", primary_label.rgba()),
        ("secondary", secondary.rgba()),
        ("secondary-foreground", fg),
        ("muted", muted.rgba()),
        ("muted-foreground", quiet.rgba()),
        ("accent", hover.rgba()),
        ("accent-foreground", fg),
        ("destructive", destructive.rgba()),
        ("destructive-foreground", on(destructive)),
        ("success", success.rgba()),
        ("success-foreground", on(success)),
        ("warning", warning.rgba()),
        ("warning-foreground", on(warning)),
        ("info", info.rgba()),
        ("info-foreground", on(info)),
        ("border", border.rgba()),
        ("input", control.rgba()),
        ("ring", ring.rgba()),
        ("chart-1", chart_1.rgba()),
        ("chart-2", fixed(charts[0])),
        ("chart-3", fixed(charts[1])),
        ("chart-4", fixed(charts[2])),
        ("chart-5", fixed(charts[3])),
        ("sidebar", side_rgb),
        ("sidebar-foreground", side_fg),
        ("sidebar-primary", badge.rgba()),
        ("sidebar-primary-foreground", badge_label.rgba()),
        ("sidebar-accent", side_hover.rgba()),
        ("sidebar-accent-foreground", side_fg),
        ("sidebar-border", side_border.rgba()),
        ("sidebar-ring", side_ring.rgba()),
        ("sidebar-muted-foreground", side_quiet.rgba()),
        ("sidebar-badge", badge.rgba()),
        ("sidebar-badge-foreground", badge_label.rgba()),
        ("link", link.rgba()),
        ("mention-highlight", mention.rgba()),
        ("mention-highlight-foreground", mention_label.rgba()),
        ("new-messages", destructive.rgba()),
        ("online", online.rgba()),
        ("away", away.rgba()),
        ("dnd", dnd.rgba()),
        ("sidebar-online", side_online.rgba()),
        ("sidebar-away", side_away.rgba()),
        ("sidebar-dnd", side_dnd.rgba()),
        ("sidebar-faint", side_faint.rgba()),
    ];
    debug_assert_eq!(colors.len(), ORDER.len());
    debug_assert!(
        colors
            .iter()
            .zip(ORDER)
            .all(|((name, _), want)| *name == want)
    );
    Tokens {
        colors,
        sidebar_field,
        sidebar_selected: side_selected.rgba(),
        sidebar_contrasts,
        adjusted,
    }
}

/// `color` as `#rrggbb`.
pub fn css_hex(color: Rgba) -> String {
    let byte = |x: f32| (x * 255.).round() as u8;
    format!(
        "#{:02x}{:02x}{:02x}",
        byte(color.r),
        byte(color.g),
        byte(color.b)
    )
}

/// The tokens that are Matty's own and not CSS variables gpuicn reads.
const OWN: [&str; 7] = [
    "online",
    "away",
    "dnd",
    "sidebar-online",
    "sidebar-away",
    "sidebar-dnd",
    "sidebar-faint",
];

/// `colors` as shadcn CSS, light in `:root` and dark in `.dark`, with
/// `radius` px corners: what gpuicn's `UiTheme::from_shadcn_css` reads.
pub fn css(colors: &StyleColors, radius: f32) -> String {
    let mut out = String::new();
    for (mode, selector) in [(ThemeMode::Light, ":root"), (ThemeMode::Dark, ".dark")] {
        let tokens = tokens(colors, mode);
        let _ = writeln!(out, "{selector} {{");
        if mode == ThemeMode::Light {
            let _ = writeln!(out, "  --radius: {radius}px;");
        }
        for (name, color) in tokens.iter().filter(|(name, _)| !OWN.contains(name)) {
            let _ = writeln!(out, "  --{name}: {};", css_hex(color));
        }
        out.push_str("}\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::Preset;
    use super::*;

    /// The pairs every theme must pass, whatever a person chooses.
    fn failing(colors: &StyleColors, mode: ThemeMode) -> Vec<String> {
        let t = tokens(colors, mode);
        let get = |name: &str| t.get(name).unwrap();
        let text = if colors.contrast == Contrast::High {
            7.
        } else {
            4.5
        };
        let mention_row = over(get("mention-highlight").opacity(0.06), get("background"));
        let row_hover = over(get("muted").opacity(0.5), get("background"));
        let mut pairs = Vec::new();
        for surface in ["background", "card", "muted", "accent", "secondary"] {
            pairs.push(("foreground", get(surface), surface, 7.));
            pairs.push(("muted-foreground", get(surface), surface, text));
            pairs.push(("link", get(surface), surface, text));
        }
        pairs.extend([
            ("muted-foreground", mention_row, "a mention row", 4.5),
            ("muted-foreground", row_hover, "a hovered row", 4.5),
            ("link", mention_row, "a mention row", 4.5),
            ("primary-foreground", get("primary"), "primary", 4.5),
            ("primary", get("background"), "background", 3.),
            ("ring", get("background"), "background", 3.),
            ("ring", get("card"), "card", 3.),
            ("destructive", get("background"), "background", 4.5),
            ("success", get("background"), "background", 4.5),
            ("warning", get("background"), "background", 4.5),
            ("info", get("background"), "background", 4.5),
            (
                "destructive-foreground",
                get("destructive"),
                "destructive",
                4.5,
            ),
            (
                "mention-highlight-foreground",
                get("mention-highlight"),
                "mention",
                4.5,
            ),
            ("sidebar-foreground", get("sidebar"), "sidebar", 7.),
            ("sidebar-foreground", get("sidebar-accent"), "hovered", 7.),
            ("sidebar-foreground", t.sidebar_selected, "selected", 7.),
            ("sidebar-muted-foreground", get("sidebar"), "sidebar", text),
            (
                "sidebar-muted-foreground",
                get("sidebar-accent"),
                "hovered",
                text,
            ),
            (
                "sidebar-muted-foreground",
                t.sidebar_selected,
                "selected",
                text,
            ),
            ("sidebar-faint", get("sidebar"), "sidebar", 3.),
            ("sidebar-foreground", t.sidebar_field, "sidebar field", 7.),
            (
                "sidebar-muted-foreground",
                t.sidebar_field,
                "sidebar field",
                4.5,
            ),
            (
                "sidebar-badge-foreground",
                get("sidebar-badge"),
                "badge",
                4.5,
            ),
            ("sidebar-badge", get("sidebar"), "sidebar", 3.),
            ("sidebar-ring", get("sidebar"), "sidebar", 3.),
        ]);
        for dot in ["online", "away", "dnd"] {
            pairs.push((dot, get("background"), "background", 3.));
        }
        for dot in ["sidebar-online", "sidebar-away", "sidebar-dnd"] {
            pairs.push((dot, get("sidebar"), "sidebar", 3.));
            pairs.push((dot, t.sidebar_selected, "selected", 3.));
        }
        let error = get("destructive");
        pairs.push((
            "destructive",
            over(error.opacity(0.08), get("background")),
            "an error banner",
            4.5,
        ));
        pairs
            .into_iter()
            .filter(|(token, bg, _, minimum)| ratio(get(token), *bg) < minimum - 0.005)
            .map(|(token, bg, on, minimum)| {
                format!(
                    "{mode:?} {token} on {on}: {:.2} < {minimum}",
                    ratio(get(token), bg)
                )
            })
            .collect()
    }

    #[test]
    fn every_built_in_theme_is_readable() {
        for preset in Preset::ALL {
            for contrast in [Contrast::Standard, Contrast::High] {
                let colors = StyleColors {
                    contrast,
                    ..preset.colors()
                };
                for mode in [ThemeMode::Light, ThemeMode::Dark] {
                    let failing = failing(&colors, mode);
                    assert!(
                        failing.is_empty(),
                        "{preset:?} {contrast:?}: {}",
                        failing.join("; ")
                    );
                }
            }
        }
    }

    #[test]
    fn built_in_themes_keep_the_accent_they_name() {
        for preset in Preset::ALL {
            for mode in [ThemeMode::Light, ThemeMode::Dark] {
                let tokens = tokens(&preset.colors(), mode);
                assert!(
                    tokens.adjusted.is_empty(),
                    "{preset:?} {mode:?}: {:?}",
                    tokens.adjusted
                );
            }
        }
    }

    /// The committed CSS is what this module writes. Refresh it with
    /// `MATTY_WRITE_THEMES=1 cargo test -p matty-app palette`.
    #[test]
    fn matches_the_committed_presets() {
        for preset in Preset::ALL {
            let name = preset.id();
            let path = format!("{}/assets/theme-{name}.css", env!("CARGO_MANIFEST_DIR"));
            let want = format!(
                "/* Matty colors: {} (docs/DESIGN.md §2).\n   \
                 shadcn CSS variable format: light reads :root, dark reads :root then\n   \
                 .dark, as gpuicn's UiTheme::from_shadcn_css does. Written by\n   \
                 crates/matty-app/src/style/palette.rs; change the theme's numbers in\n   \
                 style.rs, then run MATTY_WRITE_THEMES=1 cargo test -p matty-app palette. */\n{}",
                preset.summary(),
                css(&preset.colors(), 8.)
            );
            if std::env::var_os("MATTY_WRITE_THEMES").is_some() {
                std::fs::write(&path, &want).unwrap();
            }
            let have = std::fs::read_to_string(&path).unwrap_or_default();
            assert!(
                have == want,
                "assets/theme-{name}.css is out of date; run \
                 MATTY_WRITE_THEMES=1 cargo test -p matty-app palette"
            );
        }
    }

    #[test]
    fn a_pale_accent_keeps_its_color_under_an_ink_label() {
        let lemon = StyleColors {
            neutral_hue: 90.,
            neutral_chroma: 0.004,
            accent_hue: 100.,
            accent_chroma: 0.15,
            accent_light: 0.92,
            accent_dark: 0.92,
            sidebar: Sidebar::Soft,
            contrast: Contrast::Standard,
        };
        let light = tokens(&lemon, ThemeMode::Light);
        // On a white page the fill must come down to show at all.
        assert!(
            light
                .adjusted
                .iter()
                .any(|a| a.token == "primary" && a.to < a.from)
        );
        let page = light.get("background").unwrap();
        let primary = light.get("primary").unwrap();
        assert!(ratio(primary, page) >= 3.);
        assert!(ratio(light.get("primary-foreground").unwrap(), primary) >= 4.5);
        assert!(ratio(light.get("link").unwrap(), page) >= 4.5);
        // In dark the lemon stays lemon, under an ink label.
        let dark = tokens(&lemon, ThemeMode::Dark);
        assert!(dark.adjusted.is_empty());
        let label = dark.get("primary-foreground").unwrap();
        assert!(label.r < 0.2, "an ink label on a pale fill");
    }

    #[test]
    fn a_dim_accent_is_lightened_in_dark_mode() {
        let navy = StyleColors {
            neutral_hue: 260.,
            neutral_chroma: 0.006,
            accent_hue: 265.,
            accent_chroma: 0.12,
            accent_light: 0.3,
            accent_dark: 0.3,
            sidebar: Sidebar::Soft,
            contrast: Contrast::Standard,
        };
        let dark = tokens(&navy, ThemeMode::Dark);
        assert!(
            dark.adjusted
                .iter()
                .any(|a| a.token == "primary" && a.to > a.from)
        );
        let primary = dark.get("primary").unwrap();
        assert!(ratio(primary, dark.get("background").unwrap()) >= 3.);
        assert!(ratio(dark.get("primary-foreground").unwrap(), primary) >= 4.5);
    }

    #[test]
    fn every_choice_yields_a_readable_theme() {
        for hue in (0..360).step_by(30) {
            for (chroma, lightness) in [(0.02, 0.3), (0.16, 0.6), (0.3, 0.85), (0.1, 0.12)] {
                for sidebar in Sidebar::ALL {
                    for contrast in [Contrast::Standard, Contrast::High] {
                        for neutral_chroma in [0., 0.012, 0.03] {
                            let colors = StyleColors {
                                neutral_hue: (hue + 40) as f32,
                                neutral_chroma,
                                accent_hue: hue as f32,
                                accent_chroma: chroma,
                                accent_light: lightness,
                                accent_dark: lightness,
                                sidebar,
                                contrast,
                            };
                            for mode in [ThemeMode::Light, ThemeMode::Dark] {
                                let failing = failing(&colors, mode);
                                assert!(failing.is_empty(), "{colors:?}: {}", failing.join("; "));
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_dark_sidebar_beside_a_light_page_has_its_own_text() {
        let colors = StyleColors {
            sidebar: Sidebar::Dark,
            ..Preset::Graphite.colors()
        };
        let t = tokens(&colors, ThemeMode::Light);
        assert!(t.sidebar_contrasts);
        let side = t.get("sidebar").unwrap();
        assert!(ratio(t.get("sidebar-foreground").unwrap(), side) >= 7.);
        assert!(ratio(t.get("foreground").unwrap(), side) < 3.);
        // In dark mode the same sidebar is simply the deepest surface.
        assert!(!tokens(&colors, ThemeMode::Dark).sidebar_contrasts);
    }

    #[test]
    fn css_parses_with_the_radius() {
        let css = css(&Preset::Porcelain.colors(), 12.);
        let theme = crate::ui::theme::UiTheme::from_shadcn_css(&css, ThemeMode::Dark).unwrap();
        assert_eq!(theme.radius.base, gpui_kit::px(12.));
        assert!(theme.colors.link.is_some());
        assert!(theme.colors.sidebar_badge.is_some());
    }

    #[test]
    fn old_styles_read_with_a_soft_sidebar() {
        let old = r#"{"neutral_hue":260,"neutral_chroma":0.006,"accent_hue":268,
            "accent_chroma":0.12,"accent_light":0.52,"accent_dark":0.76}"#;
        let colors: StyleColors = serde_json::from_str(old).unwrap();
        assert_eq!(colors.sidebar, Sidebar::Soft);
        assert_eq!(colors.contrast, Contrast::Standard);
    }
}
