## Browse channels, New message, Create channel and Add members (directory.rs)

directory-cancel = Cancel
directory-search-channels = Search channels
directory-search-people = Search people
directory-nobody-matches = Nobody matches “{ $query }”

browse-title = Browse channels
browse-description = { $count ->
        [one] { $count } public channel in { $team }
       *[other] { $count } public channels in { $team }
    }
browse-members = { $count ->
        [one] { $count } member
       *[other] { $count } members
    }
browse-view = View
browse-join = Join
browse-open-channel = Open { $channel }
browse-join-channel = Join { $channel }
browse-no-match = No channels match “{ $query }”

new-message-title = New message
new-message-description = Pick someone to message, or tick up to 7 people for a group message.
new-message-person = Message { $name }
new-message-add-to-group = Add { $name } to the group
new-message-group-hint = Tick people to start a group message.
new-message-group-room = { $count } more can join.
new-message-group-full = The group is full.
new-message-go = { $count ->
        [0] Go
        [one] Message 1 person
       *[other] Message { $count } people
    }

create-channel-title = Create channel
create-channel-description = Channels are where the team talks about one thing.
create-channel-name = Name
create-channel-name-hint = Lowercase letters, numbers and dashes; the URL follows the name.
create-channel-address = Address: ~{ $handle }
create-channel-purpose = Purpose
create-channel-purpose-placeholder = What is this channel for?
create-channel-purpose-hint = Optional. Shown in Browse channels and the empty state.
create-channel-visibility = Visibility
create-channel-public = Public: anyone in the team can find and join it
create-channel-private = Private: only people you add can see it
create-channel-create = Create

add-members-title = Add members
add-members-description = They join #{ $channel } and see its history.
add-members-loading = Loading…
add-members-everyone-here = Everyone on the team is already here
invite-title = Invite people to { $team }
invite-description = They join { $team } and can find its public channels.
invite-everyone-here = Everyone on the server is already on this team
invite-emails-label = Invite by email
invite-emails-placeholder = ada@example.com, sam@example.com
invite-emails-hint = Separate addresses with commas. Each gets an email with a link to join.
invite-copy-link = Copy invite link
invite-link-copied = Invite link copied
invite-send = { $count ->
        [0] Invite
        [one] Invite { $count }
       *[other] Invite { $count }
    }
add-members-add = { $count ->
        [0] Add
        [one] Add { $count } person
       *[other] Add { $count } people
    }
