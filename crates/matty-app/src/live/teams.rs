//! Which team is on screen (#89). The sidebar, the Threads view, search,
//! mentions and Browse all follow the shown team; sync keeps every team's
//! sidebar, so switching paints from what is already here and only asks
//! sync for the new team's Threads list.

use std::sync::Arc;

use gpui_kit::SharedString;
use matty_sync::{ChangeSet, Command, CommandError, SidebarSection};

use super::{Live, Update};
use crate::sidebar::{ChannelKind, ChannelRow};

/// A team's switcher rows and the sidebar sections they were built from.
pub(super) type TeamRows = (Arc<[SidebarSection]>, Arc<[ChannelRow]>);

impl Live {
    /// Joining and leaving teams (#745): the Join a team dialog's list,
    /// a joined team, and what the server refused.
    pub(super) fn apply_membership(&mut self, change: &ChangeSet, update: &mut Update) -> bool {
        use crate::directory::{JoinableTeam, JoinableTeams};
        match change {
            ChangeSet::JoinableTeams(teams) => {
                let teams = teams
                    .iter()
                    .map(|team| JoinableTeam {
                        id: team.id.clone().into(),
                        name: team.display_name.clone().into(),
                        description: (!team.description.trim().is_empty())
                            .then(|| team.description.trim().to_owned().into()),
                    })
                    .collect();
                update.joinable = Some(JoinableTeams::Loaded(teams));
            }
            ChangeSet::TeamJoined { team_id } => {
                update.team_joined = Some(team_id.clone().into());
            }
            ChangeSet::Failed(
                CommandError::Server {
                    action: "browse teams",
                    ..
                }
                | CommandError::Offline {
                    action: "browse teams",
                },
            ) => update.joinable = Some(JoinableTeams::Unavailable),
            ChangeSet::Failed(CommandError::Server {
                action: action @ ("join team" | "leave team"),
                error,
                ..
            }) => {
                // The server's words without its id and status.
                let said = error
                    .rsplit_once(" (")
                    .map_or(error.as_str(), |(said, _)| said)
                    .to_owned();
                update.refused = Some(if *action == "join team" {
                    crate::t!("teams-join-failed", error = said)
                } else {
                    crate::t!("teams-leave-failed", error = said)
                });
            }
            _ => return false,
        }
        true
    }

    /// Makes `team` the shown one: the old team's Threads list stops
    /// following and the new one's opens, and the sidebar shows the new
    /// team's categories.
    pub(super) fn show_team(&mut self, team: Option<String>, update: &mut Update) {
        if let Some(old) = std::mem::replace(&mut self.team, team) {
            self.send(Command::CloseThreadList { team_id: old });
        }
        self.thread_list = None;
        if let Some(team_id) = self.team.clone() {
            self.send(Command::OpenThreadList { team_id });
        }
        update.categories = self.shown_categories();
    }

    /// Shows `team_id` once the user picks it on the rail, with a shortcut
    /// or by opening one of its channels. `None` when it is already shown
    /// or the user is not on it.
    pub fn switch_team(&mut self, team_id: &str) -> Option<Update> {
        if self.team.as_deref() == Some(team_id) || !self.teams.iter().any(|t| t.id == team_id) {
            return None;
        }
        let mut update = Update::default();
        self.show_team(Some(team_id.to_owned()), &mut update);
        // A team whose sidebar has not loaded yet shows an empty one rather
        // than the last team's.
        update.categories.get_or_insert_with(Vec::new);
        update.team_name = self.team_name();
        update.teams = Some(self.rail_teams());
        // The last team's threads are not this one's; the list comes back
        // from sync.
        update.threads = Some(crate::threads::ThreadsModel {
            loading: true,
            ..Default::default()
        });
        Some(update)
    }

    /// The team whose sidebar holds `channel_id`: the shown team when it
    /// does (as for every direct message), else the first other team that
    /// does.
    pub fn team_of(&self, channel_id: &str) -> Option<&str> {
        let holds = |team: &str| {
            self.sections.get(team).is_some_and(|sections| {
                sections
                    .iter()
                    .any(|section| section.channels.iter().any(|c| c.id == channel_id))
            })
        };
        if let Some(shown) = self.team.as_deref()
            && holds(shown)
        {
            return Some(shown);
        }
        self.teams
            .iter()
            .map(|team| team.id.as_str())
            .find(|team| holds(team))
    }

    /// The user's teams other than the shown one, in rail order, with
    /// their channels, for the quick switcher (#90). Direct messages are
    /// in every team's sidebar and come with the shown team's.
    pub fn other_teams(&self) -> Vec<crate::switcher::TeamChannels> {
        self.teams
            .iter()
            .filter(|team| Some(&team.id) != self.team.as_ref())
            .map(|team| crate::switcher::TeamChannels {
                id: team.id.clone().into(),
                name: team.display_name.clone().into(),
                rows: self.switcher_rows(&team.id).to_vec(),
            })
            .collect()
    }

    /// `team_id`'s channels, as the switcher lists them, built again only
    /// when its sidebar has changed since last time (#793). Channel rows
    /// read nothing but the sections, so these are the same while those
    /// are.
    fn switcher_rows(&self, team_id: &str) -> Arc<[ChannelRow]> {
        let Some(sections) = self.sections.get(team_id) else {
            return Arc::from([]);
        };
        let mut cache = self.switcher_rows.borrow_mut();
        if let Some((from, rows)) = cache.get(team_id)
            && Arc::ptr_eq(from, sections)
        {
            return rows.clone();
        }
        let rows: Arc<[ChannelRow]> = self
            .categories(sections)
            .into_iter()
            .flat_map(|category| category.rows)
            .filter(|row| matches!(row.kind, ChannelKind::Public | ChannelKind::Private))
            .collect();
        cache.insert(team_id.to_string(), (sections.clone(), rows.clone()));
        rows
    }

    /// Opens `channel_id` once some team's sidebar holds it, as after
    /// switching to a server whose cache has not loaded yet (#206).
    pub fn open_when_ready(&mut self, channel_id: String) {
        self.ready = Some(channel_id);
    }

    /// Whether the user is on `team_id`.
    pub fn on_team(&self, team_id: &str) -> bool {
        self.teams.iter().any(|team| team.id == team_id)
    }

    /// The user's teams as the rail shows them, in their order, with the
    /// shown one.
    pub(super) fn rail_teams(&self) -> (Vec<crate::teams::Team>, Option<SharedString>) {
        let teams = self
            .teams
            .iter()
            .map(|team| {
                let rail = crate::teams::Team::new(team.id.clone(), team.display_name.clone())
                    .icon(team.icon)
                    .mentions(usize::try_from(team.mentions).unwrap_or_default());
                if team.has_unreads {
                    rail.unread()
                } else {
                    rail
                }
            })
            .collect();
        (teams, self.team.clone().map(Into::into))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use matty_sync::ChangeSet;

    use super::super::tests::{channel, live, section, team};

    fn teams() -> ChangeSet {
        let mut t2 = team("t2");
        t2.has_unreads = true;
        t2.mentions = 3;
        t2.icon = 42;
        ChangeSet::Unreads {
            teams: Arc::from(vec![team("t1"), t2]),
        }
    }

    fn sidebars(live: &mut super::Live) {
        live.apply(&ChangeSet::Sidebar {
            team_id: "t1".into(),
            sections: Arc::from(vec![section(
                "c",
                vec![channel("general", "O", 0, 0), channel("dm", "D", 0, 0)],
            )]),
        });
        live.apply(&ChangeSet::Sidebar {
            team_id: "t2".into(),
            sections: Arc::from(vec![section(
                "c",
                vec![channel("design", "O", 4, 3), channel("dm", "D", 0, 0)],
            )]),
        });
    }

    #[test]
    fn the_rail_gets_every_team_with_its_badges() {
        let mut live = live();
        let update = live.apply(&teams());
        let (rail, current) = update.teams.unwrap();
        assert_eq!(current.as_deref(), Some("t1"));
        let summary: Vec<_> = rail
            .iter()
            .map(|t| (t.id.as_ref(), t.name.as_ref(), t.unread, t.mentions, t.icon))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("t1", "Team t1", false, 0, 0),
                ("t2", "Team t2", true, 3, 42)
            ]
        );
    }

    #[test]
    fn switching_shows_the_other_teams_sidebar_threads_and_name() {
        let mut live = live();
        sidebars(&mut live);
        live.apply(&teams());
        assert_eq!(live.team(), Some("t1"));
        assert!(live.row("general").is_some() && live.row("design").is_none());

        let update = live.switch_team("t2").unwrap();
        assert_eq!(live.team(), Some("t2"));
        assert_eq!(update.team_name.as_deref(), Some("Team t2"));
        let rows: Vec<_> = update.categories.unwrap()[0]
            .rows
            .iter()
            .map(|row| (row.id.to_string(), row.mentions))
            .collect();
        assert_eq!(rows, vec![("design".into(), 3), ("dm".into(), 0)]);
        assert!(update.threads.unwrap().loading, "the list comes from sync");
        assert_eq!(update.teams.unwrap().1.as_deref(), Some("t2"));
        assert!(live.row("design").is_some() && live.row("general").is_none());

        // The shown team's sidebar changing reaches the window; the other's
        // does not.
        let t1 = ChangeSet::Sidebar {
            team_id: "t1".into(),
            sections: Arc::from(vec![section("c", vec![channel("general", "O", 1, 0)])]),
        };
        assert_eq!(live.apply(&t1).categories, None);
        // Only the shown team's Threads list reaches the Threads view.
        let list = |team: &str| ChangeSet::ThreadList {
            team_id: team.into(),
            threads: Arc::from(vec![]),
            roots: Arc::from(vec![]),
            more: false,
            failed: false,
        };
        assert_eq!(live.apply(&list("t1")).threads, None);
        assert!(live.apply(&list("t2")).threads.is_some());

        // Fresh counts keep the picked team on screen.
        live.apply(&teams());
        assert_eq!(live.team(), Some("t2"));
        // Switching to the shown team, or one the user is not on, does nothing.
        assert!(live.switch_team("t2").is_none());
        assert!(live.switch_team("t9").is_none());
    }

    #[test]
    fn leaving_the_shown_team_falls_back_to_the_first() {
        let mut live = live();
        sidebars(&mut live);
        live.apply(&teams());
        live.switch_team("t2").unwrap();
        let update = live.apply(&ChangeSet::Unreads {
            teams: Arc::from(vec![team("t1")]),
        });
        assert_eq!(live.team(), Some("t1"));
        assert_eq!(update.teams.unwrap().1.as_deref(), Some("t1"));
        assert_eq!(update.categories.unwrap()[0].rows[0].id.as_ref(), "general");
    }

    #[test]
    fn the_switcher_gets_the_other_teams_channels() {
        let mut live = live();
        sidebars(&mut live);
        live.apply(&teams());
        let others = live.other_teams();
        assert_eq!(others.len(), 1);
        assert_eq!(
            (others[0].id.as_ref(), others[0].name.as_ref()),
            ("t2", "Team t2")
        );
        let rows: Vec<_> = others[0]
            .rows
            .iter()
            .map(|row| (row.id.as_ref(), row.unread, row.mentions))
            .collect();
        assert_eq!(rows, vec![("design", true, 3)], "no direct messages");
    }

    #[test]
    fn the_switchers_rows_are_built_again_only_when_that_sidebar_changes() {
        let mut live = live();
        sidebars(&mut live);
        live.apply(&teams());
        let first = live.switcher_rows("t2");
        assert!(Arc::ptr_eq(&first, &live.switcher_rows("t2")));
        live.apply(&ChangeSet::Sidebar {
            team_id: "t2".into(),
            sections: Arc::from(vec![section("c", vec![channel("motion", "O", 0, 0)])]),
        });
        let ids: Vec<_> = live.other_teams()[0]
            .rows
            .iter()
            .map(|row| row.id.to_string())
            .collect();
        assert_eq!(ids, ["motion"]);
    }

    #[test]
    fn a_channel_asked_for_opens_once_any_teams_sidebar_holds_it() {
        let mut live = live();
        live.apply(&teams());
        live.open_when_ready("design".into());
        // The shown team's sidebar does not hold it.
        let update = live.apply(&ChangeSet::Sidebar {
            team_id: "t1".into(),
            sections: Arc::from(vec![section("c", vec![channel("general", "O", 0, 0)])]),
        });
        assert_eq!(update.ready, None);
        let update = live.apply(&ChangeSet::Sidebar {
            team_id: "t2".into(),
            sections: Arc::from(vec![section("c", vec![channel("design", "O", 0, 0)])]),
        });
        assert_eq!(update.ready.as_deref(), Some("design"));
    }

    #[test]
    fn a_channel_belongs_to_the_team_whose_sidebar_holds_it() {
        let mut live = live();
        sidebars(&mut live);
        live.apply(&teams());
        assert_eq!(live.team_of("general"), Some("t1"));
        assert_eq!(live.team_of("design"), Some("t2"));
        // Direct messages are in every team's sidebar: the shown one keeps them.
        assert_eq!(live.team_of("dm"), Some("t1"));
        assert_eq!(live.team_of("elsewhere"), None);
        assert!(live.on_team("t2") && !live.on_team("t9"));
    }

    #[test]
    fn the_join_dialog_hears_the_open_teams_and_a_refused_join() {
        use crate::directory::JoinableTeams;
        use matty_sync::CommandError;
        let mut live = live();
        let team: matty_api::models::Team = serde_json::from_value(serde_json::json!({
            "id": "t9", "display_name": "Platform", "description": "  Builds  "
        }))
        .unwrap();
        let update = live.apply(&ChangeSet::JoinableTeams(Arc::from(vec![team])));
        let Some(JoinableTeams::Loaded(teams)) = update.joinable else {
            panic!("the dialog gets the list");
        };
        assert_eq!(teams[0].name.as_ref(), "Platform");
        assert_eq!(teams[0].description.as_deref(), Some("Builds"));

        let update = live.apply(&ChangeSet::Failed(CommandError::Offline {
            action: "browse teams",
        }));
        assert_eq!(update.joinable, Some(JoinableTeams::Unavailable));

        let update = live.apply(&ChangeSet::Failed(CommandError::Server {
            action: "join team",
            target: "t9".into(),
            error: "You do not have the appropriate permissions. (api.context.permissions.app_error, 403)".into(),
        }));
        assert_eq!(
            update.refused.as_deref(),
            Some("Couldn't join the team: You do not have the appropriate permissions.")
        );

        let update = live.apply(&ChangeSet::TeamJoined {
            team_id: "t9".into(),
        });
        assert_eq!(update.team_joined.as_deref(), Some("t9"));
    }
}
