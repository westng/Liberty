use chrono::{Days, Local, SecondsFormat, TimeZone, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::domain::dashboard::{
    DashboardCompanionSummary, DashboardJobSummary, DashboardMetrics, DashboardOverview,
    DashboardResourceSummary, DashboardTrendPoint,
};
use crate::local_db::{pet_leveling, LocalResult};

const ACTIVE_STATUSES: &str = "'queued', 'transcribing', 'speaker_processing', 'summarizing'";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardRange {
    Today,
    SevenDays,
    ThirtyDays,
    All,
}

impl DashboardRange {
    pub fn parse(value: &str) -> LocalResult<Self> {
        match value {
            "today" => Ok(Self::Today),
            "7d" => Ok(Self::SevenDays),
            "30d" => Ok(Self::ThirtyDays),
            "all" => Ok(Self::All),
            _ => Err("工作台时间范围无效。".into()),
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Today => "today",
            Self::SevenDays => "7d",
            Self::ThirtyDays => "30d",
            Self::All => "all",
        }
    }

    fn start_at(self) -> LocalResult<Option<String>> {
        let days_before_today = match self {
            Self::Today => 0,
            Self::SevenDays => 6,
            Self::ThirtyDays => 29,
            Self::All => return Ok(None),
        };
        let date = Local::now()
            .date_naive()
            .checked_sub_days(Days::new(days_before_today))
            .ok_or_else(|| "无法计算工作台时间范围。".to_string())?;
        let local_start = Local
            .from_local_datetime(
                &date
                    .and_hms_opt(0, 0, 0)
                    .ok_or_else(|| "无法计算工作台起始时间。".to_string())?,
            )
            .single()
            .ok_or_else(|| "工作台起始时间存在时区歧义。".to_string())?;
        Ok(Some(
            local_start
                .with_timezone(&Utc)
                .to_rfc3339_opts(SecondsFormat::Millis, true),
        ))
    }
}

pub fn get_overview(conn: &Connection, range: DashboardRange) -> LocalResult<DashboardOverview> {
    let start_at = range.start_at()?;
    let metrics = query_metrics(conn, start_at.as_deref())?;
    let trend = query_trend(conn, range, start_at.as_deref())?;
    let attention_jobs = query_jobs(conn, start_at.as_deref(), true)?;
    let recent_results = query_jobs(conn, start_at.as_deref(), false)?;
    let resources = query_resources(conn)?;
    let companion = query_companion(conn)?;

    Ok(DashboardOverview {
        range: range.key().into(),
        trend_granularity: match range {
            DashboardRange::Today => "hour",
            DashboardRange::All => "month",
            DashboardRange::SevenDays | DashboardRange::ThirtyDays => "day",
        }
        .into(),
        metrics,
        trend,
        attention_jobs,
        recent_results,
        resources,
        companion,
    })
}

fn query_metrics(conn: &Connection, start_at: Option<&str>) -> LocalResult<DashboardMetrics> {
    let range_filter = if start_at.is_some() {
        "created_at >= ?1 AND"
    } else {
        ""
    };
    let sql = format!(
            "SELECT COUNT(1),
                    COALESCE(SUM(MAX(duration_minutes, 0)), 0),
                    COALESCE(SUM(MAX(COALESCE(processing_duration_seconds, 0), 0)), 0),
                    COALESCE(SUM(CASE WHEN overall_status IN ({ACTIVE_STATUSES}) THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN overall_status = 'completed' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN overall_status = 'failed' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN overall_status = 'completed'
                        AND asr_status = 'completed' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN overall_status = 'completed'
                        AND enable_speaker = 1 THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN overall_status = 'completed'
                        AND enable_speaker = 1 AND diarization_status = 'completed'
                        THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN overall_status = 'completed' AND EXISTS (
                          SELECT 1 FROM ai_summary_runs runs
                          WHERE runs.job_id = jobs.id
                            AND runs.status = 'completed'
                            AND runs.result_json IS NOT NULL
                        ) THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN overall_status = 'completed'
                        AND last_exported_at IS NOT NULL THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN TRIM(COALESCE(warnings_json, '[]')) NOT IN ('', '[]')
                        THEN 1 ELSE 0 END), 0)
             FROM jobs
             WHERE {range_filter} NOT EXISTS (
                 SELECT 1 FROM job_deletion_ops deletions WHERE deletions.job_id = jobs.id
               )"
        );
    let map = |row: &Row<'_>| {
        Ok(DashboardMetrics {
            total_jobs: row.get(0)?,
            media_duration_minutes: row.get(1)?,
            processing_duration_seconds: row.get(2)?,
            active_jobs: row.get(3)?,
            completed_jobs: row.get(4)?,
            failed_jobs: row.get(5)?,
            transcript_ready_jobs: row.get(6)?,
            speaker_eligible_jobs: row.get(7)?,
            speaker_ready_jobs: row.get(8)?,
            summary_ready_jobs: row.get(9)?,
            exported_jobs: row.get(10)?,
            warning_jobs: row.get(11)?,
        })
    };
    match start_at {
        Some(start_at) => conn.query_row(&sql, params![start_at], map),
        None => conn.query_row(&sql, [], map),
    }
    .map_err(|error| error.to_string())
}

fn query_trend(
    conn: &Connection,
    range: DashboardRange,
    start_at: Option<&str>,
) -> LocalResult<Vec<DashboardTrendPoint>> {
    let period_expression = match range {
        DashboardRange::Today => "strftime('%H:00', created_at, 'localtime')",
        DashboardRange::All => "strftime('%Y-%m', created_at, 'localtime')",
        DashboardRange::SevenDays | DashboardRange::ThirtyDays => "date(created_at, 'localtime')",
    };
    let range_filter = if start_at.is_some() {
        "created_at >= ?1 AND"
    } else {
        ""
    };
    let limit = match range {
        DashboardRange::Today => 24,
        DashboardRange::All => 36,
        DashboardRange::SevenDays | DashboardRange::ThirtyDays => 31,
    };
    let sql = format!(
        "SELECT period, total_jobs, completed_jobs, failed_jobs,
                media_duration_minutes, processing_duration_seconds
         FROM (
           SELECT {period_expression} AS period,
                  COUNT(1) AS total_jobs,
                  SUM(CASE WHEN overall_status = 'completed' THEN 1 ELSE 0 END) AS completed_jobs,
                  SUM(CASE WHEN overall_status = 'failed' THEN 1 ELSE 0 END) AS failed_jobs,
                  COALESCE(SUM(MAX(duration_minutes, 0)), 0) AS media_duration_minutes,
                  COALESCE(SUM(MAX(COALESCE(processing_duration_seconds, 0), 0)), 0)
                    AS processing_duration_seconds
           FROM jobs
           WHERE {range_filter} NOT EXISTS (
               SELECT 1 FROM job_deletion_ops deletions WHERE deletions.job_id = jobs.id
             )
           GROUP BY period
           ORDER BY period DESC
           LIMIT {limit}
         )
         ORDER BY period ASC"
    );
    let mut statement = conn.prepare(&sql).map_err(|error| error.to_string())?;
    let map = |row: &Row<'_>| {
        Ok(DashboardTrendPoint {
            period: row.get(0)?,
            total_jobs: row.get(1)?,
            completed_jobs: row.get(2)?,
            failed_jobs: row.get(3)?,
            media_duration_minutes: row.get(4)?,
            processing_duration_seconds: row.get(5)?,
        })
    };
    let rows = match start_at {
        Some(start_at) => statement.query_map(params![start_at], map),
        None => statement.query_map([], map),
    }
    .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn query_jobs(
    conn: &Connection,
    start_at: Option<&str>,
    attention_only: bool,
) -> LocalResult<Vec<DashboardJobSummary>> {
    let attention_filter = if attention_only {
        format!(
            "AND (
               jobs.overall_status IN ({ACTIVE_STATUSES})
               OR jobs.overall_status = 'failed'
               OR TRIM(COALESCE(jobs.warnings_json, '[]')) NOT IN ('', '[]')
               OR (jobs.overall_status = 'completed' AND NOT EXISTS (
                 SELECT 1 FROM ai_summary_runs attention_runs
                 WHERE attention_runs.job_id = jobs.id
                   AND attention_runs.status = 'completed'
                   AND attention_runs.result_json IS NOT NULL
               ))
               OR (jobs.overall_status = 'completed' AND jobs.last_exported_at IS NULL)
             )"
        )
    } else {
        "AND jobs.overall_status = 'completed'".into()
    };
    let range_filter = if start_at.is_some() {
        "jobs.created_at >= ?1 AND"
    } else {
        ""
    };
    let order = if attention_only {
        format!(
            "CASE
               WHEN jobs.overall_status IN ({ACTIVE_STATUSES}) THEN 0
               WHEN jobs.overall_status = 'failed' THEN 1
               WHEN TRIM(COALESCE(jobs.warnings_json, '[]')) NOT IN ('', '[]') THEN 2
               ELSE 3
             END ASC,"
        )
    } else {
        String::new()
    };
    let sql = format!(
        "SELECT jobs.id, jobs.title, jobs.created_at, jobs.duration_minutes,
                jobs.overall_status, jobs.diarization_status, jobs.warnings_json,
                EXISTS (
                  SELECT 1 FROM ai_summary_runs runs
                  WHERE runs.job_id = jobs.id
                    AND runs.status = 'completed'
                    AND runs.result_json IS NOT NULL
                ) AS has_summary,
                jobs.last_exported_at
         FROM jobs
         WHERE {range_filter} NOT EXISTS (
             SELECT 1 FROM job_deletion_ops deletions WHERE deletions.job_id = jobs.id
           )
           {attention_filter}
         ORDER BY {order} jobs.created_at DESC
         LIMIT 6"
    );
    let mut statement = conn.prepare(&sql).map_err(|error| error.to_string())?;
    let rows = match start_at {
        Some(start_at) => statement.query_map(params![start_at], map_job_summary),
        None => statement.query_map([], map_job_summary),
    }
    .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn map_job_summary(row: &Row<'_>) -> rusqlite::Result<DashboardJobSummary> {
    let warnings_json = row.get::<_, String>(6)?;
    let warning_count = serde_json::from_str::<Vec<serde_json::Value>>(&warnings_json)
        .map(|warnings| warnings.len())
        .unwrap_or_else(|_| usize::from(!warnings_json.trim().is_empty()));
    Ok(DashboardJobSummary {
        id: row.get(0)?,
        title: row.get(1)?,
        created_at: row.get(2)?,
        duration_minutes: row.get(3)?,
        overall_status: row.get(4)?,
        diarization_status: row.get(5)?,
        warning_count,
        has_summary: row.get::<_, i64>(7)? != 0,
        last_exported_at: row.get(8)?,
    })
}

fn query_resources(conn: &Connection) -> LocalResult<DashboardResourceSummary> {
    conn.query_row(
        "SELECT
           (SELECT COUNT(1) FROM ai_model_configs),
           (SELECT COUNT(1) FROM ai_model_configs WHERE enabled = 1),
           (SELECT COUNT(1) FROM ai_summary_templates),
           (SELECT COUNT(1) FROM meeting_members)",
        [],
        |row| {
            Ok(DashboardResourceSummary {
                ai_models: row.get(0)?,
                enabled_ai_models: row.get(1)?,
                templates: row.get(2)?,
                members: row.get(3)?,
            })
        },
    )
    .map_err(|error| error.to_string())
}

fn query_companion(conn: &Connection) -> LocalResult<Option<DashboardCompanionSummary>> {
    let row = conn
        .query_row(
            "SELECT profile.name, profile.experience,
                    COALESCE(wallet.balance, 0),
                    EXISTS (
                      SELECT 1 FROM pet_daily_check_ins check_ins
                      WHERE check_ins.pet_id = profile.id
                        AND check_ins.check_in_date = date('now', 'localtime')
                    ),
                    (SELECT COUNT(1) FROM farm_plots
                     WHERE status IN ('needs_water', 'mature'))
                    + (SELECT COUNT(1) FROM work_game_tasks
                       WHERE status IN ('needsCare', 'claimable'))
             FROM pet_profile profile
             LEFT JOIN pet_wallets wallet
               ON wallet.pet_id = profile.id AND wallet.currency_key = 'LP'
             WHERE profile.id = 'default-pet'",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)? != 0,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((name, experience, lp_balance, checked_in_today, claimable_activities)) = row else {
        return Ok(None);
    };
    let level = pet_leveling::level_snapshot_from_experience(experience);
    Ok(Some(DashboardCompanionSummary {
        name,
        level: level.level,
        current_level_experience: level.current_level_exp,
        next_level_experience: level.next_level_required,
        level_progress_percent: (level.progress_ratio * 100.0).round() as i64,
        lp_balance,
        checked_in_today,
        claimable_activities,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_connection() -> Connection {
        let connection = Connection::open_in_memory().expect("database");
        crate::local_db::schema::apply_test_schema(&connection).expect("schema");
        connection
    }

    fn insert_job(
        connection: &Connection,
        id: &str,
        created_at: &str,
        status: &str,
        duration_minutes: i64,
    ) {
        connection
            .execute(
                "INSERT INTO jobs (
                   id, title, created_at, duration_minutes, lang, enable_speaker,
                   summary_template, upload_status, asr_status, summary_status,
                   overall_status, hotwords_json, export_formats_json,
                   diarization_status, warnings_json, asr_backend
                 ) VALUES (?1, ?1, ?2, ?3, 'zh', 1, '', 'completed', 'completed',
                           'idle', ?4, '[]', '[]', 'completed', '[]', 'funasr')",
                params![id, created_at, duration_minutes, status],
            )
            .expect("job");
    }

    #[test]
    fn overview_aggregates_jobs_without_loading_job_details() {
        let connection = test_connection();
        insert_job(
            &connection,
            "completed-job",
            "2026-08-13T01:00:00.000Z",
            "completed",
            45,
        );
        insert_job(
            &connection,
            "failed-job",
            "2026-08-13T02:00:00.000Z",
            "failed",
            15,
        );
        connection
            .execute(
                "UPDATE jobs SET processing_duration_seconds = 180, last_exported_at = created_at
                 WHERE id = 'completed-job'",
                [],
            )
            .expect("update");
        connection
            .execute(
                "INSERT INTO ai_summary_runs (
                   id, job_id, include_speaker, include_timestamp, extra_instructions,
                   status, result_json, created_at, updated_at
                 ) VALUES ('run-1', 'completed-job', 1, 1, '', 'completed', '{}',
                           '2026-08-13T01:10:00.000Z', '2026-08-13T01:10:00.000Z')",
                [],
            )
            .expect("summary run");

        let overview = get_overview(&connection, DashboardRange::All).expect("overview");

        assert_eq!(overview.metrics.total_jobs, 2);
        assert_eq!(overview.metrics.media_duration_minutes, 60);
        assert_eq!(overview.metrics.processing_duration_seconds, 180);
        assert_eq!(overview.metrics.completed_jobs, 1);
        assert_eq!(overview.metrics.failed_jobs, 1);
        assert_eq!(overview.metrics.summary_ready_jobs, 1);
        assert_eq!(overview.metrics.exported_jobs, 1);
        assert_eq!(overview.recent_results.len(), 1);
        assert!(overview
            .attention_jobs
            .iter()
            .any(|job| job.id == "failed-job"));
    }

    #[test]
    fn created_at_range_uses_the_existing_index() {
        let connection = test_connection();
        let detail = connection
            .prepare(
                "EXPLAIN QUERY PLAN
                 SELECT jobs.id FROM jobs
                 WHERE jobs.created_at >= ?1
                   AND NOT EXISTS (
                     SELECT 1 FROM job_deletion_ops deletions
                     WHERE deletions.job_id = jobs.id
                   )
                 ORDER BY jobs.created_at DESC
                 LIMIT 6",
            )
            .expect("query plan")
            .query_map(params!["2026-08-01T00:00:00.000Z"], |row| {
                row.get::<_, String>(3)
            })
            .expect("plan rows")
            .collect::<Result<Vec<_>, _>>()
            .expect("plan details")
            .join(" ");

        assert!(detail.contains("idx_jobs_created_at"), "{detail}");
    }

    #[test]
    fn rejects_unknown_range() {
        assert_eq!(
            DashboardRange::parse("quarter").expect_err("invalid range"),
            "工作台时间范围无效。"
        );
    }
}
