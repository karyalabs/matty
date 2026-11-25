//! Typed client for the Mattermost REST API v4 and WebSocket.
//!
//! Owns HTTP, authentication headers, error mapping, rate limits and the
//! WebSocket event stream. Plain Rust: this crate must never depend on GPUI
//! (PLAN §6.1), which CI checks with `scripts/check-no-gpui.sh`.
//!
//! The client is async and runs on tokio (spike S2, option A): Matty drives it
//! from one background runtime and hands results to the UI through channels.

pub mod capabilities;
mod client;
mod endpoints;
pub(crate) mod error;
pub mod giphy;
pub mod models;
pub mod rt;
#[cfg(not(target_family = "wasm"))]
mod tls;
pub mod ws;

pub use capabilities::{PasswordRules, ServerCapabilities, UnsupportedVersion, Version};
pub use client::{
    Client, ClientBuilder, Fetched, MAX_OUTSIDE_IMAGE, RateLimit, Sleeper, TokioSleeper, USER_AGENT,
};
pub use endpoints::apps::USER_AGENT as APPS_USER_AGENT;
pub use endpoints::browse::{ChannelPatch, NewChannel, ProfilePatch, SearchQuery};
pub use endpoints::files::{Progress, Upload, Uploaded};
pub use endpoints::posts::{Direction, NewPost, PostPatch, PostsQuery, ThreadQuery, ThreadsQuery};
pub use endpoints::system::ProbeError;
pub use endpoints::teams::TeamPatch;
pub use endpoints::users::{
    AccessToken, Credentials, MFA_BAD_CODE_ID, MFA_REQUIRED_ID, MfaSecret, SignedIn,
};
pub use error::ApiError;
#[cfg(not(target_family = "wasm"))]
pub use tls::https_config;
