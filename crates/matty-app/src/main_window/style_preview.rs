//! A theme drawn small (#855): the gallery's thumbnail and the editor's
//! preview, a tiny Matty window in the theme's own colors. Both are drawn
//! from the same tokens the app uses, so what they show is what picking
//! the theme gives: the sidebar with a selected row, an unread row and a
//! mention count, a message with a link, a hovered message that mentions
//! you, and the
//! composer with its send button.

use gpui_icons::{LucideIcon, lucide};
use gpui_kit::{
    Div, FontWeight, ParentElement as _, Rgba, SharedString, Styled as _, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::contrast::over;
use crate::style::{self, Palette, StyleSpec};
use crate::t;
use crate::ui::theme::{ThemeMode, UiColors, UiTheme};

/// A theme's colors in one mode, and its corners.
#[derive(Clone, Copy)]
pub(super) struct Look {
    pub(super) colors: UiColors,
    pub(super) palette: Palette,
    /// The base corner radius, in px.
    pub(super) radius: f32,
}

impl Look {
    /// `style` as it shows in `mode`.
    pub(super) fn of(style: &StyleSpec, mode: ThemeMode) -> Self {
        Self {
            colors: style::theme_from(style, mode).colors,
            palette: Palette::of(style, mode),
            radius: style.radius.px(),
        }
    }

    /// A Mattermost theme, in its own light or dark.
    pub(super) fn mattermost(theme: &crate::mattermost_theme::Theme) -> Self {
        let (ui, palette) = theme.ui();
        Self {
            colors: ui.colors,
            palette,
            radius: style::Radius::Soft.px(),
        }
    }

    fn corner(&self, scale: f32) -> gpui_kit::Pixels {
        px((self.radius * scale).max(1.))
    }
}

fn bar(width: f32, height: f32, color: Rgba) -> Div {
    div()
        .h(px(height))
        .w(relative(width))
        .rounded_full()
        .bg(color)
}

/// The gallery's thumbnail, `height` px tall: the window in bars and dots.
pub(super) fn thumbnail(look: &Look, height: f32) -> Div {
    let c = &look.colors;
    let quiet = c.sidebar_muted_foreground();
    // A hovered message row, as gpuicn's Message draws it.
    let hovered_row = over(c.muted.opacity(0.5), c.background);
    let side_row = |child: Div| div().h(px(9.)).flex().items_center().child(child);
    let message = |name: f32, text: Div| {
        div()
            .flex()
            .gap(px(5.))
            .child(
                div()
                    .flex_none()
                    .size(px(11.))
                    .rounded_full()
                    .bg(c.secondary),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(3.5))
                    .pt(px(1.))
                    .child(bar(name, 3.5, c.foreground))
                    .child(text),
            )
    };
    div()
        .w_full()
        .h(px(height))
        .flex()
        .overflow_hidden()
        .rounded(look.corner(0.75))
        .border_1()
        .border_color(c.border)
        .bg(c.background)
        .child(
            div()
                .w(relative(0.34))
                .h_full()
                .bg(c.sidebar)
                .border_r_1()
                .border_color(c.sidebar_border)
                .px(px(6.))
                .py(px(6.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .h(px(11.))
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .child(
                            div()
                                .flex_none()
                                .size(px(8.))
                                .rounded(px(2.5))
                                .bg(c.sidebar_primary),
                        )
                        .child(bar(0.5, 3.5, c.sidebar_foreground)),
                )
                .child(div().h(px(2.)))
                .child(side_row(bar(0.62, 3., quiet)))
                .child(
                    div()
                        .h(px(11.))
                        .mx(px(-3.))
                        .px(px(3.))
                        .flex()
                        .items_center()
                        .rounded(look.corner(0.35))
                        .bg(look.palette.sidebar_selected)
                        .child(bar(0.7, 3., c.sidebar_accent_foreground)),
                )
                .child(
                    div()
                        .h(px(9.))
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(bar(0.5, 3.5, c.sidebar_foreground))
                        .child(
                            div()
                                .flex_none()
                                .w(px(9.))
                                .h(px(6.))
                                .rounded_full()
                                .bg(c.sidebar_badge()),
                        ),
                )
                .child(side_row(bar(0.56, 3., quiet))),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .h(px(17.))
                        .flex_none()
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .border_b_1()
                        .border_color(c.border)
                        .child(bar(0.32, 4., c.foreground)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .flex_col()
                        .justify_end()
                        .child(
                            div().px(px(8.)).py(px(4.)).child(message(
                                0.3,
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(3.))
                                    .child(bar(0.45, 3., c.muted_foreground))
                                    .child(bar(0.2, 3., c.link())),
                            )),
                        )
                        .child(
                            div().px(px(8.)).py(px(4.)).bg(hovered_row).child(message(
                                0.36,
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(3.))
                                    .child(
                                        div()
                                            .flex_none()
                                            .w(px(14.))
                                            .h(px(6.))
                                            .rounded(px(2.))
                                            .bg(c.mention_highlight()),
                                    )
                                    .child(bar(0.5, 3., c.muted_foreground)),
                            )),
                        ),
                )
                .child(
                    div()
                        .flex_none()
                        .m(px(7.))
                        .h(px(16.))
                        .px(px(3.))
                        .flex()
                        .items_center()
                        .justify_between()
                        .rounded(look.corner(0.6))
                        .border_1()
                        .border_color(c.input)
                        .child(div().w(relative(0.4)).pl(px(3.)).child(bar(
                            1.,
                            3.,
                            c.muted_foreground.opacity(0.6),
                        )))
                        .child(
                            div()
                                .flex_none()
                                .size(px(10.))
                                .rounded(look.corner(0.35))
                                .bg(c.primary),
                        ),
                ),
        )
}

/// The editor's preview: the window with real, small text, so a person
/// sees how each color reads. `caption` names the mode above it.
pub(super) fn scene(look: &Look, caption: SharedString, theme: &UiTheme) -> Div {
    let c = &look.colors;
    let p = &look.palette;
    let family = theme.fonts.body.clone();
    let quiet = c.sidebar_muted_foreground();
    let text = |size: f32, color: Rgba| {
        div()
            .text_size(px(size))
            .line_height(px(size + 5.))
            .text_color(color)
    };
    let side_row = |label: SharedString, color: Rgba, weight: FontWeight| {
        div()
            .h(px(20.))
            .px(px(6.))
            .flex()
            .items_center()
            .gap(px(5.))
            .rounded(look.corner(0.6))
            .child(
                text(10.5, color)
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_weight(weight)
                    .child(label),
            )
    };
    let hash = |name: SharedString| -> SharedString { format!("# {name}").into() };
    let badge = |count: &'static str| {
        div()
            .flex_none()
            .h(px(13.))
            .min_w(px(13.))
            .px(px(4.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(c.sidebar_badge())
            .child(
                text(8.5, c.sidebar_badge_foreground())
                    .line_height(px(9.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(count),
            )
    };
    let avatar = |initials: &'static str, presence: Option<Rgba>| {
        div()
            .relative()
            .flex_none()
            .size(px(22.))
            .rounded_full()
            .bg(c.secondary)
            .flex()
            .items_center()
            .justify_center()
            .child(
                text(8.5, c.secondary_foreground)
                    .line_height(px(9.))
                    .font_weight(FontWeight::MEDIUM)
                    .child(initials),
            )
            .children(presence.map(|color| {
                div()
                    .absolute()
                    .right(px(-1.))
                    .bottom(px(-1.))
                    .size(px(8.))
                    .rounded_full()
                    .bg(color)
                    .border_1()
                    .border_color(c.background)
            }))
    };
    let author = |name: SharedString, time: &'static str| {
        div()
            .flex()
            .items_end()
            .gap(px(5.))
            .child(
                text(10.5, c.foreground)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(name),
            )
            .child(text(8.5, c.muted_foreground).child(time))
    };
    let sidebar = div()
        .w(relative(0.36))
        .flex_none()
        .h_full()
        .bg(c.sidebar)
        .border_r_1()
        .border_color(c.sidebar_border)
        .p(px(6.))
        .flex()
        .flex_col()
        .gap(px(1.))
        .child(
            div()
                .h(px(24.))
                .px(px(6.))
                .flex()
                .items_center()
                .gap(px(6.))
                .child(
                    div()
                        .flex_none()
                        .size(px(14.))
                        .rounded(look.corner(0.45))
                        .bg(c.sidebar_primary)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            text(8., c.sidebar_primary_foreground)
                                .line_height(px(8.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("M"),
                        ),
                )
                .child(
                    text(11.5, c.sidebar_foreground)
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Matty"), // i18n-ignore: the app's name
                ),
        )
        .child(
            side_row(
                t!("prefs-preview-threads"),
                c.sidebar_foreground,
                FontWeight::SEMIBOLD,
            )
            .child(badge("2")),
        )
        .child(
            div().h(px(18.)).px(px(6.)).flex().items_end().child(
                text(8.5, quiet)
                    .font_weight(FontWeight::MEDIUM)
                    .child(t!("prefs-preview-channels")),
            ),
        )
        .child(
            side_row(
                hash(t!("prefs-preview-general")),
                c.sidebar_accent_foreground,
                FontWeight::MEDIUM,
            )
            .bg(p.sidebar_selected),
        )
        .child(
            side_row(
                hash(t!("prefs-preview-town-square")),
                c.sidebar_foreground,
                FontWeight::SEMIBOLD,
            )
            .child(
                div()
                    .flex_none()
                    .size(px(5.))
                    .mr(px(3.))
                    .rounded_full()
                    .bg(c.sidebar_foreground),
            ),
        )
        .child(
            side_row(
                hash(t!("prefs-preview-design")),
                c.sidebar_accent_foreground,
                FontWeight::NORMAL,
            )
            .bg(c.sidebar_accent),
        )
        .child(side_row(
            hash(t!("prefs-preview-random")),
            quiet,
            FontWeight::NORMAL,
        ))
        .child(side_row(
            hash(t!("prefs-preview-archive")),
            p.sidebar_faint,
            FontWeight::NORMAL,
        ));
    // A hovered message row, as gpuicn's Message draws it.
    let hovered_row = over(c.muted.opacity(0.5), c.background);
    // Buttons and the status colors, as they sit on the page.
    let button = |label: SharedString, fill: Option<Rgba>, color: Rgba| {
        text(10., color)
            .flex_none()
            .h(px(20.))
            .px(px(8.))
            .flex()
            .items_center()
            .rounded(look.corner(0.7))
            .font_weight(FontWeight::MEDIUM)
            .border_1()
            .border_color(fill.unwrap_or(c.input))
            .when_some(fill, |button, fill| button.bg(fill))
            .child(label)
    };
    let status = |label: SharedString, color: Rgba| {
        div()
            .flex()
            .items_center()
            .gap(px(3.))
            .child(div().flex_none().size(px(5.)).rounded_full().bg(color))
            .child(text(10., color).child(label))
    };
    let controls = div()
        .px(px(10.))
        .pt(px(8.))
        .flex()
        .items_center()
        .gap(px(6.))
        .child(button(
            t!("prefs-preview-save"),
            Some(c.primary),
            c.primary_foreground,
        ))
        .child(button(t!("prefs-preview-cancel"), None, c.foreground))
        .child(div().flex_1())
        .child(status(t!("prefs-preview-saved"), c.success))
        .child(status(t!("prefs-preview-failed"), c.destructive));
    let messages = div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .justify_between()
        .pb(px(2.))
        .child(controls)
        .child(
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .px(px(10.))
                        .py(px(5.))
                        .flex()
                        .gap(px(7.))
                        .child(avatar("AL", Some(p.online)))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(author(t!("prefs-preview-ada"), "09:41"))
                                .child(
                                    div()
                                        .flex()
                                        .flex_wrap()
                                        .gap_x(px(3.))
                                        .child(
                                            text(10.5, c.foreground)
                                                .child(t!("prefs-preview-message")),
                                        )
                                        .child(
                                            text(10.5, c.link()).child(t!("prefs-preview-link")),
                                        ),
                                ),
                        ),
                )
                .child(
                    div()
                        .px(px(10.))
                        .py(px(5.))
                        .flex()
                        .gap(px(7.))
                        .bg(hovered_row)
                        .child(avatar("SO", Some(p.away)))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(author(t!("prefs-preview-sam"), "09:43"))
                                .child(
                                    div()
                                        .flex()
                                        .flex_wrap()
                                        .items_center()
                                        .gap_x(px(3.))
                                        .child(
                                            text(10.5, c.mention_highlight_foreground())
                                                .px(px(3.))
                                                .rounded(px(3.))
                                                .bg(c.mention_highlight())
                                                .font_weight(FontWeight::MEDIUM)
                                                .child(t!("prefs-preview-mention")),
                                        )
                                        .child(
                                            text(10.5, c.foreground)
                                                .child(t!("prefs-preview-reply")),
                                        ),
                                ),
                        ),
                ),
        );
    let composer = div()
        .flex_none()
        .mx(px(10.))
        .mb(px(10.))
        .mt(px(4.))
        .h(px(28.))
        .pl(px(8.))
        .pr(px(4.))
        .flex()
        .items_center()
        .justify_between()
        .rounded(look.corner(1.))
        .border_1()
        .border_color(c.input)
        .child(text(10.5, c.muted_foreground).child(t!("prefs-preview-composer")))
        .child(
            div()
                .flex_none()
                .size(px(20.))
                .rounded(look.corner(0.7))
                .bg(c.primary)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    lucide(LucideIcon::SendHorizontal)
                        .size(px(11.))
                        .text_color(c.primary_foreground),
                ),
        );
    let main = div()
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .flex_col()
        .child(
            div()
                .flex_none()
                .h(px(30.))
                .px(px(10.))
                .flex()
                .items_center()
                .gap(px(6.))
                .border_b_1()
                .border_color(c.border)
                .child(
                    text(11.5, c.foreground)
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(hash(t!("prefs-preview-general"))),
                )
                .child(
                    text(9., c.muted_foreground)
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(t!("prefs-preview-topic")),
                ),
        )
        .child(messages)
        .child(composer);
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(theme.space(1.5))
        .child(
            div()
                .text_size(theme.text(style::META.size))
                .line_height(theme.text(style::META.line))
                .text_color(theme.colors.muted_foreground)
                .child(caption),
        )
        .child(
            div()
                .w_full()
                .h(px(214.))
                .flex()
                .overflow_hidden()
                .font_family(family)
                .rounded(look.corner(1.2))
                .border_1()
                .border_color(theme.colors.border)
                .bg(c.background)
                .child(sidebar)
                .child(main),
        )
}
