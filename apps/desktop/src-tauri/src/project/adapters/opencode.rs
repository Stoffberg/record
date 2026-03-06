use crate::project::{DetectedContext, ProjectAdapter};
use chrono::{DateTime, Utc};
use std::path::PathBuf;

pub struct OpencodeAdapter;

fn opencode_db_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".local/share/opencode/opencode.db"))
}

fn project_from_directory(dir: &str) -> Option<String> {
    let path = std::path::Path::new(dir);
    let name = path.file_name()?.to_str()?;
    if name.is_empty() || name == "/" {
        return None;
    }

    if let Ok(home) = std::env::var("HOME") {
        let home_path = std::path::Path::new(&home);
        if let Ok(relative) = path.strip_prefix(home_path) {
            let components: Vec<_> = relative.components().collect();
            if components.len() <= 2 {
                return None;
            }
        }
    }

    Some(name.to_string())
}

impl ProjectAdapter for OpencodeAdapter {
    fn bundle_ids(&self) -> &[&str] {
        &["ai.opencode.desktop"]
    }

    fn extract(&self, title: &str) -> Option<DetectedContext> {
        let sep = if title.contains(" — ") {
            " — "
        } else if title.contains(" - ") {
            " - "
        } else {
            let trimmed = title.trim();
            if trimmed.is_empty() || trimmed == "opencode" || trimmed == "OpenCode" {
                return None;
            }
            return project_from_path(trimmed).map(|p| DetectedContext {
                project: p,
                detail: None,
            });
        };

        let parts: Vec<&str> = title.split(sep).collect();
        if parts.len() < 2 {
            return None;
        }

        for part in parts.iter().rev() {
            let trimmed = part.trim();
            if trimmed == "opencode" || trimmed == "OpenCode" || trimmed.is_empty() {
                continue;
            }
            return project_from_path(trimmed).map(|p| DetectedContext {
                project: p,
                detail: None,
            });
        }

        None
    }

    fn extract_at(&self, _window_title: &str, timestamp: DateTime<Utc>) -> Option<DetectedContext> {
        let db_path = opencode_db_path()?;
        if !db_path.exists() {
            return None;
        }

        let conn = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .ok()?;

        let ts_ms = timestamp.timestamp_millis();

        let row: (String, String) = conn
            .query_row(
                "SELECT directory, title FROM session
                 WHERE time_updated <= ?1 AND directory != '/'
                 AND parent_id IS NULL
                 ORDER BY time_updated DESC LIMIT 1",
                rusqlite::params![ts_ms],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok()?;

        let project = project_from_directory(&row.0)?;
        let detail = if row.1.is_empty() { None } else { Some(row.1) };

        Some(DetectedContext { project, detail })
    }
}

fn project_from_path(s: &str) -> Option<String> {
    let is_path = s.starts_with("~/") || s.starts_with('/') || s.contains('/');

    if is_path {
        let cleaned = s
            .trim_start_matches("~/")
            .trim_start_matches('/')
            .trim_end_matches('/');

        let segments: Vec<&str> = cleaned.split('/').filter(|seg| !seg.is_empty()).collect();
        if segments.len() <= 2 {
            return None;
        }

        let project = segments.last()?;
        if project.is_empty() {
            return None;
        }
        return Some(project.to_string());
    }

    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_title() {
        let adapter = OpencodeAdapter;
        let ctx = adapter
            .extract("~/Documents/Personal/record — opencode")
            .unwrap();
        assert_eq!(ctx.project, "record");
    }

    #[test]
    fn project_name_title() {
        let adapter = OpencodeAdapter;
        let ctx = adapter.extract("record - opencode").unwrap();
        assert_eq!(ctx.project, "record");
    }

    #[test]
    fn bare_path() {
        let adapter = OpencodeAdapter;
        let ctx = adapter.extract("~/projects/workspace/my-app").unwrap();
        assert_eq!(ctx.project, "my-app");
    }

    #[test]
    fn bare_path_too_shallow() {
        let adapter = OpencodeAdapter;
        assert_eq!(adapter.extract("~/projects/my-app"), None);
    }

    #[test]
    fn empty_and_generic() {
        let adapter = OpencodeAdapter;
        assert_eq!(adapter.extract(""), None);
        assert_eq!(adapter.extract("opencode"), None);
        assert_eq!(adapter.extract("OpenCode"), None);
    }

    #[test]
    fn project_from_directory_extracts_name() {
        assert_eq!(
            project_from_directory("/Users/dirk/Documents/Personal/record"),
            Some("record".to_string())
        );
        assert_eq!(
            project_from_directory("/Users/dirk/Documents/Payment/buddy"),
            Some("buddy".to_string())
        );
    }

    #[test]
    fn project_from_directory_skips_root() {
        assert_eq!(project_from_directory("/"), None);
    }

    #[test]
    fn project_from_directory_skips_workspace_roots() {
        assert_eq!(
            project_from_directory(&format!(
                "{}/Documents/Personal",
                std::env::var("HOME").unwrap()
            )),
            None
        );
        assert_eq!(
            project_from_directory(&format!(
                "{}/Documents/Health",
                std::env::var("HOME").unwrap()
            )),
            None
        );
        assert_eq!(
            project_from_directory(&format!(
                "{}/Documents/Payment",
                std::env::var("HOME").unwrap()
            )),
            None
        );
        assert_eq!(
            project_from_directory(&format!("{}/Documents", std::env::var("HOME").unwrap())),
            None
        );
    }

    #[test]
    fn extract_at_reads_opencode_db() {
        let db_path = match opencode_db_path() {
            Some(p) if p.exists() => p,
            _ => return,
        };

        let conn = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .unwrap();

        let has_sessions: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM session WHERE directory != '/'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        if !has_sessions {
            return;
        }

        let adapter = OpencodeAdapter;
        let ctx = adapter.extract_at("OpenCode", Utc::now()).unwrap();
        assert!(
            !ctx.project.is_empty(),
            "should detect project from opencode DB"
        );
        assert!(ctx.detail.is_some(), "should have session title as detail");
    }
}
