use super::{clean_title, SiteAdapter};
use crate::project::DetectedContext;

pub struct GitHubSite;

impl SiteAdapter for GitHubSite {
    fn matches(&self, host: &str) -> bool {
        host.contains("github.com")
    }

    fn extract(&self, _host: &str, path: &str, title: &str, _url: &str) -> Option<DetectedContext> {
        let segments: Vec<&str> = path.splitn(3, '/').collect();
        if segments.len() < 2 || segments[0].is_empty() || segments[1].is_empty() {
            return Some(DetectedContext {
                project: "GitHub".to_string(),
                detail: Some(clean_title(title)),
            });
        }
        let project = segments[1].to_string();
        let detail = if segments.len() == 3 && !segments[2].is_empty() {
            Some(segments[2].to_string())
        } else {
            None
        };
        Some(DetectedContext { project, detail })
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
    fn groups_by_repo_name() {
        let ctx = resolve(
            "https://github.com/NewPaymentInnovation/buddy/pull/906",
            "fix: IPT-660 / Company Information - GitHub",
        )
        .unwrap();
        assert_eq!(ctx.project, "buddy");
        assert_eq!(ctx.detail.as_deref(), Some("pull/906"));
    }

    #[test]
    fn different_orgs_same_repo() {
        let ctx = resolve(
            "https://github.com/CohereHealth/clinical-intelligence-ai/actions/workflows/autoci.yml",
            "Autoci Deployment · Workflow runs",
        )
        .unwrap();
        assert_eq!(ctx.project, "clinical-intelligence-ai");
        assert_eq!(ctx.detail.as_deref(), Some("actions/workflows/autoci.yml"));
    }

    #[test]
    fn actions_run() {
        let ctx = resolve(
            "https://github.com/Stoffberg/record/actions/runs/22738240019",
            "Mark onboarding complete · Stoffberg/record@1acb9d9",
        )
        .unwrap();
        assert_eq!(ctx.project, "record");
        assert_eq!(ctx.detail.as_deref(), Some("actions/runs/22738240019"));
    }

    #[test]
    fn pr_changes_strips_fragment() {
        let ctx = resolve(
            "https://github.com/NewPaymentInnovation/buddy/pull/906/changes#diff-abc123",
            "fix: IPT-660 - GitHub",
        )
        .unwrap();
        assert_eq!(ctx.project, "buddy");
        assert_eq!(ctx.detail.as_deref(), Some("pull/906/changes"));
    }

    #[test]
    fn repo_root_no_detail() {
        let ctx = resolve(
            "https://github.com/Stoffberg/record",
            "Stoffberg/record: Privacy-first activity tracker",
        )
        .unwrap();
        assert_eq!(ctx.project, "record");
        assert_eq!(ctx.detail, None);
    }

    #[test]
    fn non_repo_page() {
        let ctx = resolve(
            "https://github.com/features/copilot/cli",
            "GitHub Copilot CLI",
        )
        .unwrap();
        assert_eq!(ctx.project, "copilot");
        assert_eq!(ctx.detail.as_deref(), Some("cli"));
    }
}
