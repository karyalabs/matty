//! The updater on the web: there is nothing to install, because the page
//! always loads the latest deployed build. Same API as `updater.rs`, so the
//! windows that start it compile unchanged.

use std::time::Duration;

use gpui_kit::{App, AppContext as _, Context, Entity, Window};

use crate::t;
use crate::ui::{
    composer::ComposerState,
    toast::{Notice, ToastState},
};

/// Stands in for the desktop's update prompt and never shows.
pub(crate) struct UpdatePrompt {
    toasts: Entity<ToastState>,
}

impl UpdatePrompt {
    pub(crate) fn start(
        toasts: Entity<ToastState>,
        _window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|_| Self { toasts })
    }

    /// "Check for updates": the page is always the latest build.
    pub(crate) fn check_now(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let notice = Notice::new(t!("update-web"))
            .description(t!("update-web-detail"))
            .timeout(Duration::from_secs(5));
        self.toasts
            .update(cx, |toasts, cx| toasts.push("update.check", notice, cx));
    }
}

/// Nothing waits on composers on the web.
pub(crate) fn watch_composer(_composer: &Entity<ComposerState>, _cx: &mut App) {}
