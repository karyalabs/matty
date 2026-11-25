//! What works from the cache with no server, and how commands that need
//! one are refused (#163).
#![allow(clippy::unwrap_used)]

mod common;

use std::time::Duration;

use common::{ME, get, next, server_unpooled};
use matty_api::Client;
use matty_sync::{ChangeSet, Command, CommandError, Listing};
use serde_json::json;
use wiremock::MockServer;

/// A server whose channel holds a pinned post and a post the user saved.
async fn cached_server() -> MockServer {
    let server = server_unpooled().await;
    get(
        &format!("users/{ME}/channels/c1/posts/unread"),
        json!({"order": ["p2", "p1"], "posts": {
            "p1": {"id": "p1", "channel_id": "c1", "user_id": "u2", "create_at": 100,
                   "update_at": 100, "message": "pinned", "is_pinned": true},
            "p2": {"id": "p2", "channel_id": "c1", "user_id": "u2", "create_at": 200,
                   "update_at": 200, "message": "saved"},
        }}),
    )
    .with_priority(1)
    .mount(&server)
    .await;
    get(
        "users/me/preferences",
        json!([{"user_id": ME, "category": "flagged_post", "name": "p2", "value": "true"}]),
    )
    .with_priority(1)
    .mount(&server)
    .await;
    server
}

fn ids(change: &ChangeSet) -> Vec<&str> {
    match change {
        ChangeSet::Listing { posts, .. } | ChangeSet::Posts { posts, .. } => {
            posts.iter().map(|post| post.id.as_str()).collect()
        }
        _ => Vec::new(),
    }
}

fn listed(want: Listing) -> impl Fn(&ChangeSet) -> bool {
    move |c| matches!(c, ChangeSet::Listing { listing, .. } if *listing == want)
}

fn refused(change: &ChangeSet) -> Option<&'static str> {
    match change {
        ChangeSet::Failed(CommandError::Offline { action }) => Some(action),
        _ => None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn offline_the_cache_answers_and_server_commands_are_refused() {
    let server = cached_server().await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("account.sqlite");
    {
        let sync = matty_sync::spawn_at(path.clone(), None).unwrap();
        let changes = sync.subscribe();
        sync.send(Command::OpenChannel {
            channel_id: "c1".into(),
        });
        sync.send(Command::Connect {
            client: Client::new(server.uri()).unwrap().with_token("token"),
        });
        next(&changes, |c| ids(c) == ["p1", "p2"]).await;
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    let before = server.received_requests().await.unwrap().len();

    // `matty --offline`: the cache with no session.
    let sync = matty_sync::spawn_at(path, None).unwrap();
    let changes = sync.subscribe();
    sync.send(Command::ShowCache);
    sync.send(Command::OpenChannel {
        channel_id: "c1".into(),
    });
    next(&changes, |c| ids(c) == ["p1", "p2"]).await;

    // Saved and pinned come from the cache instead of loading forever.
    sync.send(Command::LoadListing(Listing::Saved));
    assert_eq!(ids(&next(&changes, listed(Listing::Saved)).await), ["p2"]);
    let pinned = Listing::Pinned {
        channel_id: "c1".into(),
    };
    sync.send(Command::LoadListing(pinned.clone()));
    assert_eq!(ids(&next(&changes, listed(pinned)).await), ["p1"]);
    // Mentions are the cached posts that hold a mention key.
    let mentions = Listing::Mentions {
        team_id: "t1".into(),
        terms: "@me saved".into(),
    };
    sync.send(Command::LoadListing(mentions.clone()));
    assert_eq!(ids(&next(&changes, listed(mentions)).await), ["p2"]);

    // Each command that needs the server says which it was, and changes
    // nothing here.
    let commands = [
        (
            Command::ToggleReaction {
                post_id: "p1".into(),
                emoji_name: "+1".into(),
            },
            "add reaction",
        ),
        (
            Command::SetSaved {
                post_id: "p1".into(),
                saved: true,
            },
            "save message",
        ),
        (
            Command::SetPinned {
                post_id: "p2".into(),
                pinned: true,
            },
            "pin message",
        ),
        (
            Command::EditPost {
                post_id: "p1".into(),
                message: "changed".into(),
            },
            "edit",
        ),
        (
            Command::DeletePost {
                post_id: "p2".into(),
            },
            "delete",
        ),
        (
            Command::Search {
                team_id: "t1".into(),
                terms: "pinned".into(),
                time_zone_offset: 0,
                page: 0,
                all_teams: false,
            },
            "search",
        ),
    ];
    for (command, action) in commands {
        sync.send(command);
        let change = next(&changes, |c| refused(c).is_some()).await;
        assert_eq!(refused(&change), Some(action));
    }
    // Still as cached: no reaction, not saved, nothing edited or gone.
    sync.send(Command::LoadListing(Listing::Saved));
    assert_eq!(ids(&next(&changes, listed(Listing::Saved)).await), ["p2"]);
    sync.send(Command::OpenChannel {
        channel_id: "c1".into(),
    });
    let ChangeSet::Posts { posts, .. } = next(&changes, |c| ids(c) == ["p1", "p2"]).await else {
        unreachable!()
    };
    assert!(posts[0].metadata.reactions.is_empty());
    assert_eq!(posts[0].message, "pinned");
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        before,
        "offline made requests"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_dropped_network_reads_as_offline_and_listings_fall_back_to_the_cache() {
    let server = cached_server().await;
    let sync = matty_sync::spawn(matty_store::Store::open_in_memory().unwrap()).unwrap();
    let changes = sync.subscribe();
    sync.send(Command::OpenChannel {
        channel_id: "c1".into(),
    });
    sync.send(Command::Connect {
        client: Client::new(server.uri()).unwrap().with_token("token"),
    });
    next(&changes, |c| ids(c) == ["p1", "p2"]).await;
    // The network goes: nothing answers at the server's address.
    drop(server);

    sync.send(Command::ToggleReaction {
        post_id: "p2".into(),
        emoji_name: "+1".into(),
    });
    let change = next(&changes, |c| matches!(c, ChangeSet::Failed(_))).await;
    assert_eq!(refused(&change), Some("add reaction"));
    // The reaction shown at once is taken back.
    next(&changes, |c| match c {
        ChangeSet::Posts { posts, .. } => {
            posts.iter().all(|post| post.metadata.reactions.is_empty())
        }
        _ => false,
    })
    .await;

    sync.send(Command::EditPost {
        post_id: "p1".into(),
        message: "changed".into(),
    });
    let change = next(&changes, |c| matches!(c, ChangeSet::Failed(_))).await;
    assert_eq!(refused(&change), Some("edit"));

    let pinned = Listing::Pinned {
        channel_id: "c1".into(),
    };
    sync.send(Command::LoadListing(pinned.clone()));
    assert_eq!(ids(&next(&changes, listed(pinned)).await), ["p1"]);
}

/// The web demo has no server: an older page comes from its snapshot until
/// the channel's first post, instead of failing.
#[tokio::test(flavor = "multi_thread")]
async fn the_demo_pages_older_posts_from_its_snapshot() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../matty-web/demo/demo.sqlite");
    let snapshot = std::fs::read(path).unwrap();
    // The demo's busiest channel, and how many posts it holds.
    let (channel_id, stored): (String, i64) = rusqlite::Connection::open(path)
        .unwrap()
        .query_row(
            "SELECT channel_id, count(*) FROM posts WHERE root_id = '' \
             GROUP BY channel_id ORDER BY count(*) DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert!(
        stored as usize > matty_sync::POSTS_WINDOW,
        "the demo has a long channel"
    );
    let (sync, _) = matty_sync::spawn_demo(&snapshot).unwrap().unwrap();
    let changes = sync.subscribe();
    sync.send(Command::OpenChannel {
        channel_id: channel_id.clone(),
    });
    let shown = |change: &ChangeSet| match change {
        ChangeSet::Posts {
            channel_id: id,
            posts,
        } if *id == channel_id => Some(posts.len()),
        _ => None,
    };
    let mut before = shown(&next(&changes, |c| shown(c).is_some()).await).unwrap();
    loop {
        sync.send(Command::LoadOlder {
            channel_id: channel_id.clone(),
        });
        let history = next(
            &changes,
            |c| matches!(c, ChangeSet::History { channel_id: id, .. } if *id == channel_id),
        )
        .await;
        let ChangeSet::History {
            at_start, failed, ..
        } = history
        else {
            unreachable!()
        };
        assert!(!failed, "a page from the snapshot does not fail");
        if at_start {
            break;
        }
        let after = shown(&next(&changes, |c| shown(c).is_some()).await).unwrap();
        assert!(after > before, "each page adds older posts");
        before = after;
    }
}

/// Without a server, the unread digest comes from the cache: each unread
/// channel's posts after the user last viewed it, instead of loading forever.
#[tokio::test(flavor = "multi_thread")]
async fn the_demo_lists_unread_messages_from_its_snapshot() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../matty-web/demo/demo.sqlite");
    let snapshot = std::fs::read(path).unwrap();
    // A channel with posts the demo's user has not viewed yet.
    let (channel_id, unread): (String, i64) = rusqlite::Connection::open(path)
        .unwrap()
        .query_row(
            "SELECT p.channel_id, count(*) FROM posts p
             JOIN channel_members m ON m.channel_id = p.channel_id
             JOIN sync_cursors c ON c.key = 'me' AND c.value = m.user_id
             WHERE p.create_at > m.last_viewed_at AND p.root_id = '' AND p.delete_at = 0
             GROUP BY p.channel_id ORDER BY count(*) LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    let (sync, _) = matty_sync::spawn_demo(&snapshot).unwrap().unwrap();
    let changes = sync.subscribe();
    let digest = Listing::Unreads {
        channel_ids: vec![channel_id.clone()],
        collapsed_threads: true,
    };
    sync.send(Command::LoadListing(digest.clone()));
    let ChangeSet::Listing { posts, .. } = next(&changes, listed(digest)).await else {
        unreachable!()
    };
    assert_eq!(posts.len() as i64, unread.min(5));
    assert!(posts.iter().all(|post| post.channel_id == channel_id));
    assert!(
        posts
            .windows(2)
            .all(|pair| pair[0].create_at <= pair[1].create_at)
    );
}

/// Without a server, Browse channels and New message list what the cache
/// knows instead of nothing.
#[tokio::test(flavor = "multi_thread")]
async fn the_demo_browses_channels_and_people_from_its_snapshot() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../matty-web/demo/demo.sqlite");
    let snapshot = std::fs::read(path).unwrap();
    let db = rusqlite::Connection::open(path).unwrap();
    let (team_id, public): (String, i64) = db
        .query_row(
            "SELECT team_id, count(*) FROM channels WHERE kind = 'O' GROUP BY team_id LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    let people: i64 = db
        .query_row("SELECT count(*) FROM users", [], |row| row.get(0))
        .unwrap();
    let (sync, _) = matty_sync::spawn_demo(&snapshot).unwrap().unwrap();
    let changes = sync.subscribe();

    sync.send(Command::Browse {
        team_id: team_id.clone(),
    });
    let ChangeSet::Directory { channels, .. } =
        next(&changes, |c| matches!(c, ChangeSet::Directory { .. })).await
    else {
        unreachable!()
    };
    assert_eq!(channels.len() as i64, public);

    sync.send(Command::FindPeople { team_id });
    let ChangeSet::People { users, .. } =
        next(&changes, |c| matches!(c, ChangeSet::People { .. })).await
    else {
        unreachable!()
    };
    assert_eq!(users.len() as i64, people);
}
