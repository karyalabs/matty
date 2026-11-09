//! Preferences' Fonts group (#706): menus for the interface font and the
//! code font, listing this machine's families (only monospace ones for
//! code), and "Add font file…" for a font the person brings.

use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, IntoElement as _, ParentElement as _,
    SharedString, Styled as _, Window, div, prelude::FluentBuilder as _,
};

use super::MainWindow;
use crate::fonts::{self, AddError};
use crate::settings::{Settings, SettingsStore};
use crate::style::{self, MONO, SANS, TypeStyleExt as _};
use crate::t;
use crate::ui::{
    button::{Button, ButtonSize, ButtonVariant},
    combobox::combobox,
    select::{SelectEvent, SelectItem, SelectState},
    theme::UiTheme,
};

/// Which menu.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Interface,
    Code,
}

impl Role {
    fn default_family(self) -> &'static str {
        match self {
            Self::Interface => SANS,
            Self::Code => MONO,
        }
    }

    fn setting(self, settings: &mut Settings) -> &mut Option<String> {
        match self {
            Self::Interface => &mut settings.ui_font,
            Self::Code => &mut settings.code_font,
        }
    }
}

/// The menu's options: IBM Plex first, marked as the default, then every
/// family (every monospace family, for code). `None` while the monospace
/// families aren't known yet.
fn items(role: Role, cx: &gpui_kit::App) -> Option<Vec<SelectItem>> {
    let families = fonts::families(cx);
    let default = role.default_family();
    let first = SelectItem::new(default, t!("prefs-font-default", family = default));
    let rest: Vec<SharedString> = match role {
        // Listed with the monospace ones, by `fonts::scan`.
        Role::Interface => {
            families.mono()?;
            families.names().to_vec()
        }
        Role::Code => families.mono()?.iter().cloned().collect(),
    };
    Some(
        std::iter::once(first)
            .chain(
                rest.into_iter()
                    .filter(|family| family.as_ref() != default)
                    .map(|family| SelectItem::new(family.clone(), family)),
            )
            .collect(),
    )
}

/// The family the menu shows as chosen: what the theme uses.
fn shown(role: Role, settings: &Settings, cx: &gpui_kit::App) -> SharedString {
    let chosen = fonts::chosen(settings, cx);
    match role {
        Role::Interface => chosen.sans,
        Role::Code => chosen.mono,
    }
}

fn menu(role: Role, window: &mut Window, cx: &mut Context<MainWindow>) -> Entity<SelectState> {
    let settings = SettingsStore::global(cx).read(cx).get().clone();
    let value = shown(role, &settings, cx);
    let options = items(role, cx);
    let state = cx.new(|cx| {
        let loading = options.is_none();
        let mut state = SelectState::new(options.unwrap_or_default(), window, cx);
        state.set_value(Some(value), window, cx);
        state.set_loading(loading, cx);
        state
    });
    cx.subscribe(&state, move |this, _, event: &SelectEvent, cx| {
        let SelectEvent::Change(Some(family)) = event else {
            return;
        };
        let wanted = (family.as_ref() != role.default_family()).then(|| family.to_string());
        this.settings.update(cx, |store, cx| {
            store.update(cx, |s| {
                let setting = role.setting(s);
                if *setting != wanted {
                    *setting = wanted;
                    super::style_library::fonts_changed(s);
                }
            })
        });
    })
    .detach();
    state
}

/// The two menus, for the main window to keep.
pub(super) fn menus(
    window: &mut Window,
    cx: &mut Context<MainWindow>,
) -> (Entity<SelectState>, Entity<SelectState>) {
    (
        menu(Role::Interface, window, cx),
        menu(Role::Code, window, cx),
    )
}

impl MainWindow {
    /// Lists the families again, as after a font file was added or the
    /// monospace families became known.
    fn refresh_font_menus(&mut self, cx: &mut Context<Self>) {
        for (role, state) in [
            (Role::Interface, self.ui_font.clone()),
            (Role::Code, self.code_font.clone()),
        ] {
            let Some(options) = items(role, cx) else {
                continue;
            };
            state.update(cx, |state, cx| {
                state.set_items(options, cx);
                state.set_loading(false, cx);
            });
        }
        cx.notify();
    }

    /// Shows the families the theme uses in the menus, as after a font
    /// file chose itself.
    fn show_fonts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let settings = self.settings.read(cx).get().clone();
        for (role, state) in [
            (Role::Interface, self.ui_font.clone()),
            (Role::Code, self.code_font.clone()),
        ] {
            let value = shown(role, &settings, cx);
            state.update(cx, |state, cx| state.set_value(Some(value), window, cx));
        }
    }

    /// Starts finding the monospace families, once, the first time the
    /// Fonts group shows.
    fn scan_fonts(&mut self, window: gpui_kit::AnyWindowHandle, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        fonts::scan(cx, move |cx| {
            window
                .update(cx, |_, window, cx| {
                    view.update(cx, |this, cx| {
                        this.refresh_font_menus(cx);
                        // The menus show their value's label once it is listed.
                        this.show_fonts(window, cx);
                    })
                })
                .ok();
        });
    }

    /// Asks for a font file, keeps and loads it, and makes it the interface
    /// font, or the code font when it is monospace.
    fn add_font_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let answer = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(t!("prefs-font-add-choose")),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = answer.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            this.update_in(cx, |this, window, cx| {
                match fonts::add_file(&path, cx) {
                    Ok((family, monospace)) => {
                        this.font_error = None;
                        let family = family.to_string();
                        this.settings.update(cx, |store, cx| {
                            store.update(cx, |s| {
                                if monospace {
                                    s.code_font = Some(family);
                                } else {
                                    s.ui_font = Some(family);
                                }
                                super::style_library::fonts_changed(s);
                            })
                        });
                    }
                    Err(error) => {
                        tracing::warn!(?error, path = %path.display(), "could not add a font");
                        this.font_error = Some(match error {
                            AddError::NotAFont => t!("prefs-font-add-not-a-font"),
                            AddError::Io(_) | AddError::Rejected(_) => {
                                t!("prefs-font-add-failed")
                            }
                        });
                    }
                }
                this.refresh_font_menus(cx);
                this.show_fonts(window, cx);
            })
            .ok();
        })
        .detach();
    }

    /// The font rows' controls: the interface and code font menus (220px),
    /// and what goes under them: missing fonts and Add font file….
    pub(super) fn font_controls(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (AnyElement, AnyElement, AnyElement) {
        let theme = UiTheme::read(cx).clone();
        let families = fonts::families(cx);
        if families.mono().is_none() && !families.scanning() {
            let view = cx.entity().downgrade();
            let handle = window.window_handle();
            cx.defer(move |cx| {
                view.update(cx, |this, cx| this.scan_fonts(handle, cx)).ok();
            });
        }
        let settings = self.settings.read(cx).get().clone();
        let missing = fonts::chosen(&settings, cx).missing;
        let note = |text: SharedString, color| {
            div()
                .type_style(style::META, &theme)
                .text_color(color)
                .child(text)
        };
        let menu = |state: &Entity<SelectState>, label: SharedString| {
            div()
                .w(gpui_kit::px(220.))
                .child(combobox(state).aria_label(label))
                .into_any_element()
        };
        let under = div()
            .flex()
            .flex_col()
            .items_start()
            .gap(theme.space(2.))
            .children(missing.into_iter().map(|family| {
                note(
                    t!("prefs-font-missing", family = family.to_string()),
                    theme.colors.muted_foreground,
                )
            }))
            .when(!cfg!(target_family = "wasm"), |group| {
                group
                    .child(
                        Button::new("preferences.font-add")
                            .variant(ButtonVariant::Outline)
                            .size(ButtonSize::Sm)
                            .label(t!("prefs-font-add"))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.add_font_file(window, cx)),
                            ),
                    )
                    .child(note(
                        t!("prefs-font-add-hint"),
                        theme.colors.muted_foreground,
                    ))
                    .when_some(self.font_error.clone(), |group, error| {
                        group.child(note(error, theme.colors.destructive))
                    })
            })
            .into_any_element();
        (
            menu(&self.ui_font, t!("prefs-font-interface")),
            menu(&self.code_font, t!("prefs-font-code")),
            under,
        )
    }
}
