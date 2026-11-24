//! User groups (#1057): the groups `@name` mentions on a licensed server,
//! for the composer's suggestions and for opening a group from its mention.

use std::sync::Arc;

use gpui_icons::LucideIcon;
use gpui_kit::{App, Global};
use matty_api::models::Group;

use crate::ui::composer::ComposerSuggestion;

/// The signed-in server's mentionable groups; empty without a license.
#[derive(Clone, Default)]
pub struct UserGroups(Arc<[Group]>);

impl Global for UserGroups {}

impl UserGroups {
    /// Keeps the live groups that have an `@name`.
    pub fn set(groups: &[Group], cx: &mut App) {
        let groups: Arc<[Group]> = groups
            .iter()
            .filter(|group| group.allow_reference && group.delete_at == 0 && !group.name.is_empty())
            .cloned()
            .collect();
        cx.set_global(Self(groups));
    }

    pub fn read(cx: &App) -> &[Group] {
        cx.try_global::<Self>().map_or(&[], |groups| &groups.0)
    }

    /// The group `@name` mentions, ignoring case.
    pub fn named<'a>(cx: &'a App, name: &str) -> Option<&'a Group> {
        Self::read(cx)
            .iter()
            .find(|group| group.name.eq_ignore_ascii_case(name))
    }
}

/// `@` suggestions for the groups whose handle starts with `query`
/// (lowercase), or whose name has it, as the webapp's Group Mentions.
pub(crate) fn suggestions(groups: &[Group], query: &str, limit: usize) -> Vec<ComposerSuggestion> {
    let mut found: Vec<&Group> = groups
        .iter()
        .filter(|group| {
            group.name.to_lowercase().starts_with(query)
                || group.display_name.to_lowercase().contains(query)
        })
        .collect();
    found.sort_by_key(|group| {
        (
            !group.name.to_lowercase().starts_with(query),
            group.name.clone(),
        )
    });
    found
        .into_iter()
        .take(limit)
        .map(|group| {
            ComposerSuggestion::new(format!("@{}", group.name), group.display_name.clone())
                .description(format!("@{}", group.name))
                .icon(LucideIcon::Users)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(name: &str, display: &str) -> Group {
        Group {
            id: format!("{name}-id"),
            name: name.into(),
            display_name: display.into(),
            allow_reference: true,
            member_count: 4,
            ..Group::default()
        }
    }

    #[test]
    fn groups_match_by_handle_first_then_by_name() {
        let groups = [
            group("oncall", "On-call"),
            group("design-team", "Design team"),
            group("developers", "Developers"),
        ];
        let values = |query: &str| {
            suggestions(&groups, query, 8)
                .into_iter()
                .map(|s| s.value.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(values("de"), ["@design-team", "@developers"]);
        assert_eq!(values("call"), ["@oncall"]);
        assert_eq!(values("team"), ["@design-team"]);
        assert!(values("zzz").is_empty());
        assert_eq!(suggestions(&groups, "", 2).len(), 2);
    }
}
