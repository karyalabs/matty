## The team switcher in the sidebar header (teams.rs)

# A team's accessible name and what is unread in it.
teams-team-unread = { $team }, unread messages
teams-team-mentions = { $team }, { $count ->
        [one] { $count } mention
       *[other] { $count } mentions
    }
# The header button: its name, with what is unread elsewhere.
teams-switch = Switch team or server
# The tiles after the team button: a team's tooltip says what waits there.
teams-tile-unread = unread
teams-tile-mentions = { $count ->
        [one] { $count } mention
       *[other] { $count } mentions
    }
# The "+N" tile for teams that do not fit.
teams-more = { $count ->
        [one] { $count } more team
       *[other] { $count } more teams
    }
# The round tile for another server with something waiting.
teams-server-unread = { $server }, unread messages
teams-server-mentions = { $server }, { $count ->
        [one] { $count } mention
       *[other] { $count } mentions
    }
teams-menu = Teams and servers
# Upper-case section labels in the menu.
teams-on-server = TEAMS ON { $server }
teams-other-servers = OTHER SERVERS
teams-join = Join another team…
teams-add-server = Add server…
teams-manage = Manage servers…
teams-manage-hint = Settings
