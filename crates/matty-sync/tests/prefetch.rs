//! The channel the app will open is fetched with the account, not after
//! the sidebar: what the web's first screen waits on with nothing cached.
#![allow(clippy::unwrap_used)]

mod common;

use common::{ME, next, server};
use matty_api::Client;
use matty_store::Store;
use matty_sync::{ChangeSet, Command};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test(flavor = "multi_thread")]
async fn a_prefetched_channel_and_its_authors_load_before_it_opens_and_once() {
    let server = server().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v4/users/{ME}/channels/c1/posts/unread")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "order": ["p1", "p2"],
            "posts": {
                "p1": {"id": "p1", "channel_id": "c1", "user_id": "ada", "create_at": 1, "update_at": 1, "message": "one"},
                "p2": {"id": "p2", "channel_id": "c1", "user_id": "ada", "create_at": 2, "update_at": 2, "message": "two"}
            }
        })))
        .with_priority(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v4/users/ids"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"id": "ada", "username": "ada", "first_name": "Ada"}
        ])))
        .expect(1)
        .mount(&server)
        .await;

    // The channel's counts come with it; the second ask, when it opens,
    // answers too late to be what the header first shows.
    Mock::given(method("GET"))
        .and(path("/api/v4/channels/c1/stats"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(
                json!({"channel_id": "c1", "member_count": 7, "pinnedpost_count": 0}),
            ),
        )
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v4/channels/c1/stats"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(
                    json!({"channel_id": "c1", "member_count": 7, "pinnedpost_count": 0}),
                )
                .set_delay(std::time::Duration::from_secs(30)),
        )
        .with_priority(2)
        .mount(&server)
        .await;

    let sync = matty_sync::spawn(Store::open_in_memory().unwrap()).unwrap();
    let changes = sync.subscribe();
    sync.send(Command::Connect {
        client: Client::new(server.uri()).unwrap().with_token("token"),
    });
    sync.send(Command::Prefetch {
        channel_id: "c1".into(),
    });
    // The author arrives with nothing open.
    let ChangeSet::Users(users) = next(&changes, |c| matches!(c, ChangeSet::Users(_))).await else {
        unreachable!()
    };
    assert_eq!(users[0].first_name, "Ada");

    // Its counts were asked for too, before it opens.
    let mut asked = false;
    for _ in 0..100 {
        let requests = server.received_requests().await.unwrap_or_default();
        if requests
            .iter()
            .any(|r| r.url.path() == "/api/v4/channels/c1/stats")
        {
            asked = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(asked, "the prefetch did not ask for the channel's counts");

    // Opening it shows the posts from the cache, with no second fetch, and
    // the counts at once.
    sync.send(Command::OpenChannel {
        channel_id: "c1".into(),
    });
    let ChangeSet::Stats(stats) = next(&changes, |c| matches!(c, ChangeSet::Stats(_))).await else {
        unreachable!()
    };
    assert_eq!(stats.member_count, 7);
    let shown = next(
        &changes,
        |c| matches!(c, ChangeSet::Posts { posts, .. } if posts.len() == 2),
    )
    .await;
    let ChangeSet::Posts { channel_id, .. } = shown else {
        unreachable!()
    };
    assert_eq!(channel_id, "c1");
}
