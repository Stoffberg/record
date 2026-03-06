use crate::agent::{merge_intervals, AgentWorkSlice};
use crate::types::{
    AgentProjectUsage, AgentSession, AgentSummary, AgentUsage, AppSession, AppUsage, DailySummary,
    Heartbeat, ProjectDetail, ProjectUsage, Space, SpaceUsage, SpaceWithProjects, TrackerConfig,
};
use chrono::{DateTime, Duration, NaiveDate, NaiveTime, Utc};
use rusqlite::Connection;

fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

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

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn record_heartbeat(&self, heartbeat: Heartbeat) -> rusqlite::Result<()> {
        let prev_ended_at = self.try_merge(&heartbeat)?;
        if prev_ended_at.is_none() {
            return Ok(());
        }

        let max_interval = self.config.poll_interval_secs as i64;
        let initial_secs = match prev_ended_at.unwrap() {
            Some(prev) => {
                let gap = (heartbeat.timestamp - prev).num_seconds();
                gap.max(1).min(max_interval)
            }
            None => max_interval,
        };

        let started_at = heartbeat.timestamp - Duration::seconds(initial_secs);
        let ended_at = heartbeat.timestamp;

        self.conn.execute(
            "INSERT INTO app_sessions (app_name, bundle_id, window_title, started_at, ended_at, duration_secs, is_idle, metadata, project, detail)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, '{}', ?8, ?9)",
            rusqlite::params![
                heartbeat.app_name,
                heartbeat.bundle_id,
                heartbeat.window_title,
                started_at.to_rfc3339(),
                ended_at.to_rfc3339(),
                initial_secs,
                heartbeat.is_idle,
                heartbeat.project,
                heartbeat.detail,
            ],
        )?;
        Ok(())
    }

    fn try_merge(&self, heartbeat: &Heartbeat) -> rusqlite::Result<Option<Option<DateTime<Utc>>>> {
        let result: rusqlite::Result<(i64, String, String, String, bool)> = self.conn.query_row(
            "SELECT id, bundle_id, started_at, ended_at, is_idle FROM app_sessions ORDER BY id DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        );

        let (id, bundle_id, started_at_str, ended_at_str, is_idle) = match result {
            Ok(row) => row,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(Some(None)),
            Err(e) => return Err(e),
        };

        let ended_at = DateTime::parse_from_rfc3339(&ended_at_str)
            .unwrap()
            .with_timezone(&Utc);

        if bundle_id != heartbeat.bundle_id || is_idle != heartbeat.is_idle {
            return Ok(Some(Some(ended_at)));
        }

        let gap = (heartbeat.timestamp - ended_at).num_seconds();

        if gap > self.config.merge_gap_secs {
            return Ok(Some(Some(ended_at)));
        }

        let started_at = DateTime::parse_from_rfc3339(&started_at_str)
            .unwrap()
            .with_timezone(&Utc);
        let new_duration = (heartbeat.timestamp - started_at).num_seconds();

        self.conn.execute(
            "UPDATE app_sessions SET ended_at = ?1, duration_secs = ?2 WHERE id = ?3",
            rusqlite::params![heartbeat.timestamp.to_rfc3339(), new_duration, id],
        )?;

        Ok(None)
    }

    pub fn get_sessions(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> rusqlite::Result<Vec<AppSession>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, app_name, bundle_id, window_title, started_at, ended_at, duration_secs, is_idle, metadata, project, detail
             FROM app_sessions
             WHERE started_at >= ?1 AND started_at <= ?2
             ORDER BY started_at ASC",
        )?;
        let rows = stmt.query_map(
            rusqlite::params![start.to_rfc3339(), end.to_rfc3339()],
            Self::row_to_session,
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
            "SELECT id, app_name, bundle_id, window_title, started_at, ended_at, duration_secs, is_idle, metadata, project, detail
             FROM app_sessions
             WHERE started_at >= ?1 AND started_at < ?2 AND bundle_id = ?3 AND is_idle = 0
             ORDER BY started_at DESC",
        )?;
        let rows = stmt.query_map(
            rusqlite::params![start, end, bundle_id],
            Self::row_to_session,
        )?;
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

    pub fn get_app_projects(
        &self,
        date: &str,
        bundle_id: &str,
        tz_offset_minutes: i32,
    ) -> rusqlite::Result<Vec<ProjectUsage>> {
        let (start, end) = day_bounds_utc(date, tz_offset_minutes);
        self.clean_expired_project_exclusions()?;

        let mut stmt = self.conn.prepare(
            "SELECT s.project, SUM(s.duration_secs), COUNT(*)
             FROM app_sessions s
             LEFT JOIN project_exclusions pe ON s.project = pe.project
             WHERE s.started_at >= ?1 AND s.started_at < ?2 AND s.bundle_id = ?3
             AND s.is_idle = 0 AND s.project IS NOT NULL
             AND pe.project IS NULL
             GROUP BY s.project
             HAVING SUM(s.duration_secs) >= 60
             ORDER BY SUM(s.duration_secs) DESC",
        )?;

        let projects: Vec<(String, i64, i64)> = stmt
            .query_map(rusqlite::params![start, end, bundle_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut result = Vec::with_capacity(projects.len());
        for (project, total_secs, session_count) in projects {
            let mut detail_stmt = self.conn.prepare(
                "SELECT detail, SUM(duration_secs)
                 FROM app_sessions
                 WHERE started_at >= ?1 AND started_at < ?2 AND bundle_id = ?3
                 AND is_idle = 0 AND project = ?4 AND detail IS NOT NULL
                 GROUP BY detail
                 HAVING SUM(duration_secs) >= 60
                 ORDER BY SUM(duration_secs) DESC",
            )?;

            let details: Vec<ProjectDetail> = detail_stmt
                .query_map(rusqlite::params![start, end, bundle_id, project], |row| {
                    Ok(ProjectDetail {
                        label: row.get(0)?,
                        total_secs: row.get(1)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;

            result.push(ProjectUsage {
                project,
                total_secs,
                active_secs: total_secs,
                agent_secs: 0,
                session_count,
                details,
            });
        }

        Ok(result)
    }

    pub fn get_daily_projects(
        &self,
        date: &str,
        tz_offset_minutes: i32,
    ) -> rusqlite::Result<Vec<ProjectUsage>> {
        let (start, end) = day_bounds_utc(date, tz_offset_minutes);
        self.clean_expired_project_exclusions()?;

        let day_start_ts = DateTime::parse_from_rfc3339(&start)
            .unwrap()
            .with_timezone(&Utc)
            .timestamp();
        let day_end_ts = DateTime::parse_from_rfc3339(&end)
            .unwrap()
            .with_timezone(&Utc)
            .timestamp();

        let mut active_stmt = self.conn.prepare(
            "SELECT s.project, s.started_at, s.ended_at, s.duration_secs
             FROM app_sessions s
             LEFT JOIN project_exclusions pe ON s.project = pe.project
             WHERE s.started_at >= ?1 AND s.started_at < ?2
             AND s.is_idle = 0 AND s.project IS NOT NULL
             AND pe.project IS NULL",
        )?;

        struct ActiveRow {
            project: String,
            start_ts: i64,
            end_ts: i64,
        }

        let active_rows: Vec<ActiveRow> = active_stmt
            .query_map(rusqlite::params![start, end], |row| {
                let started_str: String = row.get(1)?;
                let ended_str: String = row.get(2)?;
                let s = DateTime::parse_from_rfc3339(&started_str)
                    .unwrap()
                    .with_timezone(&Utc)
                    .timestamp()
                    .max(day_start_ts);
                let e = DateTime::parse_from_rfc3339(&ended_str)
                    .unwrap()
                    .with_timezone(&Utc)
                    .timestamp()
                    .min(day_end_ts);
                Ok(ActiveRow {
                    project: row.get(0)?,
                    start_ts: s,
                    end_ts: e,
                })
            })?
            .filter_map(|r| r.ok())
            .filter(|r| r.end_ts > r.start_ts)
            .collect();

        let mut agent_stmt = self.conn.prepare(
            "SELECT project, started_at, ended_at
             FROM agent_sessions
             WHERE started_at < ?2 AND ended_at > ?1",
        )?;

        struct AgentRow {
            project: String,
            start_ts: i64,
            end_ts: i64,
        }

        let agent_rows: Vec<AgentRow> = agent_stmt
            .query_map(rusqlite::params![start, end], |row| {
                let started_str: String = row.get(1)?;
                let ended_str: String = row.get(2)?;
                let s = DateTime::parse_from_rfc3339(&started_str)
                    .unwrap()
                    .with_timezone(&Utc)
                    .timestamp()
                    .max(day_start_ts);
                let e = DateTime::parse_from_rfc3339(&ended_str)
                    .unwrap()
                    .with_timezone(&Utc)
                    .timestamp()
                    .min(day_end_ts);
                Ok(AgentRow {
                    project: row.get(0)?,
                    start_ts: s,
                    end_ts: e,
                })
            })?
            .filter_map(|r| r.ok())
            .filter(|r| r.end_ts > r.start_ts)
            .collect();

        let mut active_map: std::collections::HashMap<String, Vec<(i64, i64)>> =
            std::collections::HashMap::new();
        let mut session_counts: std::collections::HashMap<String, i64> =
            std::collections::HashMap::new();
        for row in &active_rows {
            active_map
                .entry(row.project.clone())
                .or_default()
                .push((row.start_ts, row.end_ts));
            *session_counts.entry(row.project.clone()).or_insert(0) += 1;
        }

        let mut agent_map: std::collections::HashMap<String, Vec<(i64, i64)>> =
            std::collections::HashMap::new();
        for row in &agent_rows {
            agent_map
                .entry(row.project.clone())
                .or_default()
                .push((row.start_ts, row.end_ts));
        }

        let all_projects: std::collections::HashSet<String> =
            active_map.keys().chain(agent_map.keys()).cloned().collect();

        let mut result: Vec<ProjectUsage> = Vec::new();

        for project in &all_projects {
            let active_intervals = active_map.remove(project).unwrap_or_default();
            let agent_intervals = agent_map.remove(project).unwrap_or_default();

            let merged_active = merge_intervals(active_intervals.clone());
            let active_secs: i64 = merged_active.iter().map(|(s, e)| e - s).sum();

            let mut all_intervals = active_intervals.clone();
            all_intervals.extend(agent_intervals);
            let merged_all = merge_intervals(all_intervals);
            let total_secs: i64 = merged_all.iter().map(|(s, e)| e - s).sum();

            let agent_secs = total_secs - active_secs;

            if total_secs < 60 {
                continue;
            }

            let session_count = session_counts.get(project).copied().unwrap_or(0);

            let mut detail_stmt = self.conn.prepare(
                "SELECT detail, SUM(duration_secs)
                 FROM app_sessions
                 WHERE started_at >= ?1 AND started_at < ?2
                 AND is_idle = 0 AND project = ?3 AND detail IS NOT NULL
                 GROUP BY detail
                 HAVING SUM(duration_secs) >= 30
                 ORDER BY SUM(duration_secs) DESC
                 LIMIT 10",
            )?;

            let details: Vec<ProjectDetail> = detail_stmt
                .query_map(rusqlite::params![start, end, project], |row| {
                    Ok(ProjectDetail {
                        label: row.get(0)?,
                        total_secs: row.get(1)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;

            result.push(ProjectUsage {
                project: project.clone(),
                total_secs,
                active_secs,
                agent_secs,
                session_count,
                details,
            });
        }

        result.sort_by(|a, b| b.total_secs.cmp(&a.total_secs));

        Ok(result)
    }

    fn row_to_session(row: &rusqlite::Row) -> rusqlite::Result<AppSession> {
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
            project: row.get(9)?,
            detail: row.get(10)?,
        })
    }

    pub fn create_space(
        &self,
        name: &str,
        color: &str,
        initials: &str,
        emoji: Option<&str>,
    ) -> rusqlite::Result<Space> {
        self.conn.execute(
            "INSERT INTO spaces (name, color, initials, emoji) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![name, color, initials, emoji],
        )?;
        let id = self.conn.last_insert_rowid();
        Ok(Space {
            id,
            name: name.to_string(),
            color: color.to_string(),
            initials: initials.to_string(),
            emoji: emoji.map(String::from),
        })
    }

    pub fn update_space(
        &self,
        id: i64,
        name: &str,
        color: &str,
        initials: &str,
        emoji: Option<&str>,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE spaces SET name = ?1, color = ?2, initials = ?3, emoji = ?4 WHERE id = ?5",
            rusqlite::params![name, color, initials, emoji, id],
        )?;
        Ok(())
    }

    pub fn delete_space(&self, id: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "DELETE FROM space_projects WHERE space_id = ?1",
            rusqlite::params![id],
        )?;
        self.conn
            .execute("DELETE FROM spaces WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    }

    pub fn get_spaces(&self) -> rusqlite::Result<Vec<SpaceWithProjects>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, color, initials, emoji FROM spaces ORDER BY name")?;
        let spaces: Vec<Space> = stmt
            .query_map([], |row| {
                Ok(Space {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    initials: row.get(3)?,
                    emoji: row.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut result = Vec::with_capacity(spaces.len());
        let mut proj_stmt = self
            .conn
            .prepare("SELECT project FROM space_projects WHERE space_id = ?1 ORDER BY project")?;
        for space in spaces {
            let projects: Vec<String> = proj_stmt
                .query_map(rusqlite::params![space.id], |row| row.get(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            result.push(SpaceWithProjects { space, projects });
        }
        Ok(result)
    }

    pub fn add_project_to_space(&self, space_id: i64, project: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "DELETE FROM space_projects WHERE project = ?1",
            rusqlite::params![project],
        )?;
        self.conn.execute(
            "INSERT INTO space_projects (space_id, project) VALUES (?1, ?2)",
            rusqlite::params![space_id, project],
        )?;
        Ok(())
    }

    pub fn remove_project_from_space(&self, space_id: i64, project: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "DELETE FROM space_projects WHERE space_id = ?1 AND project = ?2",
            rusqlite::params![space_id, project],
        )?;
        Ok(())
    }

    pub fn get_daily_spaces(
        &self,
        date: &str,
        tz_offset_minutes: i32,
    ) -> rusqlite::Result<Vec<SpaceUsage>> {
        let projects = self.get_daily_projects(date, tz_offset_minutes)?;

        let mut space_map: std::collections::HashMap<Option<i64>, Vec<ProjectUsage>> =
            std::collections::HashMap::new();
        let mut space_lookup: std::collections::HashMap<i64, Space> =
            std::collections::HashMap::new();

        let mut stmt = self.conn.prepare(
            "SELECT sp.space_id, s.id, s.name, s.color, s.initials, s.emoji
             FROM space_projects sp
             JOIN spaces s ON sp.space_id = s.id
             WHERE sp.project = ?1",
        )?;

        for project in projects {
            let space: Option<Space> = stmt
                .query_row(rusqlite::params![project.project], |row| {
                    Ok(Space {
                        id: row.get(1)?,
                        name: row.get(2)?,
                        color: row.get(3)?,
                        initials: row.get(4)?,
                        emoji: row.get(5)?,
                    })
                })
                .ok();

            let key = space.as_ref().map(|s| s.id);
            if let Some(s) = space {
                space_lookup.entry(s.id).or_insert(s);
            }
            space_map.entry(key).or_default().push(project);
        }

        let mut result: Vec<SpaceUsage> = Vec::new();

        let mut grouped: Vec<(Option<i64>, Vec<ProjectUsage>)> = space_map.into_iter().collect();
        grouped.sort_by(|a, b| {
            let a_secs: i64 = a.1.iter().map(|p| p.total_secs).sum();
            let b_secs: i64 = b.1.iter().map(|p| p.total_secs).sum();
            b_secs.cmp(&a_secs)
        });

        for (space_id, projects) in grouped {
            let total_secs = projects.iter().map(|p| p.total_secs).sum();
            let session_count = projects.iter().map(|p| p.session_count).sum();
            let space = space_id.and_then(|id| space_lookup.remove(&id));

            result.push(SpaceUsage {
                space,
                projects,
                total_secs,
                session_count,
            });
        }

        Ok(result)
    }

    pub fn export_space_csv(
        &self,
        space_id: i64,
        start_date: &str,
        end_date: &str,
        tz_offset_minutes: i32,
    ) -> rusqlite::Result<String> {
        let mut stmt = self
            .conn
            .prepare("SELECT project FROM space_projects WHERE space_id = ?1")?;
        let space_projects: Vec<String> = stmt
            .query_map(rusqlite::params![space_id], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;

        let start = NaiveDate::parse_from_str(start_date, "%Y-%m-%d")
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let end = NaiveDate::parse_from_str(end_date, "%Y-%m-%d")
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut csv =
            String::from("date,project,active_secs,agent_secs,total_secs,session_count\n");

        let mut current = start;
        while current <= end {
            let date_str = current.format("%Y-%m-%d").to_string();
            let projects = self.get_daily_projects(&date_str, tz_offset_minutes)?;

            let project_map: std::collections::HashMap<&str, &ProjectUsage> =
                projects.iter().map(|p| (p.project.as_str(), p)).collect();

            for sp in &space_projects {
                match project_map.get(sp.as_str()) {
                    Some(p) => {
                        csv.push_str(&format!(
                            "{},{},{},{},{},{}\n",
                            date_str,
                            escape_csv(sp),
                            p.active_secs,
                            p.agent_secs,
                            p.total_secs,
                            p.session_count,
                        ));
                    }
                    None => {
                        csv.push_str(&format!("{},{},0,0,0,0\n", date_str, escape_csv(sp)));
                    }
                }
            }

            current += Duration::days(1);
        }

        Ok(csv)
    }

    pub fn get_all_projects(&self) -> rusqlite::Result<Vec<(String, i64)>> {
        self.clean_expired_project_exclusions()?;
        let mut stmt = self.conn.prepare(
            "SELECT s.project, SUM(s.duration_secs)
             FROM app_sessions s
             LEFT JOIN project_exclusions pe ON s.project = pe.project
             WHERE s.project IS NOT NULL AND s.is_idle = 0
             AND pe.project IS NULL
             GROUP BY s.project
             HAVING SUM(s.duration_secs) >= 60
             ORDER BY SUM(s.duration_secs) DESC",
        )?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect()
    }

    pub fn add_project_exclusion(
        &self,
        project: &str,
        expires_at: Option<&str>,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO project_exclusions (project, expires_at) VALUES (?1, ?2)",
            rusqlite::params![project, expires_at],
        )?;
        Ok(())
    }

    pub fn remove_project_exclusion(&self, project: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "DELETE FROM project_exclusions WHERE project = ?1",
            rusqlite::params![project],
        )?;
        Ok(())
    }

    pub fn get_project_exclusions(&self) -> rusqlite::Result<Vec<(String, Option<String>)>> {
        self.clean_expired_project_exclusions()?;
        let mut stmt = self
            .conn
            .prepare("SELECT project, expires_at FROM project_exclusions")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect()
    }

    fn clean_expired_project_exclusions(&self) -> rusqlite::Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "DELETE FROM project_exclusions WHERE expires_at IS NOT NULL AND expires_at < ?1",
            rusqlite::params![now],
        )?;
        Ok(())
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

    pub fn get_agent_scan_cursor(&self) -> rusqlite::Result<i64> {
        let result: rusqlite::Result<i64> = self.conn.query_row(
            "SELECT last_scanned_ms FROM agent_scan_cursor WHERE id = 1",
            [],
            |row| row.get(0),
        );
        match result {
            Ok(ms) => Ok(ms),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(0),
            Err(e) => Err(e),
        }
    }

    pub fn set_agent_scan_cursor(&self, ms: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO agent_scan_cursor (id, last_scanned_ms) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET last_scanned_ms = ?1",
            rusqlite::params![ms],
        )?;
        Ok(())
    }

    pub fn upsert_agent_sessions(&self, slices: &[AgentWorkSlice]) -> rusqlite::Result<()> {
        let mut seen_refs: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for slice in slices {
            if seen_refs.insert(&slice.session_ref) {
                self.conn.execute(
                    "DELETE FROM agent_sessions WHERE session_ref = ?1",
                    rusqlite::params![slice.session_ref],
                )?;
            }
        }

        let mut stmt = self.conn.prepare(
            "INSERT INTO agent_sessions (agent, project, session_ref, started_at, ended_at, duration_secs)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;

        for slice in slices {
            stmt.execute(rusqlite::params![
                slice.agent,
                slice.project,
                slice.session_ref,
                slice.started_at.to_rfc3339(),
                slice.ended_at.to_rfc3339(),
                slice.duration_secs,
            ])?;
        }
        Ok(())
    }

    pub fn get_daily_agent_summary(
        &self,
        date: &str,
        tz_offset_minutes: i32,
    ) -> rusqlite::Result<AgentSummary> {
        let (start, end) = day_bounds_utc(date, tz_offset_minutes);

        let mut stmt = self.conn.prepare(
            "SELECT id, agent, project, session_ref, started_at, ended_at, duration_secs
             FROM agent_sessions
             WHERE started_at < ?2 AND ended_at > ?1
             ORDER BY project, agent, started_at",
        )?;

        let sessions: Vec<AgentSession> = stmt
            .query_map(rusqlite::params![start, end], |row| {
                let started_str: String = row.get(4)?;
                let ended_str: String = row.get(5)?;
                Ok(AgentSession {
                    id: row.get(0)?,
                    agent: row.get(1)?,
                    project: row.get(2)?,
                    session_ref: row.get(3)?,
                    started_at: DateTime::parse_from_rfc3339(&started_str)
                        .unwrap()
                        .with_timezone(&Utc),
                    ended_at: DateTime::parse_from_rfc3339(&ended_str)
                        .unwrap()
                        .with_timezone(&Utc),
                    duration_secs: row.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let day_start = DateTime::parse_from_rfc3339(&start)
            .unwrap()
            .with_timezone(&Utc);
        let day_end = DateTime::parse_from_rfc3339(&end)
            .unwrap()
            .with_timezone(&Utc);

        let mut project_map: std::collections::HashMap<String, Vec<&AgentSession>> =
            std::collections::HashMap::new();
        for session in &sessions {
            project_map
                .entry(session.project.clone())
                .or_default()
                .push(session);
        }

        let mut projects: Vec<AgentProjectUsage> = Vec::new();
        let mut total_agent_secs: i64 = 0;

        for (project, proj_sessions) in &project_map {
            let all_intervals: Vec<(i64, i64)> = proj_sessions
                .iter()
                .map(|s| {
                    let clamped_start = s.started_at.max(day_start).timestamp();
                    let clamped_end = s.ended_at.min(day_end).timestamp();
                    (clamped_start, clamped_end)
                })
                .filter(|(s, e)| e > s)
                .collect();

            let merged = merge_intervals(all_intervals);
            let deduped_secs: i64 = merged.iter().map(|(s, e)| e - s).sum();

            let mut agent_map: std::collections::HashMap<&str, (i64, i64)> =
                std::collections::HashMap::new();
            for session in proj_sessions {
                let clamped_start = session.started_at.max(day_start).timestamp();
                let clamped_end = session.ended_at.min(day_end).timestamp();
                let secs = (clamped_end - clamped_start).max(0);
                let entry = agent_map.entry(&session.agent).or_insert((0, 0));
                entry.0 += secs;
                entry.1 += 1;
            }

            let agents: Vec<AgentUsage> = agent_map
                .into_iter()
                .map(|(agent, (secs, count))| AgentUsage {
                    agent: agent.to_string(),
                    total_secs: secs,
                    session_count: count,
                })
                .collect();

            total_agent_secs += deduped_secs;
            projects.push(AgentProjectUsage {
                project: project.clone(),
                total_secs: deduped_secs,
                session_count: proj_sessions.len() as i64,
                agents,
            });
        }

        projects.sort_by(|a, b| b.total_secs.cmp(&a.total_secs));

        Ok(AgentSummary {
            date: date.to_string(),
            total_agent_secs,
            projects,
        })
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
                metadata TEXT DEFAULT '{}',
                project TEXT,
                detail TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_sessions_started ON app_sessions(started_at);
            CREATE INDEX IF NOT EXISTS idx_sessions_bundle ON app_sessions(bundle_id);
            CREATE INDEX IF NOT EXISTS idx_sessions_idle ON app_sessions(is_idle);

            CREATE TABLE IF NOT EXISTS app_exclusions (
                bundle_id TEXT PRIMARY KEY,
                app_name TEXT NOT NULL,
                expires_at TEXT
            );

            CREATE TABLE IF NOT EXISTS project_exclusions (
                project TEXT PRIMARY KEY,
                expires_at TEXT
            );

            CREATE TABLE IF NOT EXISTS spaces (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                color TEXT NOT NULL,
                initials TEXT NOT NULL,
                emoji TEXT
            );

            CREATE TABLE IF NOT EXISTS space_projects (
                space_id INTEGER NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
                project TEXT NOT NULL,
                PRIMARY KEY (space_id, project)
            );
            CREATE INDEX IF NOT EXISTS idx_space_projects_project ON space_projects(project);

            CREATE TABLE IF NOT EXISTS agent_sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                agent TEXT NOT NULL,
                project TEXT NOT NULL,
                session_ref TEXT NOT NULL,
                started_at TEXT NOT NULL,
                ended_at TEXT NOT NULL,
                duration_secs INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_agent_sessions_started ON agent_sessions(started_at);
            CREATE INDEX IF NOT EXISTS idx_agent_sessions_project ON agent_sessions(project);
            CREATE INDEX IF NOT EXISTS idx_agent_sessions_ref ON agent_sessions(session_ref);

            CREATE TABLE IF NOT EXISTS agent_scan_cursor (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                last_scanned_ms INTEGER NOT NULL
            );

            PRAGMA journal_mode=WAL;",
        )?;

        self.migrate_add_project_columns()?;

        self.conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_sessions_project ON app_sessions(project);",
        )?;

        Ok(())
    }

    fn migrate_add_project_columns(&self) -> rusqlite::Result<()> {
        let has_project: bool = self
            .conn
            .prepare("SELECT sql FROM sqlite_master WHERE type='table' AND name='app_sessions'")?
            .query_row([], |row| {
                let sql: String = row.get(0)?;
                Ok(sql.contains("project"))
            })
            .unwrap_or(false);

        if !has_project {
            self.conn
                .execute_batch("ALTER TABLE app_sessions ADD COLUMN project TEXT;")?;
            self.conn
                .execute_batch("ALTER TABLE app_sessions ADD COLUMN detail TEXT;")?;
        }

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
            project: None,
            detail: None,
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
            project: None,
            detail: None,
        };
        let hb2 = Heartbeat {
            app_name: "Safari".to_string(),
            bundle_id: "com.apple.Safari".to_string(),
            window_title: String::new(),
            is_idle: false,
            timestamp: t2,
            project: None,
            detail: None,
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
        assert_eq!(sessions[0].duration_secs, 10);
        assert_eq!(sessions[0].started_at, t1 - chrono::Duration::seconds(5));
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
            project: None,
            detail: None,
        };
        let hb2 = Heartbeat {
            app_name: "Code".to_string(),
            bundle_id: "com.microsoft.VSCode".to_string(),
            window_title: String::new(),
            is_idle: false,
            timestamp: t2,
            project: None,
            detail: None,
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
            project: None,
            detail: None,
        };
        let hb2 = Heartbeat {
            app_name: "Safari".to_string(),
            bundle_id: "com.apple.Safari".to_string(),
            window_title: String::new(),
            is_idle: false,
            timestamp: t2,
            project: None,
            detail: None,
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
                project: None,
                detail: None,
            })
            .unwrap();
        store
            .record_heartbeat(Heartbeat {
                app_name: "Safari".to_string(),
                bundle_id: "com.apple.Safari".to_string(),
                window_title: String::new(),
                is_idle: false,
                timestamp: t2,
                project: None,
                detail: None,
            })
            .unwrap();
        store
            .record_heartbeat(Heartbeat {
                app_name: "Code".to_string(),
                bundle_id: "com.microsoft.VSCode".to_string(),
                window_title: String::new(),
                is_idle: false,
                timestamp: t3,
                project: None,
                detail: None,
            })
            .unwrap();
        store
            .record_heartbeat(Heartbeat {
                app_name: "Code".to_string(),
                bundle_id: "com.microsoft.VSCode".to_string(),
                window_title: String::new(),
                is_idle: false,
                timestamp: t4,
                project: None,
                detail: None,
            })
            .unwrap();

        let date = t1.format("%Y-%m-%d").to_string();
        let summary = store.get_daily_summary(&date, 0).unwrap();

        assert_eq!(summary.date, date);
        assert_eq!(summary.total_active_secs, 20);
        assert_eq!(summary.total_idle_secs, 0);
        assert_eq!(summary.apps.len(), 2);

        let safari = summary
            .apps
            .iter()
            .find(|a| a.app_name == "Safari")
            .unwrap();
        assert_eq!(safari.total_secs, 10);
        assert_eq!(safari.session_count, 1);

        let code = summary.apps.iter().find(|a| a.app_name == "Code").unwrap();
        assert_eq!(code.total_secs, 10);
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
                project: None,
                detail: None,
            })
            .unwrap();
        store
            .record_heartbeat(Heartbeat {
                app_name: "Safari".to_string(),
                bundle_id: "com.apple.Safari".to_string(),
                window_title: String::new(),
                is_idle: true,
                timestamp: t2,
                project: None,
                detail: None,
            })
            .unwrap();
        store
            .record_heartbeat(Heartbeat {
                app_name: "Safari".to_string(),
                bundle_id: "com.apple.Safari".to_string(),
                window_title: String::new(),
                is_idle: true,
                timestamp: t3,
                project: None,
                detail: None,
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
        assert_eq!(sessions[0].duration_secs, 5);
        assert_eq!(sessions[1].duration_secs, 10);

        let date = t1.format("%Y-%m-%d").to_string();
        let summary = store.get_daily_summary(&date, 0).unwrap();
        assert_eq!(summary.total_active_secs, 5);
        assert_eq!(summary.total_idle_secs, 10);
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
                project: None,
                detail: None,
            })
            .unwrap();
        store
            .record_heartbeat(Heartbeat {
                app_name: "Safari".to_string(),
                bundle_id: "com.apple.Safari".to_string(),
                window_title: String::new(),
                is_idle: false,
                timestamp: t2,
                project: None,
                detail: None,
            })
            .unwrap();
        store
            .record_heartbeat(Heartbeat {
                app_name: "Code".to_string(),
                bundle_id: "com.microsoft.VSCode".to_string(),
                window_title: String::new(),
                is_idle: false,
                timestamp: t3,
                project: None,
                detail: None,
            })
            .unwrap();
        store
            .record_heartbeat(Heartbeat {
                app_name: "Code".to_string(),
                bundle_id: "com.microsoft.VSCode".to_string(),
                window_title: String::new(),
                is_idle: false,
                timestamp: t4,
                project: None,
                detail: None,
            })
            .unwrap();

        store
            .add_exclusion("com.apple.Safari", "Safari", None)
            .unwrap();

        let date = t1.format("%Y-%m-%d").to_string();
        let summary = store.get_daily_summary(&date, 0).unwrap();

        assert_eq!(summary.apps.len(), 1, "Safari should be excluded");
        assert_eq!(summary.apps[0].app_name, "Code");
        assert_eq!(
            summary.total_active_secs, 10,
            "only Code's 10s should count"
        );
        assert_eq!(summary.total_idle_secs, 0);
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

    #[test]
    fn excluded_projects_hidden_from_daily_projects() {
        let store = test_store();
        let t1 = Utc::now();

        for i in 0..15 {
            store
                .record_heartbeat(Heartbeat {
                    app_name: "Code".to_string(),
                    bundle_id: "com.microsoft.VSCode".to_string(),
                    window_title: String::new(),
                    is_idle: false,
                    timestamp: t1 + chrono::Duration::seconds(15 * i),
                    project: Some("record".to_string()),
                    detail: None,
                })
                .unwrap();
        }

        for i in 0..15 {
            store
                .record_heartbeat(Heartbeat {
                    app_name: "Code".to_string(),
                    bundle_id: "com.microsoft.VSCode".to_string(),
                    window_title: String::new(),
                    is_idle: false,
                    timestamp: t1 + chrono::Duration::seconds(300 + 15 * i),
                    project: Some("noisy".to_string()),
                    detail: None,
                })
                .unwrap();
        }

        let date = t1.format("%Y-%m-%d").to_string();
        let projects = store.get_daily_projects(&date, 0).unwrap();
        assert_eq!(projects.len(), 2);

        store.add_project_exclusion("noisy", None).unwrap();

        let projects = store.get_daily_projects(&date, 0).unwrap();
        assert_eq!(projects.len(), 1, "excluded project should be hidden");
        assert_eq!(projects[0].project, "record");

        let all = store.get_all_projects().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].0, "record");

        store.remove_project_exclusion("noisy").unwrap();
        let projects = store.get_daily_projects(&date, 0).unwrap();
        assert_eq!(
            projects.len(),
            2,
            "should reappear after removing exclusion"
        );
    }

    #[test]
    fn agent_scan_cursor_defaults_to_zero() {
        let store = test_store();
        assert_eq!(store.get_agent_scan_cursor().unwrap(), 0);
    }

    #[test]
    fn agent_scan_cursor_roundtrip() {
        let store = test_store();
        store.set_agent_scan_cursor(1700000000000).unwrap();
        assert_eq!(store.get_agent_scan_cursor().unwrap(), 1700000000000);
        store.set_agent_scan_cursor(1700000099000).unwrap();
        assert_eq!(store.get_agent_scan_cursor().unwrap(), 1700000099000);
    }

    #[test]
    fn upsert_agent_sessions_inserts_and_replaces() {
        use crate::agent::AgentWorkSlice;

        let store = test_store();
        let t1 = Utc::now();
        let t2 = t1 + chrono::Duration::seconds(60);
        let t3 = t2 + chrono::Duration::seconds(120);
        let t4 = t3 + chrono::Duration::seconds(60);

        let slices = vec![
            AgentWorkSlice {
                agent: "opencode".to_string(),
                project: "record".to_string(),
                session_ref: "ses_1".to_string(),
                started_at: t1,
                ended_at: t2,
                duration_secs: 60,
            },
            AgentWorkSlice {
                agent: "opencode".to_string(),
                project: "record".to_string(),
                session_ref: "ses_1".to_string(),
                started_at: t3,
                ended_at: t4,
                duration_secs: 60,
            },
        ];

        store.upsert_agent_sessions(&slices).unwrap();

        let date = t1.format("%Y-%m-%d").to_string();
        let summary = store.get_daily_agent_summary(&date, 0).unwrap();
        assert_eq!(summary.projects.len(), 1);
        assert_eq!(summary.projects[0].project, "record");
        assert_eq!(summary.projects[0].total_secs, 120);
        assert_eq!(summary.projects[0].session_count, 2);

        let new_slices = vec![AgentWorkSlice {
            agent: "opencode".to_string(),
            project: "record".to_string(),
            session_ref: "ses_1".to_string(),
            started_at: t1,
            ended_at: t4,
            duration_secs: 240,
        }];
        store.upsert_agent_sessions(&new_slices).unwrap();

        let summary = store.get_daily_agent_summary(&date, 0).unwrap();
        assert_eq!(
            summary.projects[0].session_count, 1,
            "old rows should be replaced"
        );
    }

    #[test]
    fn agent_summary_deduplicates_overlapping_sessions() {
        use crate::agent::AgentWorkSlice;

        let store = test_store();
        let t1 = Utc::now();

        let slices = vec![
            AgentWorkSlice {
                agent: "opencode".to_string(),
                project: "record".to_string(),
                session_ref: "ses_a".to_string(),
                started_at: t1,
                ended_at: t1 + chrono::Duration::seconds(100),
                duration_secs: 100,
            },
            AgentWorkSlice {
                agent: "opencode".to_string(),
                project: "record".to_string(),
                session_ref: "ses_b".to_string(),
                started_at: t1 + chrono::Duration::seconds(50),
                ended_at: t1 + chrono::Duration::seconds(150),
                duration_secs: 100,
            },
        ];

        store.upsert_agent_sessions(&slices).unwrap();

        let date = t1.format("%Y-%m-%d").to_string();
        let summary = store.get_daily_agent_summary(&date, 0).unwrap();
        assert_eq!(summary.projects.len(), 1);
        assert_eq!(
            summary.projects[0].total_secs, 150,
            "overlapping intervals should be deduped to 150s, not 200s"
        );
        assert_eq!(summary.total_agent_secs, 150);
    }

    #[test]
    fn agent_summary_sums_across_projects() {
        use crate::agent::AgentWorkSlice;

        let store = test_store();
        let t1 = Utc::now();

        let slices = vec![
            AgentWorkSlice {
                agent: "opencode".to_string(),
                project: "record".to_string(),
                session_ref: "ses_a".to_string(),
                started_at: t1,
                ended_at: t1 + chrono::Duration::seconds(100),
                duration_secs: 100,
            },
            AgentWorkSlice {
                agent: "opencode".to_string(),
                project: "buddy".to_string(),
                session_ref: "ses_b".to_string(),
                started_at: t1,
                ended_at: t1 + chrono::Duration::seconds(200),
                duration_secs: 200,
            },
        ];

        store.upsert_agent_sessions(&slices).unwrap();

        let date = t1.format("%Y-%m-%d").to_string();
        let summary = store.get_daily_agent_summary(&date, 0).unwrap();
        assert_eq!(summary.projects.len(), 2);
        assert_eq!(
            summary.total_agent_secs, 300,
            "different projects should sum: 100 + 200 = 300"
        );
        assert_eq!(
            summary.projects[0].project, "buddy",
            "sorted by total_secs desc"
        );
    }
}
