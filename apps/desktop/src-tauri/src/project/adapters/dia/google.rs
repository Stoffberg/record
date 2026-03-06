use super::{clean_title, SiteAdapter};
use crate::project::DetectedContext;

pub struct GoogleSite;

impl SiteAdapter for GoogleSite {
    fn matches(&self, host: &str) -> bool {
        host.ends_with("google.com")
    }

    fn extract(&self, host: &str, _path: &str, title: &str, _url: &str) -> Option<DetectedContext> {
        match host {
            "meet.google.com" => {
                let detail = title
                    .strip_prefix("Meet - ")
                    .or(title.strip_prefix("Meet \u{2013} "))
                    .unwrap_or(title)
                    .trim();
                let detail = if detail.is_empty()
                    || detail.eq_ignore_ascii_case("Google Meet")
                    || detail.eq_ignore_ascii_case("Google Meet Landing")
                {
                    None
                } else {
                    Some(detail.to_string())
                };
                Some(DetectedContext {
                    project: "Google Meet".to_string(),
                    detail,
                })
            }
            "calendar.google.com" => Some(DetectedContext {
                project: "Google Calendar".to_string(),
                detail: None,
            }),
            "docs.google.com" => Some(DetectedContext {
                project: "Google Docs".to_string(),
                detail: Some(clean_title(title)),
            }),
            "drive.google.com" => Some(DetectedContext {
                project: "Google Drive".to_string(),
                detail: Some(clean_title(title)),
            }),
            "mail.google.com" => Some(DetectedContext {
                project: "Gmail".to_string(),
                detail: None,
            }),
            _ => {
                let detail = clean_title(title);
                Some(DetectedContext {
                    project: "Google".to_string(),
                    detail: if detail.is_empty() {
                        None
                    } else {
                        Some(detail)
                    },
                })
            }
        }
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
    fn meet_extracts_meeting_name() {
        let ctx = resolve(
            "https://meet.google.com/ofm-trdz-awq?authuser=0",
            "Meet - BCBST evidence pdf generation co development",
        )
        .unwrap();
        assert_eq!(ctx.project, "Google Meet");
        assert_eq!(
            ctx.detail.as_deref(),
            Some("BCBST evidence pdf generation co development")
        );
    }

    #[test]
    fn calendar_no_detail() {
        let ctx = resolve(
            "https://calendar.google.com/calendar/u/0/r/day",
            "Cohere Health, Inc - Calendar - Wednesday, March 4, 2026, today",
        )
        .unwrap();
        assert_eq!(ctx.project, "Google Calendar");
        assert_eq!(ctx.detail, None);
    }

    #[test]
    fn search() {
        let ctx = resolve(
            "https://www.google.com/search?q=rust+sqlite&sourceid=chrome",
            "rust sqlite - Google Search",
        )
        .unwrap();
        assert_eq!(ctx.project, "Google");
        assert_eq!(ctx.detail.as_deref(), Some("rust sqlite"));
    }
}
