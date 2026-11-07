//! The in-app updater's window side (PLAN §10, #180): checks the feed at
//! launch and every six hours, downloads and stages the update in the
//! background, then offers "Restart to update" as a toast.
//!
//! The prompt never appears while a composer holds unsent text and has
//! focus: it waits until no composer has, for [`IDLE`] in a row
//! ([`Deferral`]). "Later", or closing it, puts it off until the next
//! check, and a check never shows it twice. The feed,
//! signature and download checks live in `matty-update`.
//!
//! "Check for updates" (the app menu, Preferences > Advanced) checks at
//! once and answers in a toast: up to date, downloading, failed, or the
//! prompt itself, which then shows without waiting for the composers.

use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::{
    App, AppContext as _, Context, Entity, Focusable as _, Global, Task, WeakEntity, Window,
};
use matty_update::{UpdateError, Updater, Version};
use web_time::Instant;

use crate::settings::{SettingsChanged, SettingsStore};
use crate::t;
use crate::ui::{
    composer::ComposerState,
    toast::{Notice, NoticeDismissal, ToastState},
};

/// How long the composers must stay free before the prompt shows.
pub(crate) const IDLE: Duration = Duration::from_secs(3);
/// How often a waiting prompt looks again.
const POLL: Duration = Duration::from_secs(1);
/// The toast's id.
const TOAST: &str = "update.ready";
/// The id of the toast that answers "Check for updates".
const CHECK_TOAST: &str = "update.check";
/// Where builds that do not update themselves get new versions.
const RELEASES_URL: &str = "https://github.com/karyalabs/matty/releases";

/// Decides when a ready update may be offered: only once no composer has
/// held unsent text with focus for [`IDLE`].
#[derive(Debug, Default)]
pub(crate) struct Deferral {
    idle_since: Option<Instant>,
}

impl Deferral {
    /// Records whether a composer is `busy` at `now` and answers whether the
    /// prompt may show.
    pub(crate) fn may_prompt(&mut self, busy: bool, now: Instant) -> bool {
        if busy {
            self.idle_since = None;
            return false;
        }
        let since = *self.idle_since.get_or_insert(now);
        now.duration_since(since) >= IDLE
    }
}

/// The composers that can hold unsent text, so the prompt can wait for them.
#[derive(Default)]
struct Composers(Vec<WeakEntity<ComposerState>>);
impl Global for Composers {}

/// Counts `composer` among those the prompt waits for. Dropped composers
/// fall out on their own.
pub(crate) fn watch_composer(composer: &Entity<ComposerState>, cx: &mut App) {
    let composers = cx.default_global::<Composers>();
    composers.0.retain(|composer| composer.upgrade().is_some());
    composers.0.push(composer.downgrade());
}

/// Whether a composer in `window` has focus and unsent text.
pub(crate) fn composer_busy(window: &Window, cx: &App) -> bool {
    cx.try_global::<Composers>().is_some_and(|composers| {
        composers
            .0
            .iter()
            .filter_map(WeakEntity::upgrade)
            .any(|composer| {
                let composer = composer.read(cx);
                !composer.value(cx).trim().is_empty()
                    && composer.focus_handle(cx).contains_focused(window, cx)
            })
    })
}

/// A downloaded update waiting for a restart.
#[derive(Clone, Debug)]
struct Ready {
    version: Version,
    /// The staged `Matty.app`; `None` in preview.
    staged: Option<PathBuf>,
}

/// What one check came to.
#[derive(Debug)]
enum Outcome {
    /// Nothing newer on the channel.
    UpToDate,
    /// The newest version is the one already staged.
    Staged,
    /// A newer version, downloaded and staged.
    New(Ready),
    /// The check, the download or the staging failed.
    Failed(String),
}

/// Owns the checks and the prompt for one window.
pub(crate) struct UpdatePrompt {
    toasts: Entity<ToastState>,
    /// `None` where this build cannot update itself.
    updater: Option<Updater>,
    ready: Option<Ready>,
    prompted: bool,
    /// The prompt left the screen without updating, such as by "Later" or
    /// its close button, so the next check offers it again.
    snoozed: bool,
    deferral: Deferral,
    channel: crate::settings::UpdateChannel,
    /// A check is running.
    checking: bool,
    /// The person asked for the running check, so its outcome is shown.
    report: bool,
    _checks: Option<Task<()>>,
    _poll: Option<Task<()>>,
}

impl UpdatePrompt {
    fn new(toasts: Entity<ToastState>, cx: &App) -> Self {
        Self {
            toasts,
            updater: None,
            ready: None,
            prompted: false,
            snoozed: false,
            deferral: Deferral::default(),
            channel: SettingsStore::global(cx).read(cx).get().update_channel,
            checking: false,
            report: false,
            _checks: None,
            _poll: None,
        }
    }

    /// Starts checking for this build, or does nothing where it cannot
    /// update itself. `MATTY_PREVIEW=update` shows the prompt for a made-up
    /// release instead.
    pub(crate) fn start(
        toasts: Entity<ToastState>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| {
            let mut this = Self::new(toasts, cx);
            if std::env::var("MATTY_PREVIEW").as_deref() == Ok("update") {
                this.set_ready(
                    Ready {
                        version: Version::new(0, 2, 0),
                        staged: None,
                    },
                    window,
                    cx,
                );
                return this;
            }
            if crate::preview::enabled() || crate::offline() {
                return this;
            }
            let Some(updater) = Updater::for_this_build() else {
                tracing::info!("this build does not update itself");
                return this;
            };
            let Some(bundle) = matty_update::install::current_bundle() else {
                tracing::info!("not running from an app bundle; updates are off");
                return this;
            };
            tracing::info!(bundle = %bundle.display(), "checking for updates");
            this.updater = Some(updater);
            this.schedule_checks(window, cx);
            cx.subscribe_in(
                &SettingsStore::global(cx),
                window,
                move |this: &mut Self, store, _: &SettingsChanged, window, cx| {
                    let channel = store.read(cx).get().update_channel;
                    if channel != this.channel {
                        this.channel = channel;
                        this.schedule_checks(window, cx);
                    }
                },
            )
            .detach();
            this
        })
    }

    /// "Check for updates": checks now and says what it found. A ready
    /// update shows its prompt at once, and a check already running
    /// reports when it ends.
    pub(crate) fn check_now(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ready.is_some() {
            self.offer_now(cx);
            return;
        }
        if self.updater.is_none() {
            let notice = Notice::new(t!("update-unsupported"))
                .description(t!("update-unsupported-detail"))
                .action(t!("update-open-releases"), |_, cx| {
                    cx.open_url(RELEASES_URL)
                })
                .cancel(t!("update-dismiss"))
                .timeout(Duration::from_secs(8));
            self.push_check(notice, cx);
            return;
        }
        self.report = true;
        self.push_check(Notice::new(t!("update-checking")).persistent(), cx);
        if !self.checking {
            self.schedule_checks(window, cx);
        }
    }

    fn push_check(&self, notice: Notice, cx: &mut Context<Self>) {
        self.toasts
            .update(cx, |toasts, cx| toasts.push(CHECK_TOAST, notice, cx));
    }

    /// Checks now and then every [`matty_update::CHECK_INTERVAL`],
    /// replacing any earlier loop.
    fn schedule_checks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(updater) = self.updater.clone() else {
            return;
        };
        let dir = dirs::cache_dir().map(|dir| dir.join("Matty").join("updates")); // i18n-ignore: a folder
        self._checks = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                let Ok((channel, staged)) = this.update(cx, |this, _| {
                    this.checking = true;
                    let staged = this.ready.as_ref().map(|ready| ready.version.clone());
                    (this.channel.feed(), staged)
                }) else {
                    return;
                };
                let outcome = 'check: {
                    let Some(dir) = dir.clone() else {
                        break 'check Outcome::Failed("no cache folder for updates".into()); // i18n-ignore: an error, as matty-update's are
                    };
                    let checker = updater.clone();
                    let found = matty_sync::io::run(async move { checker.check(channel).await })
                        .recv()
                        .await;
                    let update = match found {
                        Ok(Ok(Some(update))) => update,
                        Ok(Ok(None)) => break 'check Outcome::UpToDate,
                        Ok(Err(error)) => break 'check Outcome::Failed(error.to_string()),
                        Err(_) => return,
                    };
                    if staged.as_ref() == Some(&update.version) {
                        break 'check Outcome::Staged;
                    }
                    tracing::info!(version = %update.version, "downloading an update");
                    let version = update.version.clone();
                    if this
                        .update(cx, |this, cx| this.downloading(&version, cx))
                        .is_err()
                    {
                        return;
                    }
                    let downloader = updater.clone();
                    let zip =
                        matty_sync::io::run(
                            async move { downloader.download(&update, &dir).await },
                        )
                        .recv()
                        .await;
                    let zip = match zip {
                        Ok(Ok(zip)) => zip,
                        Ok(Err(error)) => break 'check Outcome::Failed(error.to_string()),
                        Err(_) => return,
                    };
                    let staged = cx
                        .background_executor()
                        .spawn(async move { matty_update::install::stage(&zip) })
                        .await;
                    match staged {
                        Ok(app) => Outcome::New(Ready {
                            version,
                            staged: Some(app),
                        }),
                        Err(error) => {
                            tracing::warn!(%error, "the update could not be staged");
                            Outcome::Failed(error.to_string())
                        }
                    }
                };
                let alive = this.update_in(cx, |this, window, cx| {
                    this.finish_check(outcome, window, cx)
                });
                if alive.is_err() {
                    return;
                }
                cx.background_executor()
                    .timer(matty_update::CHECK_INTERVAL)
                    .await;
            }
        }));
    }

    /// A check found `version` and is downloading it.
    fn downloading(&mut self, version: &Version, cx: &mut Context<Self>) {
        if self.report {
            let notice = Notice::new(t!("update-downloading", version = version.to_string()))
                .description(t!("update-downloading-detail"))
                .persistent();
            self.push_check(notice, cx);
        }
    }

    /// Acts on what a check found, and answers the person if they asked.
    fn finish_check(&mut self, outcome: Outcome, window: &mut Window, cx: &mut Context<Self>) {
        self.checking = false;
        let report = std::mem::take(&mut self.report);
        match outcome {
            Outcome::UpToDate => {
                if report {
                    let notice = Notice::new(t!("update-up-to-date"))
                        .description(t!("update-up-to-date-detail", version = crate::VERSION))
                        .timeout(Duration::from_secs(5));
                    self.push_check(notice, cx);
                }
            }
            Outcome::Failed(error) => {
                tracing::warn!(%error, "the update check failed");
                if report {
                    let notice = Notice::new(t!("update-check-failed"))
                        .description(error)
                        .cancel(t!("update-dismiss"))
                        .timeout(Duration::from_secs(8));
                    self.push_check(notice, cx);
                }
            }
            Outcome::Staged if report => self.offer_now(cx),
            Outcome::Staged => self.checked_again(window, cx),
            Outcome::New(ready) if report => {
                tracing::info!(version = %ready.version, "an update is ready");
                self.ready = Some(ready);
                self.offer_now(cx);
            }
            Outcome::New(ready) => self.set_ready(ready, window, cx),
        }
    }

    fn set_ready(&mut self, ready: Ready, window: &mut Window, cx: &mut Context<Self>) {
        tracing::info!(version = %ready.version, "an update is ready");
        self.ready = Some(ready);
        self.rearm(window, cx);
    }

    /// A check found the update already staged: offer it again if the
    /// prompt was put off, and leave it be if it is still on screen.
    fn checked_again(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.snoozed {
            self.rearm(window, cx);
        }
    }

    /// Offers the ready update again once the composers are free.
    fn rearm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ready.is_none() {
            return;
        }
        self.prompted = false;
        self.snoozed = false;
        self.deferral = Deferral::default();
        self._poll = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                let waiting =
                    this.update_in(cx, |this, window, cx| this.tick(Instant::now(), window, cx));
                if !matches!(waiting, Ok(true)) {
                    return;
                }
                cx.background_executor().timer(POLL).await;
            }
        }));
    }

    /// Shows the ready update's prompt now, without waiting for the
    /// composers: the person asked for it.
    fn offer_now(&mut self, cx: &mut Context<Self>) {
        self.toasts
            .update(cx, |toasts, cx| toasts.dismiss(&CHECK_TOAST.into(), cx));
        self._poll = None;
        self.prompted = true;
        self.snoozed = false;
        self.show_prompt(cx);
    }

    /// Shows the prompt if it may show at `now`. Answers whether it is
    /// still waiting.
    pub(crate) fn tick(&mut self, now: Instant, window: &Window, cx: &mut Context<Self>) -> bool {
        if self.ready.is_none() || self.prompted {
            return false;
        }
        if !self.deferral.may_prompt(composer_busy(window, cx), now) {
            return true;
        }
        self.prompted = true;
        self.show_prompt(cx);
        false
    }

    /// Puts the prompt on screen unless it is there from an earlier offer.
    fn show_prompt(&mut self, cx: &mut Context<Self>) {
        let Some(ready) = self.ready.clone() else {
            return;
        };
        if self.toasts.read(cx).contains(TOAST) {
            return;
        }
        let this = cx.weak_entity();
        let dismissed = this.clone();
        let notice = Notice::new(t!("update-ready"))
            .description(t!(
                "update-ready-detail",
                version = ready.version.to_string()
            ))
            .action(t!("update-restart"), move |_, cx| {
                this.update(cx, |this, cx| this.restart(cx)).ok();
            })
            .cancel(t!("update-later"))
            .on_dismiss(move |reason, cx| {
                dismissed.update(cx, |this, _| this.dismissed(reason)).ok();
            })
            .persistent();
        self.toasts
            .update(cx, |toasts, cx| toasts.push(TOAST, notice, cx));
    }

    /// The prompt left the screen. Anything but a newer offer in its place
    /// leaves the update to offer again at the next check: "Later", the
    /// close button, or a restart that failed to install.
    fn dismissed(&mut self, reason: NoticeDismissal) {
        if !matches!(reason, NoticeDismissal::Replaced) {
            self.snoozed = true;
        }
    }

    fn restart(&mut self, cx: &mut Context<Self>) {
        let Some(Ready {
            staged: Some(staged),
            ..
        }) = self.ready.clone()
        else {
            tracing::info!("preview: Restart to update does nothing");
            return;
        };
        let installed = matty_update::install::current_bundle()
            .ok_or(UpdateError::Unsupported("not running from an app bundle")) // i18n-ignore: an error
            .and_then(|bundle| matty_update::install::install_and_relaunch(&staged, &bundle));
        match installed {
            Ok(()) => cx.quit(),
            Err(error) => {
                tracing::warn!(%error, "the update could not be installed");
                let notice = Notice::new(t!("update-install-failed"))
                    .description(error.to_string())
                    .timeout(Duration::from_secs(8))
                    .cancel(t!("update-dismiss"));
                self.toasts
                    .update(cx, |toasts, cx| toasts.push("update.failed", notice, cx));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::TestAppContext;

    #[test]
    fn the_prompt_waits_for_the_composers_to_stay_free() {
        let start = Instant::now();
        let at = |seconds: u64| start + Duration::from_secs(seconds);
        let mut deferral = Deferral::default();
        assert!(!deferral.may_prompt(true, at(0)));
        // Free, but not for long enough yet.
        assert!(!deferral.may_prompt(false, at(1)));
        assert!(!deferral.may_prompt(false, at(3)));
        // Typing again starts the wait over.
        assert!(!deferral.may_prompt(true, at(4)));
        assert!(!deferral.may_prompt(false, at(5)));
        assert!(!deferral.may_prompt(false, at(7)));
        assert!(deferral.may_prompt(false, at(8)));
    }

    #[gpui_kit::test]
    fn the_prompt_never_shows_while_the_composer_has_unsent_text_and_focus(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            crate::ui::theme::init(cx);
            SettingsStore::init(None, cx);
        });
        let (composer, cx) = cx.add_window_view(|window, cx| ComposerState::new(4, window, cx));
        let (toasts, prompt) = cx.update(|window, cx| {
            watch_composer(&composer, cx);
            composer.read(cx).focus_handle(cx).focus(window, cx);
            let toasts = cx.new(ToastState::new);
            let prompt = cx.new(|cx| {
                let mut prompt = UpdatePrompt::new(toasts.clone(), cx);
                prompt.ready = Some(Ready {
                    version: Version::new(0, 2, 0),
                    staged: None,
                });
                prompt
            });
            (toasts, prompt)
        });
        let start = Instant::now();
        let tick = |cx: &mut gpui_kit::VisualTestContext, seconds: u64| {
            cx.update(|window, cx| {
                prompt.update(cx, |prompt, cx| {
                    prompt.tick(start + Duration::from_secs(seconds), window, cx)
                })
            })
        };

        // Typing: focus and unsent text.
        cx.update(|window, cx| {
            composer.update(cx, |composer, cx| {
                composer.set_value("half a thou", window, cx)
            });
            assert!(composer_busy(window, cx));
        });
        for second in 0..20 {
            assert!(tick(cx, second), "still waiting at {second}s");
        }
        cx.update(|_, cx| assert!(toasts.read(cx).is_empty()));

        // Focus elsewhere with the text still there: free.
        cx.update(|window, cx| {
            window.blur(cx);
            assert!(!composer_busy(window, cx));
        });
        // Back to typing before the wait is up holds it off again.
        assert!(tick(cx, 20));
        cx.update(|window, cx| {
            composer.read(cx).focus_handle(cx).focus(window, cx);
            assert!(composer_busy(window, cx));
        });
        assert!(tick(cx, 22));
        assert!(tick(cx, 24));
        cx.update(|_, cx| assert!(toasts.read(cx).is_empty()));

        // Sent: the composer is empty, so after the idle wait it shows.
        cx.update(|window, cx| {
            composer.update(cx, |composer, cx| composer.set_value("", window, cx));
            assert!(!composer_busy(window, cx));
        });
        assert!(tick(cx, 25));
        assert!(!tick(cx, 25 + IDLE.as_secs()));
        cx.update(|_, cx| assert!(!toasts.read(cx).is_empty()));
        // Shown once.
        assert!(!tick(cx, 60));
    }

    #[gpui_kit::test]
    fn later_or_closing_puts_the_prompt_off_until_the_next_check(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::ui::theme::init(cx);
            SettingsStore::init(None, cx);
        });
        let (toasts, cx) = cx.add_window_view(|_, cx| ToastState::new(cx));
        let prompt = cx.update(|_, cx| {
            cx.new(|cx| {
                let mut prompt = UpdatePrompt::new(toasts.clone(), cx);
                prompt.ready = Some(Ready {
                    version: Version::new(0, 2, 0),
                    staged: None,
                });
                prompt
            })
        });
        let start = Instant::now();
        let show = |cx: &mut gpui_kit::VisualTestContext| {
            cx.update(|window, cx| {
                prompt.update(cx, |prompt, cx| {
                    prompt.tick(start, window, cx);
                    prompt.tick(start + IDLE, window, cx);
                })
            });
            cx.run_until_parked();
        };
        let check = |cx: &mut gpui_kit::VisualTestContext| {
            cx.update(|window, cx| {
                prompt.update(cx, |prompt, cx| prompt.checked_again(window, cx))
            });
            show(cx);
        };
        let showing = |cx: &mut gpui_kit::VisualTestContext| {
            cx.update(|_, cx| toasts.read(cx).contains(TOAST))
        };
        let press = |cx: &mut gpui_kit::VisualTestContext, label: &str| {
            // Past the entrance animation.
            cx.executor().advance_clock(Duration::from_secs(1));
            cx.run_until_parked();
            cx.update(|window, cx| window.draw(cx).clear(cx));
            let at = cx.update(|window, _| {
                use gpui_kit::test::TestQueryExt as _;
                window
                    .elements()
                    .into_iter()
                    .find(|element| element.label().is_some_and(|l| l == label))
                    .unwrap_or_else(|| panic!("nothing is labelled {label:?}"))
                    .bounds()
                    .center()
            });
            cx.simulate_click(at, gpui_kit::Modifiers::none());
            cx.run_until_parked();
        };

        show(cx);
        assert!(showing(cx));
        // A check while it is on screen leaves it there, untouched.
        check(cx);
        assert!(showing(cx));
        assert!(!prompt.read_with(cx, |prompt, _| prompt.snoozed));

        // "Later" hides it until the next check, which offers it again.
        press(cx, "Later");
        assert!(!showing(cx));
        assert!(prompt.read_with(cx, |prompt, _| prompt.snoozed));
        show(cx);
        assert!(!showing(cx), "not before the next check");
        check(cx);
        assert!(showing(cx), "the next check offers it again");

        // The close button does the same.
        press(cx, "Close notification");
        assert!(!showing(cx));
        check(cx);
        assert!(showing(cx));
    }
    #[gpui_kit::test]
    fn check_for_updates_always_answers(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::ui::theme::init(cx);
            SettingsStore::init(None, cx);
        });
        let (toasts, cx) = cx.add_window_view(|_, cx| ToastState::new(cx));
        let prompt = cx.update(|_, cx| cx.new(|cx| UpdatePrompt::new(toasts.clone(), cx)));
        let showing = |cx: &mut gpui_kit::VisualTestContext, id: &str| {
            cx.update(|_, cx| toasts.read(cx).contains(id))
        };

        // A build that cannot update itself says so instead of nothing.
        cx.update(|window, cx| prompt.update(cx, |prompt, cx| prompt.check_now(window, cx)));
        assert!(showing(cx, CHECK_TOAST));
        assert!(!prompt.read_with(cx, |prompt, _| prompt.report));

        // A check the person asked for reports what it found.
        let finish = |cx: &mut gpui_kit::VisualTestContext, outcome: Outcome| {
            cx.update(|window, cx| {
                prompt.update(cx, |prompt, cx| {
                    prompt.report = true;
                    prompt.finish_check(outcome, window, cx);
                })
            });
            cx.run_until_parked();
        };
        finish(cx, Outcome::UpToDate);
        assert!(showing(cx, CHECK_TOAST));
        finish(cx, Outcome::Failed("offline".into()));
        assert!(showing(cx, CHECK_TOAST));
        assert!(!showing(cx, TOAST));

        // A new version offers its prompt at once, in place of the answer.
        finish(
            cx,
            Outcome::New(Ready {
                version: Version::new(0, 2, 0),
                staged: None,
            }),
        );
        assert!(showing(cx, TOAST));
        assert!(!showing(cx, CHECK_TOAST));

        // Asking again with the update ready shows the prompt again.
        cx.update(|_, cx| toasts.update(cx, |toasts, cx| toasts.dismiss(&TOAST.into(), cx)));
        cx.run_until_parked();
        assert!(!showing(cx, TOAST));
        cx.update(|window, cx| prompt.update(cx, |prompt, cx| prompt.check_now(window, cx)));
        assert!(showing(cx, TOAST));
    }

    #[gpui_kit::test]
    fn a_background_check_stays_quiet(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::ui::theme::init(cx);
            SettingsStore::init(None, cx);
        });
        let (toasts, cx) = cx.add_window_view(|_, cx| ToastState::new(cx));
        let prompt = cx.update(|_, cx| cx.new(|cx| UpdatePrompt::new(toasts.clone(), cx)));
        for outcome in [Outcome::UpToDate, Outcome::Failed("offline".into())] {
            cx.update(|window, cx| {
                prompt.update(cx, |prompt, cx| prompt.finish_check(outcome, window, cx))
            });
        }
        cx.run_until_parked();
        assert!(cx.update(|_, cx| toasts.read(cx).is_empty()));
    }
}
