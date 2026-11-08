//! Settings' Style gallery (#707, #733): the Mattermost theme, Matty's
//! presets and the styles a person saved, as cards to pick from, and the
//! rules for changing one. Editing a preset or the Mattermost theme saves
//! a copy and edits that; editing a saved style edits it in place. Styles
//! share as text or a file, and pasting or opening one saves it.

use std::path::PathBuf;

use gpui_icons::{LucideIcon, lucide};
use gpui_kit::{
    AnyElement, App, ClipboardItem, Div, Entity, Global, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, WeakEntity,
    div, px,
};

use super::style_editor::{StyleCard, custom_or_seed, reset_accent_picker};
use crate::settings::{Settings, SettingsStore, ThemeSource};
use crate::style::share::{self, Fonts, ImportError};
use crate::style::{self, Preset, StyleSpec, TypeStyleExt as _};
use crate::t;
use crate::ui::{
    menu::{Menu, MenuItem, MenuState},
    theme::UiTheme,
    toast::{Notice, ToastState},
};

/// Where the style actions say what they did: the Settings window's
/// toasts, while it is open.
pub(super) struct StyleToasts(pub(super) WeakEntity<ToastState>);

impl Global for StyleToasts {}

/// Shows `text` as a toast in Settings.
pub(super) fn say(text: SharedString, error: bool, cx: &mut App) {
    let Some(toasts) = cx
        .try_global::<StyleToasts>()
        .and_then(|toasts| toasts.0.upgrade())
    else {
        return;
    };
    toasts.update(cx, |toasts, cx| {
        let id = if error {
            "settings.style.error"
        } else {
            "settings.style"
        };
        // A new toast replaces the last one rather than stacking.
        toasts.dismiss(&SharedString::from(id), cx);
        toasts.push(id, Notice::new(text), cx);
    });
}

/// Which card is selected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Choice {
    Mattermost,
    Graphite,
    Porcelain,
    /// A saved style, by name.
    Saved(String),
    /// The person's own style before saved styles had names.
    Unsaved,
}

/// The card that is selected now.
pub(super) fn chosen(settings: &Settings) -> Choice {
    match settings.theme_source {
        ThemeSource::Mattermost => Choice::Mattermost,
        ThemeSource::Matty => Choice::Graphite,
        ThemeSource::Porcelain => Choice::Porcelain,
        ThemeSource::Custom => selected(settings)
            .map(|saved| Choice::Saved(saved.name.clone()))
            .unwrap_or(Choice::Unsaved),
    }
}

/// The saved style in use: the one named, else the one that matches.
fn selected(settings: &Settings) -> Option<&share::SavedStyle> {
    if settings.theme_source != ThemeSource::Custom {
        return None;
    }
    let fonts = Fonts::of(settings);
    let style = custom_or_seed(settings);
    settings
        .selected_style
        .as_ref()
        .and_then(|name| settings.saved_styles.iter().find(|s| &s.name == name))
        .or_else(|| {
            settings
                .saved_styles
                .iter()
                .find(|s| s.style == style && s.fonts == fonts)
        })
}

/// The style on screen and the name it goes by: a preset's, a saved
/// style's, or the Mattermost theme's.
pub(super) fn shown(settings: &Settings, cx: &App) -> (StyleSpec, String) {
    match settings.theme_source {
        ThemeSource::Matty => (
            Preset::Graphite.style(),
            t!("prefs-colors-graphite").to_string(),
        ),
        ThemeSource::Porcelain => (
            Preset::Porcelain.style(),
            t!("prefs-colors-porcelain").to_string(),
        ),
        ThemeSource::Custom => {
            let name = selected(settings)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| t!("prefs-styles-default-name").to_string());
            (custom_or_seed(settings), name)
        }
        ThemeSource::Mattermost => {
            let theme = crate::mattermost_theme::shown(settings, cx)
                .unwrap_or_else(crate::mattermost_theme::Theme::denim);
            (share::from_mattermost(&theme), theme.name())
        }
    }
}

/// Makes the saved style `name` the one on screen, with its fonts.
fn use_saved(store: &Entity<SettingsStore>, name: &str, cx: &mut App) {
    store.update(cx, |store, cx| {
        store.update(cx, |s| {
            let Some(saved) = s
                .saved_styles
                .iter()
                .find(|saved| saved.name == name)
                .cloned()
            else {
                return;
            };
            s.custom_style = Some(saved.style);
            s.ui_font = saved.fonts.ui_font;
            s.code_font = saved.fonts.code_font;
            s.selected_style = Some(saved.name);
            s.theme_source = ThemeSource::Custom;
        })
    });
    reset_accent_picker(cx);
}

/// Saves `style` with `fonts` as a new style named after `base` and shows
/// it. Says so, or that the library is full; returns the name it saved.
fn save_new(
    store: &Entity<SettingsStore>,
    base: &str,
    style: StyleSpec,
    fonts: Fonts,
    cx: &mut App,
) -> Option<String> {
    let settings = store.read(cx).get().clone();
    let name = share::unused_name(&settings.saved_styles, &share::clean_name(base));
    let mut saved = settings.saved_styles.clone();
    if !share::save(&mut saved, &name, style, fonts.clone()) {
        say(t!("prefs-styles-full", count = share::SAVED_MAX), true, cx);
        return None;
    }
    store.update(cx, |store, cx| {
        store.update(cx, |s| {
            s.saved_styles = saved;
            s.custom_style = Some(style);
            s.ui_font = fonts.ui_font;
            s.code_font = fonts.code_font;
            s.selected_style = Some(name.clone());
            s.theme_source = ThemeSource::Custom;
        })
    });
    reset_accent_picker(cx);
    say(t!("prefs-styles-saved", name = name.clone()), false, cx);
    Some(name)
}

/// "New style from <current>": a saved copy of the style on screen.
pub(super) fn new_style(store: &Entity<SettingsStore>, cx: &mut App) {
    let settings = store.read(cx).get().clone();
    let (style, name) = shown(&settings, cx);
    let base = t!("prefs-styles-copy-name", name = name).to_string();
    save_new(store, &base, style, Fonts::of(&settings), cx);
}

/// Changes the style on screen. A saved style changes in place; a preset,
/// the Mattermost theme or an unnamed style is first saved as
/// "<name> copy", which then changes (spec §1.3, fork semantics).
pub(super) fn edit_style(
    store: &Entity<SettingsStore>,
    change: impl FnOnce(StyleSpec) -> StyleSpec,
    cx: &mut App,
) {
    let settings = store.read(cx).get().clone();
    if let Some(saved) = selected(&settings) {
        let name = saved.name.clone();
        store.update(cx, |store, cx| {
            store.update(cx, |s| {
                let style = change(custom_or_seed(s));
                s.custom_style = Some(style);
                s.selected_style = Some(name.clone());
                if let Some(saved) = s.saved_styles.iter_mut().find(|saved| saved.name == name) {
                    saved.style = style;
                }
            })
        });
        return;
    }
    let (base, name) = shown(&settings, cx);
    let copy = t!("prefs-styles-copy-name", name = name).to_string();
    save_new(store, &copy, change(base), Fonts::of(&settings), cx);
}

/// Keeps the saved style in use in step with the fonts, which are part of
/// it (#724); Settings calls it after a font changes.
pub(super) fn fonts_changed(settings: &mut Settings) {
    if settings.theme_source != ThemeSource::Custom {
        return;
    }
    let fonts = Fonts::of(settings);
    if let Some(name) = settings.selected_style.clone()
        && let Some(saved) = settings.saved_styles.iter_mut().find(|s| s.name == name)
    {
        saved.fonts = fonts;
    }
}

/// Renames the saved style `from`; a clash or an empty name keeps it.
pub(super) fn rename(store: &Entity<SettingsStore>, from: &str, to: &str, cx: &mut App) {
    let to = share::clean_name(to);
    let settings = store.read(cx).get();
    let clash = settings
        .saved_styles
        .iter()
        .any(|s| s.name != from && s.name.to_lowercase() == to.to_lowercase());
    if to.is_empty() || clash || to == from {
        return;
    }
    let from = from.to_owned();
    store.update(cx, |store, cx| {
        store.update(cx, |s| {
            if let Some(saved) = s.saved_styles.iter_mut().find(|saved| saved.name == from) {
                saved.name = to.clone();
            }
            if s.selected_style.as_deref() == Some(from.as_str()) {
                s.selected_style = Some(to.clone());
            }
        })
    });
    say(t!("prefs-styles-renamed", name = to), false, cx);
}

/// Deletes the saved style `name`. Deleting the one in use shows Graphite.
pub(super) fn delete(store: &Entity<SettingsStore>, name: &str, cx: &mut App) {
    let name = name.to_owned();
    store.update(cx, |store, cx| {
        store.update(cx, |s| {
            s.saved_styles.retain(|saved| saved.name != name);
            if s.selected_style.as_deref() == Some(name.as_str()) {
                s.selected_style = None;
                s.theme_source = ThemeSource::Matty;
            }
        })
    });
    say(t!("prefs-styles-deleted", name = name), false, cx);
}

/// Reads `text` as a style or Mattermost theme and saves it.
fn import_text(store: &Entity<SettingsStore>, text: Option<String>, cx: &mut App) {
    let result = text
        .as_deref()
        .map_or(Err(ImportError::NotAStyle), share::import);
    match result {
        Ok(imported) => {
            let settings = store.read(cx).get().clone();
            let fonts = imported
                .fonts
                .clone()
                .unwrap_or_else(|| Fonts::of(&settings));
            let base = imported
                .name
                .clone()
                .unwrap_or_else(|| t!("prefs-styles-imported-name").to_string());
            save_new(store, &base, imported.style, fonts, cx);
        }
        Err(ImportError::NotAStyle) => say(t!("prefs-styles-not-a-style"), true, cx),
        Err(ImportError::TooNew) => say(t!("prefs-styles-too-new"), true, cx),
    }
}

/// Where a save or open dialog starts: the download folder, else home.
fn start_folder(settings: &Settings) -> PathBuf {
    settings
        .download_folder
        .clone()
        .or_else(dirs::home_dir)
        .unwrap_or_default()
}

pub(super) fn copy_style(store: &Entity<SettingsStore>, cx: &mut App) {
    let settings = store.read(cx).get().clone();
    let (style, name) = shown(&settings, cx);
    let text = share::export(&style, Some(&name), &Fonts::of(&settings));
    cx.write_to_clipboard(ClipboardItem::new_string(text));
    say(t!("prefs-styles-copied"), false, cx);
}

pub(super) fn paste_style(store: &Entity<SettingsStore>, cx: &mut App) {
    let text = cx.read_from_clipboard().and_then(|item| item.text());
    import_text(store, text, cx);
}

pub(super) fn save_file(store: &Entity<SettingsStore>, cx: &mut App) {
    let settings = store.read(cx).get().clone();
    let (style, name) = shown(&settings, cx);
    let text = share::export(&style, Some(&name), &Fonts::of(&settings));
    let answer = cx.prompt_for_new_path(
        &start_folder(&settings),
        Some(&share::file_name(Some(&name))),
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

pub(super) fn open_file(store: &Entity<SettingsStore>, cx: &mut App) {
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

/// One card in the gallery: what it shows and what picking it does.
pub(super) struct Card {
    pub(super) id: SharedString,
    pub(super) choice: Choice,
    pub(super) name: SharedString,
    pub(super) subtitle: SharedString,
    pub(super) preview: StyleCard,
    /// A saved style's ⋯ menu.
    pub(super) menu: Option<Entity<MenuState>>,
}

/// The cards in gallery order: the Mattermost theme, Graphite, Porcelain,
/// then the saved styles (spec §1.3).
pub(super) fn cards(settings: &Settings, cx: &App) -> Vec<Card> {
    let mode = UiTheme::read(cx).mode;
    let mattermost = crate::mattermost_theme::shown(settings, cx)
        .unwrap_or_else(crate::mattermost_theme::Theme::denim);
    let mut cards = vec![
        Card {
            id: "preferences.style.mattermost".into(),
            choice: Choice::Mattermost,
            name: mattermost.name().into(),
            subtitle: t!("prefs-style-mattermost"),
            preview: StyleCard::mattermost(&mattermost),
            menu: None,
        },
        Card {
            id: "preferences.style.graphite".into(),
            choice: Choice::Graphite,
            name: t!("prefs-colors-graphite"),
            subtitle: t!("prefs-style-preset"),
            preview: StyleCard::of(&Preset::Graphite.style(), mode),
            menu: None,
        },
        Card {
            id: "preferences.style.porcelain".into(),
            choice: Choice::Porcelain,
            name: t!("prefs-colors-porcelain"),
            subtitle: t!("prefs-style-preset"),
            preview: StyleCard::of(&Preset::Porcelain.style(), mode),
            menu: None,
        },
    ];
    if chosen(settings) == Choice::Unsaved {
        cards.push(Card {
            id: "preferences.style.unsaved".into(),
            choice: Choice::Unsaved,
            name: t!("prefs-styles-default-name"),
            subtitle: t!("prefs-style-unsaved"),
            preview: StyleCard::of(&custom_or_seed(settings), mode),
            menu: None,
        });
    }
    cards.extend(
        settings
            .saved_styles
            .iter()
            .enumerate()
            .map(|(index, saved)| Card {
                id: format!("preferences.style.saved.{index}").into(),
                choice: Choice::Saved(saved.name.clone()),
                name: saved.name.clone().into(),
                subtitle: t!("prefs-style-saved"),
                preview: StyleCard::of(&saved.style, mode),
                menu: None,
            }),
    );
    cards
}

/// Picks `choice`'s style.
pub(super) fn pick(store: &Entity<SettingsStore>, choice: &Choice, cx: &mut App) {
    let source = match choice {
        Choice::Mattermost => ThemeSource::Mattermost,
        Choice::Graphite => ThemeSource::Matty,
        Choice::Porcelain => ThemeSource::Porcelain,
        Choice::Unsaved => ThemeSource::Custom,
        Choice::Saved(name) => return use_saved(store, name, cx),
    };
    store.update(cx, |store, cx| {
        store.update(cx, |s| {
            s.theme_source = source;
            s.selected_style = None;
        })
    });
    reset_accent_picker(cx);
}

/// The gallery: `cards` in rows of `columns`, the selected one ringed and
/// checked, a saved style's ⋯ menu at its corner.
pub(super) fn gallery(
    store: &Entity<SettingsStore>,
    cards: Vec<Card>,
    columns: usize,
    cx: &App,
) -> Div {
    let theme = UiTheme::read(cx).clone();
    let selected = chosen(store.read(cx).get());
    let gap = theme.space(3.);
    let mut rows = Vec::new();
    let mut cards = cards.into_iter().peekable();
    while cards.peek().is_some() {
        let mut row = div().flex().gap(gap).w_full();
        for _ in 0..columns {
            match cards.next() {
                Some(card) => {
                    let on = card.choice == selected;
                    row = row.child(card_element(store, card, on, &theme));
                }
                None => row = row.child(div().flex_1().min_w_0()),
            }
        }
        rows.push(row);
    }
    div().flex().flex_col().gap(gap).w_full().children(rows)
}

/// One card: the tiny window, the name and subtitle, the selected ring and
/// check, and a saved style's ⋯ menu.
fn card_element(
    store: &Entity<SettingsStore>,
    card: Card,
    selected: bool,
    theme: &UiTheme,
) -> AnyElement {
    let store = store.clone();
    let choice = card.choice.clone();
    let check = selected.then(|| {
        div()
            .absolute()
            .top(px(-6.))
            .right(px(-6.))
            .size(px(18.))
            .rounded_full()
            .bg(theme.colors.primary)
            .flex()
            .items_center()
            .justify_center()
            .child(
                lucide(LucideIcon::Check)
                    .size(px(12.))
                    .text_color(theme.colors.primary_foreground),
            )
    });
    let label: SharedString = if selected {
        t!("prefs-style-card-selected", name = card.name.to_string())
    } else {
        card.name.clone()
    };
    let menu = card.menu.clone().map(|menu| {
        let theme = theme.clone();
        div().flex_none().child(
            Menu::new(
                &menu,
                t!("prefs-style-options", name = card.name.to_string()),
            )
            .trigger(
                lucide(LucideIcon::Ellipsis)
                    .size(px(14.))
                    .text_color(theme.colors.muted_foreground),
            )
            .trigger_style_with(move |button| {
                crate::ui::button::style_button(
                    button,
                    false,
                    crate::ui::button::ButtonVariant::Ghost,
                    crate::ui::button::ButtonSize::IconSm,
                    &theme,
                )
            }),
        )
    });
    div()
        .flex_1()
        .min_w_0()
        .relative()
        .child(
            div()
                .id(gpui_kit::ElementId::Name(card.id.clone()))
                .debug_selector({
                    let id = card.id.clone();
                    move || id.to_string()
                })
                .role(gpui_kit::Role::RadioButton)
                .aria_label(label)
                .tab_stop(true)
                .cursor_pointer()
                .w_full()
                .flex()
                .flex_col()
                .gap(theme.space(2.))
                .p(theme.space(1.5))
                .rounded(theme.radius.lg)
                .border_2()
                .border_color(if selected {
                    theme.colors.primary
                } else {
                    gpui_kit::transparent_black().into()
                })
                .hover(|style| style.bg(theme.colors.accent))
                .on_click(move |_, _, cx| pick(&store, &choice, cx))
                .child(card.preview.render(theme))
                .child(
                    div()
                        .flex()
                        .items_start()
                        .gap(theme.space(1.))
                        .px(theme.space(0.5))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .type_style(style::UI_STRONG, theme)
                                        .text_color(theme.colors.foreground)
                                        .truncate()
                                        .child(card.name.clone()),
                                )
                                .child(
                                    div()
                                        .type_style(style::META, theme)
                                        .text_color(theme.colors.muted_foreground)
                                        .truncate()
                                        .child(card.subtitle.clone()),
                                ),
                        ),
                ),
        )
        .children(menu.map(|menu| {
            div()
                .absolute()
                .bottom(theme.space(1.))
                .right(theme.space(1.))
                .child(menu)
        }))
        .children(check)
        .into_any_element()
}

/// The Share ▾ menu's items: Copy style and Save to file….
pub(super) fn share_items(store: &Entity<SettingsStore>) -> Vec<MenuItem> {
    let (copy, file) = (store.clone(), store.clone());
    vec![
        MenuItem::new("preferences.style.share.copy", t!("prefs-styles-copy"))
            .icon(LucideIcon::Copy)
            .on_click(move |_, _, cx| copy_style(&copy, cx)),
        MenuItem::new("preferences.style.share.file", t!("prefs-styles-export"))
            .icon(LucideIcon::Download)
            .on_click(move |_, _, cx| save_file(&file, cx)),
    ]
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
    fn a_pasted_mattermost_theme_is_saved_as_a_style(cx: &mut gpui_kit::TestAppContext) {
        let store = setup(cx);
        cx.update(|cx| {
            let onyx = crate::mattermost_theme::BUILT_IN[4].1;
            cx.write_to_clipboard(ClipboardItem::new_string(onyx.into()));
            paste_style(&store, cx);
            let settings = store.read(cx).get();
            assert_eq!(settings.theme_source, ThemeSource::Custom);
            let theme = crate::mattermost_theme::Theme::built_in("Onyx").unwrap();
            assert_eq!(settings.custom_style, Some(share::from_mattermost(&theme)));
            assert_eq!(settings.selected_style.as_deref(), Some("Onyx"));
            assert_eq!(chosen(settings), Choice::Saved("Onyx".into()));
        });
    }

    #[gpui_kit::test]
    fn copying_and_pasting_a_style_brings_it_back(cx: &mut gpui_kit::TestAppContext) {
        let store = setup(cx);
        cx.update(|cx| {
            pick(&store, &Choice::Porcelain, cx);
            edit_style(&store, |style| style.with_accent(0.6, 0.15, 30.), cx);
            copy_style(&store, cx);
            pick(&store, &Choice::Graphite, cx);
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
    fn editing_a_preset_saves_a_copy_and_then_edits_it(cx: &mut gpui_kit::TestAppContext) {
        let store = setup(cx);
        cx.update(|cx| {
            pick(&store, &Choice::Graphite, cx);
            edit_style(&store, |style| style.with_accent(0.5, 0.1, 100.), cx);
            let names = |cx: &App| -> Vec<String> {
                store
                    .read(cx)
                    .get()
                    .saved_styles
                    .iter()
                    .map(|s| s.name.clone())
                    .collect()
            };
            assert_eq!(names(cx), ["Graphite copy"]);
            assert_eq!(
                chosen(store.read(cx).get()),
                Choice::Saved("Graphite copy".into())
            );
            // The copy changes in place now.
            edit_style(&store, |style| style.with_accent(0.5, 0.1, 200.), cx);
            assert_eq!(names(cx), ["Graphite copy"]);
            let saved = store.read(cx).get().saved_styles[0].style;
            assert!((saved.colors.accent_hue - 200.).abs() < 0.01);
            // Graphite itself did not change.
            pick(&store, &Choice::Graphite, cx);
            edit_style(&store, |style| style, cx);
            assert_eq!(names(cx), ["Graphite copy", "Graphite copy 2"]);
        });
    }

    #[gpui_kit::test]
    fn renaming_and_deleting_follow_the_style_in_use(cx: &mut gpui_kit::TestAppContext) {
        let store = setup(cx);
        cx.update(|cx| {
            new_style(&store, cx);
            let name = store.read(cx).get().selected_style.clone().unwrap();
            rename(&store, &name, "Dusk", cx);
            assert_eq!(store.read(cx).get().selected_style.as_deref(), Some("Dusk"));
            delete(&store, "Dusk", cx);
            let settings = store.read(cx).get();
            assert!(settings.saved_styles.is_empty());
            assert_eq!(chosen(settings), Choice::Graphite);
        });
    }

    #[gpui_kit::test]
    fn a_pasted_style_brings_its_fonts(cx: &mut gpui_kit::TestAppContext) {
        let store = setup(cx);
        cx.update(|cx| {
            store.update(cx, |store, cx| {
                store.update(cx, |s| s.ui_font = Some("Inter".into()))
            });
            copy_style(&store, cx);
            store.update(cx, |store, cx| store.update(cx, |s| s.ui_font = None));
            paste_style(&store, cx);
            assert_eq!(store.read(cx).get().ui_font.as_deref(), Some("Inter"));
            // A Mattermost theme leaves the fonts alone.
            let denim = crate::mattermost_theme::BUILT_IN[0].1;
            import_text(&store, Some(denim.into()), cx);
            assert_eq!(store.read(cx).get().ui_font.as_deref(), Some("Inter"));
        });
    }
}
