use super::{clean_title, path_from_url, SiteAdapter};
use crate::project::DetectedContext;

pub struct AzureSite;

impl SiteAdapter for AzureSite {
    fn matches(&self, host: &str) -> bool {
        host.contains("azure.com")
    }

    fn extract(&self, _host: &str, _path: &str, title: &str, url: &str) -> Option<DetectedContext> {
        let segments: Vec<&str> = path_from_url(url).splitn(3, '/').collect();
        let project = if segments.len() >= 2 {
            segments[1].to_string()
        } else {
            "Azure DevOps".to_string()
        };
        Some(DetectedContext {
            project,
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
    fn pipeline() {
        let ctx = resolve(
            "https://dev.azure.com/CloudDirect-Dev/Provide/_build/results?buildId=1631",
            "Pipelines - Run 20260304.2 logs",
        )
        .unwrap();
        assert_eq!(ctx.project, "Provide");
        assert_eq!(ctx.detail.as_deref(), Some("Pipelines"));
    }
}
