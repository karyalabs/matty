//! Opening, adding, switching and removing saved servers (#206). Every
//! saved server's sync keeps running (see [`super::background`]); the
//! window shows one at a time, and switching shows another over its
//! running sync.

use gpui_kit::{AppContext as _, Context, Entity, Window};

use super::{ConnectionState, MainWindow};
use crate::login::{LoginEvent, LoginModel, LoginView};
use crate::settings::SavedAccount;
use crate::sidebar::SidebarModel;
use crate::teams::{Server, TeamsEvent};

impl MainWindow {
    /// Opens `account` at launch or after a switch: its cache alone with
    /// `matty --offline`, otherwise its cache and a connection with the
    /// saved session, unless its sync is connected already. `false` when it
    /// has no session to connect with.
    pub(super) fn open_server(
        &mut self,
        account: &SavedAccount,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if crate::offline() {
            self.login = None;
            self.start_live(account, window, cx);
            // Tells the views too, so they refuse what needs the server (#163).
            self.set_connection(ConnectionState::Offline, cx);
            if let Some(live) = &self.live {
                live.send(matty_sync::Command::ShowCache);
            }
            return true;
        }
        let Some(token) = crate::secrets::load(account) else {
            return false;
        };
        let Ok(client) = matty_api::Client::new(&account.server) else {
            return false;
        };
        self.login = None;
        if self.start_live(account, window, cx) {
            return true;
        }
        if let Some(live) = &self.live {
            live.send(matty_sync::Command::Connect {
                client: client.with_token(token),
            });
            let opens_on = self
                .settings
                .read(cx)
                .get()
                .opens_on
                .get(&account.data_key())
                .cloned();
            if let Some(channel_id) = opens_on {
                live.send(matty_sync::Command::Prefetch { channel_id });
            }
        }
        true
    }

    /// Empties what the window showed of the shown server, before it shows
    /// another. The server's sync runs on for its badges.
    pub(super) fn leave_server(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(live) = self.live.take() {
            live.release();
        }
        self._live_task = None;
        self._typing_task = None;
        self._view_task = None;
        cx.set_global(crate::avatars::Avatars::default());
        cx.set_global(crate::composer_draft::DraftBook::default());
        self.selected_channel = None;
        self.history.clear();
        self.history_at = 0;
        self.kept_unread = None;
        self.link_asked = None;
        self.link_join_offer = None;
        self.link_jump = None;
        self.right_pane_open = false;
        self.threads_open = false;
        self.drafts_open = false;
        self.queued = 0;
        self.notify_props = None;
        self.collapsed_threads = None;
        self.highlight_keys = false;
        self.set_connection(ConnectionState::Online, cx);
        self.sidebar.update(cx, |sidebar, cx| {
            sidebar.set_model(SidebarModel::loading(), cx)
        });
        self.channel
            .update(cx, |channel, cx| channel.set_model(None, window, cx));
    }

    /// Shows the sign-in card to add a server, over the server shown now,
    /// which stays connected until the new one signs in. With `server`,
    /// the card checks that address at once, as when a saved server's
    /// session is gone.
    pub(super) fn add_server(
        &mut self,
        server: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.login.is_some() {
            return;
        }
        let model = LoginModel {
            adding: true,
            ..LoginModel::default()
        };
        let view = cx.new(|cx| LoginView::new(model, window, cx));
        cx.subscribe_in(&view, window, Self::on_login_event)
            .detach();
        self.login = Some(view.clone());
        if let Some(url) = server {
            Self::on_login_event(self, &view, &LoginEvent::CheckServer(url), window, cx);
        }
        cx.notify();
    }

    /// Cancel on the card that adds a server: back to the one shown.
    pub(super) fn cancel_add_server(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self
            .login
            .as_ref()
            .is_some_and(|login| login.read(cx).model().adding)
        {
            return;
        }
        self.cancel_sso();
        self.login = None;
        self.login_server = None;
        self.pending_page = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Cmd/Ctrl+number or a click on the rail: shows the `index`th saved
    /// server. One whose session is gone asks to sign in to it again.
    pub(super) fn switch_server(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.login.is_some() {
            return;
        }
        if crate::preview::enabled() {
            let servers = self.teams_model.read(cx).servers().len();
            if index < servers {
                self.teams_model.update(cx, |rail, cx| {
                    rail.set_servers(crate::preview::servers(), Some(index), cx)
                });
            }
            return;
        }
        let settings = self.settings.read(cx).get();
        if settings.current_server() == Some(index) {
            return;
        }
        let Some(account) = settings.servers.get(index).cloned() else {
            return;
        };
        if !crate::offline() && crate::secrets::load(&account).is_none() {
            return self.add_server(Some(account.server), window, cx);
        }
        self.settings.update(cx, |settings, cx| {
            settings.update(cx, |s| {
                s.select_server(index);
            })
        });
        self.open_server(&account, window, cx);
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Removes the `index`th saved server and deletes its data. The shown
    /// one signs out, as the account menu does, and the next saved server
    /// opens; another is signed out on its server in the background.
    pub(super) fn remove_server(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if crate::preview::enabled() {
            return;
        }
        let settings = self.settings.read(cx).get();
        let Some(account) = settings.servers.get(index).cloned() else {
            return;
        };
        if settings.current_server() == Some(index) {
            return self.sign_out(window, cx);
        }
        if let Some(token) = crate::secrets::load(&account)
            && let Ok(client) = matty_api::Client::new(&account.server)
        {
            let client = client.with_token(token);
            drop(matty_sync::io::run(async move {
                if let Err(error) = client.logout().await {
                    tracing::info!(%error, "could not end a removed server's session");
                }
            }));
        }
        crate::secrets::delete(&account);
        self.watches.stop(&account.data_key());
        self.settings.update(cx, |settings, cx| {
            settings.update(cx, |s| {
                s.remove_server(&account.data_key());
            })
        });
        forget_files(
            crate::live::database_path(&account),
            crate::live::media_dir(&account),
            cx,
        );
    }

    /// Saves the rail's new order after a drag; the preview's rail keeps
    /// its own.
    pub(super) fn move_server(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if crate::preview::enabled() {
            return;
        }
        self.settings.update(cx, |settings, cx| {
            settings.update(cx, |s| s.move_server(from, to))
        });
    }

    /// After signing out of the shown server, opens the next saved one;
    /// `false` when there is none, or none with a session.
    pub(super) fn open_next_server(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let next = self
            .settings
            .update(cx, |settings, cx| settings.launch_server(cx));
        next.is_some_and(|account| self.open_server(&account, window, cx))
    }

    /// The rail follows the saved servers, each of which stays connected.
    pub(super) fn refresh_servers(&mut self, cx: &mut Context<Self>) {
        self.watch_servers(cx);
        self.show_servers(cx);
    }

    /// The rail's servers, with the unreads and mentions of those not shown.
    pub(super) fn show_servers(&mut self, cx: &mut Context<Self>) {
        let (servers, current) = if crate::preview::enabled() {
            let servers = crate::preview::servers();
            let current = self.teams_model.read(cx).current_server().or(Some(0));
            (servers, current)
        } else {
            let settings = self.settings.read(cx).get();
            let current = settings.current_server();
            let servers = settings
                .servers
                .iter()
                .enumerate()
                .map(|(index, server)| {
                    let rail = Server::new(server.label().to_owned())
                        .key(server.data_key())
                        .host(crate::live::url_host(&server.server).to_owned());
                    match self.watches.unreads(&server.data_key()) {
                        Some((mentions, unread)) if current != Some(index) => {
                            rail.unread(unread).mentions(mentions)
                        }
                        _ => rail,
                    }
                })
                .collect();
            (servers, current)
        };
        self.teams_model
            .update(cx, |rail, cx| rail.set_servers(servers, current, cx));
    }

    pub(super) fn on_teams_event(
        &mut self,
        _: &Entity<crate::teams::TeamsModel>,
        event: &TeamsEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            TeamsEvent::SelectServer(index) => self.switch_server(*index, window, cx),
            TeamsEvent::AddServer => self.add_server(None, window, cx),
            TeamsEvent::Select(team) => self.switch_team(team.clone(), window, cx),
            TeamsEvent::Join => self.join_team(window, cx),
            TeamsEvent::Leave { id, name } => {
                self.ask_leave_team(id.clone(), name.clone(), window, cx)
            }
            TeamsEvent::ManageServers => {
                self.open_settings(super::settings_pages::Section::Servers, window, cx)
            }
        }
    }

    /// Join another team (#745): the Join a team dialog over this window,
    /// listing the shown server's open teams, with a field for an invite
    /// link. From Settings, this window comes forward with it.
    pub(super) fn join_team(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let url = {
            let settings = self.settings.read(cx).get();
            settings
                .current_server()
                .and_then(|index| settings.servers.get(index))
                .map(|account| account.server.clone())
                .unwrap_or_default()
        };
        // The name the server menu shows: its `SiteName`, or its host.
        let name = {
            let model = self.teams_model.read(cx);
            model
                .current_server()
                .and_then(|index| model.servers().get(index))
                .map(|server| server.name.clone())
                .unwrap_or_else(|| "Matty".into()) // i18n-ignore
        };
        if self.settings_host.overlay.is_some() {
            self.close_settings(cx);
        }
        window.activate_window();
        self.join_team_view
            .update(cx, |view, cx| view.open(name, url, window, cx));
        if let Some(live) = &self.live {
            live.send(matty_sync::Command::BrowseTeams);
        } else if crate::preview::enabled() {
            let mine = self.teams_model.read(cx).teams().to_vec();
            let teams = crate::preview::joinable_teams()
                .into_iter()
                .filter(|team| !mine.iter().any(|joined| joined.id == team.id))
                .collect();
            self.join_team_view.update(cx, |view, cx| {
                view.set_teams(crate::directory::JoinableTeams::Loaded(teams), cx)
            });
        }
    }

    pub(super) fn on_join_team_event(
        &mut self,
        _: &Entity<crate::directory::JoinTeamView>,
        event: &crate::directory::JoinTeamEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::directory::{JoinTeamEvent, JoinableTeams};
        if let Some(live) = &self.live {
            // Sync joins and loads the team; the window switches to it once
            // it is in the team list.
            match event {
                JoinTeamEvent::Join(id) => live.send(matty_sync::Command::JoinTeam {
                    team_id: id.to_string(),
                }),
                JoinTeamEvent::Invite(invite_id) => {
                    live.send(matty_sync::Command::JoinTeamByInvite {
                        invite_id: invite_id.to_string(),
                    })
                }
            }
            return;
        }
        // The preview joins at once, with the first team's channels.
        let JoinTeamEvent::Join(id) = event else {
            return;
        };
        let JoinableTeams::Loaded(joinable) = self.join_team_view.read(cx).teams().clone() else {
            return;
        };
        let Some(joined) = joinable.into_iter().find(|team| &team.id == id) else {
            return;
        };
        let (mut teams, current) = {
            let model = self.teams_model.read(cx);
            (model.teams().to_vec(), model.current().cloned())
        };
        if !teams.iter().any(|team| team.id == joined.id) {
            teams.push(crate::teams::Team::new(joined.id.clone(), joined.name));
        }
        self.teams_model
            .update(cx, |model, cx| model.set_teams(teams, current, cx));
        self.switch_team(joined.id, window, cx);
    }

    /// Shows the team sync just joined once it is in the team list, as
    /// Mattermost does after a join.
    pub(super) fn arrive_in_joined_team(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.joining.clone() else {
            return;
        };
        if self
            .teams_model
            .read(cx)
            .teams()
            .iter()
            .any(|team| team.id == id)
        {
            self.joining = None;
            self.switch_team(id, window, cx);
        }
    }

    /// Asks "Leave {team}?" over this window, from a team tab's menu.
    pub(super) fn ask_leave_team(
        &mut self,
        id: gpui_kit::SharedString,
        name: gpui_kit::SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.leaving.replace(Some((id, name)));
        self.leave_team_dialog.open(window, cx);
        cx.notify();
    }

    /// Leaves `team` once the user confirmed: sync tells the server, and
    /// the team drops out of the list when it agrees. Leaving the team on
    /// screen moves the window to the next one.
    pub(super) fn leave_team(
        &mut self,
        team: gpui_kit::SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(live) = &self.live {
            return live.send(matty_sync::Command::LeaveTeam {
                team_id: team.to_string(),
            });
        }
        if crate::preview::enabled() {
            let teams: Vec<_> = self
                .teams_model
                .read(cx)
                .teams()
                .iter()
                .filter(|kept| kept.id != team)
                .cloned()
                .collect();
            let current = self.teams_model.read(cx).current().cloned();
            let next = teams.first().map(|next| next.id.clone());
            if current.as_ref() == Some(&team)
                && let Some(next) = next
            {
                self.switch_team(next, window, cx);
            }
            let current = self.teams_model.read(cx).current().cloned();
            self.teams_model
                .update(cx, |model, cx| model.set_teams(teams, current, cx));
        }
    }

    /// Cmd/Ctrl+1…9 and Add server, on the window's root.
    pub(super) fn on_server_actions(root: gpui_kit::Div, cx: &mut Context<Self>) -> gpui_kit::Div {
        use crate::actions::*;
        use gpui_kit::InteractiveElement as _;
        macro_rules! switch {
            ($root:expr, $($action:ident => $index:literal),*) => {
                $root$(.on_action(cx.listener(|this, _: &$action, window, cx| {
                    this.switch_server($index, window, cx)
                })))*
            };
        }
        let root = root.on_action(
            cx.listener(|this, _: &AddServer, window, cx| this.add_server(None, window, cx)),
        );
        switch!(
            root,
            SwitchServer1 => 0,
            SwitchServer2 => 1,
            SwitchServer3 => 2,
            SwitchServer4 => 3,
            SwitchServer5 => 4,
            SwitchServer6 => 5,
            SwitchServer7 => 6,
            SwitchServer8 => 7,
            SwitchServer9 => 8
        )
    }
}

/// Deletes an account's database and media folder in the background. Its
/// sync may still be closing the database, so a failed delete is tried
/// again for a few seconds.
pub(super) fn forget_files(
    database: std::path::PathBuf,
    media: Option<std::path::PathBuf>,
    cx: &mut gpui_kit::App,
) {
    let executor = cx.background_executor().clone();
    cx.background_executor()
        .spawn(async move {
            for _ in 0..20 {
                match matty_sync::forget_account(&database, media.as_deref()) {
                    Ok(()) => return,
                    Err(error) => {
                        tracing::debug!(%error, "the cache is still in use");
                        executor.timer(std::time::Duration::from_millis(250)).await;
                    }
                }
            }
            tracing::warn!(path = %database.display(), "could not delete a signed-out cache");
        })
        .detach();
}
