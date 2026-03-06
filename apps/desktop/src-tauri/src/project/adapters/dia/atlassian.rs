use super::{clean_title, SiteAdapter};
use crate::project::DetectedContext;

pub struct AtlassianSite;

impl SiteAdapter for AtlassianSite {
    fn matches(&self, host: &str) -> bool {
        host.contains("atlassian.net")
    }

    fn extract(&self, host: &str, path: &str, title: &str, _url: &str) -> Option<DetectedContext> {
        if path.starts_with("browse/") {
            let ticket = path.strip_prefix("browse/").unwrap_or("");
            let project_key = ticket.split('-').next().unwrap_or("").to_string();
            if project_key.is_empty() {
                return None;
            }
            return Some(DetectedContext {
                project: project_key,
                detail: Some(ticket.to_string()),
            });
        }

        if path.contains("wiki/") {
            let site = host.split('.').next().unwrap_or(host);
            let detail = title
                .rsplit_once(" - Confluence")
                .map(|(before, _)| before)
                .unwrap_or(title)
                .rsplit_once(" - ")
                .map(|(before, _)| before)
                .unwrap_or(title)
                .trim()
                .to_string();
            return Some(DetectedContext {
                project: format!("{site} wiki"),
                detail: if detail.is_empty() {
                    None
                } else {
                    Some(detail)
                },
            });
        }

        let site = host.split('.').next().unwrap_or(host);
        Some(DetectedContext {
            project: site.to_string(),
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
    fn jira_ticket() {
        let ctx = resolve(
            "https://newpayment.atlassian.net/browse/IPT-492",
            "[IPT-492] Storage Tables for Verify Module - Jira",
        )
        .unwrap();
        assert_eq!(ctx.project, "IPT");
        assert_eq!(ctx.detail.as_deref(), Some("IPT-492"));
    }

    #[test]
    fn confluence_page() {
        let ctx = resolve(
            "https://newpayment.atlassian.net/wiki/spaces/IP/pages/837648385/Verify+Module",
            "Verify Module: Implementation Guide - Infinite Payments - Confluence",
        )
        .unwrap();
        assert_eq!(ctx.project, "newpayment wiki");
        assert_eq!(
            ctx.detail.as_deref(),
            Some("Verify Module: Implementation Guide")
        );
    }
}
