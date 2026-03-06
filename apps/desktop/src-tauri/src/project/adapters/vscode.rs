use crate::project::{DetectedContext, ProjectAdapter};

pub struct VscodeAdapter;

const BUNDLE_IDS: &[&str] = &[
    "com.microsoft.VSCode",
    "com.todesktop.230313mzl4w4u92",
    "dev.zed.Zed",
];

const APP_NAMES: &[&str] = &["Visual Studio Code", "Cursor", "Zed"];

const SKIP_TITLES: &[&str] = &["Welcome", "Get Started"];

impl ProjectAdapter for VscodeAdapter {
    fn bundle_ids(&self) -> &[&str] {
        BUNDLE_IDS
    }

    fn extract(&self, title: &str) -> Option<DetectedContext> {
        let sep = if title.contains(" — ") {
            " — "
        } else if title.contains(" - ") {
            " - "
        } else {
            return None;
        };

        let parts: Vec<&str> = title.split(sep).collect();
        if parts.len() < 2 {
            return None;
        }

        let candidate = parts.last()?.trim();
        let cleaned = candidate
            .trim_start_matches("[SSH: ")
            .trim_end_matches(']')
            .trim_start_matches("[WSL: ")
            .trim();

        let (project, detail_parts) = if APP_NAMES.iter().any(|name| cleaned.contains(name)) {
            if parts.len() >= 3 {
                let proj = parts[parts.len() - 2].trim();
                let detail: Vec<&str> = parts[..parts.len() - 2].iter().map(|s| s.trim()).collect();
                (proj, detail)
            } else {
                return None;
            }
        } else {
            let detail: Vec<&str> = parts[..parts.len() - 1].iter().map(|s| s.trim()).collect();
            (cleaned, detail)
        };

        if project.is_empty() || SKIP_TITLES.contains(&project) {
            return None;
        }

        let detail = if detail_parts.is_empty() {
            None
        } else {
            let d = detail_parts.join(sep);
            if d.is_empty() {
                None
            } else {
                Some(d)
            }
        };

        Some(DetectedContext {
            project: project.to_string(),
            detail,
        })
    }
}
