use super::{clean_title, SiteAdapter};
use crate::project::DetectedContext;

pub struct YouTubeSite;

impl SiteAdapter for YouTubeSite {
    fn matches(&self, host: &str) -> bool {
        host.contains("youtube.com")
    }

    fn extract(&self, host: &str, _path: &str, title: &str, _url: &str) -> Option<DetectedContext> {
        let project = if host == "music.youtube.com" {
            "YouTube Music"
        } else {
            "YouTube"
        };
        Some(DetectedContext {
            project: project.to_string(),
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
    fn music_separate_project() {
        let ctx = resolve("https://music.youtube.com/watch?v=abc", "Song Name").unwrap();
        assert_eq!(ctx.project, "YouTube Music");
        assert_eq!(ctx.detail.as_deref(), Some("Song Name"));
    }

    #[test]
    fn video() {
        let ctx = resolve(
            "https://www.youtube.com/watch?v=xyz",
            "Cool Video - YouTube",
        )
        .unwrap();
        assert_eq!(ctx.project, "YouTube");
        assert_eq!(ctx.detail.as_deref(), Some("Cool Video"));
    }
}
