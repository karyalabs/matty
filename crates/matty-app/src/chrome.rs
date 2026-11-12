//! The window's chrome without a title bar (spec
//! `docs/specs/sidebar-and-full-height-window.md` §1, §3): the sidebar and
//! the pane headers are the top edge. The pane that starts the top row draws
//! Toggle sidebar, Back and Forward at the left of its header; the pane that
//! ends it draws the window controls on Windows and client-decorated Linux.
//! Every header drags the window. The main window tells each pane its part
//! with [`PaneChrome`], and the panes draw it with the helpers here.

use gpui_icons::{LucideIcon, lucide};
use gpui_kit::{
    AnyElement, App, IntoElement, ParentElement as _, SharedString, Styled as _, Window, div, px,
};

use crate::actions::{self, HistoryBack, HistoryForward, ToggleSidebar};
use crate::style::{self, metrics, scaled};
use crate::ui::{
    button::{Button, ButtonSize, ButtonVariant},
    theme::UiTheme,
    title_bar::{WindowControls, draws_window_controls, traffic_light_inset, window_drag_area},
};

/// A pane header's part in the window's chrome.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PaneChrome {
    /// The header starts the window's top row: Toggle sidebar, Back and
    /// Forward lead it.
    pub leading: bool,
    /// History has somewhere to go back or forward to.
    pub back: bool,
    pub forward: bool,
    /// The docked sidebar is hidden; on macOS the header then clears the
    /// traffic lights.
    pub sidebar_hidden: bool,
    /// The header ends the window's top row: the window controls end it.
    pub controls: bool,
    /// The header is in the main window, so dragging it moves the window.
    /// Pop-outs keep their own title bar.
    pub drag: bool,
    /// While the sidebar is hidden, what it would show across every team
    /// and server: mentions as a pill on the toggle, else a dot for unread.
    pub hidden_mentions: usize,
    pub hidden_unread: bool,
    /// The listing the right pane shows, for the pressed state of the
    /// Mentions and Saved buttons that end a leading header.
    pub panel: Option<Panel>,
}

/// A panel the leading header's buttons open beside a place
/// (`docs/specs/sidebar-places-and-switching.md` §3). Panels never select a
/// sidebar row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    Mentions,
    Saved,
}

gpui_kit::actions!(
    matty,
    [
        /// The pointer rests on Toggle sidebar while the sidebar is hidden:
        /// slide it in over the content (spec §3). Not bound to a key.
        PeekSidebar,
    ]
);

/// Toggle sidebar, Back and Forward, for a header that starts the top row.
pub fn leading(chrome: PaneChrome, window: &Window, cx: &App) -> Option<AnyElement> {
    if !chrome.leading {
        return None;
    }
    let theme = UiTheme::read(cx);
    let inset = if chrome.sidebar_hidden {
        // The traffic lights sit over the header's left end.
        (traffic_light_inset(window) - theme.space(2.)).max(px(0.))
    } else {
        px(0.)
    };
    Some(
        div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(2.))
            .pl(inset)
            .child(toggle(chrome, cx))
            .child(nav_button(
                "chrome.back",
                "Back",
                LucideIcon::ChevronLeft,
                Box::new(HistoryBack),
                chrome.back,
                cx,
            ))
            .child(nav_button(
                "chrome.forward",
                "Forward",
                LucideIcon::ChevronRight,
                Box::new(HistoryForward),
                chrome.forward,
                cx,
            ))
            .into_any_element(),
    )
}

/// Recent mentions and Saved messages, for a header that starts the top
/// row: they open their listing beside the current place and show pressed
/// while it is open; pressed, a click closes the right pane (spec §3).
pub fn panels(chrome: PaneChrome, separated: bool, cx: &App) -> Option<AnyElement> {
    use gpui_kit::prelude::FluentBuilder as _;
    if !chrome.leading {
        return None;
    }
    let theme = UiTheme::read(cx);
    let button = |id: &'static str,
                  label: &'static str,
                  glyph: LucideIcon,
                  panel: Panel,
                  action: Box<dyn gpui_kit::Action>| {
        let pressed = chrome.panel == Some(panel);
        let button = crate::ui::toggle::Toggle::new(id)
            .size(crate::ui::toggle::ToggleSize::Sm)
            .w(scaled(theme, 28.))
            .h(scaled(theme, 28.))
            .aria_label(label)
            .pressed(pressed)
            .px_0()
            .when(pressed, |toggle| toggle.bg(theme.colors.accent))
            .on_change(move |&on, window, cx| {
                if on {
                    window.dispatch_action(action.boxed_clone(), cx)
                } else {
                    window.dispatch_action(Box::new(actions::CloseRightPane), cx)
                }
            })
            .child(
                lucide(glyph)
                    .size(scaled(theme, metrics::ICON))
                    .text_color(if pressed {
                        theme.colors.accent_foreground
                    } else {
                        theme.colors.muted_foreground
                    }),
            );
        style::tipped(
            SharedString::from(format!("{id}.tooltip")),
            actions::tooltip_text(label, label),
            button,
        )
    };
    Some(
        div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(2.))
            .pr(theme.space(2.))
            .when(separated, |group| {
                group.child(
                    div()
                        .w(style::hairline())
                        .h(scaled(theme, 18.))
                        .mr(theme.space(1.))
                        .bg(theme.colors.border),
                )
            })
            .child(button(
                "chrome.mentions",
                "Recent mentions",
                LucideIcon::AtSign,
                Panel::Mentions,
                Box::new(actions::RecentMentions),
            ))
            .child(button(
                "chrome.saved",
                "Saved messages",
                LucideIcon::Bookmark,
                Panel::Saved,
                Box::new(actions::SavedMessages),
            ))
            .into_any_element(),
    )
}

/// The window controls, for a header that ends the top row; `None` where
/// the platform draws its own.
pub fn trailing(chrome: PaneChrome, window: &Window) -> Option<AnyElement> {
    (chrome.controls && draws_window_controls(window)).then(|| {
        WindowControls::new()
            .h_full()
            .flex_shrink_0()
            .into_any_element()
    })
}

/// Whether the header ends in window controls, so its own right padding
/// goes and the controls sit flush with the window's edge.
pub fn has_controls(chrome: PaneChrome, window: &Window) -> bool {
    chrome.controls && draws_window_controls(window)
}

/// Makes a header drag the window when it is in the main window.
pub fn drag<E>(
    id: impl Into<gpui_kit::ElementId>,
    header: E,
    chrome: PaneChrome,
    window: &mut Window,
    cx: &mut App,
) -> E
where
    E: gpui_kit::StatefulInteractiveElement + gpui_kit::base::InteractiveElementExt + IntoElement,
{
    if chrome.drag {
        window_drag_area(id, header, window, cx)
    } else {
        header
    }
}

/// Toggle sidebar; while the sidebar is hidden it carries what waits there
/// and resting on it peeks the sidebar.
fn toggle(chrome: PaneChrome, cx: &App) -> impl IntoElement {
    use gpui_kit::prelude::FluentBuilder as _;
    use gpui_kit::{InteractiveElement as _, StatefulInteractiveElement as _};
    let theme = UiTheme::read(cx);
    let ring = theme.colors.background;
    let mark = if !chrome.sidebar_hidden {
        None
    } else if chrome.hidden_mentions > 0 {
        Some(
            crate::ui::badge::Badge::count(chrome.hidden_mentions, 99)
                .h(scaled(theme, 15.))
                .min_w(scaled(theme, 15.))
                .px(px(3.))
                .text_size(theme.text(9.5))
                .line_height(theme.text(9.5))
                .bg(theme.colors.primary)
                .text_color(theme.colors.primary_foreground)
                .border_2()
                .border_color(ring)
                .absolute()
                .top(px(-3.))
                .right(px(-5.))
                .into_any_element(),
        )
    } else if chrome.hidden_unread {
        Some(
            div()
                .absolute()
                .top(px(1.))
                .right(px(1.))
                .size(scaled(theme, 9.))
                .rounded_full()
                .bg(theme.colors.foreground)
                .border_2()
                .border_color(ring)
                .into_any_element(),
        )
    } else {
        None
    };
    let hidden = chrome.sidebar_hidden;
    div()
        .id("chrome.sidebar.peek")
        .relative()
        .when(hidden, |el| {
            el.on_hover(|hovered, window, cx| {
                if *hovered {
                    window.dispatch_action(Box::new(PeekSidebar), cx);
                }
            })
        })
        .child(nav_button(
            "chrome.sidebar",
            "Toggle sidebar",
            LucideIcon::PanelLeft,
            Box::new(ToggleSidebar),
            true,
            cx,
        ))
        .children(mark)
}

/// A 28px ghost icon button that dispatches `action`, with a tooltip
/// naming it and its shortcut (spec §3).
fn nav_button(
    id: &'static str,
    label: &'static str,
    glyph: LucideIcon,
    action: Box<dyn gpui_kit::Action>,
    enabled: bool,
    cx: &App,
) -> impl IntoElement {
    let theme = UiTheme::read(cx);
    let button = Button::new(id)
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::IconSm)
        .w(scaled(theme, 28.))
        .h(scaled(theme, 28.))
        .aria_label(label)
        .disabled(!enabled)
        .on_click(move |_, window, cx| window.dispatch_action(action.boxed_clone(), cx))
        .child(
            lucide(glyph)
                .size(scaled(theme, metrics::ICON))
                .text_color(theme.colors.muted_foreground),
        );
    style::tipped(
        SharedString::from(format!("{id}.tooltip")),
        actions::tooltip_text(label, label),
        button,
    )
}

/// `header` as the window's top edge: the leading group before it and the
/// window controls after it when this pane has them, the whole row
/// dragging the window. Each part carries the header's hairline, so the
/// line under the row stays whole.
pub fn frame(
    id: &'static str,
    chrome: PaneChrome,
    header: impl IntoElement,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let theme = UiTheme::read(cx).clone();
    let edge = |part: gpui_kit::Div| {
        part.flex_shrink_0()
            .h(scaled(&theme, metrics::HEADER_HEIGHT))
            .flex()
            .items_center()
            .border_b(style::hairline())
            .border_color(theme.colors.border)
    };
    let leading = leading(chrome, window, cx).map(|group| {
        edge(div())
            .pl(theme.space(2.))
            .child(group)
            .into_any_element()
    });
    // The channel header's own buttons come first; a hairline parts them.
    // The Threads list header is too narrow for them, and leaving Threads
    // is what a panel does anyway; their shortcuts still work there.
    let panels = (id != "threads.header")
        .then(|| panels(chrome, id == "channel.header", cx))
        .flatten()
        .map(|group| edge(div()).child(group));
    let trailing = trailing(chrome, window).map(|controls| edge(div()).child(controls));
    if leading.is_none() && panels.is_none() && trailing.is_none() && !chrome.drag {
        return header.into_any_element();
    }
    let row = gpui_kit::InteractiveElement::id(div(), id)
        .flex()
        .flex_shrink_0()
        .w_full()
        .min_w_0()
        .children(leading)
        .child(div().flex_1().min_w_0().child(header))
        .children(panels)
        .children(trailing);
    drag(
        gpui_kit::ElementId::Name(format!("{id}.drag").into()),
        row,
        chrome,
        window,
        cx,
    )
    .into_any_element()
}
