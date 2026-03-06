mod atlassian;
mod azure;
mod claude;
mod github;
mod google;
mod localhost;
mod youtube;

use crate::project::{DetectedContext, ProjectAdapter};
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;

const CHROME_EPOCH_OFFSET: i64 = 11_644_473_600;

pub trait SiteAdapter {
    fn matches(&self, host: &str) -> bool;
    fn extract(&self, host: &str, path: &str, title: &str, url: &str) -> Option<DetectedContext>;
}

pub fn host_from_url(url: &str) -> Option<&str> {
    let after_scheme = url
        .strip_prefix("https://")
        .or(url.strip_prefix("http://"))?;
    let host = after_scheme.split('/').next()?;
    let host = host.split(':').next()?;
    if host.is_empty() {
        return None;
    }
    Some(host)
}

pub fn path_from_url(url: &str) -> &str {
    let after_scheme = url
        .strip_prefix("https://")
        .or(url.strip_prefix("http://"))
        .unwrap_or(url);
    let raw = after_scheme
        .find('/')
        .map(|i| &after_scheme[i + 1..])
        .unwrap_or("");
    raw.split('?')
        .next()
        .unwrap_or("")
        .split('#')
        .next()
        .unwrap_or("")
        .trim_end_matches('/')
}

pub fn clean_title(title: &str) -> String {
    let seps: &[&str] = &[" | ", " - ", " \u{2013} ", " \u{2014} ", " \u{00b7} "];
    for sep in seps {
        if let Some(pos) = title.rfind(sep) {
            let before = title[..pos].trim();
            if !before.is_empty() {
                return before.to_string();
            }
        }
    }
    title.trim().to_string()
}

fn normalize_host(host: &str) -> String {
    let host = host.strip_prefix("www.").unwrap_or(host);
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() >= 3 {
        return parts[0].to_string();
    }
    host.to_string()
}

fn history_db_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join("Library/Application Support/Dia/User Data/Default/History"))
}

pub struct DiaAdapter {
    last_copy: Mutex<Option<SystemTime>>,
    sites: Vec<Box<dyn SiteAdapter + Send + Sync>>,
}

impl DiaAdapter {
    pub fn new() -> Self {
        Self {
            last_copy: Mutex::new(None),
            sites: vec![
                Box::new(github::GitHubSite),
                Box::new(atlassian::AtlassianSite),
                Box::new(google::GoogleSite),
                Box::new(youtube::YouTubeSite),
                Box::new(claude::ClaudeSite),
                Box::new(azure::AzureSite),
                Box::new(localhost::LocalhostSite),
            ],
        }
    }

    fn resolve(&self, url: &str, title: &str) -> Option<DetectedContext> {
        let host = host_from_url(url)?;
        let path = path_from_url(url);

        for site in &self.sites {
            if site.matches(host) {
                return site.extract(host, path, title, url);
            }
        }

        let project = normalize_host(host);
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

impl ProjectAdapter for DiaAdapter {
    fn bundle_ids(&self) -> &[&str] {
        &["company.thebrowser.dia"]
    }

    fn extract(&self, _title: &str) -> Option<DetectedContext> {
        None
    }

    fn extract_at(&self, _window_title: &str, timestamp: DateTime<Utc>) -> Option<DetectedContext> {
        let db_path = history_db_path()?;
        if !db_path.exists() {
            return None;
        }

        let tmp_path = std::env::temp_dir().join("record_dia_history.db");

        let source_mtime = std::fs::metadata(&db_path).ok()?.modified().ok()?;
        let needs_copy = {
            let last = self.last_copy.lock().ok()?;
            last.map_or(true, |t| source_mtime > t)
        };
        if needs_copy {
            std::fs::copy(&db_path, &tmp_path).ok()?;
            if let Ok(mut last) = self.last_copy.lock() {
                *last = Some(source_mtime);
            }
        }

        let conn = rusqlite::Connection::open_with_flags(
            &tmp_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .ok()?;

        let chrome_time = (timestamp.timestamp() + CHROME_EPOCH_OFFSET) * 1_000_000;

        let row: Option<(String, String)> = conn
            .query_row(
                "SELECT u.url, u.title FROM visits v
                 JOIN urls u ON v.url = u.id
                 WHERE v.visit_time <= ?1
                 AND (v.visit_duration = 0 OR v.visit_time + v.visit_duration >= ?1)
                 ORDER BY v.visit_time DESC LIMIT 1",
                rusqlite::params![chrome_time],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();

        if let Some((url, title)) = row {
            return self.resolve(&url, &title);
        }

        let row: Option<(String, String)> = conn
            .query_row(
                "SELECT u.url, u.title FROM visits v
                 JOIN urls u ON v.url = u.id
                 WHERE v.visit_time <= ?1
                 ORDER BY v.visit_time DESC LIMIT 1",
                rusqlite::params![chrome_time],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();

        let (url, title) = row?;
        self.resolve(&url, &title)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(url: &str, title: &str) -> Option<DetectedContext> {
        DiaAdapter::new().resolve(url, title)
    }

    #[test]
    fn fallback_plain_domain() {
        let ctx = resolve("https://stoff.dev/projects/", "Projects | Dirk Stoffberg").unwrap();
        assert_eq!(ctx.project, "stoff.dev");
        assert_eq!(ctx.detail.as_deref(), Some("Projects"));
    }

    #[test]
    fn fallback_subdomain_normalizes_to_name() {
        let ctx = resolve(
            "https://regress.stoff.dev/projects/abc/journey",
            "Regress | Ship Fast. Break Nothing.",
        )
        .unwrap();
        assert_eq!(ctx.project, "regress");
        assert_eq!(ctx.detail.as_deref(), Some("Regress"));
    }

    #[test]
    fn fallback_strips_www() {
        let ctx = resolve("https://www.example.com/page", "Example Page - Example.com").unwrap();
        assert_eq!(ctx.project, "example.com");
    }

    #[test]
    fn normalize_host_subdomain() {
        assert_eq!(normalize_host("regress.stoff.dev"), "regress");
        assert_eq!(normalize_host("app.example.com"), "app");
    }

    #[test]
    fn normalize_host_bare_domain() {
        assert_eq!(normalize_host("stoff.dev"), "stoff.dev");
        assert_eq!(normalize_host("example.com"), "example.com");
    }

    #[test]
    fn normalize_host_strips_www() {
        assert_eq!(normalize_host("www.example.com"), "example.com");
    }

    #[test]
    fn invalid_url() {
        assert_eq!(resolve("not-a-url", "Test"), None);
    }

    #[test]
    fn extract_defers_to_history() {
        let adapter = DiaAdapter::new();
        assert_eq!(adapter.extract("Fix login bug — Dia"), None);
        assert_eq!(adapter.extract(""), None);
    }

    #[test]
    fn extract_at_reads_history() {
        let db_path = match history_db_path() {
            Some(p) if p.exists() => p,
            _ => return,
        };

        let tmp_path = std::env::temp_dir().join("record_dia_history_test.db");
        if std::fs::copy(&db_path, &tmp_path).is_err() {
            return;
        }

        let conn = rusqlite::Connection::open_with_flags(
            &tmp_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        );
        if conn.is_err() {
            return;
        }

        let has_visits: bool = conn
            .unwrap()
            .query_row("SELECT COUNT(*) > 0 FROM visits", [], |row| row.get(0))
            .unwrap_or(false);

        if !has_visits {
            return;
        }

        let adapter = DiaAdapter::new();
        let ctx = adapter.extract_at("Dia", chrono::Utc::now());
        assert!(ctx.is_some(), "should detect context from Dia history");
        let ctx = ctx.unwrap();
        assert!(!ctx.project.is_empty(), "project should not be empty");
    }
}
