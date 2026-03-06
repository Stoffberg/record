use crate::types::{AppSession, AppUsage, DailySummary, Heartbeat, TrackerConfig};
use chrono::{DateTime, Duration, NaiveDate, NaiveTime, Utc};
use rusqlite::Connection;

fn day_bounds_utc(date: &str, tz_offset_minutes: i32) -> (String, String) {
    let d = NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap();
    let local_midnight = d.and_time(NaiveTime::MIN);
    let utc_start = local_midnight - Duration::minutes(tz_offset_minutes as i64);
    let utc_end = utc_start + Duration::days(1);
    (
        utc_start.and_utc().to_rfc3339(),
        utc_end.and_utc().to_rfc3339(),
    )
}

pub struct SessionStore {
    conn: Connection,
    config: TrackerConfig,
}

impl SessionStore {
    pub fn new(conn: Connection, config: TrackerConfig) -> rusqlite::Result<Self> {
        let store = Self { conn, config };
        store.init_schema()?;
        Ok(store)
    }

    pub fn record_heartbeat(&self, heartbeat: Heartbeat) -> rusqlite::Result<()> {
        let ts = heartbeat.timestamp.to_rfc3339();

        let merged = self.try_merge(&heartbeat)?;
        if merged {
            return Ok(());
        }

        self.conn.execute(
            "INSERT INTO app_sessions (app_name, bundle_id, window_title, started_at, ended_at, duration_secs, is_idle, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, '{}')",
            rusqlite::params![
                heartbeat.app_name,
                heartbeat.bundle_id,
                heartbeat.window_title,
                ts,
                ts,
                heartbeat.is_idle,
            ],
        )?;
        Ok(())
    }

    fn try_merge(&self, heartbeat: &Heartbeat) -> rusqlite::Result<bool> {
        let result: rusqlite::Result<(i64, String, String, String, bool)> = self.conn.query_row(
            "SELECT id, bundle_id, started_at, ended_at, is_idle FROM app_sessions ORDER BY id DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        );

        let (id, bundle_id, started_at_str, ended_at_str, is_idle) = match result {
            Ok(row) => row,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(false),
            Err(e) => return Err(e),
        };

        if bundle_id != heartbeat.bundle_id || is_idle != heartbeat.is_idle {
            return Ok(false);
        }

        let ended_at = DateTime::parse_from_rfc3339(&ended_at_str)
            .unwrap()
            .with_timezone(&Utc);
        let gap = (heartbeat.timestamp - ended_at).num_seconds();

        if gap > self.config.merge_gap_secs {
            return Ok(false);
        }

        let started_at = DateTime::parse_from_rfc3339(&started_at_str)
            .unwrap()
            .with_timezone(&Utc);
        let new_duration = (heartbeat.timestamp - started_at).num_seconds();

        self.conn.execute(
            "UPDATE app_sessions SET ended_at = ?1, duration_secs = ?2 WHERE id = ?3",
            rusqlite::params![heartbeat.timestamp.to_rfc3339(), new_duration, id],
        )?;

        Ok(true)
    }

    pub fn get_sessions(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> rusqlite::Result<Vec<AppSession>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, app_name, bundle_id, window_title, started_at, ended_at, duration_secs, is_idle, metadata
             FROM app_sessions
             WHERE started_at >= ?1 AND started_at <= ?2
             ORDER BY started_at ASC",
        )?;
        let rows = stmt.query_map(
            rusqlite::params![start.to_rfc3339(), end.to_rfc3339()],
            |row| {
                let started_str: String = row.get(4)?;
                let ended_str: String = row.get(5)?;
                Ok(AppSession {
                    id: row.get(0)?,
                    app_name: row.get(1)?,
                    bundle_id: row.get(2)?,
                    window_title: row.get(3)?,
                    started_at: DateTime::parse_from_rfc3339(&started_str)
                        .unwrap()
                        .with_timezone(&Utc),
                    ended_at: DateTime::parse_from_rfc3339(&ended_str)
                        .unwrap()
                        .with_timezone(&Utc),
                    duration_secs: row.get(6)?,
                    is_idle: row.get(7)?,
                    metadata: row.get(8)?,
                })
            },
        )?;
        rows.collect()
    }

    pub fn get_daily_summary(
        &self,
        date: &str,
        tz_offset_minutes: i32,
    ) -> rusqlite::Result<DailySummary> {
        let (start, end) = day_bounds_utc(date, tz_offset_minutes);

        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "DELETE FROM app_exclusions WHERE expires_at IS NOT NULL AND expires_at < ?1",
            rusqlite::params![now],
        )?;

        let (total_active, total_idle) = self.conn.query_row(
            "SELECT
                COALESCE(SUM(CASE WHEN s.is_idle = 0 THEN s.duration_secs ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN s.is_idle = 1 THEN s.duration_secs ELSE 0 END), 0)
             FROM app_sessions s
             LEFT JOIN app_exclusions e ON s.bundle_id = e.bundle_id
             WHERE s.started_at >= ?1 AND s.started_at < ?2
             AND e.bundle_id IS NULL",
            rusqlite::params![start, end],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )?;

        let mut stmt = self.conn.prepare(
            "SELECT s.app_name, s.bundle_id, SUM(s.duration_secs), COUNT(*)
             FROM app_sessions s
             LEFT JOIN app_exclusions e ON s.bundle_id = e.bundle_id
             WHERE s.started_at >= ?1 AND s.started_at < ?2 AND s.is_idle = 0
             AND e.bundle_id IS NULL
             GROUP BY s.bundle_id
             ORDER BY SUM(s.duration_secs) DESC",
        )?;

        let apps: Vec<AppUsage> = stmt
            .query_map(rusqlite::params![start, end], |row| {
                Ok(AppUsage {
                    app_name: row.get(0)?,
                    bundle_id: row.get(1)?,
                    total_secs: row.get(2)?,
                    session_count: row.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(DailySummary {
            date: date.to_string(),
            total_active_secs: total_active,
            total_idle_secs: total_idle,
            apps,
        })
    }

    pub fn get_app_averages(
        &self,
        bundle_id: &str,
        today: &str,
        tz_offset_minutes: i32,
    ) -> rusqlite::Result<(f64, f64)> {
        let today_date = NaiveDate::parse_from_str(today, "%Y-%m-%d").unwrap();
        let week_start_date = (today_date - Duration::days(6))
            .format("%Y-%m-%d")
            .to_string();
        let month_start_date = (today_date - Duration::days(29))
            .format("%Y-%m-%d")
            .to_string();

        let (week_start, _) = day_bounds_utc(&week_start_date, tz_offset_minutes);
        let (today_start, _) = day_bounds_utc(today, tz_offset_minutes);

        let (month_start, _) = day_bounds_utc(&month_start_date, tz_offset_minutes);

        let week_avg: f64 = self.conn.query_row(
            "SELECT COALESCE(CAST(SUM(duration_secs) AS REAL) / 7.0, 0.0)
             FROM app_sessions
             WHERE bundle_id = ?1 AND is_idle = 0
             AND started_at >= ?2 AND started_at < ?3",
            rusqlite::params![bundle_id, week_start, today_start],
            |row| row.get(0),
        )?;

        let month_avg: f64 = self.conn.query_row(
            "SELECT COALESCE(CAST(SUM(duration_secs) AS REAL) / 30.0, 0.0)
             FROM app_sessions
             WHERE bundle_id = ?1 AND is_idle = 0
             AND started_at >= ?2 AND started_at < ?3",
            rusqlite::params![bundle_id, month_start, today_start],
            |row| row.get(0),
        )?;

        Ok((week_avg, month_avg))
    }

    pub fn get_app_sessions(
        &self,
        date: &str,
        bundle_id: &str,
        tz_offset_minutes: i32,
    ) -> rusqlite::Result<Vec<AppSession>> {
        let (start, end) = day_bounds_utc(date, tz_offset_minutes);

        let mut stmt = self.conn.prepare(
            "SELECT id, app_name, bundle_id, window_title, started_at, ended_at, duration_secs, is_idle, metadata
             FROM app_sessions
             WHERE started_at >= ?1 AND started_at < ?2 AND bundle_id = ?3 AND is_idle = 0
             ORDER BY started_at ASC",
        )?;
        let rows = stmt.query_map(rusqlite::params![start, end, bundle_id], |row| {
            let started_str: String = row.get(4)?;
            let ended_str: String = row.get(5)?;
            Ok(AppSession {
                id: row.get(0)?,
                app_name: row.get(1)?,
                bundle_id: row.get(2)?,
                window_title: row.get(3)?,
                started_at: DateTime::parse_from_rfc3339(&started_str)
                    .unwrap()
                    .with_timezone(&Utc),
                ended_at: DateTime::parse_from_rfc3339(&ended_str)
                    .unwrap()
                    .with_timezone(&Utc),
                duration_secs: row.get(6)?,
                is_idle: row.get(7)?,
                metadata: row.get(8)?,
            })
        })?;
        rows.collect()
    }

    pub fn add_exclusion(
        &self,
        bundle_id: &str,
        app_name: &str,
        expires_at: Option<&str>,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO app_exclusions (bundle_id, app_name, expires_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![bundle_id, app_name, expires_at],
        )?;
        Ok(())
    }

    pub fn remove_exclusion(&self, bundle_id: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "DELETE FROM app_exclusions WHERE bundle_id = ?1",
            rusqlite::params![bundle_id],
        )?;
        Ok(())
    }

    pub fn get_exclusions(&self) -> rusqlite::Result<Vec<(String, String, Option<String>)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT bundle_id, app_name, expires_at FROM app_exclusions")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        rows.collect()
    }

    pub fn set_app_category(&self, bundle_id: &str, category: &str) -> rusqlite::Result<()> {
        if category == "neutral" {
            self.conn.execute(
                "DELETE FROM app_categories WHERE bundle_id = ?1",
                rusqlite::params![bundle_id],
            )?;
        } else {
            self.conn.execute(
                "INSERT OR REPLACE INTO app_categories (bundle_id, category) VALUES (?1, ?2)",
                rusqlite::params![bundle_id, category],
            )?;
        }
        Ok(())
    }

    pub fn get_app_category(&self, bundle_id: &str) -> rusqlite::Result<String> {
        let result: rusqlite::Result<String> = self.conn.query_row(
            "SELECT category FROM app_categories WHERE bundle_id = ?1",
            rusqlite::params![bundle_id],
            |row| row.get(0),
        );
        match result {
            Ok(cat) => Ok(cat),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok("neutral".to_string()),
            Err(e) => Err(e),
        }
    }

    pub fn get_all_categories(&self) -> rusqlite::Result<Vec<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT bundle_id, category FROM app_categories")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect()
    }

    pub fn is_excluded(&self, bundle_id: &str) -> rusqlite::Result<bool> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "DELETE FROM app_exclusions WHERE expires_at IS NOT NULL AND expires_at < ?1",
            rusqlite::params![now],
        )?;

        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM app_exclusions WHERE bundle_id = ?1",
            rusqlite::params![bundle_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn init_schema(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS app_sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                app_name TEXT NOT NULL,
                bundle_id TEXT NOT NULL,
                window_title TEXT DEFAULT '',
                started_at TEXT NOT NULL,
                ended_at TEXT NOT NULL,
                duration_secs INTEGER NOT NULL,
                is_idle BOOLEAN DEFAULT 0,
                metadata TEXT DEFAULT '{}'
            );
            CREATE INDEX IF NOT EXISTS idx_sessions_started ON app_sessions(started_at);
            CREATE INDEX IF NOT EXISTS idx_sessions_bundle ON app_sessions(bundle_id);
            CREATE INDEX IF NOT EXISTS idx_sessions_idle ON app_sessions(is_idle);

            CREATE TABLE IF NOT EXISTS app_exclusions (
                bundle_id TEXT PRIMARY KEY,
                app_name TEXT NOT NULL,
                expires_at TEXT
            );

            CREATE TABLE IF NOT EXISTS app_categories (
                bundle_id TEXT PRIMARY KEY,
                category TEXT NOT NULL DEFAULT 'neutral'
            );

            PRAGMA journal_mode=WAL;",
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TrackerConfig;
    use chrono::Utc;
    use rusqlite::Connection;

    fn test_store() -> SessionStore {
        let conn = Connection::open_in_memory().unwrap();
        SessionStore::new(conn, TrackerConfig::default()).unwrap()
    }

    fn heartbeat(app: &str, bundle: &str) -> Heartbeat {
        Heartbeat {
            app_name: app.to_string(),
            bundle_id: bundle.to_string(),
            window_title: String::new(),
            is_idle: false,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn consecutive_heartbeats_same_app_merge_into_one_session() {
        let store = test_store();
        let t1 = Utc::now();
        let t2 = t1 + chrono::Duration::seconds(5);

        let hb1 = Heartbeat {
            app_name: "Safari".to_string(),
            bundle_id: "com.apple.Safari".to_string(),
            window_title: String::new(),
            is_idle: false,
            timestamp: t1,
        };
        let hb2 = Heartbeat {
            app_name: "Safari".to_string(),
            bundle_id: "com.apple.Safari".to_string(),
            window_title: String::new(),
            is_idle: false,
            timestamp: t2,
        };

        store.record_heartbeat(hb1).unwrap();
        store.record_heartbeat(hb2).unwrap();

        let sessions = store
            .get_sessions(
                t1 - chrono::Duration::hours(1),
                t2 + chrono::Duration::hours(1),
            )
            .unwrap();

        assert_eq!(sessions.len(), 1, "should merge into one session");
        assert_eq!(sessions[0].duration_secs, 5);
        assert_eq!(sessions[0].started_at, t1);
        assert_eq!(sessions[0].ended_at, t2);
    }

    #[test]
    fn different_apps_create_separate_sessions() {
        let store = test_store();
        let t1 = Utc::now();
        let t2 = t1 + chrono::Duration::seconds(5);

        let hb1 = Heartbeat {
            app_name: "Safari".to_string(),
            bundle_id: "com.apple.Safari".to_string(),
            window_title: String::new(),
            is_idle: false,
            timestamp: t1,
        };
        let hb2 = Heartbeat {
            app_name: "Code".to_string(),
            bundle_id: "com.microsoft.VSCode".to_string(),
            window_title: String::new(),
            is_idle: false,
            timestamp: t2,
        };

        store.record_heartbeat(hb1).unwrap();
        store.record_heartbeat(hb2).unwrap();

        let sessions = store
            .get_sessions(
                t1 - chrono::Duration::hours(1),
                t2 + chrono::Duration::hours(1),
            )
            .unwrap();

        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].app_name, "Safari");
        assert_eq!(sessions[1].app_name, "Code");
    }

    #[test]
    fn same_app_beyond_merge_gap_creates_separate_sessions() {
        let store = test_store();
        let t1 = Utc::now();
        let t2 = t1 + chrono::Duration::seconds(15);

        let hb1 = Heartbeat {
            app_name: "Safari".to_string(),
            bundle_id: "com.apple.Safari".to_string(),
            window_title: String::new(),
            is_idle: false,
            timestamp: t1,
        };
        let hb2 = Heartbeat {
            app_name: "Safari".to_string(),
            bundle_id: "com.apple.Safari".to_string(),
            window_title: String::new(),
            is_idle: false,
            timestamp: t2,
        };

        store.record_heartbeat(hb1).unwrap();
        store.record_heartbeat(hb2).unwrap();

        let sessions = store
            .get_sessions(
                t1 - chrono::Duration::hours(1),
                t2 + chrono::Duration::hours(1),
            )
            .unwrap();

        assert_eq!(
            sessions.len(),
            2,
            "15s gap exceeds default 10s merge threshold"
        );
    }

    #[test]
    fn get_daily_summary_aggregates_sessions() {
        let store = test_store();
        let t1 = Utc::now();
        let t2 = t1 + chrono::Duration::seconds(5);
        let t3 = t2 + chrono::Duration::seconds(5);
        let t4 = t3 + chrono::Duration::seconds(5);

        store
            .record_heartbeat(Heartbeat {
                app_name: "Safari".to_string(),
                bundle_id: "com.apple.Safari".to_string(),
                window_title: String::new(),
                is_idle: false,
                timestamp: t1,
            })
            .unwrap();
        store
            .record_heartbeat(Heartbeat {
                app_name: "Safari".to_string(),
                bundle_id: "com.apple.Safari".to_string(),
                window_title: String::new(),
                is_idle: false,
                timestamp: t2,
            })
            .unwrap();
        store
            .record_heartbeat(Heartbeat {
                app_name: "Code".to_string(),
                bundle_id: "com.microsoft.VSCode".to_string(),
                window_title: String::new(),
                is_idle: false,
                timestamp: t3,
            })
            .unwrap();
        store
            .record_heartbeat(Heartbeat {
                app_name: "Code".to_string(),
                bundle_id: "com.microsoft.VSCode".to_string(),
                window_title: String::new(),
                is_idle: false,
                timestamp: t4,
            })
            .unwrap();

        let date = t1.format("%Y-%m-%d").to_string();
        let summary = store.get_daily_summary(&date, 0).unwrap();

        assert_eq!(summary.date, date);
        assert_eq!(summary.total_active_secs, 10);
        assert_eq!(summary.total_idle_secs, 0);
        assert_eq!(summary.apps.len(), 2);

        let safari = summary
            .apps
            .iter()
            .find(|a| a.app_name == "Safari")
            .unwrap();
        assert_eq!(safari.total_secs, 5);
        assert_eq!(safari.session_count, 1);

        let code = summary.apps.iter().find(|a| a.app_name == "Code").unwrap();
        assert_eq!(code.total_secs, 5);
        assert_eq!(code.session_count, 1);
    }

    #[test]
    fn idle_heartbeat_does_not_merge_with_active_session() {
        let store = test_store();
        let t1 = Utc::now();
        let t2 = t1 + chrono::Duration::seconds(5);
        let t3 = t2 + chrono::Duration::seconds(5);

        store
            .record_heartbeat(Heartbeat {
                app_name: "Safari".to_string(),
                bundle_id: "com.apple.Safari".to_string(),
                window_title: String::new(),
                is_idle: false,
                timestamp: t1,
            })
            .unwrap();
        store
            .record_heartbeat(Heartbeat {
                app_name: "Safari".to_string(),
                bundle_id: "com.apple.Safari".to_string(),
                window_title: String::new(),
                is_idle: true,
                timestamp: t2,
            })
            .unwrap();
        store
            .record_heartbeat(Heartbeat {
                app_name: "Safari".to_string(),
                bundle_id: "com.apple.Safari".to_string(),
                window_title: String::new(),
                is_idle: true,
                timestamp: t3,
            })
            .unwrap();

        let sessions = store
            .get_sessions(
                t1 - chrono::Duration::hours(1),
                t3 + chrono::Duration::hours(1),
            )
            .unwrap();

        assert_eq!(sessions.len(), 2, "idle should not merge with active");
        assert!(!sessions[0].is_idle);
        assert!(sessions[1].is_idle);
        assert_eq!(sessions[1].duration_secs, 5);

        let date = t1.format("%Y-%m-%d").to_string();
        let summary = store.get_daily_summary(&date, 0).unwrap();
        assert_eq!(
            summary.total_active_secs, 0,
            "single active heartbeat has 0 duration"
        );
        assert_eq!(summary.total_idle_secs, 5);
    }

    #[test]
    fn excluded_apps_are_hidden_from_daily_summary() {
        let store = test_store();
        let t1 = Utc::now();
        let t2 = t1 + chrono::Duration::seconds(5);
        let t3 = t2 + chrono::Duration::seconds(15);
        let t4 = t3 + chrono::Duration::seconds(5);

        store
            .record_heartbeat(Heartbeat {
                app_name: "Safari".to_string(),
                bundle_id: "com.apple.Safari".to_string(),
                window_title: String::new(),
                is_idle: false,
                timestamp: t1,
            })
            .unwrap();
        store
            .record_heartbeat(Heartbeat {
                app_name: "Safari".to_string(),
                bundle_id: "com.apple.Safari".to_string(),
                window_title: String::new(),
                is_idle: false,
                timestamp: t2,
            })
            .unwrap();
        store
            .record_heartbeat(Heartbeat {
                app_name: "Code".to_string(),
                bundle_id: "com.microsoft.VSCode".to_string(),
                window_title: String::new(),
                is_idle: false,
                timestamp: t3,
            })
            .unwrap();
        store
            .record_heartbeat(Heartbeat {
                app_name: "Code".to_string(),
                bundle_id: "com.microsoft.VSCode".to_string(),
                window_title: String::new(),
                is_idle: false,
                timestamp: t4,
            })
            .unwrap();

        store
            .add_exclusion("com.apple.Safari", "Safari", None)
            .unwrap();

        let date = t1.format("%Y-%m-%d").to_string();
        let summary = store.get_daily_summary(&date, 0).unwrap();

        assert_eq!(summary.apps.len(), 1, "Safari should be excluded");
        assert_eq!(summary.apps[0].app_name, "Code");
        assert_eq!(summary.total_active_secs, 5, "only Code's 5s should count");
        assert_eq!(summary.total_idle_secs, 0);
    }

    #[test]
    fn set_and_get_app_category() {
        let store = test_store();

        assert_eq!(
            store.get_app_category("com.apple.Safari").unwrap(),
            "neutral"
        );

        store
            .set_app_category("com.apple.Safari", "productive")
            .unwrap();
        assert_eq!(
            store.get_app_category("com.apple.Safari").unwrap(),
            "productive"
        );

        store
            .set_app_category("com.apple.Safari", "neutral")
            .unwrap();
        assert_eq!(
            store.get_app_category("com.apple.Safari").unwrap(),
            "neutral"
        );

        let all = store.get_all_categories().unwrap();
        assert!(all.is_empty(), "neutral should be deleted from table");

        store
            .set_app_category("com.apple.Safari", "distracting")
            .unwrap();
        store
            .set_app_category("com.microsoft.VSCode", "productive")
            .unwrap();
        let all = store.get_all_categories().unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn single_heartbeat_creates_one_session() {
        let store = test_store();
        let hb = heartbeat("Safari", "com.apple.Safari");

        store.record_heartbeat(hb.clone()).unwrap();

        let now = Utc::now();
        let hour_ago = now - chrono::Duration::hours(1);
        let sessions = store.get_sessions(hour_ago, now).unwrap();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].app_name, "Safari");
        assert_eq!(sessions[0].bundle_id, "com.apple.Safari");
        assert!(!sessions[0].is_idle);
    }
}
