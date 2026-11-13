//! Every action from PLAN §4.4 and its key bindings.
//!
//! Each action's doc comment names where its binding comes from: (MM) is
//! Mattermost's published shortcut, (Slack) a Slack convention, (Matty) our
//! own addition. Most are declared ahead of the views that handle them; an
//! unhandled action does nothing, and the shortcut sheet (`Cmd+/`) lists them all.

use gpui_kit::{Action, App, KeyBinding};

use crate::ui::kbd::format_shortcut;

gpui_kit::actions!(
    matty,
    [
        /// Open the quick switcher. (MM)
        QuickSwitcher,
        /// Open the direct messages menu. (MM)
        DirectMessagesMenu,
        /// Browse the team's channels. (Matty)
        BrowseChannels,
        /// Create a channel. (Matty)
        CreateChannel,
        /// Open search. (MM)
        Search,
        /// Focus search when focus is not in a text field. (Slack)
        FocusSearch,
        /// Go to the previous channel in the sidebar. (MM)
        PreviousChannel,
        /// Go to the next channel in the sidebar. (MM)
        NextChannel,
        /// Go to the previous team on the rail. (MM)
        PreviousTeam,
        /// Go to the next team on the rail. (MM)
        NextTeam,
        /// Go to the previous unread channel. (MM)
        PreviousUnreadChannel,
        /// Go to the next unread channel. (MM)
        NextUnreadChannel,
        /// Go back in channel history. (MM)
        HistoryBack,
        /// Go forward in channel history. (MM)
        HistoryForward,
        /// Show only unread channels in the sidebar. (MM)
        UnreadsFilter,
        /// Show channel info in the right pane. (MM)
        ChannelInfo,
        /// Open or close the right pane. (MM)
        ToggleRightPane,
        /// Expand the right pane. (MM)
        ExpandRightPane,
        /// Show recent mentions. (MM)
        RecentMentions,
        /// Show the first unread messages of every unread channel. (Slack)
        AllUnreads,
        /// Show saved messages. (Matty)
        SavedMessages,
        /// Pick files to upload to the current channel. (MM)
        UploadFiles,
        /// Edit your last message, from an empty composer. (MM)
        EditLastMessage,
        /// Reply to the last message, from an empty composer. (MM)
        ReplyToLastMessage,
        /// React to the last message. (MM)
        ReactToLastMessage,
        /// Wrap the composer selection in bold. (MM)
        FormatBold,
        /// Wrap the composer selection in italics. (MM)
        FormatItalic,
        /// Turn the composer selection into a link. (MM)
        InsertLink,
        /// Wrap the composer selection in strikethrough. (Slack)
        FormatStrike,
        /// Show the message as it will post, above the composer. (MM)
        TogglePreview,
        /// Wrap the composer selection in inline code. (Slack)
        FormatCode,
        /// Focus the composer. (MM)
        FocusComposer,
        /// Mark the current channel read. (Matty, from Slack)
        MarkChannelRead,
        /// Mark every channel read. (Matty, from Slack)
        MarkAllRead,
        /// Show or hide the sidebar. (Slack)
        ToggleSidebar,
        /// Close the right pane. (MM)
        CloseRightPane,
        /// Zoom in. (Matty)
        ZoomIn,
        /// Zoom out. (Matty)
        ZoomOut,
        /// Reset the zoom level. (Matty)
        ResetZoom,
        /// Open Settings. (Matty)
        OpenSettings,
        /// Show the keyboard shortcut sheet. (MM)
        ShowShortcuts,
        /// Open the current channel, or the thread in the right pane, in a
        /// window of its own. (Matty, #222)
        OpenInNewWindow,
        /// Close a channel or thread window. (Matty, #222)
        CloseWindow,
        /// Open a channel beside the current one in split view. (Matty, #222)
        OpenInSplitView,
        /// Move focus to split view's other pane. (Matty, #222)
        FocusOtherPane,
        /// Swap split view's two channels. (Matty, #222)
        SwapPanes,
        /// Close split view's focused pane; the other fills the window.
        /// (Matty, #222)
        ClosePane,
    ]
);

gpui_kit::actions!(
    matty,
    [
        /// Add another server and sign in to it. (Matty, #206)
        AddServer,
        /// Switch to the first saved server. (Slack)
        SwitchServer1,
        /// Switch to the second saved server. (Slack)
        SwitchServer2,
        /// Switch to the third saved server. (Slack)
        SwitchServer3,
        /// Switch to the fourth saved server. (Slack)
        SwitchServer4,
        /// Switch to the fifth saved server. (Slack)
        SwitchServer5,
        /// Switch to the sixth saved server. (Slack)
        SwitchServer6,
        /// Switch to the seventh saved server. (Slack)
        SwitchServer7,
        /// Switch to the eighth saved server. (Slack)
        SwitchServer8,
        /// Switch to the ninth saved server. (Slack)
        SwitchServer9,
    ]
);

gpui_kit::actions!(
    matty,
    [
        /// Switch to the first team on the rail. (MM)
        SwitchTeam1,
        /// Switch to the second team on the rail. (MM)
        SwitchTeam2,
        /// Switch to the third team on the rail. (MM)
        SwitchTeam3,
        /// Switch to the fourth team on the rail. (MM)
        SwitchTeam4,
        /// Switch to the fifth team on the rail. (MM)
        SwitchTeam5,
        /// Switch to the sixth team on the rail. (MM)
        SwitchTeam6,
        /// Switch to the seventh team on the rail. (MM)
        SwitchTeam7,
        /// Switch to the eighth team on the rail. (MM)
        SwitchTeam8,
        /// Switch to the ninth team on the rail. (MM)
        SwitchTeam9,
    ]
);

/// Cmd+Option+1…9 on macOS and Ctrl+Alt+1…9 elsewhere switch to the team
/// in that place on the rail, as in Mattermost (#89). Like
/// [`server_bindings`] they stay out of [`bindings`].
pub fn team_bindings(platform: Platform) -> Vec<KeyBinding> {
    team_bindings_in(platform, MAIN_WINDOW)
}

fn team_bindings_in(platform: Platform, context: &'static str) -> Vec<KeyBinding> {
    const MACOS: [&str; 9] = [
        "cmd-alt-1",
        "cmd-alt-2",
        "cmd-alt-3",
        "cmd-alt-4",
        "cmd-alt-5",
        "cmd-alt-6",
        "cmd-alt-7",
        "cmd-alt-8",
        "cmd-alt-9",
    ];
    const OTHER: [&str; 9] = [
        "ctrl-alt-1",
        "ctrl-alt-2",
        "ctrl-alt-3",
        "ctrl-alt-4",
        "ctrl-alt-5",
        "ctrl-alt-6",
        "ctrl-alt-7",
        "ctrl-alt-8",
        "ctrl-alt-9",
    ];
    let keys = match platform {
        Platform::MacOs => MACOS,
        Platform::Other => OTHER,
    };
    let main = Some(context);
    vec![
        KeyBinding::new(keys[0], SwitchTeam1, main),
        KeyBinding::new(keys[1], SwitchTeam2, main),
        KeyBinding::new(keys[2], SwitchTeam3, main),
        KeyBinding::new(keys[3], SwitchTeam4, main),
        KeyBinding::new(keys[4], SwitchTeam5, main),
        KeyBinding::new(keys[5], SwitchTeam6, main),
        KeyBinding::new(keys[6], SwitchTeam7, main),
        KeyBinding::new(keys[7], SwitchTeam8, main),
        KeyBinding::new(keys[8], SwitchTeam9, main),
    ]
}

/// Cmd/Ctrl+1…9 switch to the saved server in that place on the rail
/// (PLAN §4.4, #206). They stay out of [`bindings`], whose rows the
/// shortcut sheet and the menu bar list one by one.
pub fn server_bindings(platform: Platform) -> Vec<KeyBinding> {
    server_bindings_in(platform, MAIN_WINDOW)
}

fn server_bindings_in(platform: Platform, context: &'static str) -> Vec<KeyBinding> {
    const MACOS: [&str; 9] = [
        "cmd-1", "cmd-2", "cmd-3", "cmd-4", "cmd-5", "cmd-6", "cmd-7", "cmd-8", "cmd-9",
    ];
    const OTHER: [&str; 9] = [
        "ctrl-1", "ctrl-2", "ctrl-3", "ctrl-4", "ctrl-5", "ctrl-6", "ctrl-7", "ctrl-8", "ctrl-9",
    ];
    let keys = match platform {
        Platform::MacOs => MACOS,
        Platform::Other => OTHER,
    };
    let main = Some(context);
    vec![
        KeyBinding::new(keys[0], SwitchServer1, main),
        KeyBinding::new(keys[1], SwitchServer2, main),
        KeyBinding::new(keys[2], SwitchServer3, main),
        KeyBinding::new(keys[3], SwitchServer4, main),
        KeyBinding::new(keys[4], SwitchServer5, main),
        KeyBinding::new(keys[5], SwitchServer6, main),
        KeyBinding::new(keys[6], SwitchServer7, main),
        KeyBinding::new(keys[7], SwitchServer8, main),
        KeyBinding::new(keys[8], SwitchServer9, main),
    ]
}

/// Which platform a binding table is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    MacOs,
    Other,
}

impl Platform {
    /// The platform whose keymap Matty uses. Native builds know it at
    /// compile time; in a browser it is the visitor's OS, so a Mac browser
    /// gets the Cmd keymap and hints like the desktop app.
    pub fn current() -> Self {
        #[cfg(target_family = "wasm")]
        {
            thread_local! {
                static CURRENT: Platform = Platform::from_browser();
            }
            CURRENT.with(|platform| *platform)
        }
        #[cfg(not(target_family = "wasm"))]
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Other
        }
    }

    /// Reads the OS from the browser the way GPUI's web platform does, so
    /// the keymap agrees with the modifiers GPUI reports: Macs, and iPads
    /// and iPhones with a keyboard, use Cmd.
    #[cfg(target_family = "wasm")]
    fn from_browser() -> Self {
        let Some(navigator) = web_sys::window().map(|window| window.navigator()) else {
            return Self::Other;
        };
        let user_agent = navigator.user_agent().unwrap_or_default();
        #[allow(deprecated)]
        let platform = navigator.platform().unwrap_or_default();
        Self::from_user_agent(&user_agent, &platform)
    }

    /// Whether a browser with this user agent and `navigator.platform` runs
    /// on an Apple OS whose shortcuts use Cmd.
    #[cfg_attr(not(target_family = "wasm"), allow(dead_code))]
    fn from_user_agent(user_agent: &str, platform: &str) -> Self {
        let apple = ["Mac", "iPhone", "iPad", "iPod"];
        if user_agent.contains("Android") {
            Self::Other
        } else if apple
            .iter()
            .any(|name| platform.contains(name) || user_agent.contains(name))
        {
            Self::MacOs
        } else {
            Self::Other
        }
    }

    /// The operating system gpuicn formats shortcuts for.
    pub fn os(self) -> gpui_kit::OperatingSystem {
        match self {
            Self::MacOs => gpui_kit::OperatingSystem::Mac,
            Self::Other => gpui_kit::OperatingSystem::Linux,
        }
    }
}

/// One row of the keymap: an action, its label in the shortcut sheet, the
/// key context it applies in, and its keystrokes on macOS and elsewhere.
pub struct Binding {
    pub label: &'static str,
    pub action: fn() -> Box<dyn Action>,
    bind: fn(&'static str, Option<&'static str>) -> KeyBinding,
    pub context: Option<&'static str>,
    pub macos: &'static str,
    pub other: &'static str,
}

/// Where Settings > Keyboard lists a binding (#732).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyGroup {
    Navigation,
    Messages,
    TeamsAndServers,
    Window,
}

impl Binding {
    /// The Keyboard page's group for this binding.
    pub fn group(&self) -> KeyGroup {
        match self.label {
            "Quick switcher"
            | "Direct messages"
            | "Browse channels"
            | "Create channel"
            | "Search"
            | "Focus search"
            | "Previous channel"
            | "Next channel"
            | "Previous unread channel"
            | "Next unread channel"
            | "Back"
            | "Forward"
            | "Unreads filter"
            | "Channel info"
            | "Recent mentions"
            | "All unreads"
            | "Saved messages" => KeyGroup::Navigation,
            "Previous team" | "Next team" => KeyGroup::TeamsAndServers,
            "Upload files"
            | "Bold"
            | "Preview message"
            | "Italic"
            | "Link"
            | "Strikethrough"
            | "Inline code"
            | "Edit last message"
            | "Reply to last message"
            | "React to last message"
            | "Focus composer"
            | "Mark channel read"
            | "Mark all read" => KeyGroup::Messages,
            _ => KeyGroup::Window,
        }
    }
}

/// The Fluent id of a keymap or message-key label: `prefix`, then the
/// English label in kebab case, such as `keymap-quick-switcher`.
fn label_id(prefix: &str, label: &str) -> String {
    format!("{prefix}-{}", label.to_lowercase().replace(' ', "-"))
}

/// What the shortcuts sheet calls the message key labelled `label` in
/// [`MESSAGE_KEYS`], in the user's language.
pub fn message_key_name(label: &str) -> gpui_kit::SharedString {
    crate::i18n::text(&label_id("message-key", label), &[])
}

impl Binding {
    /// The row's name in the user's language, for the menu and the
    /// shortcuts sheet. `label` stays the English key that finds the row.
    pub fn name(&self) -> gpui_kit::SharedString {
        crate::i18n::text(&label_id("keymap", self.label), &[])
    }

    pub fn keys(&self, platform: Platform) -> &'static str {
        match platform {
            Platform::MacOs => self.macos,
            Platform::Other => self.other,
        }
    }
}

/// The keystrokes bound to the row labelled `label`, formatted for this
/// platform (`⌘⇧I` on macOS, `Ctrl+Shift+I` elsewhere), or `None` when the
/// keymap has no such row.
pub fn shortcut_for(label: &str) -> Option<gpui_kit::SharedString> {
    let platform = Platform::current();
    bindings()
        .into_iter()
        .find(|row| row.label == label)
        .map(|row| format_shortcut(row.keys(platform), platform.os()))
}

/// `macos` or `other`, whichever this platform uses, formatted like
/// [`shortcut_for`]; for keys the composer handles outside the keymap.
pub fn format_keys(macos: &str, other: &str) -> gpui_kit::SharedString {
    let platform = Platform::current();
    let keys = match platform {
        Platform::MacOs => macos,
        Platform::Other => other,
    };
    format_shortcut(keys, platform.os())
}

/// Tooltip text for the send button: `label` and the key that sends.
pub fn send_tooltip(label: &str, send_on: crate::settings::SendKey) -> gpui_kit::SharedString {
    let keys = match send_on {
        crate::settings::SendKey::Enter | crate::settings::SendKey::CodeBlocks => {
            format_keys("enter", "enter")
        }
        crate::settings::SendKey::ModEnter => format_keys("cmd-enter", "ctrl-enter"),
    };
    format!("{label}  {keys}").into()
}

/// Tooltip text for an icon-only control (DESIGN.md §5): `label`, followed
/// by the shortcut of the keymap row labelled `binding` when there is one.
pub fn tooltip_text(label: &str, binding: &str) -> gpui_kit::SharedString {
    match shortcut_for(binding) {
        Some(keys) => format!("{label}  {keys}").into(),
        None => label.to_string().into(),
    }
}

/// Key contexts set by views.
/// Single keys on a selected message (Shift+Tab from the composer, then
/// Up and Down), for the shortcut sheet (#213). The channel and the thread
/// pane handle them in their `selection_key`.
pub const MESSAGE_KEYS: [(&str, &str); 12] = [
    ("R", "Reply in thread"),
    ("E", "Edit your message"),
    ("Delete", "Delete your message"),
    ("+", "Add reaction"),
    ("F", "Forward"),
    ("T", "Follow or unfollow thread"),
    ("U", "Mark as unread"),
    ("S", "Save or unsave"),
    ("P", "Pin or unpin"),
    ("C", "Copy text"),
    ("L", "Copy link"),
    ("Enter", "View images"),
];

pub const MAIN_WINDOW: &str = "MainWindow";
/// [`MAIN_WINDOW`] at the depth of whatever has focus inside the window:
/// `Nowhere` is a context nothing sets. Window shortcuts are bound here a
/// second time, see [`bind`].
pub const MAIN_WINDOW_ANYWHERE: &str = "MainWindow > !Nowhere";
/// The main window with focus outside any text field.
pub const NOT_TYPING: &str = "MainWindow && !Input";
/// A reorderable list: the sidebar's, or the rail's servers (#206). With
/// focus in it, Alt+Shift+Up and Down move the focused row or category
/// instead of walking unreads.
pub const SIDEBAR_SORTABLE: &str = "SidebarSortable";
pub const CHANNEL_VIEW: &str = "ChannelView";
pub const RIGHT_PANE: &str = "RightPane";
pub const COMPOSER: &str = "Composer";
pub const COMPOSER_EMPTY: &str = "Composer && empty";
/// A channel or thread in a window of its own (#222).
pub const POP_OUT: &str = "PopOut";
/// Split view's second pane, in the main window (#222).
pub const SPLIT_PANE: &str = "SplitPane";
/// The Settings window, or the overlay on the web (#732).
pub const SETTINGS: &str = "Settings";

macro_rules! row {
    ($label:literal, $action:ident, $context:expr, $mac:literal, $other:literal) => {
        Binding {
            label: $label,
            action: || Box::new($action),
            bind: |keys, context| KeyBinding::new(keys, $action, context),
            context: $context,
            macos: $mac,
            other: $other,
        }
    };
}

/// The PLAN §4.4 table. Cmd on macOS, Ctrl elsewhere, unless noted.
pub fn bindings() -> Vec<Binding> {
    let main = Some(MAIN_WINDOW);
    vec![
        row!("Quick switcher", QuickSwitcher, main, "cmd-k", "ctrl-k"),
        row!(
            "Direct messages",
            DirectMessagesMenu,
            main,
            "cmd-shift-k",
            "ctrl-shift-k"
        ),
        row!(
            "Browse channels",
            BrowseChannels,
            main,
            "cmd-shift-j",
            "ctrl-shift-j"
        ),
        row!(
            "Create channel",
            CreateChannel,
            main,
            "cmd-shift-n",
            "ctrl-shift-n"
        ),
        row!("Search", Search, main, "cmd-shift-f", "ctrl-shift-f"),
        row!(
            "Focus search",
            FocusSearch,
            Some(NOT_TYPING),
            "cmd-f",
            "ctrl-f"
        ),
        row!(
            "Previous channel",
            PreviousChannel,
            main,
            "alt-up",
            "alt-up"
        ),
        row!("Next channel", NextChannel, main, "alt-down", "alt-down"),
        row!(
            "Previous team",
            PreviousTeam,
            main,
            "cmd-alt-up",
            "ctrl-alt-up"
        ),
        row!("Next team", NextTeam, main, "cmd-alt-down", "ctrl-alt-down"),
        row!(
            "Previous unread channel",
            PreviousUnreadChannel,
            Some("MainWindow && !SidebarSortable"),
            "alt-shift-up",
            "alt-shift-up"
        ),
        row!(
            "Next unread channel",
            NextUnreadChannel,
            Some("MainWindow && !SidebarSortable"),
            "alt-shift-down",
            "alt-shift-down"
        ),
        row!("Back", HistoryBack, main, "cmd-[", "alt-left"),
        row!("Forward", HistoryForward, main, "cmd-]", "alt-right"),
        row!(
            "Unreads filter",
            UnreadsFilter,
            main,
            "cmd-shift-u",
            "ctrl-shift-u"
        ),
        row!(
            "Channel info",
            ChannelInfo,
            main,
            "cmd-shift-i",
            "ctrl-shift-i"
        ),
        row!(
            "Toggle right pane",
            ToggleRightPane,
            main,
            "cmd-.",
            "ctrl-."
        ),
        row!(
            "Expand right pane",
            ExpandRightPane,
            main,
            "cmd-shift-.",
            "ctrl-shift-."
        ),
        row!(
            "Recent mentions",
            RecentMentions,
            main,
            "cmd-shift-m",
            "ctrl-shift-m"
        ),
        row!(
            "All unreads",
            AllUnreads,
            main,
            "cmd-shift-a",
            "ctrl-shift-a"
        ),
        row!(
            "Saved messages",
            SavedMessages,
            main,
            "cmd-shift-b",
            "ctrl-shift-b"
        ),
        row!("Upload files", UploadFiles, main, "cmd-u", "ctrl-u"),
        row!("Bold", FormatBold, Some(COMPOSER), "cmd-b", "ctrl-b"),
        row!(
            "Preview message",
            TogglePreview,
            // A reply has no preview.
            Some("ChannelView > Composer"),
            "cmd-alt-p",
            "ctrl-alt-p"
        ),
        row!("Italic", FormatItalic, Some(COMPOSER), "cmd-i", "ctrl-i"),
        row!(
            "Link",
            InsertLink,
            Some(COMPOSER),
            "cmd-alt-k",
            "ctrl-alt-k"
        ),
        row!(
            "Strikethrough",
            FormatStrike,
            Some(COMPOSER),
            "cmd-shift-x",
            "ctrl-shift-x"
        ),
        row!(
            "Inline code",
            FormatCode,
            Some(COMPOSER),
            "cmd-shift-c",
            "ctrl-shift-c"
        ),
        row!(
            "Edit last message",
            EditLastMessage,
            Some(COMPOSER_EMPTY),
            "up",
            "up"
        ),
        row!(
            "Reply to last message",
            ReplyToLastMessage,
            Some(COMPOSER_EMPTY),
            "shift-up",
            "shift-up"
        ),
        row!(
            "React to last message",
            ReactToLastMessage,
            main,
            "cmd-shift-\\",
            "ctrl-shift-\\"
        ),
        row!(
            "Focus composer",
            FocusComposer,
            main,
            "cmd-shift-l",
            "ctrl-shift-l"
        ),
        row!(
            "Mark channel read",
            MarkChannelRead,
            Some(CHANNEL_VIEW),
            "escape",
            "escape"
        ),
        row!(
            "Mark all read",
            MarkAllRead,
            Some(CHANNEL_VIEW),
            "shift-escape",
            "shift-escape"
        ),
        row!(
            "Toggle sidebar",
            ToggleSidebar,
            main,
            "cmd-shift-d",
            "ctrl-shift-d"
        ),
        row!(
            "Close right pane",
            CloseRightPane,
            Some(RIGHT_PANE),
            "escape",
            "escape"
        ),
        row!("Zoom in", ZoomIn, main, "cmd-=", "ctrl-="),
        row!("Zoom out", ZoomOut, main, "cmd--", "ctrl--"),
        row!("Reset zoom", ResetZoom, main, "cmd-0", "ctrl-0"),
        row!("Settings", OpenSettings, None, "cmd-,", "ctrl-,"),
        row!("Keyboard shortcuts", ShowShortcuts, main, "cmd-/", "ctrl-/"),
        row!(
            "Open in new window",
            OpenInNewWindow,
            main,
            "cmd-shift-o",
            "ctrl-shift-o"
        ),
        row!(
            "Close window",
            CloseWindow,
            // A pop-out closes itself; the main window closes as its red
            // button does, and the dock brings it back (#615).
            None,
            "cmd-w",
            "ctrl-w"
        ),
        row!(
            "Open in split view",
            OpenInSplitView,
            main,
            "cmd-\\",
            "ctrl-\\"
        ),
        row!("Focus other pane", FocusOtherPane, main, "f6", "f6"),
        row!("Swap panes", SwapPanes, main, "cmd-alt-\\", "ctrl-alt-\\"),
        row!("Close pane", ClosePane, main, "cmd-alt-w", "ctrl-alt-w"),
    ]
}

/// A window row's context as a predicate that matches at the depth of the
/// focused element, not of the window: `MainWindow && !Input` becomes
/// `!Input && (MainWindow > !Nowhere)`. The `!Input` stays outside the
/// `>`, where it still sees the whole stack; under it, it would see only
/// the part above the text field and match there. `None` for a row that
/// belongs to one view.
pub fn anywhere(context: &'static str) -> Option<&'static str> {
    if context == MAIN_WINDOW {
        return Some(MAIN_WINDOW_ANYWHERE);
    }
    let rest = context.strip_prefix("MainWindow && ")?;
    Some(interned(format!("{rest} && ({MAIN_WINDOW_ANYWHERE})")))
}

/// Every way a platform reports the keys a row names. A row says
/// `cmd-shift-.`, but Shift with a symbol key never arrives like that:
/// macOS and Linux report the shifted character without Shift (`cmd->`),
/// and a browser reports it with Shift (`cmd-shift->`). Bound only as
/// written, such a row works in tests, which replay the written form, and
/// nowhere else. Letters and named keys arrive as written.
pub fn spellings(keys: &'static str) -> Vec<&'static str> {
    const MODIFIERS: [&str; 5] = ["ctrl-", "alt-", "shift-", "cmd-", "fn-"];
    const SHIFTED: [(&str, &str); 21] = [
        (".", ">"),
        (",", "<"),
        ("/", "?"),
        ("\\", "|"),
        ("=", "+"),
        ("-", "_"),
        ("[", "{"),
        ("]", "}"),
        (";", ":"),
        ("'", "\""),
        ("`", "~"),
        ("1", "!"),
        ("2", "@"),
        ("3", "#"),
        ("4", "$"),
        ("5", "%"),
        ("6", "^"),
        ("7", "&"),
        ("8", "*"),
        ("9", "("),
        ("0", ")"),
    ];
    let mut written = vec![keys];
    let mut key = keys;
    let mut modifiers = Vec::new();
    while let Some(modifier) = MODIFIERS
        .iter()
        .find(|modifier| key.starts_with(**modifier) && key.len() > modifier.len())
    {
        modifiers.push(*modifier);
        key = &key[modifier.len()..];
    }
    let shifted = SHIFTED
        .iter()
        .find(|(plain, _)| *plain == key)
        .map(|(_, shifted)| *shifted);
    if let Some(shifted) = shifted
        && modifiers.contains(&"shift-")
    {
        let without: String = modifiers
            .iter()
            .filter(|modifier| **modifier != "shift-")
            .copied()
            .collect();
        written.push(interned(format!("{without}{shifted}")));
        written.push(interned(format!("{}{shifted}", modifiers.concat())));
    }
    written
}

/// `keys` for as long as the app runs, which is how long GPUI keeps a
/// binding's keys. The handful of spellings are kept once each.
fn interned(keys: String) -> &'static str {
    static KEPT: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());
    let mut kept = KEPT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(found) = kept.iter().find(|found| **found == keys) {
        return found;
    }
    let keys: &'static str = keys.leak();
    kept.push(keys);
    keys
}

/// Installs the bindings for the platform Matty runs on.
pub fn bind(cx: &mut App) {
    let platform = Platform::current();
    let (zoom_in, sidebar) = if platform == Platform::MacOs {
        ("cmd-shift-=", "cmd-b")
    } else {
        ("ctrl-shift-=", "ctrl-b")
    };
    let mut keymap = Vec::new();
    for row in bindings() {
        for keys in spellings(row.keys(platform)) {
            keymap.push((row.bind)(keys, row.context));
        }
    }
    // Cmd/Ctrl with the + key zooms in whether or not Shift is held, as in
    // every browser and editor.
    for keys in spellings(zoom_in) {
        keymap.push(KeyBinding::new(keys, ZoomIn, Some(MAIN_WINDOW)));
    }
    // Cmd/Ctrl+B is Bold while typing, as in Mattermost and Slack. Anywhere
    // else in the window it shows or hides the sidebar, as in most editors.
    keymap.push(KeyBinding::new(sidebar, ToggleSidebar, Some(NOT_TYPING)));
    keymap.extend(server_bindings(platform));
    keymap.extend(team_bindings(platform));
    cx.bind_keys(keymap);

    // GPUI gives a key to the binding whose context matches deepest, and
    // counts a binding with no context as deep as the focused element. A
    // shortcut bound on `MainWindow` matches at the window's own depth, so
    // with focus anywhere inside it lost the key to whatever else bound it:
    // a text field's editing keys, which differ by OS and in the browser
    // (Cmd+[ and Cmd+] indent on macOS, Alt+Left and Alt+Right move by word
    // on the web, Alt+Shift+Up and Down add a cursor on Linux), or a
    // toolkit-wide key such as the inspector's Ctrl+Shift+I, which took
    // Channel info away from the sidebar. Every window shortcut is bound a
    // second time where it matches at the focused element's depth, after
    // the toolkit's keys, and the later binding wins a tie: a window
    // shortcut works the same wherever focus is, whatever binds its key
    // now or later.
    let mut keymap = Vec::new();
    for row in bindings() {
        let Some(context) = row.context.and_then(anywhere) else {
            continue;
        };
        for keys in spellings(row.keys(platform)) {
            keymap.push((row.bind)(keys, Some(context)));
        }
    }
    for keys in spellings(zoom_in) {
        keymap.push(KeyBinding::new(keys, ZoomIn, Some(MAIN_WINDOW_ANYWHERE)));
    }
    keymap.push(KeyBinding::new(
        sidebar,
        ToggleSidebar,
        anywhere(NOT_TYPING),
    ));
    keymap.extend(server_bindings_in(platform, MAIN_WINDOW_ANYWHERE));
    keymap.extend(team_bindings_in(platform, MAIN_WINDOW_ANYWHERE));
    cx.bind_keys(keymap);

    // Cmd/Ctrl+F finds a setting.
    let find = if platform == Platform::MacOs {
        "cmd-f"
    } else {
        "ctrl-f"
    };
    cx.bind_keys([KeyBinding::new(find, FocusSearch, Some(SETTINGS))]);
}

#[cfg(test)]
mod tests {
    #[test]
    fn browsers_on_apple_platforms_use_the_mac_keymap() {
        let mac = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";
        assert_eq!(Platform::from_user_agent(mac, "MacIntel"), Platform::MacOs);
        let ipad = "Mozilla/5.0 (iPad; CPU OS 17_0 like Mac OS X) AppleWebKit/605.1.15";
        assert_eq!(Platform::from_user_agent(ipad, "iPad"), Platform::MacOs);
        let windows =
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/130.0.0.0";
        assert_eq!(Platform::from_user_agent(windows, "Win32"), Platform::Other);
        let linux = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/130.0.0.0";
        assert_eq!(
            Platform::from_user_agent(linux, "Linux x86_64"),
            Platform::Other
        );
        let android =
            "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 Chrome/130.0.0.0 Mobile";
        assert_eq!(
            Platform::from_user_agent(android, "Linux armv8l"),
            Platform::Other
        );
    }

    use std::collections::HashMap;

    use super::*;

    #[test]
    fn no_two_bindings_collide_per_platform() {
        for platform in [Platform::MacOs, Platform::Other] {
            let mut seen: HashMap<(&str, Option<&str>), &str> = HashMap::new();
            for row in bindings() {
                if let Some(other) = seen.insert((row.keys(platform), row.context), row.label) {
                    panic!(
                        "{platform:?}: `{}` in {:?} is bound to both {other} and {}",
                        row.keys(platform),
                        row.context,
                        row.label
                    );
                }
            }
        }
    }

    #[test]
    fn every_keystroke_parses() {
        for platform in [Platform::MacOs, Platform::Other] {
            for row in bindings() {
                for key in row.keys(platform).split(' ') {
                    gpui_kit::Keystroke::parse(key)
                        .unwrap_or_else(|e| panic!("{}: `{key}` does not parse: {e:?}", row.label));
                }
            }
        }
    }

    #[test]
    fn server_switches_take_their_place_and_no_other_shortcut() {
        for platform in [Platform::MacOs, Platform::Other] {
            let taken: Vec<_> = bindings()
                .into_iter()
                .filter(|row| row.context == Some(MAIN_WINDOW))
                .map(|row| row.keys(platform))
                .collect();
            for n in 1..=9 {
                let keys = match platform {
                    Platform::MacOs => format!("cmd-{n}"),
                    Platform::Other => format!("ctrl-{n}"),
                };
                assert!(!taken.contains(&keys.as_str()), "{keys} is taken");
            }
            assert_eq!(server_bindings(platform).len(), 9);
        }
    }

    #[test]
    fn team_switches_take_their_place_and_no_other_shortcut() {
        for platform in [Platform::MacOs, Platform::Other] {
            let taken: Vec<_> = bindings()
                .into_iter()
                .map(|row| row.keys(platform))
                .collect();
            for n in 1..=9 {
                let keys = match platform {
                    Platform::MacOs => format!("cmd-alt-{n}"),
                    Platform::Other => format!("ctrl-alt-{n}"),
                };
                assert!(!taken.iter().any(|taken| *taken == keys), "{keys} is taken");
            }
            assert_eq!(team_bindings(platform).len(), 9);
        }
    }

    /// Shift with a symbol key is bound the way each platform reports it.
    #[test]
    fn a_shifted_symbol_is_bound_as_every_platform_reports_it() {
        use super::spellings;
        assert_eq!(
            spellings("cmd-shift-."),
            ["cmd-shift-.", "cmd->", "cmd-shift->"]
        );
        assert_eq!(
            spellings("ctrl-shift-\\"),
            ["ctrl-shift-\\", "ctrl-|", "ctrl-shift-|"]
        );
        assert_eq!(
            spellings("ctrl-shift-="),
            ["ctrl-shift-=", "ctrl-+", "ctrl-shift-+"]
        );
        // Letters, named keys and unshifted symbols arrive as written.
        assert_eq!(spellings("cmd-shift-k"), ["cmd-shift-k"]);
        assert_eq!(spellings("alt-shift-up"), ["alt-shift-up"]);
        assert_eq!(spellings("cmd-["), ["cmd-["]);
        assert_eq!(spellings("ctrl--"), ["ctrl--"]);
        for keys in super::bindings()
            .iter()
            .flat_map(|row| [row.macos, row.other])
            .flat_map(spellings)
        {
            for keystroke in keys.split(' ') {
                assert!(
                    gpui_kit::Keystroke::parse(keystroke).is_ok(),
                    "{keys} does not parse"
                );
            }
        }
    }

    /// A window row is bound again at the focused element's depth, where it
    /// outranks a text field's keys. That is only right for a key no text
    /// field needs: one with Cmd, Ctrl or Alt, or a function key.
    #[test]
    fn a_window_row_matches_at_any_depth_and_never_takes_a_typing_key() {
        use super::{MAIN_WINDOW, anywhere, bindings};
        use gpui_kit::{KeyBindingContextPredicate, KeyContext};
        let stack = |names: &[&str]| -> Vec<KeyContext> {
            names
                .iter()
                .map(|name| KeyContext::parse(name).unwrap())
                .collect()
        };
        let depth = |context: &str, names: &[&str]| {
            KeyBindingContextPredicate::parse(context)
                .unwrap()
                .depth_of(&stack(names))
        };
        let composer = ["MainWindow", "ChannelView", "Composer", "Input"];
        let sidebar = ["MainWindow", "SelectionRoot", "SidebarSortable"];
        // As written a window row matches at the window; rebound, at the
        // focused element.
        assert_eq!(depth(MAIN_WINDOW, &composer), Some(1));
        assert_eq!(depth(anywhere(MAIN_WINDOW).unwrap(), &composer), Some(4));
        assert_eq!(depth(anywhere(MAIN_WINDOW).unwrap(), &sidebar), Some(3));
        // What a row leaves out stays out.
        let not_typing = anywhere(super::NOT_TYPING).unwrap();
        assert_eq!(depth(not_typing, &composer), None);
        assert_eq!(depth(not_typing, &sidebar), Some(3));
        // Another window's fields are not this window's.
        assert_eq!(
            depth(anywhere(MAIN_WINDOW).unwrap(), &["Settings", "Input"]),
            None
        );
        assert_eq!(anywhere(super::COMPOSER), None);

        for row in bindings() {
            if row.context.and_then(anywhere).is_none() {
                continue;
            }
            for keys in [row.macos, row.other] {
                let keystroke = gpui_kit::Keystroke::parse(keys).unwrap();
                let modifiers = keystroke.modifiers;
                let function_key = keystroke.key.len() > 1
                    && keystroke.key.starts_with('f')
                    && keystroke.key[1..].chars().all(|c| c.is_ascii_digit());
                assert!(
                    modifiers.control || modifiers.platform || modifiers.alt || function_key,
                    "{} ({keys}) would take a key from text fields",
                    row.label
                );
            }
        }
    }

    #[test]
    fn every_action_has_one_row() {
        let mut names: Vec<&str> = bindings().iter().map(|row| (row.action)().name()).collect();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count, "an action appears in two rows");
        assert_eq!(count, 47);
    }

    #[test]
    fn every_row_and_message_key_has_a_name() {
        for row in bindings() {
            assert_eq!(row.name(), row.label, "keymap.ftl lacks {:?}", row.label);
        }
        for (_, label) in MESSAGE_KEYS {
            assert_eq!(message_key_name(label), label, "keymap.ftl lacks {label:?}");
        }
    }
}
