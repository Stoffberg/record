use super::{AgentProvider, AgentWorkSlice};
use chrono::{TimeZone, Utc};
use std::path::PathBuf;

const MERGE_GAP_MS: i64 = 60_000;

pub struct OpencodeProvider;

fn db_path() -> Option<PathBuf> {
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

struct RawSlice {
    start_ms: i64,
    end_ms: i64,
}

fn build_work_slices(
    conn: &rusqlite::Connection,
    session_id: &str,
    directory: &str,
    agent_name: &str,
) -> Vec<AgentWorkSlice> {
    let project = match project_from_directory(directory) {
        Some(p) => p,
        None => return vec![],
    };

    let mut stmt = match conn.prepare(
        "SELECT time_created, time_updated FROM message
         WHERE session_id = ?1 AND json_extract(data, '$.role') = 'assistant'
         ORDER BY time_created ASC",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    let raw: Vec<RawSlice> = match stmt.query_map(rusqlite::params![session_id], |row| {
        Ok(RawSlice {
            start_ms: row.get(0)?,
            end_ms: row.get(1)?,
        })
    }) {
        Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
        Err(_) => return vec![],
    };

    if raw.is_empty() {
        return vec![];
    }

    let mut blocks: Vec<(i64, i64)> = vec![];
    let mut block_start = raw[0].start_ms;
    let mut block_end = raw[0].end_ms;

    for slice in raw.iter().skip(1) {
        if slice.start_ms - block_end <= MERGE_GAP_MS {
            block_end = block_end.max(slice.end_ms);
        } else {
            blocks.push((block_start, block_end));
            block_start = slice.start_ms;
            block_end = slice.end_ms;
        }
    }
    blocks.push((block_start, block_end));

    blocks
        .into_iter()
        .filter_map(|(start_ms, end_ms)| {
            let duration_ms = end_ms - start_ms;
            if duration_ms < 1000 {
                return None;
            }
            let started_at = Utc.timestamp_millis_opt(start_ms).single()?;
            let ended_at = Utc.timestamp_millis_opt(end_ms).single()?;
            Some(AgentWorkSlice {
                agent: agent_name.to_string(),
                project: project.clone(),
                session_ref: session_id.to_string(),
                started_at,
                ended_at,
                duration_secs: duration_ms / 1000,
            })
        })
        .collect()
}

impl AgentProvider for OpencodeProvider {
    fn name(&self) -> &str {
        "opencode"
    }

    fn scan(&self, since_ms: i64) -> Vec<AgentWorkSlice> {
        let path = match db_path() {
            Some(p) if p.exists() => p,
            _ => return vec![],
        };

        let conn = match rusqlite::Connection::open_with_flags(
            &path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            Ok(c) => c,
            Err(_) => return vec![],
        };

        let mut stmt = match conn.prepare(
            "SELECT id, directory FROM session
             WHERE parent_id IS NULL AND directory != '/'
             AND time_updated >= ?1",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let sessions: Vec<(String, String)> = match stmt
            .query_map(rusqlite::params![since_ms], |row| {
                Ok((row.get(0)?, row.get(1)?))
            }) {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(_) => return vec![],
        };

        let mut all_slices = Vec::new();
        for (session_id, directory) in &sessions {
            let slices = build_work_slices(&conn, session_id, directory, self.name());
            all_slices.extend(slices);
        }
        all_slices
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE session (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                parent_id TEXT,
                slug TEXT NOT NULL,
                directory TEXT NOT NULL,
                title TEXT NOT NULL,
                version TEXT NOT NULL,
                time_created INTEGER NOT NULL,
                time_updated INTEGER NOT NULL
            );
            CREATE TABLE message (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                time_created INTEGER NOT NULL,
                time_updated INTEGER NOT NULL,
                data TEXT NOT NULL
            );",
        )
        .unwrap();
        conn
    }

    fn insert_session(conn: &rusqlite::Connection, id: &str, dir: &str, updated_ms: i64) {
        conn.execute(
            "INSERT INTO session (id, project_id, parent_id, slug, directory, title, version, time_created, time_updated)
             VALUES (?1, 'proj1', NULL, 'test', ?2, 'Test', '1.0', ?3, ?3)",
            rusqlite::params![id, dir, updated_ms],
        )
        .unwrap();
    }

    fn insert_message(
        conn: &rusqlite::Connection,
        id: &str,
        session_id: &str,
        role: &str,
        created_ms: i64,
        updated_ms: i64,
    ) {
        conn.execute(
            "INSERT INTO message (id, session_id, time_created, time_updated, data)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                id,
                session_id,
                created_ms,
                updated_ms,
                format!(r#"{{"role":"{}"}}"#, role)
            ],
        )
        .unwrap();
    }

    #[test]
    fn builds_single_block_from_consecutive_messages() {
        let conn = setup_test_db();
        let base = 1700000000000i64;
        insert_session(
            &conn,
            "s1",
            "/Users/test/Documents/Work/myapp",
            base + 30000,
        );
        insert_message(&conn, "m1", "s1", "user", base, base);
        insert_message(&conn, "m2", "s1", "assistant", base + 1000, base + 5000);
        insert_message(&conn, "m3", "s1", "assistant", base + 6000, base + 10000);
        insert_message(&conn, "m4", "s1", "assistant", base + 11000, base + 15000);

        let slices = build_work_slices(&conn, "s1", "/Users/test/Documents/Work/myapp", "opencode");
        assert_eq!(slices.len(), 1);
        assert_eq!(slices[0].project, "myapp");
        assert_eq!(slices[0].duration_secs, 14);
        assert_eq!(slices[0].session_ref, "s1");
    }

    #[test]
    fn splits_on_gap_exceeding_threshold() {
        let conn = setup_test_db();
        let base = 1700000000000i64;
        insert_session(
            &conn,
            "s1",
            "/Users/test/Documents/Work/myapp",
            base + 200000,
        );
        insert_message(&conn, "m1", "s1", "assistant", base, base + 5000);
        insert_message(&conn, "m2", "s1", "assistant", base + 6000, base + 10000);
        insert_message(&conn, "m3", "s1", "assistant", base + 120000, base + 130000);
        insert_message(&conn, "m4", "s1", "assistant", base + 131000, base + 140000);

        let slices = build_work_slices(&conn, "s1", "/Users/test/Documents/Work/myapp", "opencode");
        assert_eq!(slices.len(), 2);
        assert_eq!(slices[0].duration_secs, 10);
        assert_eq!(slices[1].duration_secs, 20);
    }

    #[test]
    fn ignores_user_messages() {
        let conn = setup_test_db();
        let base = 1700000000000i64;
        insert_session(
            &conn,
            "s1",
            "/Users/test/Documents/Work/myapp",
            base + 20000,
        );
        insert_message(&conn, "m1", "s1", "user", base, base + 1000);
        insert_message(&conn, "m2", "s1", "assistant", base + 2000, base + 8000);
        insert_message(&conn, "m3", "s1", "user", base + 9000, base + 10000);
        insert_message(&conn, "m4", "s1", "assistant", base + 11000, base + 15000);

        let slices = build_work_slices(&conn, "s1", "/Users/test/Documents/Work/myapp", "opencode");
        assert_eq!(slices.len(), 1);
        assert_eq!(slices[0].duration_secs, 13);
    }

    #[test]
    fn skips_shallow_directories() {
        let conn = setup_test_db();
        let base = 1700000000000i64;

        let home = std::env::var("HOME").unwrap();
        let shallow = format!("{}/Documents/Work", home);

        insert_session(&conn, "s1", &shallow, base + 10000);
        insert_message(&conn, "m1", "s1", "assistant", base, base + 5000);

        let slices = build_work_slices(&conn, "s1", &shallow, "opencode");
        assert!(slices.is_empty());
    }

    #[test]
    fn filters_sub_second_slices() {
        let conn = setup_test_db();
        let base = 1700000000000i64;
        insert_session(&conn, "s1", "/Users/test/Documents/Work/myapp", base + 5000);
        insert_message(&conn, "m1", "s1", "assistant", base, base + 500);

        let slices = build_work_slices(&conn, "s1", "/Users/test/Documents/Work/myapp", "opencode");
        assert!(slices.is_empty());
    }

    #[test]
    fn scan_reads_sessions_since_cursor() {
        let conn = setup_test_db();
        let base = 1700000000000i64;
        insert_session(&conn, "s1", "/Users/test/Documents/Work/old", base);
        insert_message(&conn, "m1", "s1", "assistant", base - 10000, base - 5000);

        insert_session(&conn, "s2", "/Users/test/Documents/Work/new", base + 100000);
        insert_message(&conn, "m2", "s2", "assistant", base + 90000, base + 100000);

        let mut stmt = conn
            .prepare(
                "SELECT id, directory FROM session
                 WHERE parent_id IS NULL AND directory != '/'
                 AND time_updated >= ?1",
            )
            .unwrap();

        let sessions: Vec<(String, String)> = stmt
            .query_map(rusqlite::params![base + 1], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].0, "s2");
    }

    #[test]
    fn scan_with_real_db() {
        let path = match db_path() {
            Some(p) if p.exists() => p,
            _ => return,
        };

        let conn = rusqlite::Connection::open_with_flags(
            &path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .unwrap();

        let has_sessions: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM session WHERE directory != '/' AND parent_id IS NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();

        if !has_sessions {
            return;
        }

        let provider = OpencodeProvider;
        let slices = provider.scan(0);
        assert!(
            !slices.is_empty(),
            "should produce work slices from real opencode DB"
        );

        for slice in &slices {
            assert_eq!(slice.agent, "opencode");
            assert!(!slice.project.is_empty());
            assert!(slice.duration_secs > 0);
            assert!(slice.started_at < slice.ended_at);
        }
    }
}
