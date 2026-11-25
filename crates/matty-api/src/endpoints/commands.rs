//! Slash commands (#210): listing a team's commands, the server's
//! suggestions while the user types one, and running one.

use crate::{
    ApiError, Client,
    models::{
        Bot, CommandResponse, CommandSuggestion, CustomCommand, ExecuteCommand, IncomingWebhook,
        OutgoingWebhook, SlashCommand,
    },
};

impl Client {
    /// `GET commands?team_id=&custom_only=true`: the team's custom
    /// commands with their URLs and tokens, for whoever may manage them.
    pub async fn custom_commands(&self, team_id: &str) -> Result<Vec<CustomCommand>, ApiError> {
        let commands: Option<Vec<CustomCommand>> = self
            .get_query(
                "commands",
                &[
                    ("team_id", team_id.to_owned()),
                    ("custom_only", "true".to_owned()),
                ],
            )
            .await?;
        Ok(commands.unwrap_or_default())
    }

    /// `POST commands`: a custom slash command in its team.
    pub async fn create_command(&self, command: &CustomCommand) -> Result<CustomCommand, ApiError> {
        self.post("commands", command).await
    }

    /// `PUT commands/{id}`: the command as changed; its id says which.
    pub async fn update_command(&self, command: &CustomCommand) -> Result<CustomCommand, ApiError> {
        self.put(&format!("commands/{}", command.id), command).await
    }

    /// `PUT hooks/incoming/{id}`: the webhook as changed.
    pub async fn update_incoming_webhook(
        &self,
        hook: &IncomingWebhook,
    ) -> Result<IncomingWebhook, ApiError> {
        self.put(&format!("hooks/incoming/{}", hook.id), hook).await
    }

    /// `GET hooks/outgoing?team_id=`: the team's outgoing webhooks the
    /// user may manage.
    pub async fn outgoing_webhooks(&self, team_id: &str) -> Result<Vec<OutgoingWebhook>, ApiError> {
        let hooks: Option<Vec<OutgoingWebhook>> = self
            .get_query(
                "hooks/outgoing",
                &[
                    ("team_id", team_id.to_owned()),
                    ("page", "0".to_owned()),
                    ("per_page", "200".to_owned()),
                ],
            )
            .await?;
        Ok(hooks.unwrap_or_default())
    }

    /// `POST hooks/outgoing`.
    pub async fn create_outgoing_webhook(
        &self,
        hook: &OutgoingWebhook,
    ) -> Result<OutgoingWebhook, ApiError> {
        self.post("hooks/outgoing", hook).await
    }

    /// `PUT hooks/outgoing/{id}`: the webhook as changed.
    pub async fn update_outgoing_webhook(
        &self,
        hook: &OutgoingWebhook,
    ) -> Result<OutgoingWebhook, ApiError> {
        self.put(&format!("hooks/outgoing/{}", hook.id), hook).await
    }

    /// `DELETE hooks/outgoing/{hook_id}`.
    pub async fn delete_outgoing_webhook(&self, hook_id: &str) -> Result<(), ApiError> {
        let _: serde_json::Value = self.delete(&format!("hooks/outgoing/{hook_id}")).await?;
        Ok(())
    }

    /// `GET bots?include_deleted=true`: the bot accounts the user may see,
    /// disabled ones too.
    pub async fn bots(&self) -> Result<Vec<Bot>, ApiError> {
        let bots: Option<Vec<Bot>> = self
            .get_query(
                "bots",
                &[
                    ("include_deleted", "true".to_owned()),
                    ("page", "0".to_owned()),
                    ("per_page", "200".to_owned()),
                ],
            )
            .await?;
        Ok(bots.unwrap_or_default())
    }

    /// `POST bots`.
    pub async fn create_bot(
        &self,
        username: &str,
        display_name: &str,
        description: &str,
    ) -> Result<Bot, ApiError> {
        self.post(
            "bots",
            &serde_json::json!({
                "username": username,
                "display_name": display_name,
                "description": description,
            }),
        )
        .await
    }

    /// `PUT bots/{user_id}`: a new `@name`, name or description.
    pub async fn patch_bot(&self, bot: &Bot) -> Result<Bot, ApiError> {
        self.put(
            &format!("bots/{}", bot.user_id),
            &serde_json::json!({
                "username": bot.username,
                "display_name": bot.display_name,
                "description": bot.description,
            }),
        )
        .await
    }

    /// `POST bots/{user_id}/enable` or `/disable`.
    pub async fn set_bot_active(&self, user_id: &str, active: bool) -> Result<Bot, ApiError> {
        let verb = if active { "enable" } else { "disable" };
        self.post(&format!("bots/{user_id}/{verb}"), &serde_json::json!({}))
            .await
    }

    /// `DELETE commands/{command_id}`.
    pub async fn delete_command(&self, command_id: &str) -> Result<(), ApiError> {
        let _: serde_json::Value = self.delete(&format!("commands/{command_id}")).await?;
        Ok(())
    }

    /// `GET hooks/incoming?team_id=`: the team's incoming webhooks the
    /// user may manage.
    pub async fn incoming_webhooks(&self, team_id: &str) -> Result<Vec<IncomingWebhook>, ApiError> {
        let hooks: Option<Vec<IncomingWebhook>> = self
            .get_query(
                "hooks/incoming",
                &[
                    ("team_id", team_id.to_owned()),
                    ("page", "0".to_owned()),
                    ("per_page", "200".to_owned()),
                ],
            )
            .await?;
        Ok(hooks.unwrap_or_default())
    }

    /// `POST hooks/incoming`: a webhook that posts to `channel_id`.
    pub async fn create_incoming_webhook(
        &self,
        channel_id: &str,
        display_name: &str,
        description: &str,
    ) -> Result<IncomingWebhook, ApiError> {
        self.post(
            "hooks/incoming",
            &serde_json::json!({
                "channel_id": channel_id,
                "display_name": display_name,
                "description": description,
            }),
        )
        .await
    }

    /// `DELETE hooks/incoming/{hook_id}`.
    pub async fn delete_incoming_webhook(&self, hook_id: &str) -> Result<(), ApiError> {
        let _: serde_json::Value = self.delete(&format!("hooks/incoming/{hook_id}")).await?;
        Ok(())
    }

    /// `GET teams/{team_id}/commands/autocomplete`: the commands the user
    /// may run in the team that autocomplete offers, the server's own and
    /// plugins' among them.
    pub async fn autocomplete_commands(
        &self,
        team_id: &str,
    ) -> Result<Vec<SlashCommand>, ApiError> {
        let commands: Option<Vec<SlashCommand>> = self
            .get(&format!("teams/{team_id}/commands/autocomplete"))
            .await?;
        Ok(commands.unwrap_or_default())
    }

    /// `GET commands?team_id=&custom_only=`: the team's commands; with
    /// `custom_only` off the server's own come too. Integrations' commands
    /// need the permission to manage them.
    pub async fn list_commands(
        &self,
        team_id: &str,
        custom_only: bool,
    ) -> Result<Vec<SlashCommand>, ApiError> {
        let commands: Option<Vec<SlashCommand>> = self
            .get_query(
                "commands",
                &[
                    ("team_id", team_id.to_owned()),
                    ("custom_only", custom_only.to_string()),
                ],
            )
            .await?;
        Ok(commands.unwrap_or_default())
    }

    /// `GET teams/{team_id}/commands/autocomplete_suggestions`: what may
    /// come next in `user_input` (the line so far, from its `/`): command
    /// names, subcommands, or a plugin's dynamic argument values.
    pub async fn command_suggestions(
        &self,
        team_id: &str,
        user_input: &str,
        channel_id: &str,
        root_id: &str,
    ) -> Result<Vec<CommandSuggestion>, ApiError> {
        let mut query = vec![
            ("user_input", user_input.to_owned()),
            ("channel_id", channel_id.to_owned()),
        ];
        if !root_id.is_empty() {
            query.push(("root_id", root_id.to_owned()));
        }
        let suggestions: Option<Vec<CommandSuggestion>> = self
            .get_query(
                &format!("teams/{team_id}/commands/autocomplete_suggestions"),
                &query,
            )
            .await?;
        Ok(suggestions.unwrap_or_default())
    }

    /// `POST commands/execute`: runs a slash command. An unknown command
    /// or one the integration refused is an [`ApiError::Server`] with the
    /// server's words.
    pub async fn execute_command(
        &self,
        command: &ExecuteCommand,
    ) -> Result<CommandResponse, ApiError> {
        self.post("commands/execute", command).await
    }
}
