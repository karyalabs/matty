//! Joining and leaving teams (#745): the teams open to join, joining one
//! or accepting an invite link, and leaving.

use serde_json::{Value, json};

use crate::{
    ApiError, Client,
    models::{Team, TeamMember},
};

/// The fields of a team to change, as `PUT teams/{id}/patch` takes them;
/// `None` leaves a field alone.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct TeamPatch {
    /// The name people see.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// What the team is for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Anyone on the server may join without an invite.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_open_invite: Option<bool>,
}

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

    /// `POST teams`: an open team with the user as its admin.
    pub async fn create_team(&self, display_name: &str, name: &str) -> Result<Team, ApiError> {
        self.post(
            "teams",
            &serde_json::json!({ "display_name": display_name, "name": name, "type": "O" }),
        )
        .await
    }

    /// `GET teams/{team_id}`.
    pub async fn team(&self, team_id: &str) -> Result<Team, ApiError> {
        self.get(&format!("teams/{team_id}")).await
    }

    /// `PUT teams/{team_id}/patch`: renames a team, changes its
    /// description, or opens or closes it to anyone on the server.
    pub async fn patch_team(&self, team_id: &str, patch: &TeamPatch) -> Result<Team, ApiError> {
        self.put(&format!("teams/{team_id}/patch"), patch).await
    }

    /// `GET teams/{team_id}/members`: one page of a team's memberships,
    /// with their roles.
    pub async fn team_members(
        &self,
        team_id: &str,
        page: u32,
        per_page: u32,
    ) -> Result<Vec<TeamMember>, ApiError> {
        self.get_query(
            &format!("teams/{team_id}/members"),
            &[
                ("page", page.to_string()),
                ("per_page", per_page.to_string()),
            ],
        )
        .await
    }

    /// `PUT teams/{team_id}/members/{user_id}/roles`: makes someone a team
    /// admin, or takes it away.
    pub async fn set_team_admin(
        &self,
        team_id: &str,
        user_id: &str,
        admin: bool,
    ) -> Result<(), ApiError> {
        let roles = if admin {
            "team_user team_admin"
        } else {
            "team_user"
        };
        self.put::<_, Value>(
            &format!("teams/{team_id}/members/{user_id}/roles"),
            &json!({ "roles": roles }),
        )
        .await
        .map(|_| ())
    }

    /// `POST teams/{team_id}/regenerate_invite_id`: a new invite link, so
    /// the old one stops working.
    pub async fn regenerate_invite_id(&self, team_id: &str) -> Result<Team, ApiError> {
        self.post(&format!("teams/{team_id}/regenerate_invite_id"), &json!({}))
            .await
    }

    /// `POST teams/{team_id}/invite/email`: emails an invite to each address.
    pub async fn invite_by_email(&self, team_id: &str, emails: &[String]) -> Result<(), ApiError> {
        self.post::<_, Value>(&format!("teams/{team_id}/invite/email"), &emails)
            .await
            .map(|_| ())
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
