use super::{clean_title, SiteAdapter};
use crate::project::DetectedContext;

pub struct ClaudeSite;

impl SiteAdapter for ClaudeSite {
    fn matches(&self, host: &str) -> bool {
        host == "claude.ai"
    }

    fn extract(
        &self,
        _host: &str,
        _path: &str,
        title: &str,
        _url: &str,
    ) -> Option<DetectedContext> {
        Some(DetectedContext {
            project: "Claude".to_string(),
            detail: Some(clean_title(title)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::DiaAdapter;
    use crate::project::DetectedContext;

    fn resolve(url: &str, title: &str) -> Option<DetectedContext> {
        DiaAdapter::new().resolve(url, title)
    }

    #[test]
    fn conversation() {
        let ctx = resolve(
            "https://claude.ai/chat/abc-123",
            "Finding your next product to build - Claude",
        )
        .unwrap();
        assert_eq!(ctx.project, "Claude");
        assert_eq!(
            ctx.detail.as_deref(),
            Some("Finding your next product to build")
        );
    }
}
