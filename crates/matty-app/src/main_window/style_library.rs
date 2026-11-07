//! Preferences' Styles group (#707): the styles a person saved under a
//! name, and sharing a style as text or a file. Pasting or opening a
//! Mattermost theme turns it into the person's own style.

use std::path::PathBuf;

use gpui_icons::{LucideIcon, lucide};
use gpui_kit::{
    App, AppContext as _, ClipboardItem, Div, Entity, Global, InteractiveElement as _,
    ParentElement as _, SharedString, Stateful, Styled as _, Window, div,
    prelude::FluentBuilder as _,
};

use super::style_editor::{custom_or_seed, reset_accent_picker};
use crate::settings::{Settings, SettingsStore, ThemeSource};
use crate::style::share::{self, ImportError, SavedStyle};
use crate::style::{self, Preset, StyleSpec, TypeStyleExt as _, metrics, palette, scaled};
use crate::t;
use crate::ui::{
    button::{Button, ButtonSize, ButtonVariant},
    input::{Input, InputEvent, InputState},
    tag::{Tag, TagVariant},
    theme::UiTheme,
};

/// What the group keeps between renders: the name field, the line saying
/// what the last action did, and the name an imported style came with.
struct Library {
    name: Entity<InputState>,
    notice: Option<Notice>,
    imported_name: Option<String>,
}

impl Global for Library {}

#[derive(Clone)]
struct Notice {
    text: SharedString,
    error: bool,
}

/// Makes the name field, once; Preferences calls it before drawing
/// Appearance, while it has the window.
pub(super) fn prepare(store: &Entity<SettingsStore>, window: &mut Window, cx: &mut App) {
    if cx.has_global::<Library>() {
        return;
    }
    let name =
        cx.new(|cx| InputState::new(window, cx).placeholder(t!("prefs-styles-name-placeholder")));
    let store = store.clone();
    cx.subscribe(&name, move |_, event: &InputEvent, cx| {
        if matches!(event, InputEvent::PressEnter { .. }) {
            save_current(&store, cx);
        }
    })
    .detach();
    cx.set_global(Library {
        name,
        notice: None,
        imported_name: None,
    });
}

fn say(text: SharedString, error: bool, cx: &mut App) {
    if cx.has_global::<Library>() {
        let library = cx.global_mut::<Library>();
        library.notice = Some(Notice { text, error });
    }
    cx.refresh_windows();
}

/// The style on screen and the name it goes by: a preset's, a saved
/// style's, or the Mattermost theme's.
fn shown(settings: &Settings, cx: &App) -> (StyleSpec, Option<String>) {
    match settings.theme_source {
        ThemeSource::Matty => (
            Preset::Graphite.style(),
            Some(t!("prefs-colors-graphite").to_string()),
        ),
        ThemeSource::Porcelain => (
            Preset::Porcelain.style(),
            Some(t!("prefs-colors-porcelain").to_string()),
        ),
        ThemeSource::Custom => {
            let style = custom_or_seed(settings);
            (style, saved_name(&settings.saved_styles, &style))
        }
        ThemeSource::Mattermost => match crate::mattermost_theme::shown(settings, cx) {
            Some(theme) => (share::from_mattermost(&theme), Some(theme.name())),
            None => (Preset::Graphite.style(), None),
        },
    }
}

/// The name of the saved style that is `style`, if one is.
fn saved_name(saved: &[SavedStyle], style: &StyleSpec) -> Option<String> {
    saved
        .iter()
        .find(|s| s.style == *style)
        .map(|s| s.name.clone())
}

/// Makes `style` the person's own and shows it.
fn use_style(store: &Entity<SettingsStore>, style: StyleSpec, cx: &mut App) {
    store.update(cx, |store, cx| {
        store.update(cx, |s| {
            s.custom_style = Some(style);
            s.theme_source = ThemeSource::Custom;
        })
    });
    reset_accent_picker(cx);
}

/// Reads `text` as a style or Mattermost theme and uses it.
fn import_text(store: &Entity<SettingsStore>, text: Option<String>, cx: &mut App) {
    let result = text
        .as_deref()
        .map_or(Err(ImportError::NotAStyle), share::import);
    match result {
        Ok(imported) => {
            use_style(store, imported.style, cx);
            let name = imported.name.clone();
            if cx.has_global::<Library>() {
                let library = cx.global_mut::<Library>();
                library.imported_name = name.clone();
            }
            let text = match (imported.from_mattermost, name) {
                (true, Some(name)) => t!("prefs-styles-imported-mattermost-named", name = name),
                (true, None) => t!("prefs-styles-imported-mattermost"),
                (false, Some(name)) => t!("prefs-styles-imported-named", name = name),
                (false, None) => t!("prefs-styles-imported"),
            };
            say(text, false, cx);
        }
        Err(ImportError::NotAStyle) => say(t!("prefs-styles-not-a-style"), true, cx),
        Err(ImportError::TooNew) => say(t!("prefs-styles-too-new"), true, cx),
    }
}

/// Saves the person's own style under the name typed, else the name it was
/// imported with, else a new "My style". Returns whether it saved.
fn save_current(store: &Entity<SettingsStore>, cx: &mut App) -> bool {
    let Some((input, imported)) = cx
        .try_global::<Library>()
        .map(|l| (l.name.clone(), l.imported_name.clone()))
    else {
        return false;
    };
    let typed = share::clean_name(input.read(cx).value().as_ref());
    let settings = store.read(cx).get().clone();
    let style = custom_or_seed(&settings);
    let name = if !typed.is_empty() {
        typed
    } else {
        imported.unwrap_or_else(|| {
            share::unused_name(&settings.saved_styles, &t!("prefs-styles-default-name"))
        })
    };
    let mut saved = settings.saved_styles.clone();
    if !share::save(&mut saved, &name, style) {
        say(t!("prefs-styles-full", count = share::SAVED_MAX), true, cx);
        return false;
    }
    store.update(cx, |store, cx| {
        store.update(cx, |s| {
            s.saved_styles = saved;
            s.custom_style = Some(style);
            s.theme_source = ThemeSource::Custom;
        })
    });
    if cx.has_global::<Library>() {
        let library = cx.global_mut::<Library>();
        library.imported_name = None;
    }
    let name = share::clean_name(&name);
    say(t!("prefs-styles-saved", name = name), false, cx);
    true
}

/// Where a save or open dialog starts: the download folder, else home.
fn start_folder(settings: &Settings) -> PathBuf {
    settings
        .download_folder
        .clone()
        .or_else(dirs::home_dir)
        .unwrap_or_default()
}

fn copy_style(store: &Entity<SettingsStore>, cx: &mut App) {
    let (style, name) = shown(store.read(cx).get(), cx);
    cx.write_to_clipboard(ClipboardItem::new_string(share::export(
        &style,
        name.as_deref(),
    )));
    say(t!("prefs-styles-copied"), false, cx);
}

fn paste_style(store: &Entity<SettingsStore>, cx: &mut App) {
    let text = cx.read_from_clipboard().and_then(|item| item.text());
    import_text(store, text, cx);
}

fn save_file(store: &Entity<SettingsStore>, cx: &mut App) {
    let settings = store.read(cx).get().clone();
    let (style, name) = shown(&settings, cx);
    let text = share::export(&style, name.as_deref());
    let answer = cx.prompt_for_new_path(
        &start_folder(&settings),
        Some(&share::file_name(name.as_deref())),
    );
    cx.spawn(async move |cx| {
        let Ok(Ok(Some(path))) = answer.await else {
            return;
        };
        let written = std::fs::write(&path, text);
        cx.update(|cx| match written {
            Ok(()) => {
                let file = path
                    .file_name()
                    .map(|f| f.to_string_lossy().into_owned())
                    .unwrap_or_default();
                say(t!("prefs-styles-file-saved", file = file), false, cx)
            }
            Err(error) => {
                tracing::warn!(%error, "could not save the style");
                say(t!("prefs-styles-file-failed"), true, cx)
            }
        });
    })
    .detach();
}

fn open_file(store: &Entity<SettingsStore>, cx: &mut App) {
    let answer = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: Some(t!("prefs-styles-open")),
    });
    let store = store.clone();
    cx.spawn(async move |cx| {
        let Ok(Ok(Some(paths))) = answer.await else {
            return;
        };
        let Some(path) = paths.into_iter().next() else {
            return;
        };
        // A style is a few hundred bytes; anything far bigger is not one.
        let text = std::fs::metadata(&path)
            .ok()
            .filter(|m| m.len() <= 64 * 1024)
            .and_then(|_| std::fs::read_to_string(&path).ok());
        cx.update(|cx| import_text(&store, text, cx));
    })
    .detach();
}

/// A dot in a style's accent, as it shows in the mode on screen.
fn accent_dot(style: &StyleSpec, theme: &UiTheme) -> Div {
    let tokens = palette::tokens(&style.colors, theme.mode);
    let accent = tokens.get("primary").unwrap_or(theme.colors.primary);
    let page = tokens.get("background").unwrap_or(theme.colors.background);
    div()
        .size(theme.space(4.))
        .flex_none()
        .rounded_full()
        .border_1()
        .border_color(theme.colors.border)
        .bg(page)
        .flex()
        .items_center()
        .justify_center()
        .child(div().size(theme.space(2.5)).rounded_full().bg(accent))
}

fn small_button(id: impl Into<gpui_kit::ElementId>, label: SharedString) -> Button {
    Button::new(id)
        .variant(ButtonVariant::Outline)
        .size(ButtonSize::Sm)
        .aria_label(label.clone())
        .label(label)
}

/// One saved style: its accent, its name, "In use" when it is the style
/// on screen, and Use and Delete.
fn saved_row(
    index: usize,
    saved: &SavedStyle,
    in_use: bool,
    store: &Entity<SettingsStore>,
    theme: &UiTheme,
) -> Stateful<Div> {
    let use_store = store.clone();
    let delete_store = store.clone();
    let style = saved.style;
    let name = saved.name.clone();
    div()
        .id(gpui_kit::ElementId::Name(
            format!("preferences.styles.saved.{index}").into(),
        ))
        .flex()
        .items_center()
        .gap(theme.space(2.))
        .px(theme.space(3.))
        .py(theme.space(1.5))
        .child(accent_dot(&style, theme))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .type_style(style::UI, theme)
                .text_color(theme.colors.foreground)
                .child(saved.name.clone()),
        )
        .when(in_use, |row| {
            row.child(
                Tag::new(
                    gpui_kit::ElementId::Name(format!("preferences.styles.in-use.{index}").into()),
                    t!("prefs-styles-in-use"),
                )
                .variant(TagVariant::Secondary),
            )
        })
        .when(!in_use, |row| {
            row.child(
                Button::new(gpui_kit::ElementId::Name(
                    format!("preferences.styles.use.{index}").into(),
                ))
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Sm)
                .aria_label(t!("prefs-styles-use-label", name = saved.name.clone()))
                .label(t!("prefs-styles-use"))
                .on_click(move |_, _, cx| {
                    use_style(&use_store, style, cx);
                    say(t!("prefs-styles-using", name = name.clone()), false, cx);
                }),
            )
        })
        .child(
            Button::new(gpui_kit::ElementId::Name(
                format!("preferences.styles.delete.{index}").into(),
            ))
            .variant(ButtonVariant::Ghost)
            .size(ButtonSize::IconSm)
            .aria_label(t!("prefs-styles-delete-label", name = saved.name.clone()))
            .child(
                lucide(LucideIcon::Trash2)
                    .size(scaled(theme, metrics::ICON_SMALL))
                    .text_color(theme.colors.muted_foreground),
            )
            .on_click({
                let name = saved.name.clone();
                move |_, _, cx| {
                    delete_store.update(cx, |store, cx| {
                        store.update(cx, |s| s.saved_styles.retain(|s| s.name != name))
                    });
                    say(t!("prefs-styles-deleted", name = name.clone()), false, cx);
                }
            }),
        )
}

/// The Styles group: saving the person's own style under a name, the
/// saved styles, and sharing.
pub(super) fn library(store: &Entity<SettingsStore>, cx: &mut App) -> Div {
    let theme = UiTheme::read(cx).clone();
    let settings = store.read(cx).get().clone();
    let Some((input, notice)) = cx
        .try_global::<Library>()
        .map(|l| (l.name.clone(), l.notice.clone()))
    else {
        return div();
    };
    let custom = settings.theme_source == ThemeSource::Custom;
    let current = custom.then(|| custom_or_seed(&settings));
    let save_store = store.clone();
    let input_to_clear = input.clone();
    let save_row = div()
        .flex()
        .items_center()
        .gap(theme.space(2.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(Input::new(&input).aria_label(t!("prefs-styles-name"))),
        )
        .child(
            small_button("preferences.styles.save", t!("prefs-styles-save")).on_click(
                move |_, window, cx| {
                    if save_current(&save_store, cx) {
                        input_to_clear.update(cx, |input, cx| input.set_value("", window, cx));
                    }
                },
            ),
        );
    let saved = (!settings.saved_styles.is_empty()).then(|| {
        div()
            .flex()
            .flex_col()
            .rounded(theme.radius.lg)
            .border_1()
            .border_color(theme.colors.border)
            .children(settings.saved_styles.iter().enumerate().map(|(i, s)| {
                saved_row(i, s, current == Some(s.style), store, &theme).when(i > 0, |row| {
                    row.border_t_1().border_color(theme.colors.border)
                })
            }))
    });
    let (copy_store, file_store, paste_store, open_store, own_store) = (
        store.clone(),
        store.clone(),
        store.clone(),
        store.clone(),
        store.clone(),
    );
    let mattermost = (settings.theme_source == ThemeSource::Mattermost)
        .then(|| crate::mattermost_theme::shown(&settings, cx))
        .flatten();
    let actions = div()
        .flex()
        .flex_wrap()
        .gap(theme.space(2.))
        .child(
            small_button("preferences.styles.paste", t!("prefs-styles-paste"))
                .on_click(move |_, _, cx| paste_style(&paste_store, cx)),
        )
        .child(
            small_button("preferences.styles.open", t!("prefs-styles-open"))
                .on_click(move |_, _, cx| open_file(&open_store, cx)),
        )
        .child(
            small_button("preferences.styles.copy", t!("prefs-styles-copy"))
                .on_click(move |_, _, cx| copy_style(&copy_store, cx)),
        )
        .child(
            small_button("preferences.styles.export", t!("prefs-styles-export"))
                .on_click(move |_, _, cx| save_file(&file_store, cx)),
        )
        .when_some(mattermost, |row, theme_now| {
            row.child(
                small_button("preferences.styles.make-own", t!("prefs-styles-make-own")).on_click(
                    move |_, _, cx| {
                        let name = theme_now.name();
                        use_style(&own_store, share::from_mattermost(&theme_now), cx);
                        if cx.has_global::<Library>() {
                            let library = cx.global_mut::<Library>();
                            library.imported_name = Some(name.clone());
                        }
                        say(
                            t!("prefs-styles-imported-mattermost-named", name = name),
                            false,
                            cx,
                        );
                    },
                ),
            )
        });
    let muted = |text: SharedString, color| {
        div()
            .type_style(style::META, &theme)
            .text_color(color)
            .child(text)
    };
    div()
        .flex()
        .flex_col()
        .gap(theme.space(3.))
        .when(custom, |group| group.child(save_row))
        .when_some(saved, |group, saved| group.child(saved))
        .child(actions)
        .child(muted(
            t!("prefs-styles-hint"),
            theme.colors.muted_foreground,
        ))
        .when_some(notice, |group, notice| {
            group.child(
                div()
                    .id("preferences.styles.notice")
                    .debug_selector(|| "preferences.styles.notice".into())
                    .child(muted(
                        notice.text,
                        if notice.error {
                            theme.colors.destructive
                        } else {
                            theme.colors.foreground
                        },
                    )),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(cx: &mut gpui_kit::TestAppContext) -> Entity<SettingsStore> {
        cx.update(|cx| {
            crate::ui::theme::init(cx);
            SettingsStore::init(None, cx)
        })
    }

    #[gpui_kit::test]
    fn a_pasted_mattermost_theme_becomes_your_own(cx: &mut gpui_kit::TestAppContext) {
        let store = setup(cx);
        cx.update(|cx| {
            let onyx = crate::mattermost_theme::BUILT_IN[4].1;
            cx.write_to_clipboard(ClipboardItem::new_string(onyx.into()));
            paste_style(&store, cx);
            let settings = store.read(cx).get();
            assert_eq!(settings.theme_source, ThemeSource::Custom);
            let theme = crate::mattermost_theme::Theme::built_in("Onyx").unwrap();
            assert_eq!(settings.custom_style, Some(share::from_mattermost(&theme)));
        });
    }

    #[gpui_kit::test]
    fn copying_and_pasting_a_style_brings_it_back(cx: &mut gpui_kit::TestAppContext) {
        let store = setup(cx);
        cx.update(|cx| {
            let style = Preset::Porcelain.style().with_accent(0.6, 0.15, 30.);
            use_style(&store, style, cx);
            copy_style(&store, cx);
            use_style(&store, Preset::Graphite.style(), cx);
            paste_style(&store, cx);
            let back = store.read(cx).get().custom_style.unwrap();
            assert!((back.colors.accent_hue - 30.).abs() < 0.01);
            assert!((back.colors.accent_light - 0.6).abs() < 0.01);
            // Text that is not a style changes nothing.
            cx.write_to_clipboard(ClipboardItem::new_string("hello".into()));
            paste_style(&store, cx);
            assert_eq!(store.read(cx).get().custom_style, Some(back));
        });
    }

    #[gpui_kit::test]
    fn saved_styles_take_the_imported_name(cx: &mut gpui_kit::TestAppContext) {
        let store = setup(cx);
        let window = cx.add_empty_window();
        window.update(|window, cx| prepare(&store, window, cx));
        cx.update(|cx| {
            let denim = crate::mattermost_theme::BUILT_IN[0].1;
            import_text(&store, Some(denim.into()), cx);
            assert!(save_current(&store, cx));
            // Nothing typed and nothing imported: a fresh "My style".
            assert!(save_current(&store, cx));
            let names: Vec<_> = store
                .read(cx)
                .get()
                .saved_styles
                .iter()
                .map(|s| s.name.clone())
                .collect();
            assert_eq!(names, ["Denim", "My style"]);
        });
    }
}
