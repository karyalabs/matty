//! One post row for every pane that shows posts (#684): the channel (and
//! its pop-out) and the thread pane. Each pane implements [`PostHost`] for
//! what a row's controls do there; [`post_message`] builds the row and
//! [`hover_actions`] its toolbar, so the panes can't drift apart again.

use std::rc::Rc;

use gpui_icons::LucideIcon;
use gpui_kit::{
    AnyElement, App, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    Pixels, Point, SharedString, Styled as _, WeakEntity, Window, div, prelude::FluentBuilder as _,
};

use crate::attachments::post_files;
use crate::channel::{
    Post, PostKind, PostStatus, copy_prefix, message_density, post_editor, post_markers, post_time,
    queued, queued_marker, quick_reaction_buttons, reaction_tooltip,
};
use crate::interactive::{ActionChoice, Menus};
use crate::style;
use crate::t;
use crate::ui::menu::MenuItem;
use crate::ui::{
    button::{Button, ButtonSize, ButtonVariant},
    composer::ComposerState,
    embed::MessageEmbed,
    message::{Message, MessageHighlight, MessageStatus, message_action, message_actions},
    theme::UiTheme,
};

/// What a row's controls do in the pane that shows it.
pub(crate) trait PostHost: Sized + 'static {
    /// The start of the row's element ids: `post` in the channel, `thread`
    /// in the thread pane.
    const PREFIX: &'static str;

    /// Sends a failed post again.
    fn retry_post(&mut self, id: SharedString, cx: &mut Context<Self>);
    /// Deletes a failed post, or dismisses an ephemeral one.
    fn discard_post(&mut self, id: SharedString, window: &mut Window, cx: &mut Context<Self>);
    /// Adds or removes the user's `emoji` on post `id`.
    fn react_to(&mut self, id: SharedString, emoji: SharedString, cx: &mut Context<Self>);
    /// Opens the emoji picker to react to post `id`.
    fn pick_reaction(&mut self, id: SharedString, cx: &mut Context<Self>);
    /// Whether the pane's post menu is open, which hides the toolbar's
    /// tooltips so they don't sit over it.
    fn post_menu_open(&self, cx: &App) -> bool;
    /// Opens the More menu for `post` at `at`.
    fn open_post_menu_at(
        &mut self,
        post: &Post,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    );
    /// Opens a link in a post's text.
    fn open_post_link(&mut self, url: SharedString, cx: &mut Context<Self>);
    /// Opens the right-click menu of post `id` for a link `url` in its
    /// text: [`link_items`] first, then the post's own menu (#757).
    fn open_link_menu(
        &mut self,
        id: SharedString,
        url: SharedString,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    );
    /// Opens or downloads one of a post's files.
    fn open_post_file(&mut self, file: SharedString, cx: &mut Context<Self>);
    /// Opens a post the client can't draw in the browser.
    fn open_in_browser(&mut self, post: SharedString, cx: &mut Context<Self>);
    /// Opens the image viewer at a post's `index`th file image.
    fn open_post_image(
        &mut self,
        post: &Post,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    );
    /// Opens the image viewer at a linked image of a post.
    fn open_linked_post_image(
        &mut self,
        post: &Post,
        url: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    );
    /// Acknowledges a priority post, or takes the acknowledgement back.
    fn acknowledge_post(&mut self, post: SharedString, acknowledged: bool, cx: &mut Context<Self>);
    /// Runs an attachment button or menu choice.
    fn run_post_action(&mut self, choice: ActionChoice, cx: &mut Context<Self>);
    /// Shows a quoted post in its channel.
    fn jump_to_post(&mut self, channel: SharedString, post: SharedString, cx: &mut Context<Self>);
    /// Collapses or expands a post's link previews.
    fn collapse_post_previews(
        &mut self,
        post: SharedString,
        collapsed: bool,
        cx: &mut Context<Self>,
    );
    /// Leaves the inline editor without saving.
    fn cancel_post_edit(&mut self, window: &mut Window, cx: &mut Context<Self>);
    /// Saves the inline editor's text.
    fn save_post_edit(&mut self, text: SharedString, window: &mut Window, cx: &mut Context<Self>);
}

/// What differs between rows, beyond the post itself.
pub(crate) struct RowParts<'a> {
    /// Offline: unsent posts wait to send.
    pub away: bool,
    pub highlight: MessageHighlight,
    /// Joins the previous post's header.
    pub continuation: bool,
    pub avatar: AnyElement,
    /// Replaces the author's plain name, as the channel's profile card does.
    pub author: Option<AnyElement>,
    /// The inline editor while the post is being edited.
    pub editor: Option<&'a (Entity<ComposerState>, Pixels)>,
    /// Shown above the text, as the channel's "replying to" line.
    pub above_body: Option<AnyElement>,
    /// The attachment menus and whether link previews are collapsed;
    /// without them the post's cards are left out (search listings).
    pub embeds: Option<(&'a Menus, bool)>,
    /// Whether reactions can be added here: the chip row's add button.
    pub reactable: bool,
}

fn element_id<H: PostHost>(id: &str, part: &str) -> gpui_kit::ElementId {
    gpui_kit::ElementId::Name(format!("{}.{id}.{part}", H::PREFIX).into())
}

/// Runs `f` on the host behind `view`, if it is still there.
fn with<H: PostHost>(view: &WeakEntity<H>, cx: &mut App, f: impl FnOnce(&mut H, &mut Context<H>)) {
    view.update(cx, f).ok();
}

/// The row for `post`: header, text or editor, cards, files, reactions and
/// the controls every pane shares. The caller adds its own padding, toolbar
/// and click handlers.
pub(crate) fn post_message<H: PostHost>(
    view: &WeakEntity<H>,
    post: &Post,
    parts: RowParts,
    cx: &App,
) -> Message {
    let theme = UiTheme::read(cx).clone();
    let id = post.id.clone();
    let away = parts.away;
    let time = post_time(post, away, cx);
    let mut row = Message::new(gpui_kit::ElementId::Name(
        format!("{}.{id}", H::PREFIX).into(),
    ))
    .author(post.author.clone())
    .when_some(parts.author, |row, name| row.author_element(name))
    .timestamp(time.clone())
    .gutter_time(time.clone())
    .copy_prefix(copy_prefix(post, cx))
    .when(
        matches!(post.status, PostStatus::Sent | PostStatus::Ephemeral) && !post.date.is_empty(),
        |row| {
            row.timestamp_tooltip(t!(
                "chan-sent-at",
                date = post.date.to_string(),
                time = time.to_string()
            ))
        },
    )
    // A queued post keeps its header so the clock shows (#195).
    .continuation(parts.continuation && !queued(post, away))
    .density(message_density(cx))
    .highlight(parts.highlight)
    .avatar(parts.avatar)
    .status(match post.status {
        PostStatus::Sent | PostStatus::Ephemeral => MessageStatus::Sent,
        PostStatus::Pending => MessageStatus::Pending,
        PostStatus::Failed => MessageStatus::Failed,
    })
    .on_retry({
        let (view, id) = (view.clone(), id.clone());
        move |_, _, cx| with(&view, cx, |this, cx| this.retry_post(id.clone(), cx))
    })
    .on_delete({
        let (view, id) = (view.clone(), id.clone());
        move |_, window, cx| {
            with(&view, cx, |this, cx| {
                this.discard_post(id.clone(), window, cx)
            })
        }
    })
    .reactions(
        post.reactions.iter().map(|reaction| {
            crate::emoji::reaction(&reaction.emoji, reaction.count, reaction.mine, cx)
        }),
    )
    .reaction_tooltip(reaction_tooltip(post.reactions.clone()))
    .on_reaction({
        let (view, id) = (view.clone(), id.clone());
        move |emoji, _, cx| {
            let emoji = emoji.clone();
            with(&view, cx, |this, cx| this.react_to(id.clone(), emoji, cx))
        }
    })
    .children(parts.above_body)
    .child(body(view, post, parts.editor, cx));
    if queued(post, away) {
        // Queued: a clock marks "Will send when back online" (#195).
        row = row.header_child(queued_marker(&theme));
    }
    if let Some(label) = crate::priority::label(post.priority.as_ref(), cx) {
        row = row.header_child(label);
    }
    for marker in post_markers(post, &theme) {
        row = row.header_child(marker);
    }
    let ack_view = view.clone();
    let ack_post = id.clone();
    row = row.children(crate::priority::ack_bar(
        &format!("{}.{id}", H::PREFIX),
        post.priority.as_ref(),
        post.status == PostStatus::Sent,
        Rc::new(move |acknowledged, _, cx| {
            with(&ack_view, cx, |this, cx| {
                this.acknowledge_post(ack_post.clone(), acknowledged, cx)
            })
        }),
        cx,
    ));
    if parts.reactable && !post.reactions.is_empty() {
        // The chip row gets an add button; posts without reactions use the
        // hover toolbar, so the transcript stays quiet.
        let (view, id) = (view.clone(), id.clone());
        row = row.on_add_reaction(move |_, _, cx| {
            with(&view, cx, |this, cx| this.pick_reaction(id.clone(), cx))
        });
    }
    if let Some((menus, collapsed)) = parts.embeds.filter(|_| !post.embeds.is_empty()) {
        row = row.child(embeds(view, post, menus, collapsed, cx));
    }
    if !post.files.is_empty() {
        let (image_view, image_post) = (view.clone(), post.clone());
        let file_view = view.clone();
        row = row.child(post_files(
            &format!("{}.{id}", H::PREFIX),
            &post.files,
            move |index, window, cx| {
                with(&image_view, cx, |this, cx| {
                    this.open_post_image(&image_post, index, window, cx)
                })
            },
            move |file, _, cx| with(&file_view, cx, |this, cx| this.open_post_file(file, cx)),
            cx,
        ));
    }
    row
}

/// The post's text, its inline editor, or a card for a type the client
/// can't draw.
fn body<H: PostHost>(
    view: &WeakEntity<H>,
    post: &Post,
    editor: Option<&(Entity<ComposerState>, Pixels)>,
    cx: &App,
) -> AnyElement {
    let id = &post.id;
    if let Some((composer, cap)) = editor {
        let (cancel, save) = (view.clone(), view.clone());
        return post_editor(
            &format!("{}.edit", H::PREFIX),
            composer,
            *cap,
            move |window, cx| with(&cancel, cx, |this, cx| this.cancel_post_edit(window, cx)),
            move |text, window, cx| {
                with(&save, cx, |this, cx| this.save_post_edit(text, window, cx))
            },
            cx,
        )
        .into_any_element();
    }
    if let PostKind::Unsupported(_) = &post.kind {
        return unsupported_card(view, id, cx).into_any_element();
    }
    let link_view = view.clone();
    let (menu_view, menu_id) = (view.clone(), id.clone());
    let link_menu: crate::links::LinkMenu = std::sync::Arc::new(move |url, at, window, cx| {
        let url = url.clone();
        with(&menu_view, cx, |this, cx| {
            this.open_link_menu(menu_id.clone(), url, at, window, cx)
        })
    });
    crate::links::message_body(
        element_id::<H>(id, "body"),
        &post.body,
        &post.image_sizes,
        move |url, _, cx| {
            let url = url.clone();
            with(&link_view, cx, |this, cx| this.open_post_link(url, cx))
        },
        Some(link_menu),
        cx,
    )
    .into_any_element()
}

/// A post type the client can't draw yet, with a way to see it on the web.
fn unsupported_card<H: PostHost>(
    view: &WeakEntity<H>,
    id: &SharedString,
    cx: &App,
) -> impl IntoElement + use<H> {
    let theme = UiTheme::read(cx).clone();
    let (view, open) = (view.clone(), id.clone());
    MessageEmbed::new(element_id::<H>(id, "unsupported"))
        .density(message_density(cx))
        .mt(theme.space(1.))
        .body(
            div()
                .flex()
                .flex_col()
                .items_start()
                .gap(theme.space(2.))
                .child(
                    div()
                        .text_color(theme.colors.muted_foreground)
                        .child(t!("chan-unsupported")),
                )
                .child(
                    Button::new(element_id::<H>(id, "open-in-browser"))
                        .variant(ButtonVariant::Outline)
                        .size(ButtonSize::Sm)
                        .label(t!("chan-open-in-browser"))
                        .on_click(move |_, _, cx| {
                            with(&view, cx, |this, cx| this.open_in_browser(open.clone(), cx))
                        }),
                ),
        )
}

/// The cards under a post: link previews, quoted posts and attachments,
/// with an attachment's buttons and menus.
fn embeds<H: PostHost>(
    view: &WeakEntity<H>,
    post: &Post,
    menus: &Menus,
    collapsed: bool,
    cx: &App,
) -> AnyElement {
    // The real id: a choice names its post by it.
    let id = post.id.clone();
    let (toggle_view, toggle_id) = (view.clone(), id.clone());
    let (image_view, image_post) = (view.clone(), post.clone());
    let jump_view = view.clone();
    let action_view = view.clone();
    crate::embeds::post_embeds(
        &id,
        &post.embeds,
        collapsed,
        Rc::new(move |collapsed, _, cx| {
            with(&toggle_view, cx, |this, cx| {
                this.collapse_post_previews(toggle_id.clone(), collapsed, cx)
            })
        }),
        Rc::new(move |url, window, cx| {
            with(&image_view, cx, |this, cx| {
                this.open_linked_post_image(&image_post, url, window, cx)
            })
        }),
        Rc::new(move |channel, post, _, cx| {
            let (channel, post) = (channel.clone(), post.clone());
            with(&jump_view, cx, |this, cx| {
                this.jump_to_post(channel, post, cx)
            })
        }),
        (
            menus,
            Rc::new(move |choice, _, cx| {
                with(&action_view, cx, |this, cx| {
                    this.run_post_action(choice, cx)
                })
            }),
        ),
        cx,
    )
    .into_any_element()
}

/// A post's hover toolbar: Dismiss for an ephemeral post; for a sent one
/// the quick reactions, the emoji picker and the pane's `extra` actions;
/// then More.
pub(crate) fn hover_actions<H: PostHost>(
    view: &WeakEntity<H>,
    post: &Post,
    extra: Vec<AnyElement>,
    cx: &App,
) -> AnyElement {
    let id = post.id.clone();
    let menu_open = view
        .upgrade()
        .is_some_and(|host| host.read(cx).post_menu_open(cx));
    let ephemeral = post.status == PostStatus::Ephemeral;
    let sent = post.status == PostStatus::Sent;
    let tipped = |part: &str, label: SharedString, icon: LucideIcon, cx: &App| {
        let button = message_action(element_id::<H>(&id, part), icon, label.clone(), cx);
        (element_id::<H>(&id, &format!("{part}.tip")), label, button)
    };
    let mut actions = message_actions(cx);
    if ephemeral {
        let (tip, label, button) = tipped("dismiss", t!("chan-dismiss"), LucideIcon::X, cx);
        let (view, dismiss) = (view.clone(), id.clone());
        actions = actions.child(style::tipped_when(
            !menu_open,
            tip,
            label,
            button.on_click(move |_, window, cx| {
                with(&view, cx, |this, cx| {
                    this.discard_post(dismiss.clone(), window, cx)
                })
            }),
        ));
    }
    if sent {
        let quick_view = view.clone();
        let quick_id = id.clone();
        let quick = quick_reaction_buttons(
            &id,
            move |emoji, cx| {
                with(&quick_view, cx, |this, cx| {
                    this.react_to(quick_id.clone(), emoji, cx)
                })
            },
            cx,
        );
        let (tip, label, button) = tipped(
            "react",
            t!("chan-add-reaction"),
            LucideIcon::FaceSlightlySmilingPlus,
            cx,
        );
        let (react_view, react_id) = (view.clone(), id.clone());
        let selector = format!("{}.{id}.react", H::PREFIX);
        actions = actions
            .children(quick)
            .child(
                style::tipped_when(
                    !menu_open,
                    tip,
                    label,
                    button.on_click(move |_, _, cx| {
                        with(&react_view, cx, |this, cx| {
                            this.pick_reaction(react_id.clone(), cx)
                        })
                    }),
                )
                .debug_selector(move || selector.clone()),
            )
            .children(extra);
    }
    let (tip, label, button) = tipped("more", t!("chan-more-actions"), LucideIcon::Ellipsis, cx);
    let (more_view, more_post) = (view.clone(), Rc::new(post.clone()));
    actions
        .child(style::tipped_when(
            !menu_open,
            tip,
            label,
            button.on_click(move |event: &gpui_kit::ClickEvent, window, cx| {
                let at = event.position();
                with(&more_view, cx, |this, cx| {
                    this.open_post_menu_at(&more_post, at, window, cx)
                })
            }),
        ))
        .into_any_element()
}

/// A toolbar button that `extra` can hold: `label` as tooltip, unless the
/// post menu is open.
pub(crate) fn extra_action<H: PostHost>(
    view: &WeakEntity<H>,
    post: &SharedString,
    part: &str,
    icon: LucideIcon,
    label: SharedString,
    on_click: impl Fn(&mut H, &mut Window, &mut Context<H>) + 'static,
    cx: &App,
) -> AnyElement {
    let menu_open = view
        .upgrade()
        .is_some_and(|host| host.read(cx).post_menu_open(cx));
    let view = view.clone();
    style::tipped_when(
        !menu_open,
        element_id::<H>(post, &format!("{part}.tip")),
        label.clone(),
        message_action(element_id::<H>(post, part), icon, label, cx)
            .on_click(move |_, window, cx| with(&view, cx, |this, cx| on_click(this, window, cx))),
    )
    .into_any_element()
}

/// A link's own menu items (#757): open it, copy it, then a separator
/// before the post's menu. `matty:` links (mentions, channels, hashtags)
/// only open.
pub(crate) fn link_items<H: PostHost>(view: &WeakEntity<H>, url: &SharedString) -> Vec<MenuItem> {
    let open = {
        let (view, url) = (view.clone(), url.clone());
        MenuItem::new(
            gpui_kit::ElementId::Name(format!("{}.link.open", H::PREFIX).into()),
            t!("chan-open-link"),
        )
        .on_click(move |_, _, cx| {
            let url = url.clone();
            with(&view, cx, |this, cx| this.open_post_link(url, cx))
        })
    };
    let mut items = vec![open];
    if !url.starts_with("matty:") {
        let url = url.clone();
        items.push(
            MenuItem::new(
                gpui_kit::ElementId::Name(format!("{}.link.copy", H::PREFIX).into()),
                t!("chan-copy-link-address"),
            )
            .on_click(move |_, _, cx| {
                cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(url.to_string()))
            }),
        );
    }
    items.push(MenuItem::separator());
    items
}
