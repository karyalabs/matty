//! A sidebar peek answers with a channel's newest posts without opening it:
//! fetched once when nothing is cached, from the cache after that.
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
async fn a_peek_fetches_the_latest_posts_once_then_answers_from_the_cache() {
    let server = server().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v4/users/{ME}/channels/c1/posts/unread")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "order": ["p4", "p3", "p2", "p1"],
            "posts": {
                "p1": {"id": "p1", "channel_id": "c1", "user_id": "ada", "create_at": 1, "update_at": 1, "message": "one"},
                "p2": {"id": "p2", "channel_id": "c1", "user_id": "ada", "create_at": 2, "update_at": 2, "message": "two"},
                "p3": {"id": "p3", "channel_id": "c1", "user_id": "ada", "create_at": 3, "update_at": 3, "message": "three"},
                "p4": {"id": "p4", "channel_id": "c1", "user_id": "ada", "create_at": 4, "update_at": 4, "message": "four"}
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
        .mount(&server)
        .await;

    let sync = matty_sync::spawn(Store::open_in_memory().unwrap()).unwrap();
    let changes = sync.subscribe();
    sync.send(Command::Connect {
        client: Client::new(server.uri()).unwrap().with_token("token"),
    });
    for _ in 0..2 {
        sync.send(Command::Peek {
            channel_id: "c1".into(),
        });
        let ChangeSet::Peeked { channel_id, posts } =
            next(&changes, |c| matches!(c, ChangeSet::Peeked { .. })).await
        else {
            unreachable!()
        };
        assert_eq!(channel_id, "c1");
        let ids: Vec<_> = posts.iter().map(|post| post.id.as_str()).collect();
        assert_eq!(ids, ["p2", "p3", "p4"], "the newest three, oldest first");
    }
}
