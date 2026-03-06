use crate::project::{DetectedContext, ProjectAdapter};

pub struct TerminalAdapter;

impl ProjectAdapter for TerminalAdapter {
    fn bundle_ids(&self) -> &[&str] {
        &[
            "com.googlecode.iterm2",
            "com.apple.Terminal",
            "dev.warp.Warp-Stable",
            "net.kovidgoyal.kitty",
            "co.zeit.hyper",
            "com.github.wez.wezterm",
        ]
    }

    fn extract(&self, title: &str) -> Option<DetectedContext> {
        let path = if let Some(idx) = title.find(':') {
            let after = title[idx + 1..].trim();
            if after.starts_with('~') || after.starts_with('/') {
                after
            } else {
                title
            }
        } else {
            title
        };

        let path = path.trim();
        if path.is_empty() || path == "~" || path == "/" {
            return None;
        }

        let cleaned = path
            .trim_start_matches("~/")
            .trim_start_matches('/')
            .trim_end_matches('/');

        let project = cleaned.rsplit('/').next()?;

        if project.is_empty() {
            return None;
        }

        Some(DetectedContext {
            project: project.to_string(),
            detail: None,
        })
    }
}
