//! When a post notifies (#910), end to end: each case sends the `posted`
//! frame the server sends, as JSON, through the sync engine and the app's
//! live state to the notification decision, with the user's and the
//! channel's `notify_props` in the cache as the server stores them. The
//! rules are the official web app's (`shouldSkipNotification`).
#![allow(clippy::unwrap_used)] // Test helpers unwrap like the tests do.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use matty_api::models::{Channel, ChannelMember, Status, Team, User};
use matty_api::ws::{RawEvent, WsEvent};
use matty_app::live::Live;
use matty_app::settings::SavedAccount;
use matty_notify::{AppState, Notification, Sound};
use matty_store::{Store, posts, sidebar};
use matty_sync::{ChangeSet, Command};
use serde_json::json;

const ME: &str = "me";
const ADA: &str = "ada-id";

/// Team "Acme" with Town Square, a muted channel, a channel with its sound
/// off, a direct message with Ada and a group message; collapsed threads on.
fn cache(path: &std::path::Path) {
    let store = Store::open(path).unwrap();
    let conn = store.conn();
    sidebar::upsert_teams(
        conn,
        &[Team {
            id: "t1".into(),
            name: "acme".into(),
            display_name: "Acme".into(),
            ..Team::default()
        }],
    )
    .unwrap();
    let channel = |id: &str, team: &str, kind: &str, display: &str| Channel {
        id: id.into(),
        team_id: team.into(),
        kind: kind.into(),
        name: id.into(),
        display_name: display.into(),
        ..Channel::default()
    };
    sidebar::upsert_channels(
        conn,
        &[
            channel("town", "t1", "O", "Town Square"),
            channel("muted", "t1", "O", "Random"),
            channel("hushed", "t1", "P", "Builds"),
            channel("dm", "", "D", "me__ada-id"),
            channel("gm", "", "G", "ada, me, sam"),
        ],
    )
    .unwrap();
    let member = |channel: &str, props: &[(&str, &str)]| ChannelMember {
        channel_id: channel.into(),
        user_id: ME.into(),
        notify_props: props
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect(),
        ..ChannelMember::default()
    };
    sidebar::upsert_channel_members(
        conn,
        &[
            member("town", &[("desktop", "default"), ("mark_unread", "all")]),
            member("muted", &[("mark_unread", "mention")]),
            member(
                "hushed",
                &[("desktop_sound", "off"), ("desktop", "default")],
            ),
            member("dm", &[]),
            member("gm", &[]),
        ],
    )
    .unwrap();
    posts::set_cursor(conn, "me", ME).unwrap();
    posts::set_cursor(conn, "collapsed_threads", "1").unwrap();
}

/// The app's live state over an engine on that cache, and its change sets
/// on a channel the test can wait on with a timeout.
struct Harness {
    live: Live,
    changes: mpsc::Receiver<ChangeSet>,
    next: usize,
    /// Do not disturb, as the window reads it from the user's own status.
    dnd: bool,
    _dir: tempfile::TempDir,
}

impl Harness {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("account.sqlite");
        cache(&path);
        let sync = matty_sync::spawn(Store::open(&path).unwrap()).unwrap();
        let account = SavedAccount {
            server: "http://localhost".into(),
            user_id: ME.into(),
            username: "arya".into(),
            display_name: "Arya".into(),
            ..SavedAccount::default()
        };
        let (live, receiver) = Live::start_with(sync, &account);
        let (sender, changes) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(change) = receiver.recv_blocking() {
                if sender.send(change).is_err() {
                    break;
                }
            }
        });
        let mut harness = Self {
            live,
            changes,
            next: 0,
            dnd: false,
            _dir: dir,
        };
        harness.prefs(&[]);
        harness
    }

    /// The user's own `notify_props`, as `/users/me` sends them.
    fn prefs(&mut self, props: &[(&str, &str)]) {
        let mut notify_props: HashMap<String, String> = [
            ("desktop", "mention"),
            ("desktop_threads", "all"),
            ("desktop_sound", "true"),
            ("channel", "true"),
            ("mention_keys", "arya,@arya"),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect();
        for (key, value) in props {
            notify_props.insert((*key).to_owned(), (*value).to_owned());
        }
        self.live.apply(&ChangeSet::Me(Arc::new(User {
            id: ME.into(),
            username: "arya".into(),
            notify_props,
            ..User::default()
        })));
    }

    /// The user's own status changes, as `status_change` reports it.
    fn status(&mut self, status: &str) {
        let update = self.live.apply(&ChangeSet::Presence(Arc::from(vec![Status {
            user_id: ME.into(),
            status: status.into(),
            ..Status::default()
        }])));
        let (presence, end) = update.own_presence.unwrap();
        self.dnd = presence == matty_app::sidebar::Presence::Dnd
            && matty_notify::dnd_active(end, 1_700_000_000);
    }

    /// Sends `post` in a `posted` frame and returns what the app decides,
    /// away from the window. A later post from Ada marks the end, so a post
    /// that never reaches the app reads as `None` rather than a hang.
    fn posted(&mut self, frame: Frame) -> Option<Notification> {
        let id = frame.post["id"].as_str().unwrap().to_owned();
        self.send(frame);
        self.next += 1;
        let marker = format!("marker{}", self.next);
        self.send(Frame::new(&marker, "town", ADA, "marker"));
        let mut decided = None;
        loop {
            let change = self
                .changes
                .recv_timeout(Duration::from_secs(10))
                .expect("the marker post arrives");
            let Some(arrival) = self.live.apply(&change).arrived else {
                continue;
            };
            if arrival.event.post_id == marker {
                return decided;
            }
            if arrival.event.post_id == id {
                decided = arrival.decide(AppState {
                    dnd: self.dnd,
                    show_preview: true,
                    ..AppState::default()
                });
            }
        }
    }

    fn send(&self, frame: Frame) {
        let raw: RawEvent = serde_json::from_value(json!({
            "event": "posted",
            "data": {
                "channel_display_name": frame.channel_display_name,
                "channel_name": frame.post["channel_id"],
                "channel_type": frame.channel_type,
                "mentions": serde_json::to_string(&frame.mentions).unwrap(),
                "followers": serde_json::to_string(&frame.followers).unwrap(),
                "post": frame.post.to_string(),
                "sender_name": "@ada",
                "set_online": true,
                "team_id": frame.team_id,
            },
            "broadcast": {"channel_id": frame.post["channel_id"]},
            "seq": 1,
        }))
        .unwrap();
        let event = WsEvent::decode(&raw);
        assert!(matches!(event, WsEvent::Posted(_)), "{event:?}");
        self.live.send(Command::Event(Box::new(event)));
    }
}

/// A `posted` frame's parts.
struct Frame {
    post: serde_json::Value,
    channel_type: &'static str,
    channel_display_name: &'static str,
    team_id: &'static str,
    mentions: Vec<&'static str>,
    followers: Vec<&'static str>,
}

impl Frame {
    /// A post in a team's open channel, `town` unless named.
    fn new(id: &str, channel: &str, author: &str, message: &str) -> Self {
        let (channel_type, channel_display_name, team_id) = match channel {
            "dm" => ("D", "@ada", ""),
            "gm" => ("G", "ada, me, sam", ""),
            "hushed" => ("P", "Builds", "t1"),
            "muted" => ("O", "Random", "t1"),
            _ => ("O", "Town Square", "t1"),
        };
        Self {
            post: json!({
                "id": id,
                "create_at": 1_700_000_000_000_i64,
                "update_at": 1_700_000_000_000_i64,
                "edit_at": 0,
                "delete_at": 0,
                "is_pinned": false,
                "user_id": author,
                "channel_id": channel,
                "root_id": "",
                "original_id": "",
                "message": message,
                "type": "",
                "props": {},
                "hashtags": "",
                "pending_post_id": "",
                "reply_count": 0,
                "metadata": {},
            }),
            channel_type,
            channel_display_name,
            team_id,
            mentions: Vec::new(),
            followers: Vec::new(),
        }
    }

    fn mentioning_me(mut self) -> Self {
        self.mentions = vec![ME];
        self
    }

    fn followed(mut self) -> Self {
        self.followers = vec![ME];
        self
    }

    fn reply_to(mut self, root: &str) -> Self {
        self.post["root_id"] = json!(root);
        self
    }

    fn kind(mut self, kind: &str) -> Self {
        self.post["type"] = json!(kind);
        self
    }

    fn prop(mut self, key: &str, value: serde_json::Value) -> Self {
        self.post["props"][key] = value;
        self
    }
}

#[test]
fn levels_mentions_and_direct_messages() {
    let mut app = Harness::new();
    // Mattermost's default level: mentions, direct and group messages.
    assert_eq!(app.posted(Frame::new("p1", "town", ADA, "lunch?")), None);
    let mention = app
        .posted(Frame::new("p2", "town", ADA, "lunch @arya?").mentioning_me())
        .unwrap();
    assert_eq!(mention.title, "Acme: Town Square");
    assert_eq!(mention.body, "@ada: lunch @arya?");
    assert_eq!(mention.sound, Some(Sound::Bing));
    // The server's `mentions` alone is enough, as for a keyword it knows.
    assert!(
        app.posted(Frame::new("p3", "town", ADA, "ship it").mentioning_me())
            .is_some()
    );
    let direct = app.posted(Frame::new("p4", "dm", ADA, "hi")).unwrap();
    assert_eq!((direct.title.as_str(), direct.body.as_str()), ("ada", "hi"));
    let group = app.posted(Frame::new("p5", "gm", ADA, "hi all")).unwrap();
    assert_eq!(group.title, "ada, me, sam");

    // "All new messages": every post, with its sound.
    app.prefs(&[("desktop", "all")]);
    let plain = app.posted(Frame::new("p6", "town", ADA, "lunch?")).unwrap();
    assert_eq!((plain.mention, plain.sound), (false, Some(Sound::Bing)));

    // "Nothing": not even a direct message.
    app.prefs(&[("desktop", "none")]);
    assert_eq!(app.posted(Frame::new("p7", "dm", ADA, "hi")), None);
}

#[test]
fn followed_threads_with_collapsed_replies() {
    let mut app = Harness::new();
    app.prefs(&[("desktop", "all")]);
    let followed = app
        .posted(
            Frame::new("r1", "town", ADA, "done")
                .reply_to("root")
                .followed(),
        )
        .unwrap();
    assert_eq!(followed.title, "Reply in Acme: Town Square");
    assert_eq!(followed.root_id, "root");
    // Not following: replies stay in their thread, even at "all".
    assert_eq!(
        app.posted(Frame::new("r2", "town", ADA, "done").reply_to("root")),
        None
    );
    assert!(
        app.posted(
            Frame::new("r3", "town", ADA, "@arya done")
                .reply_to("root")
                .mentioning_me()
        )
        .is_some()
    );
    // Thread notifications set to mentions only.
    app.prefs(&[("desktop", "all"), ("desktop_threads", "mention")]);
    assert_eq!(
        app.posted(
            Frame::new("r4", "town", ADA, "done")
                .reply_to("root")
                .followed()
        ),
        None
    );
}

#[test]
fn mute_status_and_own_posts() {
    let mut app = Harness::new();
    assert_eq!(
        app.posted(Frame::new("m1", "muted", ADA, "@arya hi").mentioning_me()),
        None
    );
    assert_eq!(app.posted(Frame::new("m2", "town", ME, "@arya note")), None);
    app.status("dnd");
    assert_eq!(app.posted(Frame::new("m3", "dm", ADA, "hi")), None);
    app.status("ooo");
    assert_eq!(app.posted(Frame::new("m4", "dm", ADA, "hi")), None);
    app.status("online");
    assert!(app.posted(Frame::new("m5", "dm", ADA, "hi")).is_some());
}

#[test]
fn system_messages_webhooks_and_forced_posts() {
    let mut app = Harness::new();
    app.prefs(&[("desktop", "all")]);
    assert_eq!(
        app.posted(Frame::new("s1", "town", ADA, "ada joined").kind("system_join_channel")),
        None
    );
    // Being added to a channel is the one system message that notifies.
    assert!(
        app.posted(
            Frame::new("s2", "town", ADA, "arya added to the channel by ada")
                .kind("system_add_to_channel")
                .prop("addedUserId", json!(ME))
        )
        .is_some()
    );
    assert_eq!(
        app.posted(
            Frame::new("s3", "town", ADA, "sam added to the channel by ada")
                .kind("system_add_to_channel")
                .prop("addedUserId", json!("sam-id"))
        ),
        None
    );
    // A webhook posting under the user's own name.
    assert!(
        app.posted(
            Frame::new("s4", "town", ME, "Build 42 failed")
                .prop("from_webhook", json!("true"))
                .prop("override_username", json!("ci"))
        )
        .is_some()
    );
    assert_eq!(
        app.posted(Frame::new("s5", "town", ADA, "fyi").prop("silent_notification", json!(true))),
        None
    );
    assert!(
        app.posted(
            Frame::new("s6", "muted", ADA, "Server restarting")
                .prop("force_notification", json!(true))
        )
        .is_some()
    );
}

#[test]
fn the_channels_sound_settings_win() {
    let mut app = Harness::new();
    app.prefs(&[("desktop", "all"), ("desktop_notification_sound", "Ripple")]);
    let town = app.posted(Frame::new("n1", "town", ADA, "hi")).unwrap();
    assert_eq!(town.sound, Some(Sound::Ripple));
    let hushed = app.posted(Frame::new("n2", "hushed", ADA, "hi")).unwrap();
    assert_eq!(hushed.sound, None);
    app.prefs(&[("desktop", "all"), ("desktop_sound", "false")]);
    assert_eq!(
        app.posted(Frame::new("n3", "town", ADA, "hi"))
            .unwrap()
            .sound,
        None
    );
}
