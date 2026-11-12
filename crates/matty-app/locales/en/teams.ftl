## The server switcher and team tabs in the sidebar header (teams.rs)

# A team's accessible name and what is unread in it.
teams-team-unread = { $team }, unread messages
teams-team-mentions = { $team }, { $count ->
        [one] { $count } mention
       *[other] { $count } mentions
    }
# The header button: its tooltip, and its name with what waits on other servers.
teams-switch = Switch server
teams-elsewhere-unread = { $server }, unread messages on other servers
teams-elsewhere-mentions = { $server }, { $count ->
        [one] { $count } mention on other servers
       *[other] { $count } mentions on other servers
    }
# The team tabs, the More tab for teams that do not fit, and its menu.
teams-tabs = Teams
teams-more-tab = More
teams-more-label = More teams
teams-join-short = Join another team
# The server menu.
teams-join = Join another team…
teams-join-on = Join a team on { $server }…
teams-add-server = Add a server…
teams-manage = Manage servers…
