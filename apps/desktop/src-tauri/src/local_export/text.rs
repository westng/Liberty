use crate::local_export::output::{authorized_output_path, write_text_atomically};
use crate::{
    domain::asr::DiarizationStatus,
    local_db::{self, LocalResult, MeetingJob, MeetingSummary, TranscriptSegment},
};
use serde::Deserialize;
use tauri::{AppHandle, Webview};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TextExportInput {
    job_id: String,
    source: String,
    kind: String,
    file_path: String,
    labels: TextExportLabels,
    remote_job: Option<TextExportJobSnapshot>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TextExportJobSnapshot {
    id: String,
    source: String,
    title: String,
    diarization_status: DiarizationStatus,
    transcript_segments: Vec<TranscriptSegment>,
    speaker_segments: Vec<TranscriptSegment>,
    summary: MeetingSummary,
}

impl From<TextExportJobSnapshot> for MeetingJob {
    fn from(snapshot: TextExportJobSnapshot) -> Self {
        Self {
            id: snapshot.id,
            source: snapshot.source,
            title: snapshot.title,
            diarization_status: snapshot.diarization_status,
            transcript_segments: snapshot.transcript_segments,
            speaker_segments: snapshot.speaker_segments,
            summary: snapshot.summary,
            ..Self::default()
        }
    }
}

#[derive(Clone, Copy)]
enum TextExportKind {
    Transcript,
    Notes,
    Bundle,
}

impl TextExportKind {
    fn parse(value: &str) -> LocalResult<Self> {
        match value {
            "transcript" => Ok(Self::Transcript),
            "notes" => Ok(Self::Notes),
            "bundle" => Ok(Self::Bundle),
            _ => Err("不支持的文本导出格式。".into()),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TextExportLabels {
    unknown_speaker: String,
    transcript_heading: String,
    summary_heading: String,
    topics_heading: String,
    decisions_heading: String,
    action_items_heading: String,
    risks_heading: String,
    follow_ups_heading: String,
    empty_summary: String,
}

pub(crate) fn export_job_text(
    app: AppHandle,
    webview: Webview,
    mut input: TextExportInput,
) -> LocalResult<()> {
    let output_path = authorized_output_path(&webview, &input.file_path)?;
    let kind = TextExportKind::parse(&input.kind)?;
    let job = resolve_export_job(&app, &mut input)?;
    let content = render_text_export(&job, kind, &input.labels);
    write_text_atomically(&output_path, &content)
}

fn resolve_export_job(app: &AppHandle, input: &mut TextExportInput) -> LocalResult<MeetingJob> {
    let job_id = input.job_id.trim();
    if job_id.is_empty() {
        return Err("导出任务 ID 不能为空。".into());
    }

    let job = match input.source.as_str() {
        "local" => {
            if input.remote_job.is_some() {
                return Err("本地任务导出不能使用 Web 快照。".into());
            }
            local_db::get_job(app, job_id)?
        }
        "remote" => {
            let job = input
                .remote_job
                .take()
                .ok_or_else(|| "远端任务导出缺少只读任务快照。".to_string())?;
            if job.id != job_id || job.source != "remote" {
                return Err("远端任务导出快照与任务引用不匹配。".into());
            }
            job.into()
        }
        _ => return Err("不支持的任务来源。".into()),
    };

    Ok(job)
}

fn render_text_export(job: &MeetingJob, kind: TextExportKind, labels: &TextExportLabels) -> String {
    match kind {
        TextExportKind::Transcript => render_transcript(job, labels),
        TextExportKind::Notes => render_notes(&job.title, &job.summary, labels),
        TextExportKind::Bundle => format!(
            "{}\n\n{}\n{}",
            render_notes(&job.title, &job.summary, labels),
            labels.transcript_heading,
            render_transcript(job, labels)
        ),
    }
}

fn render_transcript(job: &MeetingJob, labels: &TextExportLabels) -> String {
    primary_transcript_segments(job)
        .iter()
        .map(|segment| {
            format!(
                "[{} - {}] {}: {}",
                timestamp(segment.start_ms),
                timestamp(segment.end_ms),
                segment
                    .speaker
                    .as_deref()
                    .filter(|speaker| !speaker.trim().is_empty())
                    .unwrap_or(&labels.unknown_speaker),
                segment.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn primary_transcript_segments(job: &MeetingJob) -> &[TranscriptSegment] {
    if job.diarization_status.is_verified() && !job.speaker_segments.is_empty() {
        &job.speaker_segments
    } else {
        &job.transcript_segments
    }
}

fn timestamp(milliseconds: u64) -> String {
    let seconds = (milliseconds / 1_000) % (24 * 60 * 60);
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3_600,
        (seconds % 3_600) / 60,
        seconds % 60
    )
}

fn render_notes(title: &str, summary: &MeetingSummary, labels: &TextExportLabels) -> String {
    let mut lines = vec![
        format!("# {title}"),
        String::new(),
        labels.summary_heading.clone(),
        if summary.overview.is_empty() {
            labels.empty_summary.clone()
        } else {
            summary.overview.clone()
        },
        String::new(),
        labels.topics_heading.clone(),
    ];
    append_list(&mut lines, &summary.topics);
    lines.extend([String::new(), labels.decisions_heading.clone()]);
    append_list(&mut lines, &summary.decisions);
    lines.extend([String::new(), labels.action_items_heading.clone()]);
    append_list(&mut lines, &summary.action_items);
    append_optional_section(&mut lines, &labels.risks_heading, &summary.risks);
    append_optional_section(&mut lines, &labels.follow_ups_heading, &summary.follow_ups);
    lines.join("\n")
}

fn append_list(lines: &mut Vec<String>, items: &[String]) {
    lines.extend(items.iter().map(|item| format!("- {item}")));
}

fn append_optional_section(lines: &mut Vec<String>, heading: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    lines.extend([String::new(), heading.into()]);
    append_list(lines, items);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::asr::DiarizationStatus;

    fn job() -> MeetingJob {
        MeetingJob {
            title: "Weekly Sync".into(),
            diarization_status: DiarizationStatus::Completed,
            transcript_segments: vec![TranscriptSegment {
                start_ms: 0,
                end_ms: 1_000,
                text: "raw".into(),
                ..TranscriptSegment::default()
            }],
            speaker_segments: vec![TranscriptSegment {
                start_ms: 3_661_000,
                end_ms: 3_662_000,
                speaker: Some("Alice".into()),
                text: "verified".into(),
                ..TranscriptSegment::default()
            }],
            summary: MeetingSummary {
                overview: "Overview".into(),
                topics: vec!["Topic".into()],
                decisions: vec!["Decision".into()],
                action_items: vec!["Action".into()],
                risks: vec!["Risk".into()],
                follow_ups: vec!["Follow-up".into()],
            },
            ..MeetingJob::default()
        }
    }

    fn labels() -> TextExportLabels {
        TextExportLabels {
            unknown_speaker: "Unknown speaker".into(),
            transcript_heading: "## Transcript".into(),
            summary_heading: "## Overview".into(),
            topics_heading: "## Topics".into(),
            decisions_heading: "## Decisions".into(),
            action_items_heading: "## Action Items".into(),
            risks_heading: "## Risks".into(),
            follow_ups_heading: "## Follow-ups".into(),
            empty_summary: "No summary".into(),
        }
    }

    #[test]
    fn transcript_uses_verified_speaker_projection() {
        let rendered = render_transcript(&job(), &labels());

        assert_eq!(rendered, "[01:01:01 - 01:01:02] Alice: verified");
    }

    #[test]
    fn notes_render_all_structured_sections() {
        let rendered = render_notes(&job().title, &job().summary, &labels());

        for expected in [
            "# Weekly Sync",
            "## Overview\nOverview",
            "## Topics\n- Topic",
            "## Decisions\n- Decision",
            "## Action Items\n- Action",
            "## Risks\n- Risk",
            "## Follow-ups\n- Follow-up",
        ] {
            assert!(rendered.contains(expected), "missing: {expected}");
        }
    }

    #[test]
    fn invalid_kind_is_rejected() {
        assert!(TextExportKind::parse("html").is_err());
    }
}
