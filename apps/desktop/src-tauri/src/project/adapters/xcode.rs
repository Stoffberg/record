use crate::project::{DetectedContext, ProjectAdapter};

pub struct XcodeAdapter;

const SKIP_TITLES: &[&str] = &["Xcode", "Welcome to Xcode"];

impl ProjectAdapter for XcodeAdapter {
    fn bundle_ids(&self) -> &[&str] {
        &["com.apple.dt.Xcode"]
    }

    fn extract(&self, title: &str) -> Option<DetectedContext> {
        let sep = if title.contains(" — ") {
            " — "
        } else if title.contains(" - ") {
            " - "
        } else {
            let trimmed = title.trim();
            if trimmed.is_empty() || SKIP_TITLES.contains(&trimmed) {
                return None;
            }
            return Some(DetectedContext {
                project: trimmed.to_string(),
                detail: None,
            });
        };

        let parts: Vec<&str> = title.split(sep).collect();
        let project = parts.first()?.trim();

        if project.is_empty() || SKIP_TITLES.contains(&project) {
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
