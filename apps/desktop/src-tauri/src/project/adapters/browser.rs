use crate::project::{DetectedContext, ProjectAdapter};

pub struct BrowserAdapter;

const SKIP_SITES: &[&str] = &[
    "Google Chrome",
    "Safari",
    "Arc",
    "Brave",
    "Firefox",
    "Opera",
    "New Tab",
    "Google Search",
];

impl ProjectAdapter for BrowserAdapter {
    fn bundle_ids(&self) -> &[&str] {
        &[
            "com.google.Chrome",
            "com.apple.Safari",
            "company.thebrowser.Browser",
            "com.brave.Browser",
            "org.mozilla.firefox",
            "com.operasoftware.Opera",
        ]
    }

    fn extract(&self, title: &str) -> Option<DetectedContext> {
        let sep = if title.contains(" - ") {
            " - "
        } else if title.contains(" — ") {
            " — "
        } else if title.contains(" | ") {
            " | "
        } else {
            return None;
        };

        let parts: Vec<&str> = title.rsplitn(2, sep).collect();
        let site = parts.first()?.trim();

        if site.is_empty() || SKIP_SITES.iter().any(|s| site.eq_ignore_ascii_case(s)) {
            return None;
        }

        let detail = if parts.len() >= 2 {
            let d = parts[1].trim().to_string();
            if d.is_empty() {
                None
            } else {
                Some(d)
            }
        } else {
            None
        };

        Some(DetectedContext {
            project: site.to_string(),
            detail,
        })
    }
}
