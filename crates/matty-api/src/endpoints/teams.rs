//! Joining and leaving teams (#745): the teams open to join, joining one
//! or accepting an invite link, and leaving.

use serde_json::{Value, json};

use crate::{
    ApiError, Client,
    models::{Team, TeamMember},
};

impl Client {
    /// `GET teams`: one page of the teams the user may see. For anyone but
    /// a system admin these are the open teams (`allow_open_invite`), the
    /// user's own among them.
    pub async fn teams(&self, page: u32, per_page: u32) -> Result<Vec<Team>, ApiError> {
        self.get_query(
            "teams",
            &[
                ("page", page.to_string()),
                ("per_page", per_page.to_string()),
            ],
        )
        .await
    }

    /// `POST teams/{team_id}/members`: adds a user to a team, which for
    /// the user themself is joining an open team.
    pub async fn add_team_member(
        &self,
        team_id: &str,
        user_id: &str,
    ) -> Result<TeamMember, ApiError> {
        self.post(
            &format!("teams/{team_id}/members"),
            &json!({ "team_id": team_id, "user_id": user_id }),
        )
        .await
    }

    /// `POST teams/members/invite?invite_id=`: joins the team an invite
    /// link names, open or not.
    pub async fn join_team_by_invite(&self, invite_id: &str) -> Result<TeamMember, ApiError> {
        self.post_query(
            "teams/members/invite",
            &[("invite_id", invite_id.to_owned())],
        )
        .await
    }

    /// `DELETE teams/{team_id}/members/{user_id}`: leaves a team, or
    /// removes someone from it.
    pub async fn remove_team_member(&self, team_id: &str, user_id: &str) -> Result<(), ApiError> {
        self.delete::<Value>(&format!("teams/{team_id}/members/{user_id}"))
            .await
            .map(|_| ())
    }
}
