use super::{clean_title, SiteAdapter};
use crate::project::DetectedContext;

pub struct LocalhostSite;

fn project_from_title(title: &str) -> Option<String> {
    let seps: &[&str] = &[" | ", " - ", " \u{2013} ", " \u{2014} ", " \u{00b7} "];
    for sep in seps {
        if let Some(pos) = title.rfind(sep) {
            let suffix = title[pos + sep.len()..].trim();
            if !suffix.is_empty() {
                return Some(suffix.to_lowercase());
            }
        }
    }
    None
}

impl SiteAdapter for LocalhostSite {
    fn matches(&self, host: &str) -> bool {
        host.starts_with("localhost") || host == "127.0.0.1"
    }

    fn extract(&self, _host: &str, _path: &str, title: &str, url: &str) -> Option<DetectedContext> {
        let after_scheme = url
            .strip_prefix("https://")
            .or(url.strip_prefix("http://"))
            .unwrap_or(url);
        let host_port = after_scheme.split('/').next().unwrap_or("localhost");

        let project = project_from_title(title).unwrap_or_else(|| host_port.to_string());
        let detail = clean_title(title);

        Some(DetectedContext {
            project,
            detail: if detail.is_empty() {
                None
            } else {
                Some(detail)
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::DiaAdapter;
    use super::*;
    use crate::project::DetectedContext;

    fn resolve(url: &str, title: &str) -> Option<DetectedContext> {
        DiaAdapter::new().resolve(url, title)
    }

    #[test]
    fn extracts_project_from_title_suffix() {
        let ctx = resolve(
            "http://localhost:4200/lifecycle/applications/18",
            "Application | buddy",
        )
        .unwrap();
        assert_eq!(ctx.project, "buddy");
        assert_eq!(ctx.detail.as_deref(), Some("Application"));
    }

    #[test]
    fn falls_back_to_host_port() {
        let ctx = resolve("http://localhost:5174/", "").unwrap();
        assert_eq!(ctx.project, "localhost:5174");
        assert_eq!(ctx.detail, None);
    }

    #[test]
    fn project_from_title_extracts_suffix() {
        assert_eq!(
            project_from_title("Application | buddy"),
            Some("buddy".to_string())
        );
        assert_eq!(
            project_from_title("Dashboard - Record"),
            Some("record".to_string())
        );
    }

    #[test]
    fn project_from_title_none_for_bare() {
        assert_eq!(project_from_title("localhost:4200"), None);
        assert_eq!(project_from_title(""), None);
    }
}
