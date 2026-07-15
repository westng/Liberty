use rusqlite::{params, Connection};

use crate::infrastructure::migrations;
use crate::local_db::{pet_leveling, AiSummaryTemplate, LocalResult};

const BUILTIN_TEMPLATE_TIMESTAMP: &str = "2026-04-28T00:00:00.000Z";

pub(crate) fn apply_schema(conn: &Connection) -> LocalResult<()> {
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS app_meta (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS app_settings (
          id INTEGER PRIMARY KEY CHECK (id = 1),
          theme_mode TEXT NOT NULL,
          liquid_glass_style TEXT NOT NULL,
          accent_color TEXT NOT NULL,
          locale TEXT NOT NULL,
          backend_url TEXT NOT NULL,
          api_token TEXT NOT NULL,
          default_hotwords TEXT NOT NULL,
          summary_template TEXT NOT NULL,
          concurrency INTEGER NOT NULL DEFAULT 2,
          python_path TEXT NOT NULL,
          ffmpeg_path TEXT NOT NULL DEFAULT '',
          python_runtime_source TEXT NOT NULL DEFAULT 'managed',
          ffmpeg_runtime_source TEXT NOT NULL DEFAULT 'managed',
          runner_script_path TEXT NOT NULL,
          local_asr_device TEXT NOT NULL DEFAULT 'auto',
          local_asr_threads INTEGER NOT NULL DEFAULT 0,
          local_asr_batch_size_seconds INTEGER NOT NULL DEFAULT 300,
          runtime_download_source TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS runtime_state (
          platform_id TEXT PRIMARY KEY,
          runtime_version TEXT NOT NULL,
          python_version TEXT NOT NULL,
          status TEXT NOT NULL,
          python_executable_path TEXT,
          models_root TEXT,
          install_root TEXT,
          last_error TEXT,
          installed_at TEXT,
          updated_at TEXT NOT NULL,
          last_log_path TEXT
        );

        CREATE TABLE IF NOT EXISTS runtime_component_state (
          platform_id TEXT NOT NULL,
          component TEXT NOT NULL,
          source TEXT NOT NULL,
          availability TEXT NOT NULL DEFAULT 'unavailable',
          active_generation_id TEXT,
          artifact_version TEXT,
          resolved_path TEXT,
          operation_kind TEXT NOT NULL DEFAULT 'idle',
          operation_generation INTEGER NOT NULL DEFAULT 0,
          phase TEXT NOT NULL DEFAULT 'idle',
          progress INTEGER,
          last_error TEXT,
          updated_at TEXT NOT NULL,
          PRIMARY KEY(platform_id, component, source)
        );

        CREATE TABLE IF NOT EXISTS jobs (
          id TEXT PRIMARY KEY,
          title TEXT NOT NULL,
          created_at TEXT NOT NULL,
          duration_minutes INTEGER NOT NULL DEFAULT 0,
          lang TEXT NOT NULL,
          enable_speaker INTEGER NOT NULL DEFAULT 1,
          summary_template TEXT NOT NULL,
         upload_status TEXT NOT NULL,
         asr_status TEXT NOT NULL,
         summary_status TEXT NOT NULL,
         overall_status TEXT NOT NULL,
          processing_started_at_ms INTEGER,
          processing_finished_at_ms INTEGER,
          processing_duration_seconds INTEGER,
          failure_reason TEXT,
          process_log TEXT,
          python_path TEXT,
          runner_script_path TEXT,
          active_summary_run_id TEXT,
          last_exported_at TEXT,
          hotwords_json TEXT NOT NULL,
          export_formats_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS job_source_files (
          id TEXT PRIMARY KEY,
          job_id TEXT NOT NULL,
          name TEXT NOT NULL,
          path TEXT,
          size_label TEXT NOT NULL,
          kind TEXT NOT NULL,
          FOREIGN KEY(job_id) REFERENCES jobs(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS transcript_segments (
          id TEXT PRIMARY KEY,
          job_id TEXT NOT NULL,
          segment_type TEXT NOT NULL,
          start_ms INTEGER NOT NULL,
          end_ms INTEGER NOT NULL,
          speaker TEXT,
          text TEXT NOT NULL,
          segment_order INTEGER NOT NULL,
          FOREIGN KEY(job_id) REFERENCES jobs(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS ai_model_configs (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          base_url TEXT NOT NULL,
          api_key TEXT NOT NULL,
          api_key_ref TEXT NOT NULL DEFAULT '',
          model TEXT NOT NULL,
          enabled INTEGER NOT NULL DEFAULT 1,
          is_default INTEGER NOT NULL DEFAULT 0,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS ai_summary_templates (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          description TEXT NOT NULL,
          prompt TEXT NOT NULL,
          include_speaker_by_default INTEGER NOT NULL DEFAULT 1,
          include_timestamp_by_default INTEGER NOT NULL DEFAULT 1,
          builtin INTEGER NOT NULL DEFAULT 0,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS ai_summary_runs (
          id TEXT PRIMARY KEY,
          job_id TEXT NOT NULL,
          model_config_id TEXT,
          template_id TEXT,
          include_speaker INTEGER NOT NULL DEFAULT 1,
          include_timestamp INTEGER NOT NULL DEFAULT 1,
          extra_instructions TEXT NOT NULL DEFAULT '',
          status TEXT NOT NULL,
          error_message TEXT,
          prompt_preview TEXT,
          raw_response TEXT,
          result_json TEXT,
          minutes_payload_json TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          FOREIGN KEY(job_id) REFERENCES jobs(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS job_events (
          id TEXT PRIMARY KEY,
          job_id TEXT NOT NULL,
          event_type TEXT NOT NULL,
          message TEXT NOT NULL DEFAULT '',
          metadata_json TEXT,
          created_at TEXT NOT NULL,
          FOREIGN KEY(job_id) REFERENCES jobs(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS meeting_members (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          department TEXT NOT NULL DEFAULT '',
          sort_order INTEGER NOT NULL DEFAULT 0,
          is_recorder INTEGER NOT NULL DEFAULT 0,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS pet_profile (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          level INTEGER NOT NULL DEFAULT 1,
          experience INTEGER NOT NULL DEFAULT 0,
          stage TEXT NOT NULL,
          current_mood TEXT NOT NULL,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS pet_settings (
          pet_id TEXT PRIMARY KEY,
          desktop_enabled INTEGER NOT NULL DEFAULT 1,
          always_on_top INTEGER NOT NULL DEFAULT 1,
          muted INTEGER NOT NULL DEFAULT 0,
          focus_mode_enabled INTEGER NOT NULL DEFAULT 0,
          proactive_level INTEGER NOT NULL DEFAULT 2,
          last_window_x REAL,
          last_window_y REAL,
          updated_at TEXT NOT NULL,
          FOREIGN KEY(pet_id) REFERENCES pet_profile(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS pet_cosmetic_unlocks (
          id TEXT PRIMARY KEY,
          pet_id TEXT NOT NULL,
          cosmetic_type TEXT NOT NULL,
          cosmetic_key TEXT NOT NULL,
          unlocked_at TEXT NOT NULL,
          equipped INTEGER NOT NULL DEFAULT 0,
          FOREIGN KEY(pet_id) REFERENCES pet_profile(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS pet_event_ledger (
          id TEXT PRIMARY KEY,
          pet_id TEXT NOT NULL,
          event_type TEXT NOT NULL,
          event_source TEXT NOT NULL,
          event_value INTEGER NOT NULL DEFAULT 0,
          event_time TEXT NOT NULL,
          metadata TEXT,
          FOREIGN KEY(pet_id) REFERENCES pet_profile(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS pet_wallets (
          pet_id TEXT NOT NULL,
          currency_key TEXT NOT NULL,
          balance INTEGER NOT NULL DEFAULT 0,
          lifetime_earned INTEGER NOT NULL DEFAULT 0,
          lifetime_spent INTEGER NOT NULL DEFAULT 0,
          updated_at TEXT NOT NULL,
          PRIMARY KEY(pet_id, currency_key),
          FOREIGN KEY(pet_id) REFERENCES pet_profile(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS pet_inventory (
          id TEXT PRIMARY KEY,
          pet_id TEXT NOT NULL,
          item_key TEXT NOT NULL,
          item_type TEXT NOT NULL,
          slot TEXT NOT NULL,
          quantity INTEGER NOT NULL DEFAULT 1,
          equipped INTEGER NOT NULL DEFAULT 0,
          source TEXT NOT NULL,
          purchased_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          UNIQUE(pet_id, item_key),
          FOREIGN KEY(pet_id) REFERENCES pet_profile(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS pet_economy_ledger (
          id TEXT PRIMARY KEY,
          pet_id TEXT NOT NULL,
          entry_type TEXT NOT NULL,
          currency_key TEXT NOT NULL,
          amount INTEGER NOT NULL,
          balance_after INTEGER NOT NULL,
          source_type TEXT NOT NULL,
          source_key TEXT NOT NULL,
          metadata TEXT,
          created_at TEXT NOT NULL,
          UNIQUE(pet_id, currency_key, source_type, source_key),
          FOREIGN KEY(pet_id) REFERENCES pet_profile(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS pet_redeem_key_redemptions (
          id TEXT PRIMARY KEY,
          pet_id TEXT NOT NULL,
          key_hash TEXT NOT NULL,
          code_prefix TEXT NOT NULL,
          campaign_id TEXT NOT NULL,
          reward_json TEXT NOT NULL,
          status TEXT NOT NULL,
          redeemed_at TEXT NOT NULL,
          metadata TEXT,
          UNIQUE(pet_id, key_hash),
          FOREIGN KEY(pet_id) REFERENCES pet_profile(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS pet_blind_box_draws (
          id TEXT PRIMARY KEY,
          pet_id TEXT NOT NULL,
          draw_date TEXT NOT NULL,
          item_key TEXT NOT NULL,
          item_type TEXT NOT NULL,
          quantity INTEGER NOT NULL DEFAULT 1,
          duplicate_compensation_lp INTEGER NOT NULL DEFAULT 0,
          created_at TEXT NOT NULL,
          FOREIGN KEY(pet_id) REFERENCES pet_profile(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS pet_daily_check_ins (
          id TEXT PRIMARY KEY,
          pet_id TEXT NOT NULL,
          check_in_date TEXT NOT NULL,
          streak_count INTEGER NOT NULL DEFAULT 1,
          cycle_day INTEGER NOT NULL DEFAULT 1,
          reward_lp INTEGER NOT NULL DEFAULT 0,
          growth_value INTEGER NOT NULL DEFAULT 0,
          reward_items_json TEXT NOT NULL DEFAULT '[]',
          created_at TEXT NOT NULL,
          UNIQUE(pet_id, check_in_date),
          FOREIGN KEY(pet_id) REFERENCES pet_profile(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS pet_store_daily_limits (
          pet_id TEXT NOT NULL,
          item_key TEXT NOT NULL,
          limit_date TEXT NOT NULL,
          free_claimed INTEGER NOT NULL DEFAULT 0,
          updated_at TEXT NOT NULL,
          PRIMARY KEY(pet_id, item_key, limit_date),
          FOREIGN KEY(pet_id) REFERENCES pet_profile(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS pet_milestone_counters (
          pet_id TEXT NOT NULL,
          counter_key TEXT NOT NULL,
          counter_value INTEGER NOT NULL DEFAULT 0,
          last_event_key TEXT NOT NULL DEFAULT '',
          updated_at TEXT NOT NULL,
          PRIMARY KEY(pet_id, counter_key),
          FOREIGN KEY(pet_id) REFERENCES pet_profile(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_jobs_created_at ON jobs(created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_job_source_files_job_id ON job_source_files(job_id);
        CREATE INDEX IF NOT EXISTS idx_segments_job_id ON transcript_segments(job_id, segment_type, segment_order);
        CREATE INDEX IF NOT EXISTS idx_ai_runs_job_id ON ai_summary_runs(job_id, updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_job_events_job_id ON job_events(job_id, created_at ASC);
        CREATE INDEX IF NOT EXISTS idx_meeting_members_sort_order ON meeting_members(sort_order ASC, updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_pet_event_ledger_time ON pet_event_ledger(event_time DESC);
        CREATE INDEX IF NOT EXISTS idx_pet_cosmetic_unlocks_pet_id ON pet_cosmetic_unlocks(pet_id, unlocked_at DESC);
        CREATE INDEX IF NOT EXISTS idx_pet_inventory_pet_id ON pet_inventory(pet_id, updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_pet_economy_pet_id ON pet_economy_ledger(pet_id, created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_pet_redeem_key_pet_time ON pet_redeem_key_redemptions(pet_id, redeemed_at DESC);
        CREATE INDEX IF NOT EXISTS idx_pet_blind_box_draws_pet_date ON pet_blind_box_draws(pet_id, draw_date, created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_pet_daily_check_ins_pet_date ON pet_daily_check_ins(pet_id, check_in_date DESC);
        CREATE INDEX IF NOT EXISTS idx_pet_store_daily_limits_pet_date ON pet_store_daily_limits(pet_id, limit_date DESC);
        CREATE INDEX IF NOT EXISTS idx_pet_milestones_pet_id ON pet_milestone_counters(pet_id, counter_key);
        CREATE INDEX IF NOT EXISTS idx_runtime_state_status ON runtime_state(status);
        CREATE INDEX IF NOT EXISTS idx_runtime_component_operation ON runtime_component_state(operation_kind, component);
        ",
    )
    .map_err(|err| err.to_string())?;

    migrations::add_column_if_missing(
        conn,
        "ALTER TABLE jobs ADD COLUMN active_summary_run_id TEXT",
    )?;

    for statement in [
        "ALTER TABLE jobs ADD COLUMN processing_started_at_ms INTEGER",
        "ALTER TABLE jobs ADD COLUMN processing_finished_at_ms INTEGER",
        "ALTER TABLE jobs ADD COLUMN processing_duration_seconds INTEGER",
    ] {
        migrations::add_column_if_missing(conn, statement)?;
    }

    for statement in [
        "ALTER TABLE app_settings ADD COLUMN local_asr_device TEXT NOT NULL DEFAULT 'auto'",
        "ALTER TABLE app_settings ADD COLUMN local_asr_threads INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE app_settings ADD COLUMN local_asr_batch_size_seconds INTEGER NOT NULL DEFAULT 300",
        "ALTER TABLE app_settings ADD COLUMN runtime_download_source TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE app_settings ADD COLUMN ffmpeg_path TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE app_settings ADD COLUMN python_runtime_source TEXT NOT NULL DEFAULT 'managed'",
        "ALTER TABLE app_settings ADD COLUMN ffmpeg_runtime_source TEXT NOT NULL DEFAULT 'managed'",
        "ALTER TABLE ai_model_configs ADD COLUMN api_key_ref TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE ai_summary_runs ADD COLUMN minutes_payload_json TEXT",
    ] {
        migrations::add_column_if_missing(conn, statement)?;
    }

    migrate_runtime_source_settings(conn)?;

    migrate_pet_leveling_255(conn)?;

    Ok(())
}

fn migrate_runtime_source_settings(conn: &Connection) -> LocalResult<()> {
    let migrated = conn
        .query_row(
            "SELECT value FROM app_meta WHERE key = 'runtime_sources_migrated'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok();
    if migrated.is_some() {
        return Ok(());
    }

    let transaction = conn
        .unchecked_transaction()
        .map_err(|err| err.to_string())?;
    transaction
        .execute(
            "UPDATE app_settings
             SET python_runtime_source = CASE
                   WHEN TRIM(python_path) <> '' THEN 'system'
                   ELSE 'managed'
                 END,
                 ffmpeg_runtime_source = CASE
                   WHEN TRIM(ffmpeg_path) <> '' THEN 'system'
                   ELSE 'managed'
                 END
             WHERE id = 1",
            [],
        )
        .map_err(|err| err.to_string())?;
    transaction
        .execute(
            "INSERT INTO app_meta(key, value)
             VALUES('runtime_sources_migrated', '2026-07-15')",
            [],
        )
        .map_err(|err| err.to_string())?;
    transaction.commit().map_err(|err| err.to_string())
}

#[cfg(test)]
pub(crate) fn apply_test_schema(conn: &Connection) -> LocalResult<()> {
    apply_schema(conn)
}

#[cfg(test)]
mod runtime_source_tests {
    use super::apply_test_schema;
    use rusqlite::{params, Connection};

    #[test]
    fn migrates_legacy_paths_to_explicit_sources_only_once() {
        let conn = Connection::open_in_memory().expect("database");
        conn.execute_batch(
            "CREATE TABLE app_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                theme_mode TEXT NOT NULL,
                liquid_glass_style TEXT NOT NULL,
                accent_color TEXT NOT NULL,
                locale TEXT NOT NULL,
                backend_url TEXT NOT NULL,
                api_token TEXT NOT NULL,
                default_hotwords TEXT NOT NULL,
                summary_template TEXT NOT NULL,
                concurrency INTEGER NOT NULL,
                python_path TEXT NOT NULL,
                ffmpeg_path TEXT NOT NULL,
                runner_script_path TEXT NOT NULL,
                local_asr_device TEXT NOT NULL,
                local_asr_threads INTEGER NOT NULL,
                local_asr_batch_size_seconds INTEGER NOT NULL,
                runtime_download_source TEXT NOT NULL
            );
            INSERT INTO app_settings VALUES (
                1, 'auto', 'transparent', '#2f6dff', 'zh-CN', '', '', '', '', 2,
                '/custom/python', '', '', 'auto', 0, 300, 'official'
            );",
        )
        .expect("legacy settings");

        apply_test_schema(&conn).expect("migrate schema");
        let migrated = conn
            .query_row(
                "SELECT python_runtime_source, ffmpeg_runtime_source FROM app_settings WHERE id = 1",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .expect("migrated sources");
        assert_eq!(migrated, ("system".into(), "managed".into()));

        conn.execute(
            "UPDATE app_settings SET python_runtime_source = ?1 WHERE id = 1",
            params!["managed"],
        )
        .expect("user changes source");
        apply_test_schema(&conn).expect("reapply schema");
        let source = conn
            .query_row(
                "SELECT python_runtime_source FROM app_settings WHERE id = 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("source after second apply");
        assert_eq!(source, "managed");
    }
}

fn migrate_pet_leveling_255(conn: &Connection) -> LocalResult<()> {
    let migrated = conn
        .query_row(
            "SELECT value FROM app_meta WHERE key = 'pet_leveling_255_migrated'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok();
    if migrated.is_some() {
        return Ok(());
    }

    let profile = conn
        .query_row(
            "SELECT level, experience FROM pet_profile WHERE id = 'default-pet'",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .ok();

    if let Some((stored_level, experience)) = profile {
        let snapshot = pet_leveling::level_snapshot_from_experience(experience);
        let effective_level = stored_level
            .max(snapshot.level)
            .clamp(1, pet_leveling::MAX_PET_LEVEL);
        let effective_experience = if effective_level > snapshot.level {
            pet_leveling::total_required_exp_for_level(effective_level)
        } else {
            experience
        };
        let next_snapshot = pet_leveling::level_snapshot_from_experience(effective_experience);
        conn.execute(
            "UPDATE pet_profile
             SET level = ?2, experience = ?3, stage = ?4, updated_at = ?5
             WHERE id = ?1",
            params![
                "default-pet",
                next_snapshot.level,
                effective_experience,
                next_snapshot.current_stage,
                chrono::Utc::now().to_rfc3339()
            ],
        )
        .map_err(|err| err.to_string())?;
    }

    conn.execute(
        "INSERT OR REPLACE INTO app_meta(key, value) VALUES('pet_leveling_255_migrated', ?1)",
        params!["2026-05-21"],
    )
    .map_err(|err| err.to_string())?;

    Ok(())
}

pub(crate) fn seed_builtin_templates(conn: &Connection) -> LocalResult<()> {
    let templates = [
        AiSummaryTemplate {
            id: "builtin-formal-meeting-minutes".into(),
            name: "表格版会议纪要".into(),
            description: "按正式会议纪要版式整理，适合管理例会、周会和部门汇报。".into(),
            prompt: "你是资深会议纪要助手。请基于用户提供的会议转写内容输出结构化 JSON，用于生成正式会议纪要。\n\n要求：\n1. 只输出合法 JSON，不要输出 Markdown、解释或额外文本。\n2. 保持客观，不要编造原文中不存在的事实；无法确认的信息写“待补充”或返回空字符串。\n3. 结果字段固定为 title、overview、topics、decisions、actionItems、risks、followUps。\n4. title 填会议名称；若原文无法判断，则使用用户提供的 Meeting title。\n5. overview 必须输出一整段可直接展示的正式会议纪要正文，并严格使用以下固定结构与字段顺序，保留换行：\n会议名称：...\n会议时间：...\n会议地点：...\n记录人：...\n\n出席人员：...\n缺席人员：...\n主要议题：...\n会议主持人：...\n审阅：...\n\n发言内容\n\n【部门】：【姓名】\n上周总结：\n1、...\n2、...\n\n本周计划：\n1、...\n2、...\n\n总结：\n1、...\n2、...\n6. 发言内容必须按发言人分组整理。只要转写内容里已经带有说话人标签，姓名就直接使用该标签，不要改写、合并或重新猜测姓名。部门如果无法从原文判断，可以写“待补充部门”，后续会由人员管理信息补齐。\n7. topics 返回“主要议题”的字符串列表，用于辅助展示；如果 overview 中已经完整写明，也仍然返回数组。\n8. decisions 固定返回空数组，不要输出会议结论内容。\n9. actionItems 固定返回空数组，不要输出待办事项内容。\n10. risks 固定返回空数组，除非用户明确要求额外输出风险信息。\n11. followUps 固定返回空数组，除非用户明确要求额外输出后续跟进信息。".into(),
            include_speaker_by_default: true,
            include_timestamp_by_default: false,
            builtin: true,
            created_at: BUILTIN_TEMPLATE_TIMESTAMP.into(),
            updated_at: BUILTIN_TEMPLATE_TIMESTAMP.into(),
        },
        AiSummaryTemplate {
            id: "builtin-standard-summary".into(),
            name: "标准会议纪要".into(),
            description: "输出摘要、议题、结论、行动项、风险与跟进事项。".into(),
            prompt: "你是资深会议纪要助手。请基于用户提供的会议转写内容输出结构化 JSON。\n\n要求：\n1. 只输出合法 JSON，不要输出 Markdown、解释或额外文本。\n2. 保持客观，不要编造原文中不存在的事实。\n3. 结果必须包含 title、overview、topics、decisions、actionItems、risks、followUps。\n4. actionItems 必须是数组，每项包含 task、owner、dueDate 三个字段；无法判断时 owner 和 dueDate 置为空字符串。\n5. topics、decisions、risks、followUps 都返回字符串数组。\n6. overview 用简洁中文概述会议重点。".into(),
            include_speaker_by_default: true,
            include_timestamp_by_default: true,
            builtin: true,
            created_at: BUILTIN_TEMPLATE_TIMESTAMP.into(),
            updated_at: BUILTIN_TEMPLATE_TIMESTAMP.into(),
        },
        AiSummaryTemplate {
            id: "builtin-decisions-actions".into(),
            name: "决策与待办".into(),
            description: "更强调最终决策、责任归属和后续执行。".into(),
            prompt: "你是会议行动项整理助手。请根据会议内容输出结构化 JSON。\n\n要求：\n1. 只输出合法 JSON。\n2. 重点提炼已确认的决策、待办事项、负责人和时间信息。\n3. 如果原文没有明确负责人或截止日期，请返回空字符串，不要猜测。\n4. 结果字段固定为 title、overview、topics、decisions、actionItems、risks、followUps。".into(),
            include_speaker_by_default: true,
            include_timestamp_by_default: false,
            builtin: true,
            created_at: BUILTIN_TEMPLATE_TIMESTAMP.into(),
            updated_at: BUILTIN_TEMPLATE_TIMESTAMP.into(),
        },
        AiSummaryTemplate {
            id: "builtin-project-weekly-review".into(),
            name: "项目周会总结".into(),
            description: "适合项目推进类会议，重点整理进展、风险和下一步。".into(),
            prompt: "你是项目周会总结助手。请把会议内容整理成结构化 JSON。\n\n要求：\n1. 只输出合法 JSON。\n2. overview 要覆盖进度、阻塞点和下一步方向。\n3. topics 聚焦当前进度与关键议题。\n4. risks 与 followUps 必须尽量完整。\n5. 结果字段固定为 title、overview、topics、decisions、actionItems、risks、followUps。".into(),
            include_speaker_by_default: true,
            include_timestamp_by_default: false,
            builtin: true,
            created_at: BUILTIN_TEMPLATE_TIMESTAMP.into(),
            updated_at: BUILTIN_TEMPLATE_TIMESTAMP.into(),
        },
        AiSummaryTemplate {
            id: "builtin-interview-notes".into(),
            name: "访谈整理".into(),
            description: "适合客户访谈、需求访谈或复盘访谈。".into(),
            prompt: "你是访谈内容整理助手。请把访谈内容整理成结构化 JSON。\n\n要求：\n1. 只输出合法 JSON。\n2. summary 需要突出受访者核心观点和关键诉求。\n3. topics 用于概括主题，decisions 记录明确共识，actionItems 记录后续动作。\n4. 结果字段固定为 title、overview、topics、decisions、actionItems、risks、followUps。".into(),
            include_speaker_by_default: true,
            include_timestamp_by_default: true,
            builtin: true,
            created_at: BUILTIN_TEMPLATE_TIMESTAMP.into(),
            updated_at: BUILTIN_TEMPLATE_TIMESTAMP.into(),
        },
    ];

    for template in templates {
        save_ai_template_inner(conn, &template)?;
    }

    Ok(())
}

fn save_ai_template_inner(conn: &Connection, template: &AiSummaryTemplate) -> LocalResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO ai_summary_templates (
            id, name, description, prompt, include_speaker_by_default,
            include_timestamp_by_default, builtin, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            template.id,
            template.name,
            template.description,
            template.prompt,
            if template.include_speaker_by_default {
                1
            } else {
                0
            },
            if template.include_timestamp_by_default {
                1
            } else {
                0
            },
            if template.builtin { 1 } else { 0 },
            template.created_at,
            template.updated_at
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}
