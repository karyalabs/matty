## The team switcher in the sidebar header (teams.rs)

# A team's accessible name and what is unread in it.
teams-team-unread = { $team }, unread messages
teams-team-mentions = { $team }, { $count ->
        [one] { $count } mention
       *[other] { $count } mentions
    }
# The header button: its name, with what is unread elsewhere.
teams-switch = Switch team or server
teams-switch-elsewhere = { $team }, { $count ->
        [0] unread messages in other teams or servers
        [one] { $count } mention in other teams or servers
       *[other] { $count } mentions in other teams or servers
    }
teams-menu = Teams and servers
# Upper-case section labels in the menu.
teams-on-server = TEAMS ON { $server }
teams-other-servers = OTHER SERVERS
teams-join = Join another team…
teams-add-server = Add server…
teams-manage = Manage servers…
teams-manage-hint = Settings
