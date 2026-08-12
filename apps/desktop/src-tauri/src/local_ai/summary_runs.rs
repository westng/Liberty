use std::{
    collections::HashSet,
    sync::{Mutex, OnceLock},
};

use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use tauri::AppHandle;

use crate::{
    infrastructure::{
        ids,
        repositories::{
            ai_models,
            ai_summary_run_models::{
                AiSummaryChunkSeed, AiSummaryCompletion, AiSummaryRunLease, NewAiSummaryExecution,
            },
            ai_summary_runs,
        },
    },
    local_ai::{
        client::{plan_summary_request, send_ai_chat_completion_chunk, SummaryRequestChunks},
        prompt::build_summary_prompt_preview,
        response::{
            merge_ai_summary_responses, parse_ai_summary_result, parse_ai_summary_structured,
        },
        AiChatCompletionInput, GenerateAiSummaryInput, StartAiSummaryRunInput,
    },
    local_db::{
        self, AiModelConfig, AiSummaryRun, AiSummaryTemplate, LocalResult, MeetingJob,
        MeetingMember, MeetingMinutesPayload,
    },
};

static ACTIVE_RUNS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExecutionSnapshot {
    schema_version: u32,
    job: MeetingJob,
    model_base_url: String,
    model_name: String,
    template_id: String,
    members: Vec<MeetingMember>,
    required_speakers: Vec<String>,
    fallback_title: String,
    system_prompt: String,
}

pub fn start_or_resume(
    app: &AppHandle,
    input: StartAiSummaryRunInput,
) -> LocalResult<AiSummaryRun> {
    if input.source != "local" {
        return Err("远端 AI 总结能力尚未接入。".into());
    }
    local_db::init_database(app)?;
    let job = local_db::get_job(app, input.job_id.trim())?;
    if job.source != "local" {
        return Err("远端 AI 总结能力尚未接入，不能写入本地总结运行。".into());
    }

    if let Some(run_id) = normalized_optional(&input.run_id) {
        let conn = local_db::open_connection(app)?;
        let run = ai_summary_runs::get_summary_run(&conn, &job.id, run_id)?;
        drop(conn);
        if run.status != "completed" {
            spawn_claimed_execution(app.clone(), &job.id, &run.id)?;
        }
        return Ok(run);
    }

    let conn = local_db::open_connection(app)?;
    let existing_running = ai_summary_runs::get_running_summary_run(&conn, &job.id)?;
    drop(conn);
    if let Some(run) = existing_running {
        spawn_claimed_execution(app.clone(), &job.id, &run.id)?;
        return Ok(run);
    }

    let model_id = required(&input.model_config_id, "请选择 AI 模型。")?;
    let template_id = required(&input.template_id, "请选择 AI 总结模板。")?;
    let model = find_model(app, model_id)?;
    let template = find_template(app, template_id)?;
    let include_speaker = input.include_speaker && job.diarization_status.is_verified();
    let use_member_mapping = input.use_member_mapping && include_speaker;
    let members = if use_member_mapping {
        local_db::list_meeting_members(app)?
    } else {
        Vec::new()
    };
    let generate_input = GenerateAiSummaryInput {
        job: job.clone(),
        template: template.clone(),
        include_speaker,
        include_timestamp: input.include_timestamp,
        use_member_mapping,
        members: members.clone(),
        extra_instructions: input.extra_instructions.trim().to_string(),
    };
    let prompt = build_summary_prompt_preview(&generate_input);
    let request = AiChatCompletionInput {
        base_url: model.base_url.clone(),
        api_key: model.api_key.clone(),
        model: model.model.clone(),
        system_prompt: prompt.system.clone(),
        user_prompt: prompt.user.clone(),
    };
    let SummaryRequestChunks {
        fallback_title,
        required_speakers,
        user_prompts,
    } = plan_summary_request(&request)
        .ok_or_else(|| "AI 总结提示缺少可持久化的逐字稿边界。".to_string())?;
    if user_prompts.is_empty() {
        return Err("当前任务没有可用于总结的逐字稿内容。".into());
    }

    let transcript_snapshot_json = serde_json::to_string(primary_segments(&job))
        .map_err(|error| format!("逐字稿快照序列化失败: {error}"))?;
    let transcript_sha256 = sha256_hex(transcript_snapshot_json.as_bytes());
    let transcript_revision = format!("sha256:{transcript_sha256}");
    let snapshot = ExecutionSnapshot {
        schema_version: 1,
        job,
        model_base_url: model.base_url,
        model_name: model.model,
        template_id: template.id,
        members,
        required_speakers,
        fallback_title,
        system_prompt: prompt.system.clone(),
    };
    let snapshot_json = serde_json::to_string(&snapshot)
        .map_err(|error| format!("AI 总结执行快照序列化失败: {error}"))?;
    let chunks = user_prompts
        .into_iter()
        .enumerate()
        .map(|(index, user_prompt)| AiSummaryChunkSeed {
            index,
            sha256: sha256_hex(user_prompt.as_bytes()),
            user_prompt,
        })
        .collect::<Vec<_>>();
    let now = chrono::Utc::now().to_rfc3339();
    let run = AiSummaryRun {
        id: ids::timestamped_id("summary-run"),
        job_id: snapshot.job.id.clone(),
        model_config_id: model_id.to_string(),
        template_id: template_id.to_string(),
        include_speaker,
        include_timestamp: input.include_timestamp,
        extra_instructions: input.extra_instructions.trim().to_string(),
        status: "running".into(),
        error_message: None,
        prompt_preview: Some(format!("{}\n\n---\n\n{}", prompt.system, prompt.user)),
        raw_response: None,
        result: None,
        minutes_payload: None,
        created_at: now.clone(),
        updated_at: now,
    };
    let mut conn = local_db::open_connection(app)?;
    let persisted_run = ai_summary_runs::create_execution(
        &mut conn,
        &run,
        &NewAiSummaryExecution {
            transcript_revision: &transcript_revision,
            transcript_sha256: &transcript_sha256,
            transcript_snapshot_json: &transcript_snapshot_json,
            execution_snapshot_json: &snapshot_json,
            chunks: &chunks,
        },
    )?;
    drop(conn);
    spawn_claimed_execution(app.clone(), &persisted_run.job_id, &persisted_run.id)?;
    Ok(persisted_run)
}

pub fn resume_running_on_startup(app: &AppHandle) -> LocalResult<()> {
    local_db::init_database(app)?;
    let conn = local_db::open_connection(app)?;
    let recoverable = ai_summary_runs::list_recoverable_executions(&conn)?;
    drop(conn);
    for (job_id, run_id) in recoverable {
        if let Err(error) = spawn_claimed_execution(app.clone(), &job_id, &run_id) {
            eprintln!("[ai-summary] failed to resume run {run_id}: {error}");
        }
    }
    Ok(())
}

fn spawn_claimed_execution(app: AppHandle, job_id: &str, run_id: &str) -> LocalResult<()> {
    let mut active = ACTIVE_RUNS
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .map_err(|error| format!("AI 总结运行锁异常: {error}"))?;
    if !active.insert(run_id.to_string()) {
        return Ok(());
    }
    drop(active);

    let claim_result = (|| {
        let mut conn = local_db::open_connection(&app)?;
        ai_summary_runs::claim_execution(
            &mut conn,
            job_id,
            run_id,
            &chrono::Utc::now().to_rfc3339(),
        )
    })();
    let lease = match claim_result {
        Ok(Some(lease)) => lease,
        Ok(None) => {
            release_active_run(run_id);
            return Ok(());
        }
        Err(error) => {
            release_active_run(run_id);
            return Err(error);
        }
    };

    tauri::async_runtime::spawn(async move {
        let result = execute_claimed_run(&app, &lease).await;
        if let Err(error) = result {
            if let Ok(mut conn) = local_db::open_connection(&app) {
                let _ = ai_summary_runs::fail_execution(
                    &mut conn,
                    &lease,
                    &error,
                    &chrono::Utc::now().to_rfc3339(),
                );
            }
        }
        release_active_run(&lease.run_id);
    });
    Ok(())
}

async fn execute_claimed_run(app: &AppHandle, lease: &AiSummaryRunLease) -> LocalResult<()> {
    let conn = local_db::open_connection(app)?;
    let execution = ai_summary_runs::load_execution(&conn, lease)?;
    let pending_chunks = ai_summary_runs::list_pending_chunks(&conn, lease)?;
    drop(conn);
    let snapshot: ExecutionSnapshot = serde_json::from_str(&execution.execution_snapshot_json)
        .map_err(|error| format!("AI 总结执行快照无法读取: {error}"))?;
    if snapshot.schema_version != 1 {
        return Err(format!(
            "不支持的 AI 总结执行快照版本: {}。",
            snapshot.schema_version
        ));
    }
    if snapshot.job.id != execution.job_id {
        return Err("AI 总结执行快照与运行所属任务不一致。".into());
    }
    let snapshot_transcript_json = serde_json::to_string(primary_segments(&snapshot.job))
        .map_err(|error| format!("逐字稿快照校验失败: {error}"))?;
    let snapshot_transcript_sha256 = sha256_hex(snapshot_transcript_json.as_bytes());
    if snapshot_transcript_sha256 != execution.transcript_sha256
        || execution.transcript_revision != format!("sha256:{snapshot_transcript_sha256}")
    {
        return Err("AI 总结逐字稿快照校验失败，运行已停止。".into());
    }
    let model = find_model(app, &execution.model_config_id)?;
    if model.api_key.trim().is_empty() {
        return Err("AI 模型凭据不可用，请重新配置 API Key 后恢复运行。".into());
    }

    for chunk in pending_chunks {
        let completion = send_ai_chat_completion_chunk(AiChatCompletionInput {
            base_url: snapshot.model_base_url.clone(),
            api_key: model.api_key.clone(),
            model: snapshot.model_name.clone(),
            system_prompt: snapshot.system_prompt.clone(),
            user_prompt: chunk.user_prompt.clone(),
        })
        .await?;
        let structured = parse_ai_summary_structured(&completion.raw_response)?;
        let structured_json = serde_json::to_string(&structured)
            .map_err(|error| format!("AI 分块结构化结果序列化失败: {error}"))?;
        let conn = local_db::open_connection(app)?;
        if !ai_summary_runs::save_chunk_result(
            &conn,
            lease,
            &chunk,
            &completion.raw_response,
            &structured_json,
            &chrono::Utc::now().to_rfc3339(),
        )? {
            return Err("AI 总结运行租约已失效。".into());
        }
    }

    let conn = local_db::open_connection(app)?;
    let raw_chunks = ai_summary_runs::load_completed_chunk_responses(&conn, lease)?;
    drop(conn);
    let raw_response = merge_ai_summary_responses(
        &raw_chunks,
        &snapshot.fallback_title,
        &snapshot.required_speakers,
    )?;
    let result = parse_ai_summary_result(&raw_response, &snapshot.fallback_title)?;
    let mut minutes_payload = crate::local_export::derive_meeting_minutes_payload(
        &snapshot.job,
        &result,
        &snapshot.members,
        &snapshot.template_id,
        Some(execution.run_id.clone()),
    );
    crate::local_export::source::mark_missing_ai_speakers(&snapshot.job, &mut minutes_payload);
    crate::local_export::source::validate_speaker_coverage(&snapshot.job, &minutes_payload)?;
    let diagnostics = build_diagnostics(&execution, lease, &minutes_payload);
    let result_json = serde_json::to_string(&result)
        .map_err(|error| format!("AI 总结结果序列化失败: {error}"))?;
    let minutes_payload_json = serde_json::to_string(&minutes_payload)
        .map_err(|error| format!("会议纪要 payload 序列化失败: {error}"))?;
    let diagnostics_json = serde_json::to_string(&diagnostics)
        .map_err(|error| format!("AI 总结诊断序列化失败: {error}"))?;
    let completed_at = chrono::Utc::now().to_rfc3339();
    let mut conn = local_db::open_connection(app)?;
    let completed = ai_summary_runs::complete_execution(
        &mut conn,
        lease,
        &AiSummaryCompletion {
            raw_response: &raw_response,
            result_json: &result_json,
            minutes_payload_json: &minutes_payload_json,
            diagnostics_json: &diagnostics_json,
            completed_at: &completed_at,
        },
    )?;
    drop(conn);
    if !completed {
        return Err("AI 总结运行租约已失效，结果未发布。".into());
    }
    let _ = local_db::apply_pet_growth_event(
        app,
        "workflow",
        "ai_summary_completed",
        10,
        "proud",
        Some(&execution.run_id),
    );
    Ok(())
}

fn find_model(app: &AppHandle, model_id: &str) -> LocalResult<AiModelConfig> {
    let conn = local_db::open_connection(app)?;
    ai_models::get_ai_model(&conn, model_id)?
        .filter(|model| model.enabled)
        .ok_or_else(|| "没有找到可用的 AI 模型配置。".to_string())
}

fn find_template(app: &AppHandle, template_id: &str) -> LocalResult<AiSummaryTemplate> {
    local_db::list_ai_templates(app)?
        .into_iter()
        .find(|template| template.id == template_id)
        .ok_or_else(|| "没有找到 AI 总结模板。".to_string())
}

fn primary_segments(job: &MeetingJob) -> &[crate::local_db::TranscriptSegment] {
    if job.diarization_status.is_verified() && !job.speaker_segments.is_empty() {
        &job.speaker_segments
    } else {
        &job.transcript_segments
    }
}

fn build_diagnostics(
    execution: &crate::infrastructure::repositories::ai_summary_run_models::AiSummaryExecutionRecord,
    lease: &AiSummaryRunLease,
    payload: &MeetingMinutesPayload,
) -> serde_json::Value {
    let missing_from_ai = payload
        .speaker_reports
        .iter()
        .filter(|report| report.match_status == "missing_from_ai")
        .map(|report| report.speaker_label.clone())
        .collect::<Vec<_>>();
    let unmatched = payload
        .speaker_reports
        .iter()
        .filter(|report| report.match_status == "unmatched")
        .map(|report| report.speaker_label.clone())
        .collect::<Vec<_>>();
    json!({
        "schemaVersion": 1,
        "transcriptRevision": execution.transcript_revision,
        "transcriptSha256": execution.transcript_sha256,
        "chunkCount": execution.chunk_count,
        "attemptId": lease.attempt_id,
        "leaseToken": lease.lease_token,
        "minutesSchemaVersion": payload.schema_version,
        "missingFromAi": missing_from_ai,
        "unmatchedSpeakers": unmatched,
    })
}

fn sha256_hex(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

fn required<'a>(value: &'a str, message: &str) -> LocalResult<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        Err(message.into())
    } else {
        Ok(value)
    }
}

fn normalized_optional(value: &Option<String>) -> Option<&str> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn release_active_run(run_id: &str) {
    if let Ok(mut active) = ACTIVE_RUNS
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
    {
        active.remove(run_id);
    }
}
