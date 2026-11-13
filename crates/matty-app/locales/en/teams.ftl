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

## Joining and leaving teams (#745)

# The Join a team dialog: the server's open teams, then an invite link.
teams-join-title = Join a team
teams-join-description = Teams on { $server } that anyone can join.
teams-join-search = Search teams
teams-join-loading = Loading teams…
teams-join-unavailable = Couldn't load the teams. You can still join with an invite link.
teams-join-none = No other teams on { $server } are open to join. Ask a team admin for an invite link.
teams-join-no-match = No teams match “{ $query }”
teams-join-button = Join
teams-join-team = Join { $team }
teams-invite-label = Have an invite link?
teams-invite-placeholder = Paste an invite link
teams-invite-join = Join
teams-invite-invalid = That isn't an invite link. Invite links end in ?id= and a code.
teams-invite-other-server = That link is for { $host }. Switch to that server, or add it, to join.
# A team's menu: right-click on a tab, or ⋯ in Settings.
teams-options = { $team } options
teams-leave = Leave team…
teams-leave-title = Leave { $team }?
teams-leave-detail = You'll leave all of its channels. To come back, join it again or ask for an invite.
teams-leave-confirm = Leave team
# What the server refused.
teams-join-failed = Couldn't join the team: { $error }
teams-leave-failed = Couldn't leave the team: { $error }
