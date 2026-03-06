use crate::project::{DetectedContext, ProjectAdapter};

pub struct TeamsAdapter;

impl ProjectAdapter for TeamsAdapter {
    fn bundle_ids(&self) -> &[&str] {
        &["com.microsoft.teams2", "com.microsoft.teams"]
    }

    fn extract(&self, title: &str) -> Option<DetectedContext> {
        let parts: Vec<&str> = title.split(" | ").collect();
        if parts.len() < 3 {
            return None;
        }

        if parts.last().map(|s| s.trim()) != Some("Microsoft Teams") {
            return None;
        }

        let org_index = if parts.len() >= 4 {
            parts.len() - 3
        } else {
            parts.len() - 2
        };

        let project = parts[org_index].trim().to_string();
        if project.is_empty() || project.contains('@') {
            return None;
        }

        let context_parts = &parts[..org_index];
        let detail = if context_parts.is_empty() {
            None
        } else {
            let joined = context_parts
                .iter()
                .map(|s| s.trim())
                .collect::<Vec<_>>()
                .join(": ");
            if joined.is_empty() {
                None
            } else {
                Some(joined)
            }
        };

        Some(DetectedContext { project, detail })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract(title: &str) -> Option<DetectedContext> {
        TeamsAdapter.extract(title)
    }

    #[test]
    fn chat_with_person() {
        let ctx = extract(
            "Chat | Ian Kavanagh | New Payment Innovation Limited | Dirk.Stoffberg@infinitepay.tech | Microsoft Teams",
        ).unwrap();
        assert_eq!(ctx.project, "New Payment Innovation Limited");
        assert_eq!(ctx.detail.as_deref(), Some("Chat: Ian Kavanagh"));
    }

    #[test]
    fn chat_with_group() {
        let ctx = extract(
            "Chat | The Expandables ↕️ | New Payment Innovation Limited | Dirk.Stoffberg@infinitepay.tech | Microsoft Teams",
        ).unwrap();
        assert_eq!(ctx.project, "New Payment Innovation Limited");
        assert_eq!(ctx.detail.as_deref(), Some("Chat: The Expandables ↕️"));
    }

    #[test]
    fn calendar() {
        let ctx = extract(
            "Calendar | Calendar | New Payment Innovation Limited | Dirk.Stoffberg@infinitepay.tech | Microsoft Teams",
        ).unwrap();
        assert_eq!(ctx.project, "New Payment Innovation Limited");
        assert_eq!(ctx.detail.as_deref(), Some("Calendar: Calendar"));
    }

    #[test]
    fn meeting() {
        let ctx = extract(
            "The Expandables Daily Stand Up | New Payment Innovation Limited | Dirk.Stoffberg@infinitepay.tech | Microsoft Teams",
        ).unwrap();
        assert_eq!(ctx.project, "New Payment Innovation Limited");
        assert_eq!(
            ctx.detail.as_deref(),
            Some("The Expandables Daily Stand Up")
        );
    }

    #[test]
    fn different_org() {
        let ctx = extract(
            "Chat | Business Systems Huddle | Cloud Direct | Dirk.Beukes@clouddirect.net | Microsoft Teams",
        ).unwrap();
        assert_eq!(ctx.project, "Cloud Direct");
        assert_eq!(ctx.detail.as_deref(), Some("Chat: Business Systems Huddle"));
    }

    #[test]
    fn meeting_no_chat_prefix() {
        let ctx = extract(
            "Business Systems Huddle | Cloud Direct | Dirk.Beukes@clouddirect.net | Microsoft Teams",
        ).unwrap();
        assert_eq!(ctx.project, "Cloud Direct");
        assert_eq!(ctx.detail.as_deref(), Some("Business Systems Huddle"));
    }

    #[test]
    fn too_few_parts() {
        assert_eq!(extract("Microsoft Teams"), None);
        assert_eq!(extract("Chat | Microsoft Teams"), None);
    }

    #[test]
    fn empty_and_missing() {
        assert_eq!(extract(""), None);
        assert_eq!(extract("Some Random App Title"), None);
    }
}
