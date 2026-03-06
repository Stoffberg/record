use crate::project::{DetectedContext, ProjectAdapter};

pub struct JetbrainsAdapter;

impl ProjectAdapter for JetbrainsAdapter {
    fn bundle_ids(&self) -> &[&str] {
        &[]
    }

    fn matches(&self, bundle_id: &str) -> bool {
        bundle_id.starts_with("com.jetbrains.")
    }

    fn extract(&self, title: &str) -> Option<DetectedContext> {
        let sep = if title.contains(" \u{2013} ") {
            " \u{2013} "
        } else if title.contains(" \u{2014} ") {
            " \u{2014} "
        } else if title.contains(" - ") {
            " - "
        } else {
            return None;
        };

        let parts: Vec<&str> = title.split(sep).collect();
        let project = parts.first()?.trim();

        if project.is_empty() || project == "Welcome" {
            return None;
        }

        let detail = if parts.len() >= 2 {
            let d = parts[1..].join(sep).trim().to_string();
            if d.is_empty() {
                None
            } else {
                Some(d)
            }
        } else {
            None
        };

        Some(DetectedContext {
            project: project.to_string(),
            detail,
        })
    }
}
