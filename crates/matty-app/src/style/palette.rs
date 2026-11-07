//! Colors from a few numbers (#704, DESIGN.md §2).
//!
//! A style's colors are the neutral hue and chroma that tint every grey,
//! and the accent's hue, chroma and lightness in each mode. Every shadcn
//! token is derived from them in OKLCH, so light and dark stay in step and
//! any accent a person picks yields a whole, readable theme.
//!
//! `scripts/theme_palette.py` does the same derivation for the docs and the
//! committed `assets/theme-*.css`; a test keeps the two in step.

use std::fmt::Write as _;

use gpui_kit::Rgba;
use serde::{Deserialize, Serialize};

use crate::contrast::ratio;
use crate::ui::theme::ThemeMode;

/// The numbers a style's colors come from.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct StyleColors {
    /// Hue of the grey tint, in degrees.
    pub neutral_hue: f32,
    /// How tinted the greys are; 0 is pure grey, 0.01 is clearly tinted.
    pub neutral_chroma: f32,
    /// Hue of the accent, in degrees.
    pub accent_hue: f32,
    /// Accent chroma; under 0.05 the accent is an ink, not a color.
    pub accent_chroma: f32,
    /// Accent lightness in light mode, 0 to 1.
    pub accent_light: f32,
    /// Accent lightness in dark mode, 0 to 1.
    pub accent_dark: f32,
}

impl StyleColors {
    /// Whether the accent is a neutral ink rather than a color.
    fn ink(&self) -> bool {
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
    /// As sRGB, clipped to the gamut and rounded to 8 bits a channel, the
    /// way the CSS files store it.
    pub fn rgba(self) -> Rgba {
        let h = self.h.to_radians();
        let (a, b) = (self.c * h.cos(), self.c * h.sin());
        let l_ = self.l + 0.396_337_78 * a + 0.215_803_76 * b;
        let m_ = self.l - 0.105_561_346 * a - 0.063_854_17 * b;
        let s_ = self.l - 0.089_484_18 * a - 1.291_485_5 * b;
        let (l, m, s) = (l_.powi(3), m_.powi(3), s_.powi(3));
        let linear = [
            4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s,
            -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s,
            -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
        ];
        let [r, g, b] = linear.map(|x| {
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
    /// The shadcn token name, such as `muted-foreground`.
    pub token: &'static str,
    /// The lightness it would have had.
    pub from: f32,
    /// The lightness it has now.
    pub to: f32,
}

/// Every token a style sets, in the order the CSS files list them.
pub const ORDER: [&str; 32] = [
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
    "link",
];

/// A style's tokens for one mode.
#[derive(Clone, Debug, PartialEq)]
pub struct Tokens {
    colors: Vec<(&'static str, Rgba)>,
    /// What the contrast guard moved, empty for a style that passes as is.
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
}

fn fixed(value: u32) -> Rgba {
    gpui_kit::rgb(value)
}

/// Moves `color`'s lightness, a step at a time, until it reaches `minimum`
/// against every one of `against`: darker in light mode, lighter in dark.
fn guard(
    token: &'static str,
    color: Oklch,
    against: &[Rgba],
    minimum: f32,
    mode: ThemeMode,
    adjusted: &mut Vec<Adjustment>,
) -> Oklch {
    let passes = |c: Oklch| against.iter().all(|bg| ratio(c.rgba(), *bg) >= minimum);
    let step = match mode {
        ThemeMode::Light => -0.005,
        ThemeMode::Dark => 0.005,
    };
    let mut next = color;
    while !passes(next) && (0.0..=1.0).contains(&(next.l + step)) {
        next.l += step;
    }
    if next != color {
        adjusted.push(Adjustment {
            token,
            from: color.l,
            to: next.l,
        });
    }
    next
}

/// Derives `colors`' tokens for `mode`, with the contrast guard applied:
/// text and links reach 4.5:1 on the surfaces they sit on, and the accent
/// 3:1 on the page and 4.5:1 under its own label.
pub fn tokens(colors: &StyleColors, mode: ThemeMode) -> Tokens {
    let StyleColors {
        neutral_hue: nh,
        neutral_chroma: nc,
        accent_hue: ah,
        accent_chroma: ac,
        accent_light: al,
        accent_dark: ald,
    } = *colors;
    let mut adjusted = Vec::new();
    let light = mode == ThemeMode::Light;

    let (page, raised, sidebar, muted) = if light {
        let page = lch(0.995, nc * 0.3, nh);
        (page, page, lch(0.973, nc, nh), lch(0.955, nc, nh))
    } else {
        (
            lch(0.19, nc * 1.2, nh),
            lch(0.225, nc * 1.2, nh),
            lch(0.163, nc * 1.2, nh),
            lch(0.25, nc, nh),
        )
    };
    let surfaces = [page.rgba(), raised.rgba(), sidebar.rgba(), muted.rgba()];
    let foreground = guard(
        "foreground",
        if light {
            lch(0.21, nc * 2., nh)
        } else {
            lch(0.94, nc * 0.8, nh)
        },
        &surfaces,
        4.5,
        mode,
        &mut adjusted,
    );
    let muted_foreground = guard(
        "muted-foreground",
        if light {
            lch(0.50, nc * 2., nh)
        } else {
            lch(0.70, nc * 1.2, nh)
        },
        &surfaces,
        4.5,
        mode,
        &mut adjusted,
    );
    let primary_foreground = if light {
        lch(0.99, 0., 0.)
    } else {
        lch(0.18, if colors.ink() { 0. } else { 0.01 }, ah)
    };
    let primary = guard(
        "primary",
        if light {
            lch(al, ac, ah)
        } else {
            lch(ald, ac * 0.9, ah)
        },
        &[page.rgba()],
        3.,
        mode,
        &mut adjusted,
    );
    // The label on a filled button is near white in light mode and near
    // black in dark, so the accent moves away from it until the label reads.
    let primary = guard(
        "primary",
        primary,
        &[primary_foreground.rgba()],
        4.5,
        mode,
        &mut adjusted,
    );
    // Links follow the accent, except under an ink accent, where they would
    // read as plain text; there they take a quiet blue.
    let link = guard(
        "link",
        if !colors.ink() {
            primary
        } else if light {
            lch(0.50, 0.13, 262.)
        } else {
            lch(0.78, 0.10, 262.)
        },
        &surfaces,
        4.5,
        mode,
        &mut adjusted,
    );
    let (secondary, accent, border, input, sidebar_accent, sidebar_border) = if light {
        (
            lch(0.945, nc, nh),
            lch(0.945, nc, nh),
            lch(0.915, nc, nh),
            lch(0.885, nc, nh),
            lch(0.925, nc * 1.3, nh),
            lch(0.915, nc, nh),
        )
    } else {
        (
            lch(0.255, nc, nh),
            lch(0.25, nc * 1.2, nh),
            lch(0.26, nc, nh),
            lch(0.30, nc, nh),
            lch(0.255, nc * 1.5, nh),
            lch(0.245, nc, nh),
        )
    };
    let chart_1 = if !colors.ink() {
        primary
    } else if light {
        lch(al + 0.27, 0.12, 268.)
    } else {
        lch(ald - 0.2, 0.12, 268.)
    };
    let (charts, destructive) = if light {
        ([0x16a34a, 0xcc7a00, 0xd42020, 0x8b5cf6], 0xcf2a2a)
    } else {
        ([0x3fb950, 0xf2b038, 0xf0564f, 0xa78bfa], 0xf0564f)
    };

    let fg = foreground.rgba();
    let colors = vec![
        ("background", page.rgba()),
        ("foreground", fg),
        ("card", raised.rgba()),
        ("card-foreground", fg),
        ("popover", raised.rgba()),
        ("popover-foreground", fg),
        ("primary", primary.rgba()),
        ("primary-foreground", primary_foreground.rgba()),
        ("secondary", secondary.rgba()),
        ("secondary-foreground", fg),
        ("muted", muted.rgba()),
        ("muted-foreground", muted_foreground.rgba()),
        ("accent", accent.rgba()),
        ("accent-foreground", fg),
        ("destructive", fixed(destructive)),
        ("border", border.rgba()),
        ("input", input.rgba()),
        ("ring", primary.rgba()),
        ("chart-1", chart_1.rgba()),
        ("chart-2", fixed(charts[0])),
        ("chart-3", fixed(charts[1])),
        ("chart-4", fixed(charts[2])),
        ("chart-5", fixed(charts[3])),
        ("sidebar", sidebar.rgba()),
        ("sidebar-foreground", fg),
        ("sidebar-primary", primary.rgba()),
        ("sidebar-primary-foreground", primary_foreground.rgba()),
        ("sidebar-accent", sidebar_accent.rgba()),
        ("sidebar-accent-foreground", fg),
        ("sidebar-border", sidebar_border.rgba()),
        ("sidebar-ring", primary.rgba()),
        ("link", link.rgba()),
    ];
    debug_assert_eq!(colors.len(), ORDER.len());
    Tokens { colors, adjusted }
}

fn css_hex(color: Rgba) -> String {
    let byte = |x: f32| (x * 255.).round() as u8;
    format!(
        "#{:02x}{:02x}{:02x}",
        byte(color.r),
        byte(color.g),
        byte(color.b)
    )
}

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
        for (name, color) in &tokens.colors {
            let _ = writeln!(out, "  --{name}: {};", css_hex(*color));
        }
        out.push_str("}\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `--name: #rrggbb;` pairs from one block of a committed CSS file.
    fn committed(css: &str, selector: &str) -> Vec<(String, Rgba)> {
        let start = css.find(&format!("{selector} {{")).unwrap();
        let body = &css[start..css[start..].find('}').unwrap() + start];
        body.lines()
            .filter_map(|line| {
                let (name, value) = line.trim().strip_prefix("--")?.split_once(':')?;
                let value = value.trim().trim_end_matches(';');
                let hex = u32::from_str_radix(value.strip_prefix('#')?, 16).ok()?;
                Some((name.to_owned(), fixed(hex)))
            })
            .collect()
    }

    #[test]
    fn matches_the_committed_presets() {
        for (preset, css) in [
            (
                super::super::Preset::Graphite,
                include_str!("../../assets/theme-graphite.css"),
            ),
            (
                super::super::Preset::Porcelain,
                include_str!("../../assets/theme-porcelain.css"),
            ),
        ] {
            for (mode, selector) in [(ThemeMode::Light, ":root"), (ThemeMode::Dark, ".dark")] {
                let tokens = tokens(&preset.colors(), mode);
                assert!(
                    tokens.adjusted.is_empty(),
                    "{preset:?} {mode:?}: {:?}",
                    tokens.adjusted
                );
                let expected = committed(css, selector);
                assert_eq!(expected.len(), ORDER.len(), "{preset:?} {selector}");
                for (name, want) in expected {
                    let got = tokens.get(&name).unwrap();
                    // Python and Rust round a half differently: one step.
                    let close = |a: f32, b: f32| (a - b).abs() <= 1.5 / 255.;
                    assert!(
                        close(got.r, want.r) && close(got.g, want.g) && close(got.b, want.b),
                        "{preset:?} {mode:?} {name}: {} vs {}",
                        css_hex(got),
                        css_hex(want)
                    );
                }
            }
        }
    }

    #[test]
    fn a_pale_accent_is_darkened_until_it_reads() {
        let lemon = StyleColors {
            neutral_hue: 90.,
            neutral_chroma: 0.004,
            accent_hue: 100.,
            accent_chroma: 0.15,
            accent_light: 0.92,
            accent_dark: 0.92,
        };
        let light = tokens(&lemon, ThemeMode::Light);
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
    fn every_hue_yields_a_readable_theme() {
        for hue in (0..360).step_by(15) {
            let colors = StyleColors {
                neutral_hue: hue as f32,
                neutral_chroma: 0.012,
                accent_hue: hue as f32,
                accent_chroma: 0.16,
                accent_light: 0.6,
                accent_dark: 0.6,
            };
            for mode in [ThemeMode::Light, ThemeMode::Dark] {
                let t = tokens(&colors, mode);
                let page = t.get("background").unwrap();
                for text in ["foreground", "muted-foreground", "link"] {
                    assert!(
                        ratio(t.get(text).unwrap(), page) >= 4.5,
                        "{hue} {mode:?} {text}"
                    );
                }
                let primary = t.get("primary").unwrap();
                assert!(
                    ratio(t.get("primary-foreground").unwrap(), primary) >= 4.5,
                    "{hue} {mode:?}"
                );
            }
        }
    }

    #[test]
    fn css_parses_with_the_radius() {
        let css = css(&super::super::Preset::Porcelain.colors(), 12.);
        let theme = crate::ui::theme::UiTheme::from_shadcn_css(&css, ThemeMode::Dark).unwrap();
        assert_eq!(theme.radius.base, gpui_kit::px(12.));
    }
}
