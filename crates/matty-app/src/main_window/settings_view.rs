//! The Settings window (#732, docs/specs/settings-window-and-team-switcher.md
//! §1): a window of its own on the desktop, a full-window overlay on the web.
//! Both host one [`SettingsView`]: a section list with a search field on the
//! left, the chosen section's page on the right. The pages are the main
//! window's (`settings_pages.rs`); this view only hosts and lays them out, so
//! every change applies to the main window at once.

use std::collections::HashMap;
use std::rc::Rc;

use gpui_icons::{LucideIcon, lucide};
use gpui_kit::{
    AnyElement, App, AppContext as _, Bounds, Context, Entity, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, Styled as _,
    Subscription, WeakEntity, Window, WindowBounds, WindowHandle, WindowOptions, div, point, px,
    size,
};

use super::MainWindow;
use super::settings_pages::{Section, section_list};
use crate::actions::{self, CloseWindow, FocusSearch};
use crate::settings::{SettingsChanged, SettingsStore, WindowGeometry};
use crate::style::{self, TypeStyleExt as _, metrics, scaled};
use crate::t;
use crate::ui::{
    button::{Button, ButtonSize, ButtonVariant},
    dialog::DialogHandle,
    input::{Input, InputEvent, InputState},
    input_group::{InputGroup, InputGroupAddon},
    menu::MenuState,
    scroll_area::ScrollArea,
    select::{SelectEvent, SelectItem, SelectState},
    theme::UiTheme,
    title_bar::TitleBar,
    toast::ToastState,
};

/// The window's size the first time it opens, in logical px.
const DEFAULT_SIZE: (f32, f32) = (880., 640.);
/// The smallest the window gets.
const MIN_SIZE: (f32, f32) = (720., 520.);
/// The section list's width, in px at zoom 1.
pub(super) const SECTION_LIST_WIDTH: f32 = 212.;

/// Where a [`SettingsView`] is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Host {
    /// A window of its own (macOS, Linux, Windows).
    Window,
    /// Over the main window's workspace (the web, which has one window).
    Overlay,
}

impl Host {
    /// The host this build uses: GPUI on the web has one window.
    pub(crate) fn current() -> Self {
        if cfg!(target_family = "wasm") {
            Self::Overlay
        } else {
            Self::Window
        }
    }
}

/// What the main window keeps of an open Settings.
#[derive(Default)]
pub(super) struct SettingsHost {
    /// The open window, on the desktop.
    pub(super) window: Option<WindowHandle<SettingsView>>,
    /// The open overlay, on the web.
    pub(super) overlay: Option<Entity<SettingsView>>,
}

/// What a page select does with the value picked.
type OnChange = Rc<dyn Fn(&str, &mut MainWindow, &mut Context<MainWindow>)>;

/// One select on a settings page, kept while Settings is open: a select
/// owns its popup state, so it cannot be rebuilt each frame. `on_change`
/// is the page's latest handler.
pub(super) struct PageSelect {
    pub(super) state: Entity<SelectState>,
    on_change: Rc<std::cell::RefCell<OnChange>>,
    _subscription: Subscription,
}

/// A saved server's place when its menu was built: index, count, shown.
pub(super) type MenuPlace = (usize, usize, bool);

/// What the pages keep while Settings is open: the selects drawn, by id,
/// and the confirmations Advanced opens.
pub(super) struct PageSelects {
    selects: HashMap<&'static str, PageSelect>,
    pub(super) clear_cache: DialogHandle,
    pub(super) reset_all: DialogHandle,
    /// "Remove {server}?" and "Sign out of {server}?", for the saved
    /// server in `server_action`.
    pub(super) remove_server: DialogHandle,
    pub(super) sign_out: DialogHandle,
    pub(super) server_action: Rc<std::cell::Cell<Option<usize>>>,
    /// Each saved server's ⋯ menu, by its key, with what its items were
    /// built for.
    pub(super) server_menus: HashMap<SharedString, (Entity<MenuState>, MenuPlace)>,
    /// The saved style a ⋯ menu acts on, "Rename…" with its name field,
    /// and "Delete…".
    pub(super) style_target: Rc<std::cell::RefCell<Option<String>>>,
    pub(super) rename_style: DialogHandle,
    pub(super) delete_style: DialogHandle,
    pub(super) rename_input: Option<(Entity<InputState>, Subscription)>,
    /// The style the name field was filled for, while Rename is open.
    pub(super) rename_for: Option<String>,
    /// Each saved style's ⋯ menu, by name.
    pub(super) style_menus: HashMap<String, Entity<MenuState>>,
    /// Appearance's Share ▾ menu.
    pub(super) share_menu: Option<Entity<MenuState>>,
}

impl Default for PageSelects {
    fn default() -> Self {
        Self {
            selects: HashMap::new(),
            clear_cache: DialogHandle::new(false),
            reset_all: DialogHandle::new(false),
            remove_server: DialogHandle::new(false),
            sign_out: DialogHandle::new(false),
            server_action: Rc::default(),
            server_menus: HashMap::new(),
            style_target: Rc::default(),
            rename_style: DialogHandle::new(false),
            delete_style: DialogHandle::new(false),
            rename_input: None,
            rename_for: None,
            style_menus: HashMap::new(),
            share_menu: None,
        }
    }
}

impl PageSelects {
    /// The select `id` with `options` (value, label), showing `value`;
    /// picking an option calls `on_change` with its value. Created on first
    /// use in this window; later calls update its value and handler.
    pub(super) fn get(
        &mut self,
        id: &'static str,
        options: &[(&'static str, SharedString)],
        value: &str,
        on_change: impl Fn(&str, &mut MainWindow, &mut Context<MainWindow>) + 'static,
        window: &mut Window,
        cx: &mut Context<MainWindow>,
    ) -> Entity<SelectState> {
        let on_change: OnChange = Rc::new(on_change);
        let entry = self.selects.entry(id).or_insert_with(|| {
            let items: Vec<SelectItem> = options
                .iter()
                .map(|(value, label)| SelectItem::new(*value, label.clone()))
                .collect();
            let state = cx.new(|cx| SelectState::new(items, window, cx));
            let handler = Rc::new(std::cell::RefCell::new(on_change.clone()));
            let changed = handler.clone();
            let subscription = cx.subscribe(&state, move |this, _, event: &SelectEvent, cx| {
                if let SelectEvent::Change(Some(value)) = event {
                    let handler = changed.borrow().clone();
                    handler(value, this, cx);
                }
            });
            PageSelect {
                state,
                on_change: handler,
                _subscription: subscription,
            }
        });
        *entry.on_change.borrow_mut() = on_change;
        let value = SharedString::from(value.to_owned());
        if entry.state.read(cx).value() != Some(&value) {
            entry
                .state
                .update(cx, |state, cx| state.set_value(Some(value), window, cx));
        }
        entry.state.clone()
    }
}

/// Settings: the section list and search on the left, a page on the right.
pub(crate) struct SettingsView {
    main: WeakEntity<MainWindow>,
    host: Host,
    section: Section,
    search: Entity<InputState>,
    focus: FocusHandle,
    pub(super) selects: PageSelects,
    /// What the style actions did, as toasts (spec §1.3).
    toasts: Entity<ToastState>,
    _subscriptions: Vec<Subscription>,
}

impl SettingsView {
    fn new(
        main: &Entity<MainWindow>,
        host: Host,
        section: Section,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(t!("settings-search")));
        let mut subscriptions = vec![
            // A setting changed here or anywhere else redraws the page.
            cx.observe(main, |_, _, cx| cx.notify()),
            cx.subscribe(
                &SettingsStore::global(cx),
                |_, _, _: &SettingsChanged, cx| cx.notify(),
            ),
            cx.subscribe_in(&search, window, |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
                let _ = this;
            }),
        ];
        if host == Host::Window {
            subscriptions.push(
                cx.observe_window_appearance(window, |_, window, cx| {
                    crate::apply_theme(window, cx)
                }),
            );
            subscriptions.push(
                cx.observe_window_bounds(window, |_, window, cx| remember_bounds(window, cx)),
            );
        }
        let toasts = cx.new(ToastState::new);
        cx.set_global(super::style_library::StyleToasts(toasts.downgrade()));
        Self {
            main: main.downgrade(),
            host,
            section,
            search,
            focus: cx.focus_handle(),
            selects: PageSelects::default(),
            toasts,
            _subscriptions: subscriptions,
        }
    }

    /// Shows `section`, clearing any search.
    pub(crate) fn show(&mut self, section: Section, window: &mut Window, cx: &mut Context<Self>) {
        self.section = section;
        if !self.query(cx).is_empty() {
            self.search
                .update(cx, |search, cx| search.set_value("", window, cx));
        }
        cx.notify();
    }

    /// The search field's text, trimmed.
    pub(crate) fn query(&self, cx: &App) -> String {
        self.search.read(cx).value().trim().to_owned()
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.host {
            Host::Window => window.remove_window(),
            Host::Overlay => {
                self.main
                    .update(cx, |main, cx| main.close_settings(cx))
                    .ok();
            }
        }
    }

    /// Esc clears a search, else closes Settings. Popovers and dialogs in
    /// the page take Esc first.
    fn escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.query(cx).is_empty() {
            self.close(window, cx);
        } else {
            self.search
                .update(cx, |search, cx| search.set_value("", window, cx));
            cx.notify();
        }
    }

    fn search_field(&self, theme: &UiTheme) -> impl IntoElement {
        InputGroup::new(
            "settings.search",
            Input::new(&self.search).aria_label(t!("settings-search")),
        )
        .addon(
            InputGroupAddon::new("settings.search.icon").child(
                lucide(LucideIcon::Search)
                    .size(scaled(theme, metrics::ICON_SMALL))
                    .text_color(theme.colors.muted_foreground),
            ),
        )
        .h(scaled(theme, 30.))
    }

    fn footer(&self, theme: &UiTheme, cx: &App) -> impl IntoElement {
        let settings = SettingsStore::global(cx).read(cx).get();
        let signed_in = settings.account.as_ref().map(|account| {
            let host = crate::live::url_host(&account.server).to_owned();
            let name = if account.display_name.is_empty() {
                account.username.clone()
            } else {
                account.display_name.clone()
            };
            t!("settings-signed-in-as", name = name, host = host)
        });
        let version = match settings.update_channel {
            crate::settings::UpdateChannel::Beta => {
                t!("settings-version-beta", version = crate::VERSION)
            }
            crate::settings::UpdateChannel::Stable => {
                t!("settings-version", version = crate::VERSION)
            }
        };
        div()
            .flex()
            .flex_col()
            .gap(theme.space(0.5))
            .px(theme.space(3.))
            .pb(theme.space(3.))
            .type_style(style::META, theme)
            .text_color(theme.colors.muted_foreground)
            .children(signed_in.map(|line| div().truncate().child(line)))
            .child(div().truncate().child(version))
    }
}

impl Focusable for SettingsView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = UiTheme::read(cx).clone();
        let Some(main) = self.main.upgrade() else {
            return div().into_any_element();
        };
        let query = self.query(cx);
        let section = self.section;
        let selects = &mut self.selects;
        let page = main.update(cx, |main, cx| {
            if query.is_empty() {
                main.settings_page(section, selects, window, cx)
            } else {
                main.settings_search(&query, selects, window, cx)
            }
        });
        let view = cx.entity().downgrade();
        let list = section_list(
            section,
            !query.is_empty(),
            move |section, window, cx| {
                view.update(cx, |this, cx| this.show(section, window, cx))
                    .ok();
            },
            cx,
        );
        let done = (self.host == Host::Overlay).then(|| {
            let view = cx.entity().downgrade();
            Button::new("settings.done")
                .variant(ButtonVariant::Outline)
                .size(ButtonSize::Sm)
                .label(t!("prefs-done"))
                .on_click(move |_, window, cx| {
                    view.update(cx, |this, cx| this.close(window, cx)).ok();
                })
        });
        let sidebar = div()
            .flex_shrink_0()
            .w(scaled(&theme, SECTION_LIST_WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.colors.sidebar)
            .border_r_1()
            .border_color(theme.colors.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme.space(2.))
                    .p(theme.space(3.))
                    .child(div().flex_1().min_w_0().child(self.search_field(&theme)))
                    .children(done),
            )
            .child(div().flex_1().min_h_0().px(theme.space(1.)).child(list))
            .child(self.footer(&theme, cx));
        let body = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(theme.colors.background)
            .child(
                ScrollArea::new(("settings.page", section.index()))
                    .aria_label(section.label())
                    .size_full()
                    .child(page),
            );
        let view = cx.entity().downgrade();
        let escape = view.clone();
        let root = div()
            .key_context(actions::SETTINGS)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &CloseWindow, window, cx| this.close(window, cx)))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                let focus = this.search.read(cx).focus_handle(cx);
                window.focus(&focus, cx);
            }))
            .on_action(cx.listener(|_, _: &actions::OpenSettings, _, _| {}))
            .on_key_down(move |event, window, cx| {
                if event.keystroke.key == "escape" && !event.keystroke.modifiers.modified() {
                    escape.update(cx, |this, cx| this.escape(window, cx)).ok();
                }
            })
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.colors.background)
            .text_color(theme.colors.foreground)
            .font_family(theme.fonts.body.clone())
            .type_style(style::UI, &theme);
        match self.host {
            Host::Window => root
                .child(
                    TitleBar::new()
                        .h(px(metrics::TITLE_BAR_HEIGHT))
                        .on_close(move |_, window, cx| {
                            view.update(cx, |this, cx| this.close(window, cx)).ok();
                        })
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .justify_center()
                                .text_color(theme.colors.muted_foreground)
                                .child(t!("prefs-title")),
                        ),
                )
                .child(div().flex_1().min_h_0().flex().child(sidebar).child(body))
                .child(self.toasts.clone())
                .into_any_element(),
            // The title bar stays above the overlay.
            Host::Overlay => root
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .top(px(metrics::TITLE_BAR_HEIGHT))
                .child(div().flex_1().min_h_0().flex().child(sidebar).child(body))
                .child(self.toasts.clone())
                .into_any_element(),
        }
    }
}

/// Saves where the Settings window is, for next time.
fn remember_bounds(window: &mut Window, cx: &mut App) {
    if window.is_fullscreen() {
        return;
    }
    let bounds = window.window_bounds().get_bounds();
    let geometry = WindowGeometry {
        x: f32::from(bounds.origin.x),
        y: f32::from(bounds.origin.y),
        width: f32::from(bounds.size.width),
        height: f32::from(bounds.size.height),
        maximized: window.is_maximized(),
    };
    SettingsStore::global(cx).update(cx, |store, cx| {
        if store.get().settings_window != Some(geometry) {
            store.update(cx, |s| s.settings_window = Some(geometry));
        }
    });
}

fn window_options(saved: Option<WindowGeometry>, cx: &App) -> WindowOptions {
    let bounds = match saved {
        Some(geometry) => {
            let bounds = Bounds::new(
                point(px(geometry.x), px(geometry.y)),
                size(
                    px(geometry.width.max(MIN_SIZE.0)),
                    px(geometry.height.max(MIN_SIZE.1)),
                ),
            );
            if geometry.maximized {
                WindowBounds::Maximized(bounds)
            } else {
                WindowBounds::Windowed(bounds)
            }
        }
        None => WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(DEFAULT_SIZE.0), px(DEFAULT_SIZE.1)),
            cx,
        )),
    };
    let mut options = WindowOptions {
        window_bounds: Some(bounds),
        window_min_size: Some(size(px(MIN_SIZE.0), px(MIN_SIZE.1))),
        app_id: Some(crate::APP_ID.into()),
        ..style::title_bar_window_options()
    };
    if let Some(titlebar) = options.titlebar.as_mut() {
        titlebar.title = Some(t!("prefs-title"));
    }
    options
}

impl MainWindow {
    /// Opens Settings on `section`, or shows that section in the one open.
    pub(crate) fn open_settings(
        &mut self,
        section: Section,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(handle) = self.settings_host.window
            && handle
                .update(cx, |view, window, cx| {
                    window.activate_window();
                    view.show(section, window, cx);
                })
                .is_ok()
        {
            return;
        }
        if let Some(overlay) = &self.settings_host.overlay {
            overlay.update(cx, |view, cx| view.show(section, window, cx));
            return;
        }
        let main = cx.entity();
        match Host::current() {
            Host::Overlay => {
                let view =
                    cx.new(|cx| SettingsView::new(&main, Host::Overlay, section, window, cx));
                let focus = view.read(cx).focus.clone();
                window.focus(&focus, cx);
                self.settings_host.overlay = Some(view);
                cx.notify();
            }
            Host::Window => {
                let saved = self.settings.read(cx).get().settings_window;
                let options = window_options(saved, cx);
                cx.defer(move |cx| {
                    let opened = cx.open_window(options, |window, cx| {
                        crate::apply_theme(window, cx);
                        let view = cx
                            .new(|cx| SettingsView::new(&main, Host::Window, section, window, cx));
                        let focus = view.read(cx).focus.clone();
                        window.focus(&focus, cx);
                        view
                    });
                    match opened {
                        Ok(handle) => main.update(cx, |main, _| {
                            main.settings_host.window = Some(handle);
                        }),
                        Err(error) => tracing::warn!(%error, "could not open Settings"),
                    }
                });
            }
        }
    }

    /// Closes Settings wherever it is open.
    pub(crate) fn close_settings(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.settings_host.window.take() {
            handle
                .update(cx, |_, window, _| window.remove_window())
                .ok();
        }
        if self.settings_host.overlay.take().is_some() {
            cx.notify();
        }
    }

    /// The overlay to draw over the workspace, on the web.
    pub(super) fn settings_overlay(&self) -> Option<AnyElement> {
        self.settings_host
            .overlay
            .clone()
            .map(|view| view.into_any_element())
    }
}
