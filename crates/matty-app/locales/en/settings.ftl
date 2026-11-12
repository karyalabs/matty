## Settings (main_window/settings_pages.rs, settings_view.rs)

prefs-title = Settings
prefs-done = Done
prefs-sections = Settings sections
prefs-synced = Synced with your Mattermost account

prefs-section-general = General
prefs-section-appearance = Appearance
prefs-section-messages = Messages
prefs-section-notifications = Notifications
prefs-section-servers = Servers
prefs-section-keyboard = Keyboard
prefs-section-advanced = Advanced

prefs-summary-general = How Matty starts, where files go and how it updates.
prefs-summary-general-web = Which version of Matty this is.
prefs-summary-appearance = How Matty looks on this computer.
prefs-summary-messages = How messages read and how you write them.
prefs-summary-notifications = What notifies you, and how.
prefs-summary-servers = The servers you're signed in to, and your teams.
prefs-summary-keyboard = Every shortcut. Cmd is Ctrl on Linux and Windows.
prefs-summary-advanced = Logs, the cache, and starting over.
prefs-this-computer = This computer only
prefs-cancel = Cancel

settings-search = Search settings
settings-search-results = Search results
settings-search-none = No settings match “{ $query }”
settings-search-none-detail = Try another word, or press Esc to go back.
settings-signed-in-as = Signed in as { $name } on { $host }
settings-version = Matty { $version }
settings-version-beta = Matty { $version } (Beta)

## General

prefs-window = Startup and window
prefs-close-to-tray = Keep running when closed
prefs-close-to-tray-detail = Closing the window leaves Matty in the tray.
prefs-launch-at-login = Open at login
prefs-launch-at-login-detail = Start Matty when you sign in.
prefs-files = Files
prefs-download-folder = Download folder
prefs-download-folder-default = Your Downloads folder
prefs-download-folder-change = Change…
prefs-download-folder-change-label = Change download folder
prefs-download-folder-choose = Choose

## Appearance

prefs-theme = Theme
prefs-mode = Mode
prefs-theme-system = System
prefs-theme-light = Light
prefs-theme-dark = Dark
prefs-theme-from-mattermost = Your Mattermost theme decides this itself.
prefs-theme-detail = System follows your computer's setting.
prefs-light-dark = Light or dark
prefs-colors-graphite = Graphite
prefs-colors-porcelain = Porcelain
prefs-colors-harbor = Harbor
prefs-colors-linen = Linen
prefs-colors-tide = Tide
prefs-colors-dusk = Dusk
prefs-colors-ember = Ember
prefs-theme-about-graphite = Quiet ink on grey
prefs-theme-about-porcelain = Cool white, lapis accent
prefs-theme-about-harbor = Navy sidebar, blue accent
prefs-theme-about-linen = Warm paper, clay accent
prefs-theme-about-tide = Sea-tinted, teal accent
prefs-theme-about-dusk = Violet-tinted
prefs-theme-about-ember = Dark sidebar, orange accent
prefs-colors-custom = Custom theme
prefs-accent = Accent
prefs-accent-detail = Buttons, links, badges and the focus ring.
prefs-accent-hex = Accent as a hex color
prefs-accent-hex-detail = Type or paste a hex color, such as your team's brand color.
prefs-accent-adjusted = Matty moved this color a little where a button label or a link would be hard to read.
prefs-accent-ink = Ink
prefs-accent-lapis = Lapis
prefs-accent-blue = Blue
prefs-accent-teal = Teal
prefs-accent-green = Green
prefs-accent-amber = Amber
prefs-accent-orange = Orange
prefs-accent-clay = Clay
prefs-accent-rose = Rose
prefs-accent-violet = Violet
prefs-greys = Background
prefs-greys-detail = The tint of the window, panels and lines.
prefs-greys-neutral = Neutral
prefs-greys-cool = Cool
prefs-greys-warm = Warm
prefs-greys-tinted = Tinted
prefs-corners = Corners
prefs-corners-square = Square
prefs-corners-soft = Soft
prefs-corners-round = Round
prefs-sidebar-fill = Sidebar
prefs-sidebar-fill-detail = Dark and Accent keep a dark sidebar beside a light window.
prefs-sidebar-fill-soft = Soft
prefs-sidebar-fill-tinted = Tinted
prefs-sidebar-fill-dark = Dark
prefs-sidebar-fill-accent = Accent
prefs-contrast = Contrast
prefs-contrast-detail = High makes text and outlines stronger.
prefs-contrast-standard = Standard
prefs-contrast-high = High
prefs-preview = Preview
prefs-preview-detail = This theme in light and in dark, with the states you meet most.
prefs-preview-threads = Threads
prefs-preview-channels = Channels
prefs-preview-general = general
prefs-preview-town-square = town-square
prefs-preview-design = design
prefs-preview-random = random
prefs-preview-archive = archive
prefs-preview-ada = Ada Lovelace
prefs-preview-sam = Sam Okafor
prefs-preview-message = The notes are in
prefs-preview-link = the plan
prefs-preview-mention = @you
prefs-preview-reply = can you take a look?
prefs-preview-save = Save
prefs-preview-cancel = Cancel
prefs-preview-saved = Saved
prefs-preview-failed = Not sent
prefs-preview-composer = Write to general
prefs-preview-topic = Selected, unread, hovered and muted rows
prefs-readability = Readability
prefs-readability-good = Readable
prefs-readability-passes = All { $count } color pairs pass WCAG AA in light and dark. The closest is { $pair }, at { $ratio }:1.
prefs-readability-high = All { $count } color pairs pass in light and dark, and text reaches at least { $ratio }:1 ({ $pair }).
prefs-readability-failing = { $count ->
        [one] One color pair is
       *[other] { $count } color pairs are
    } below WCAG AA, starting with { $pair }.
prefs-text = Text
prefs-styles = Themes
prefs-styles-name = Theme name
prefs-styles-default-name = My theme
prefs-styles-saved = Saved as “{ $name }”.
prefs-styles-full = You can keep up to { $count } themes. Delete one to save another.
prefs-styles-deleted = Deleted “{ $name }”.
prefs-styles-paste = Paste theme
prefs-styles-open = Open file…
prefs-styles-copy = Copy theme
prefs-styles-export = Save to file…
prefs-styles-copied = Copied. Anyone can paste it into Matty's themes.
prefs-styles-file-saved = Saved { $file }.
prefs-styles-file-failed = Matty couldn't save that file.
prefs-styles-not-a-style = That isn't a Matty theme or a Mattermost theme.
prefs-styles-too-new = That theme is from a newer Matty. Update Matty to use it.
prefs-styles-copy-name = { $name } copy
prefs-styles-imported-name = Imported theme
prefs-styles-renamed = Renamed to “{ $name }”.
prefs-style = Theme
prefs-style-from-mattermost = { $name } comes from your Mattermost account
prefs-style-mattermost = Your Mattermost theme
prefs-style-preset = Built in
prefs-style-saved = Saved by you
prefs-style-unsaved = Not saved yet
prefs-style-card-selected = { $name }, selected
prefs-style-options = { $name } options
prefs-style-new = New theme from { $name }
prefs-style-share = Share
prefs-style-rename = Rename…
prefs-style-delete = Delete…
prefs-style-rename-title = Rename theme
prefs-style-rename-confirm = Rename
prefs-style-delete-title = Delete “{ $name }”?
prefs-style-delete-detail = The theme goes from this computer. Anyone you shared it with keeps their copy.
prefs-style-delete-confirm = Delete
prefs-customise = Edit { $name }
prefs-customise-forks = Changes go to a copy
prefs-contrast-tag = Hard to read
prefs-contrast-warning = { $count ->
        [one] One color pair in this theme falls
       *[other] { $count } color pairs in this theme fall
    } below WCAG AA contrast (4.5:1 for text, 3:1 for other UI), such as { $examples }.
prefs-contrast-pair = { $name } ({ $ratio }:1)
prefs-list-and = { $rest } and { $last }
prefs-list-separator = {", "}
prefs-text-size = Text size
prefs-text-size-option = Text size { $size }
prefs-display = Message display
prefs-display-standard = Standard
prefs-display-compact = Compact
prefs-display-standard-hint = Avatars and a name line above each message.
prefs-display-compact-hint = One line per message, with the time in the margin.
prefs-clock = Clock
prefs-clock-12 = 12-hour
prefs-clock-24 = 24-hour
prefs-names = Names
prefs-names-label = Show names as
prefs-names-nickname = Nickname
prefs-names-full = Full name
prefs-names-username = Username
prefs-names-nickname-hint = The nickname, else the full name, else the username.
prefs-names-full-hint = The full name, else the username.
prefs-names-username-hint = Usernames only.
prefs-motion = Motion
prefs-reduce-motion = Reduce motion
prefs-reduce-motion-detail = Panes and banners change without sliding.

## Messages

prefs-send = Send messages with
prefs-send-enter = Enter
prefs-send-cmd-enter = Cmd+Enter
prefs-send-ctrl-enter = Ctrl+Enter
prefs-send-code-blocks = Enter, but not in code
prefs-send-enter-hint = Shift+Enter starts a new line.
prefs-send-mod-enter-hint = Enter starts a new line.
prefs-send-code-blocks-hint = In a code block opened with ```, Enter starts a new line and { $key } sends.
prefs-link-previews = Link previews
prefs-collapse-previews = Collapse link previews
prefs-collapse-previews-detail = Previews start closed. Each message's toggle still opens them.
prefs-remote-images = Load remote images
prefs-remote-images-always = Always
prefs-remote-images-ask = Ask first
prefs-remote-images-always-hint = Images from other sites show in messages and previews, through your server's image proxy when it has one.
prefs-remote-images-ask-hint = Images from other sites wait until you click them.
prefs-threads = Threads
prefs-reading = Reading
prefs-writing = Writing
prefs-privacy = Privacy
prefs-format-bar = Formatting bar
prefs-format-bar-detail = Strikethrough, link, heading, quote and list buttons above the message box.
prefs-skin-tone = Emoji skin tone
prefs-skin-tone-detail = For hands and people in the emoji picker.
prefs-skin-tone-option = Skin tone { $tone }
prefs-collapsed-threads = Collapsed reply threads
prefs-collapsed-threads-detail = Replies stay in their thread instead of the channel.

## Notifications

prefs-notifications-offline = Notification settings show once Matty is connected to your server.
prefs-status = Status
prefs-dnd = Do not disturb
prefs-desktop = Desktop notifications
prefs-desktop-label = Notify me about
prefs-desktop-all = All messages
prefs-desktop-mention = Mentions
prefs-desktop-none = Nothing
prefs-desktop-threads = Replies in threads I follow
prefs-desktop-threads-detail = Notify for every reply, not only mentions.
prefs-sound = Sound
prefs-sound-detail = Play a sound with each notification.
prefs-notification-sound = Notification sound
prefs-notification-sound-detail = Choosing one plays it.
prefs-mentions = Mentions
prefs-first-name = My first name
prefs-first-name-detail = Your first name counts as a mention.
prefs-first-name-named = My first name, “{ $name }”
prefs-channel-wide = Channel-wide mentions
prefs-channel-wide-detail = @channel, @all and @here
prefs-keywords = Keywords
prefs-keyword-placeholder = Add a keyword
prefs-keywords-detail = Words that count as mentioning you and notify. Press Enter to add; commas add several. @username always counts.
prefs-highlights = Highlights
prefs-highlight-keywords = Highlight keywords
prefs-highlight-keywords-detail = Words that highlight a message without notifying you.

## Advanced

prefs-updates = Updates
prefs-update-channel = Update channel
prefs-update-stable = Stable
prefs-update-beta = Beta
prefs-update-stable-hint = Releases only.
prefs-update-beta-hint = Betas and alphas too, before they're released.
prefs-check-updates = Check for updates
prefs-version = Matty { $version }
prefs-update-status-unsupported = This copy of Matty is updated by your package manager.
prefs-update-web = Matty on the web runs the latest release. Reload the page to pick up a new one.
prefs-update-status-not-checked = Matty checks for updates every few hours.
prefs-update-status-checking = Checking for updates…
prefs-update-status-up-to-date = { $minutes ->
        [0] Up to date. Checked just now.
        [one] Up to date. Checked a minute ago.
       *[other] Up to date. Checked { $minutes } minutes ago.
    }
prefs-update-status-failed = The last check didn't get through. Try again.
prefs-update-status-ready = Update ready: { $version }. Restart Matty to use it.
prefs-check-updates-button = Check now
prefs-logs = Logs
prefs-log-detail = Log detail
prefs-log-detail-hint = How much goes into the logs.
prefs-logs-detail = Attach them when you report a problem.
prefs-open-logs = Open folder
prefs-open-logs-label = Open logs folder
prefs-storage = Storage
prefs-cache-size = Cache size limit
prefs-cache-size-detail = Images, files and older messages go first, least used first.
prefs-cache-mb = { $size } MB
prefs-cache-gb = { $size } GB
prefs-clear-cache = Clear cached images and files
prefs-clear-cache-detail = Your messages stay; images download again as you scroll.
prefs-clear-cache-button = Clear…
prefs-clear-cache-confirm-title = Clear cached images and files?
prefs-clear-cache-confirm-detail = Matty downloads them again when you see them. Your messages and drafts stay.
prefs-clear-cache-confirm = Clear
prefs-reset = Reset
prefs-reset-appearance = Reset appearance to defaults
prefs-reset-appearance-detail = The theme, fonts and text size. Your saved themes stay.
prefs-reset-button = Reset
prefs-reset-all = Reset all local settings
prefs-reset-all-detail = Everything on this computer except your servers, saved themes and window places.
prefs-reset-all-button = Reset…
prefs-reset-all-confirm-title = Reset all local settings?
prefs-reset-all-confirm-detail = Settings kept by your Mattermost account stay as they are.
prefs-reset-all-confirm = Reset

## Keyboard

prefs-keys-navigation = Navigation
prefs-keys-messages = Messages
prefs-keys-teams = Teams and servers
prefs-keys-window = Window
prefs-keys-teams-by-number = Team 1 to 9

## Servers

prefs-servers = Saved servers
prefs-server-detail = { $host } · Signed in as @{ $username }
prefs-server-switch = Switch to
prefs-add-server = Add server…
prefs-server-options = { $server } options
prefs-server-move-up = Move up
prefs-server-move-down = Move down
prefs-server-sign-out = Sign out…
prefs-server-remove = Remove…
prefs-remove-title = Remove { $server }?
prefs-remove-detail = This signs you out of the server and removes Matty's copy of its messages, files and drafts from this computer.
prefs-remove = Remove
prefs-sign-out-title = Sign out of { $server }?
prefs-sign-out-detail = This removes Matty's copy of your messages, files and drafts from this computer. You'll sign in again next time.
prefs-sign-out = Sign out
prefs-teams = Teams on { $server }
prefs-teams-detail = Your teams on this server, in the order the team menu lists them.
prefs-team-current = Current team
prefs-team-switch = Switch to
prefs-join-team = Join another team…

## Fonts (#706)
prefs-font-interface = Interface and messages
prefs-font-code = Code
prefs-font-default = { $family } (default)
prefs-font-missing = “{ $family }” isn’t on this computer, so Matty uses IBM Plex instead.
prefs-font-add = Add font file…
prefs-font-add-choose = Add font
prefs-font-add-hint = A .ttf or .otf file. Matty keeps a copy and loads it every time it starts.
prefs-font-add-not-a-font = That file isn’t a font Matty can read. Choose a .ttf or .otf file.
prefs-font-add-failed = Matty couldn’t add that font. Try again, or choose another file.
