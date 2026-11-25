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
teams-join-title = Join or create a team
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

## Invite people (#962)

teams-invite = Invite people…
teams-invite-to = Invite people to { $team }…

## Create a team (#971)

teams-create-label = Start a new team
teams-create-placeholder = Team name
teams-create-button = Create
teams-create-url = Its address: { $url }
teams-create-short = Use at least two letters.
teams-create-failed = Couldn't create the team: { $error }

## Team settings (#987)

teams-settings = Team settings…
team-settings-title = Team settings
team-settings-detail = Everyone on the team sees these.
team-settings-name = Team name
team-settings-name-placeholder = Team name
team-settings-description = Description
team-settings-description-placeholder = What the team is for
team-settings-open = Anyone on the server can join
team-settings-open-detail = The team shows in Join a team for everyone with an account. Off, people need an invite.
team-settings-invite = Invite link
team-settings-loading = Loading…
team-settings-regenerate = Regenerate
team-settings-regenerate-detail = Regenerating stops the old link from working.
team-settings-failed = Couldn't change the team's settings: { $error }
team-settings-regenerate-failed = Couldn't regenerate the invite link: { $error }

## Team members (#988)

teams-view-members = View members
teams-manage-members = Manage members
team-members-title = Members of { $team }
team-members-manage-title = Manage members of { $team }
team-members-count = { $count ->
        [one] { $count } member
       *[other] { $count } members
    }
team-members-make-admin = Make admin
team-members-remove-admin = Remove admin
team-members-remove = Remove { $name } from the team
team-members-close = Close
team-members-failed = Couldn't change the team's members: { $error }
user-group-description = @{ $name } · { $count ->
    [one] 1 member
   *[other] { $count } members
}
user-groups-title = User groups
user-groups-description = Mention everyone in a group at once with its @name.
user-groups-all = All groups
user-groups-mine = My groups
user-groups-search = Search groups
user-groups-members = { $count ->
    [one] 1 member
   *[other] { $count } members
}
user-groups-none = This server has no groups yet.
user-groups-none-mine = You're not in any groups.
user-groups-none-match = No groups match “{ $query }”.
user-groups-create = Create group
group-editor-create-title = Create a group
group-editor-edit-title = Edit { $name }
group-editor-description = A group lets people mention everyone in it at once.
group-editor-name = Name
group-editor-name-placeholder = Design team
group-editor-handle = Mention
group-editor-handle-placeholder = design-team
group-editor-handle-hint = People mention the group with @{ $handle }.
group-editor-name-empty = Give the group a name.
group-editor-handle-empty = Give the group a mention, like design-team.
group-editor-too-long = Use { $max } characters or fewer.
group-editor-handle-characters = Use lowercase letters, numbers, dots, dashes and underscores.
group-editor-handle-taken = @{ $handle } is already taken.
group-editor-people = { $count ->
    [0] People
    [one] People · 1 picked
   *[other] People · { $count } picked
}
group-editor-nobody = Pick at least one person.
group-editor-archive = Archive group
group-editor-archive-confirm = Confirm archive
group-editor-cancel = Cancel
group-editor-save = Save
group-editor-create = Create group
team-members-edit-group = Edit group
group-create-failed = Couldn't create the group: { $error }
group-change-failed = Couldn't change the group: { $error }
group-editor-archive-hint = Archiving the group stops its @mention from reaching anyone.
add-members-group = Add everyone in { $name }
add-members-group-pick = { $count ->
    [one] Add its 1 member
   *[other] Add its { $count } members
}
add-members-group-finding = Finding its people…

## Integrations (#1071)
teams-integrations = Integrations…
integrations-title = Integrations
integrations-description-text = Incoming webhooks post to a channel from other apps. Slash commands send what follows them to another app.
integrations-hooks = Incoming webhooks
integrations-commands = Slash commands
integrations-loading = Loading…
integrations-no-hooks = No incoming webhooks yet.
integrations-no-commands = No custom slash commands yet.
integrations-untitled = Untitled webhook
integrations-channel-unknown = a channel you're not in
integrations-hook-detail = Posts to { $channel }
integrations-hook-detail-described = Posts to { $channel } · { $description }
integrations-delete = Delete
integrations-delete-confirm = Confirm delete
integrations-keep = Keep
integrations-add-hook = Add incoming webhook
integrations-add-hook-text = Anything that sends a post to its URL shows up in the channel.
integrations-add-command = Add slash command
integrations-add-command-text = Typing the command sends it, and what follows it, to your app's URL.
integrations-name = Name
integrations-name-placeholder = Build alerts
integrations-description = Description
integrations-description-placeholder = What it is for
integrations-channel = Channel
integrations-channel-placeholder = Pick a channel
integrations-channel-missing = Pick the channel its posts land in
integrations-trigger = Command
integrations-trigger-placeholder = deploy
integrations-trigger-hint = Typed after a slash, as in /deploy.
integrations-trigger-empty = Type the word that follows the slash
integrations-trigger-slash = Leave out the slash
integrations-trigger-space = Use one word, without spaces
integrations-url = Request URL
integrations-url-placeholder = https://example.com/command
integrations-url-hint = Where the server sends the command.
integrations-url-invalid = Type a URL that starts with http:// or https://
integrations-method = Request method
integrations-autocomplete = List it in the slash command menu
integrations-hint = Argument hint
integrations-hint-placeholder = [service] [version]
integrations-save = Save
integrations-done = Done
integrations-copy = Copy
integrations-copied = Copied
integrations-hook-created = The webhook's URL:
integrations-hook-created-text = Anyone with this URL can post to the channel, so share it only with the app that posts.
integrations-command-created = The command's token:
integrations-command-created-text = The server sends this token with each command, so your app can check it came from Mattermost.
integrations-failed = Couldn't load integrations: { $error }
