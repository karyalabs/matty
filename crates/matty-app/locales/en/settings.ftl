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
prefs-theme-system = System
prefs-theme-light = Light
prefs-theme-dark = Dark
prefs-theme-from-mattermost = Your Mattermost theme decides this itself.
prefs-theme-detail = System follows your computer's setting.
prefs-light-dark = Light or dark
prefs-colors-label = Colors and corners
prefs-text = Text
prefs-colors = Colors
prefs-colors-graphite = Graphite
prefs-colors-porcelain = Porcelain
prefs-colors-mattermost = My Mattermost theme
prefs-colors-custom = Custom theme
prefs-colors-own = Your own
prefs-colors-own-hint = Your own colors and corners, light or dark as chosen above. Change them below.
prefs-accent = Accent
prefs-accent-adjusted = Some colors were adjusted slightly so text stays readable.
prefs-accent-ink = Ink
prefs-accent-lapis = Lapis
prefs-accent-blue = Blue
prefs-accent-teal = Teal
prefs-accent-green = Green
prefs-accent-amber = Amber
prefs-accent-rose = Rose
prefs-accent-violet = Violet
prefs-greys = Greys
prefs-greys-neutral = Neutral
prefs-greys-cool = Cool
prefs-greys-warm = Warm
prefs-greys-tinted = Tinted
prefs-corners = Corners
prefs-corners-square = Square
prefs-corners-soft = Soft
prefs-corners-round = Round
prefs-colors-matty-hint = Matty's own colors, light or dark as chosen above.
prefs-colors-imported-hint = { $name } from your Mattermost account, for the team on screen. It sets light or dark itself.
prefs-colors-waiting-hint = Your Mattermost theme shows once Matty connects to your server.
prefs-styles = Styles
prefs-styles-name = Style name
prefs-styles-name-placeholder = Name this style
prefs-styles-save = Save style
prefs-styles-default-name = My style
prefs-styles-saved = Saved as “{ $name }”.
prefs-styles-full = You can keep up to { $count } styles. Delete one to save another.
prefs-styles-in-use = In use
prefs-styles-use = Use
prefs-styles-use-label = Use { $name }
prefs-styles-using = Using “{ $name }”.
prefs-styles-delete-label = Delete { $name }
prefs-styles-deleted = Deleted “{ $name }”.
prefs-styles-paste = Paste style
prefs-styles-open = Open file…
prefs-styles-copy = Copy style
prefs-styles-export = Save to file…
prefs-styles-make-own = Make it my own
prefs-styles-hint = Share the style on screen, fonts included, as text or a file. Pasting also takes a Mattermost theme's colors and makes them your own.
prefs-styles-copied = Copied. Anyone can paste it into Matty's Styles.
prefs-styles-file-saved = Saved { $file }.
prefs-styles-file-failed = Matty couldn't save that file.
prefs-styles-imported = Using the pasted style as your own. Save it to keep it.
prefs-styles-imported-named = Using “{ $name }” as your own. Save it to keep it.
prefs-styles-imported-mattermost = Made your own style from that Mattermost theme. Save it to keep it.
prefs-styles-imported-mattermost-named = Made your own style from { $name }. Save it to keep it.
prefs-styles-not-a-style = That isn't a Matty style or a Mattermost theme.
prefs-styles-too-new = That style is from a newer Matty. Update Matty to use it.
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
prefs-reset-appearance-detail = The style, fonts and text size. Your saved styles stay.
prefs-reset-button = Reset
prefs-reset-all = Reset all local settings
prefs-reset-all-detail = Everything on this computer except your servers, saved styles and window places.
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
prefs-fonts = Fonts
prefs-font-interface = Interface and messages
prefs-font-code = Code
prefs-font-default = { $family } (default)
prefs-font-missing = “{ $family }” isn’t on this computer, so Matty uses IBM Plex instead.
prefs-font-add = Add font file…
prefs-font-add-choose = Add font
prefs-font-add-hint = A .ttf or .otf file. Matty keeps a copy and loads it every time it starts.
prefs-font-add-not-a-font = That file isn’t a font Matty can read. Choose a .ttf or .otf file.
prefs-font-add-failed = Matty couldn’t add that font. Try again, or choose another file.
