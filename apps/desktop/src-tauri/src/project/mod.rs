mod adapters;

use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq)]
pub struct DetectedContext {
    pub project: String,
    pub detail: Option<String>,
}

pub trait ProjectAdapter: Send + Sync {
    fn bundle_ids(&self) -> &[&str];
    fn matches(&self, bundle_id: &str) -> bool {
        self.bundle_ids().contains(&bundle_id)
    }
    fn extract(&self, window_title: &str) -> Option<DetectedContext>;
    fn extract_at(
        &self,
        _window_title: &str,
        _timestamp: DateTime<Utc>,
    ) -> Option<DetectedContext> {
        None
    }
}

pub struct ProjectDetector {
    adapters: Vec<Box<dyn ProjectAdapter>>,
}

impl ProjectDetector {
    pub fn new() -> Self {
        Self {
            adapters: vec![
                Box::new(adapters::VscodeAdapter),
                Box::new(adapters::JetbrainsAdapter),
                Box::new(adapters::XcodeAdapter),
                Box::new(adapters::TerminalAdapter),
                Box::new(adapters::BrowserAdapter),
                Box::new(adapters::OpencodeAdapter),
                Box::new(adapters::TeamsAdapter),
                Box::new(adapters::DiaAdapter::new()),
            ],
        }
    }

    pub fn detect(&self, bundle_id: &str, window_title: &str) -> Option<DetectedContext> {
        self.detect_at(bundle_id, window_title, Utc::now())
    }

    pub fn detect_at(
        &self,
        bundle_id: &str,
        window_title: &str,
        timestamp: DateTime<Utc>,
    ) -> Option<DetectedContext> {
        for adapter in &self.adapters {
            if adapter.matches(bundle_id) {
                if let Some(ctx) = adapter.extract(window_title) {
                    return Some(ctx);
                }
                return adapter.extract_at(window_title, timestamp);
            }
        }
        None
    }
}

impl Default for ProjectDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detect(bundle_id: &str, title: &str) -> Option<DetectedContext> {
        ProjectDetector::new().detect(bundle_id, title)
    }

    fn project(bundle_id: &str, title: &str) -> Option<String> {
        detect(bundle_id, title).map(|c| c.project)
    }

    #[test]
    fn vscode_project_and_detail() {
        let ctx = detect(
            "com.microsoft.VSCode",
            "main.rs — record — Visual Studio Code",
        )
        .unwrap();
        assert_eq!(ctx.project, "record");
        assert_eq!(ctx.detail.as_deref(), Some("main.rs"));

        let ctx = detect("com.microsoft.VSCode", "index.ts — my-project").unwrap();
        assert_eq!(ctx.project, "my-project");
        assert_eq!(ctx.detail.as_deref(), Some("index.ts"));
    }

    #[test]
    fn vscode_no_project() {
        assert_eq!(project("com.microsoft.VSCode", "Welcome"), None);
        assert_eq!(project("com.microsoft.VSCode", ""), None);
    }

    #[test]
    fn cursor_project_name() {
        assert_eq!(
            project("com.todesktop.230313mzl4w4u92", "App.tsx — record"),
            Some("record".to_string())
        );
    }

    #[test]
    fn zed_project_and_detail() {
        let ctx = detect("dev.zed.Zed", "main.rs — record").unwrap();
        assert_eq!(ctx.project, "record");
        assert_eq!(ctx.detail.as_deref(), Some("main.rs"));
    }

    #[test]
    fn jetbrains_project_and_detail() {
        let ctx = detect("com.jetbrains.intellij", "my-service \u{2013} Main.kt").unwrap();
        assert_eq!(ctx.project, "my-service");
        assert_eq!(ctx.detail.as_deref(), Some("Main.kt"));

        let ctx = detect("com.jetbrains.WebStorm", "frontend - package.json").unwrap();
        assert_eq!(ctx.project, "frontend");
        assert_eq!(ctx.detail.as_deref(), Some("package.json"));
    }

    #[test]
    fn xcode_project_and_detail() {
        let ctx = detect("com.apple.dt.Xcode", "MyApp — ContentView.swift").unwrap();
        assert_eq!(ctx.project, "MyApp");
        assert_eq!(ctx.detail.as_deref(), Some("ContentView.swift"));

        assert_eq!(project("com.apple.dt.Xcode", "Welcome to Xcode"), None);
    }

    #[test]
    fn terminal_project_from_path() {
        assert_eq!(
            project(
                "com.apple.Terminal",
                "user@host: ~/Documents/Personal/record"
            ),
            Some("record".to_string())
        );
        assert_eq!(
            project("com.googlecode.iterm2", "dirk@mac:~/projects/api-service"),
            Some("api-service".to_string())
        );
    }

    #[test]
    fn terminal_home_only() {
        assert_eq!(project("com.apple.Terminal", "user@host: ~"), None);
    }

    #[test]
    fn browser_site_and_detail() {
        let ctx = detect("com.google.Chrome", "Issues · record - GitHub").unwrap();
        assert_eq!(ctx.project, "GitHub");
        assert_eq!(ctx.detail.as_deref(), Some("Issues · record"));

        let ctx = detect("com.apple.Safari", "Dashboard | Vercel").unwrap();
        assert_eq!(ctx.project, "Vercel");
        assert_eq!(ctx.detail.as_deref(), Some("Dashboard"));
    }

    #[test]
    fn browser_skips_generic() {
        assert_eq!(
            project("com.google.Chrome", "New Tab - Google Chrome"),
            None
        );
    }

    #[test]
    fn teams_org_and_chat() {
        let ctx = detect(
            "com.microsoft.teams2",
            "Chat | Ian Kavanagh | New Payment Innovation Limited | Dirk.Stoffberg@infinitepay.tech | Microsoft Teams",
        ).unwrap();
        assert_eq!(ctx.project, "New Payment Innovation Limited");
        assert_eq!(ctx.detail.as_deref(), Some("Chat: Ian Kavanagh"));
    }

    #[test]
    fn teams_meeting() {
        let ctx = detect(
            "com.microsoft.teams2",
            "Daily Stand Up | Cloud Direct | Dirk.Beukes@clouddirect.net | Microsoft Teams",
        )
        .unwrap();
        assert_eq!(ctx.project, "Cloud Direct");
        assert_eq!(ctx.detail.as_deref(), Some("Daily Stand Up"));
    }

    #[test]
    fn unknown_app_returns_none() {
        assert_eq!(project("com.spotify.client", "Spotify Premium"), None);
    }
}
