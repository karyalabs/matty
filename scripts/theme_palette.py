#!/usr/bin/env python3
"""Writes Matty's built-in color presets, crates/matty-app/assets/theme-*.css
(docs/DESIGN.md §2).

A preset is a few numbers: the neutral hue and chroma that tint every grey,
and the accent's hue, chroma and lightness in each mode. Every token is
derived from them in OKLCH, so light and dark stay in step and a new preset
costs one line. Run it after changing PRESETS, then scripts/contrast.py.

The app derives the same tokens at runtime (crates/matty-app/src/style/
palette.rs), with a contrast guard for people's own styles; keep
Preset::colors there in step with PRESETS (a test checks).
"""
import math
from pathlib import Path

ASSETS = Path(__file__).resolve().parent.parent / "crates/matty-app/assets"

# name: (neutral hue, neutral chroma, accent hue, accent chroma,
#        accent lightness in light mode, in dark mode, description)
PRESETS = {
    "graphite": (260, 0.002, 260, 0.008, 0.25, 0.93,
                 "Graphite, the default: ink on paper, near-neutral greys and an ink accent"),
    "porcelain": (260, 0.006, 268, 0.12, 0.52, 0.76,
                  "Porcelain: cool greys with a soft lapis accent"),
}

# Presence and status colors are not part of a preset.
CHARTS = {
    "light": ["#16a34a", "#cc7a00", "#d42020", "#8b5cf6"],
    "dark": ["#3fb950", "#f2b038", "#f0564f", "#a78bfa"],
}
DESTRUCTIVE = {"light": "#cf2a2a", "dark": "#f0564f"}

ORDER = [
    "background", "foreground", "card", "card-foreground", "popover",
    "popover-foreground", "primary", "primary-foreground", "secondary",
    "secondary-foreground", "muted", "muted-foreground", "accent",
    "accent-foreground", "destructive", "border", "input", "ring", "chart-1",
    "chart-2", "chart-3", "chart-4", "chart-5", "sidebar", "sidebar-foreground",
    "sidebar-primary", "sidebar-primary-foreground", "sidebar-accent",
    "sidebar-accent-foreground", "sidebar-border", "sidebar-ring", "link",
]


def oklch(lightness, chroma, hue):
    """An OKLCH color as sRGB hex, clipped to the gamut."""
    h = math.radians(hue)
    a, b = chroma * math.cos(h), chroma * math.sin(h)
    l_ = lightness + 0.3963377774 * a + 0.2158037573 * b
    m_ = lightness - 0.1055613458 * a - 0.0638541728 * b
    s_ = lightness - 0.0894841775 * a - 1.2914855480 * b
    l, m, s = l_**3, m_**3, s_**3
    rgb = (
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    )

    def encode(x):
        x = max(0.0, min(1.0, x))
        return 12.92 * x if x <= 0.0031308 else 1.055 * x ** (1 / 2.4) - 0.055

    return "#%02x%02x%02x" % tuple(round(encode(v) * 255) for v in rgb)


def tokens(preset, mode):
    nh, nc, ah, ac, al, ald, _ = preset
    if mode == "light":
        page = oklch(0.995, nc * 0.3, nh)
        t = {
            "background": page, "card": page, "popover": page,
            "foreground": oklch(0.21, nc * 2, nh),
            "primary": oklch(al, ac, ah), "primary-foreground": oklch(0.99, 0, 0),
            "secondary": oklch(0.945, nc, nh), "accent": oklch(0.945, nc, nh),
            "muted": oklch(0.955, nc, nh), "muted-foreground": oklch(0.50, nc * 2, nh),
            "border": oklch(0.915, nc, nh), "input": oklch(0.885, nc, nh),
            "sidebar": oklch(0.973, nc, nh), "sidebar-accent": oklch(0.925, nc * 1.3, nh),
            "sidebar-border": oklch(0.915, nc, nh),
        }
    else:
        raised = oklch(0.225, nc * 1.2, nh)
        t = {
            "background": oklch(0.19, nc * 1.2, nh), "card": raised, "popover": raised,
            "foreground": oklch(0.94, nc * 0.8, nh),
            "primary": oklch(ald, ac * 0.9, ah),
            "primary-foreground": oklch(0.18, 0.01 if ac > 0.05 else 0, ah),
            "secondary": oklch(0.255, nc, nh), "accent": oklch(0.25, nc * 1.2, nh),
            "muted": oklch(0.25, nc, nh), "muted-foreground": oklch(0.70, nc * 1.2, nh),
            "border": oklch(0.26, nc, nh), "input": oklch(0.30, nc, nh),
            "sidebar": oklch(0.163, nc * 1.2, nh), "sidebar-accent": oklch(0.255, nc * 1.5, nh),
            "sidebar-border": oklch(0.245, nc, nh),
        }
    for name in ("card", "popover", "secondary", "accent", "sidebar", "sidebar-accent"):
        t[name + "-foreground"] = t["foreground"]
    t["ring"] = t["sidebar-ring"] = t["sidebar-primary"] = t["primary"]
    t["sidebar-primary-foreground"] = t["primary-foreground"]
    t["destructive"] = DESTRUCTIVE[mode]
    # Links follow the accent, except under an ink accent, where they would
    # read as plain text; there they take a quiet blue.
    if ac > 0.05:
        t["link"] = t["primary"]
    else:
        t["link"] = oklch(0.50, 0.13, 262) if mode == "light" else oklch(0.78, 0.10, 262)
    # chart-1 follows the accent unless the accent is a neutral ink.
    t["chart-1"] = t["primary"] if ac > 0.05 else oklch(al + 0.27 if mode == "light" else ald - 0.2, 0.12, 268)
    for i, color in enumerate(CHARTS[mode]):
        t[f"chart-{i + 2}"] = color
    return t


def write(name, preset):
    lines = [
        f"/* Matty colors: {preset[6]} (docs/DESIGN.md §2).",
        "   shadcn CSS variable format: light reads :root, dark reads :root then",
        "   .dark, as gpuicn's UiTheme::from_shadcn_css does. Written by",
        "   scripts/theme_palette.py; change the preset there, not here. */",
    ]
    for mode, selector in (("light", ":root"), ("dark", ".dark")):
        t = tokens(preset, mode)
        lines.append(selector + " {")
        if mode == "light":
            lines.append("  --radius: 0.5rem;")
        lines += [f"  --{key}: {t[key]};" for key in ORDER]
        lines.append("}")
        lines.append("")
    (ASSETS / f"theme-{name}.css").write_text("\n".join(lines))


if __name__ == "__main__":
    for name, preset in PRESETS.items():
        write(name, preset)
        print(f"wrote theme-{name}.css")
