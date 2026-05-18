use crate::local_ai::GenerateAiSummaryInput;
use crate::local_db::{MeetingJob, TranscriptSegment};

#[derive(Debug)]
pub(super) struct PromptPreview {
    pub system: String,
    pub user: String,
}

pub(super) fn build_summary_prompt_preview(input: &GenerateAiSummaryInput) -> PromptPreview {
    let transcript = primary_transcript_segments(&input.job)
        .iter()
        .map(|segment| format_segment(segment, input.include_speaker, input.include_timestamp))
        .collect::<Vec<_>>()
        .join("\n");

    let mut lines = vec![
        format!("Meeting title: {}", input.job.title),
        format!("Meeting language: {}", input.job.lang),
        format!(
            "Hotwords: {}",
            if input.job.hotwords.is_empty() {
                "none".to_string()
            } else {
                input.job.hotwords.join(", ")
            }
        ),
        format!(
            "Include speaker info: {}",
            if input.include_speaker { "yes" } else { "no" }
        ),
        format!(
            "Include timestamps: {}",
            if input.include_timestamp { "yes" } else { "no" }
        ),
        format!(
            "Use member mapping: {}",
            if input.use_member_mapping {
                "yes"
            } else {
                "no"
            }
        ),
        format!(
            "Extra instructions: {}",
            non_empty(&input.extra_instructions).unwrap_or("none")
        ),
    ];

    if input.use_member_mapping {
        append_member_mapping(&mut lines, input);
    }

    lines.push(String::new());
    lines.push("Please output JSON based on the following meeting content:".into());
    lines.push(if transcript.trim().is_empty() {
        "Transcript is missing.".into()
    } else {
        transcript
    });

    PromptPreview {
        system: input.template.prompt.trim().to_string(),
        user: lines.join("\n"),
    }
}

fn append_member_mapping(lines: &mut Vec<String>, input: &GenerateAiSummaryInput) {
    lines.push(String::new());
    lines.push("Member directory mapping:".into());
    if input.members.is_empty() {
        lines.push("- No member directory records available.".into());
    } else {
        let mut members = input.members.clone();
        members.sort_by_key(|member| member.sort_order);
        lines.extend(members.into_iter().map(|member| {
            format!(
                "- {} | department={} | sortOrder={} | recorder={}",
                member.name,
                if member.department.trim().is_empty() {
                    "未设置"
                } else {
                    member.department.trim()
                },
                member.sort_order,
                if member.is_recorder { "yes" } else { "no" }
            )
        }));
    }
    lines.push(String::new());
    lines.push("When the transcript already contains speaker names, keep those names exactly as they appear and use the member directory only to补充部门、排序相关上下文，不要改写姓名。".into());
}

fn primary_transcript_segments(job: &MeetingJob) -> &[TranscriptSegment] {
    if job.enable_speaker && !job.speaker_segments.is_empty() {
        &job.speaker_segments
    } else {
        &job.transcript_segments
    }
}

fn format_timestamp(ms: u64) -> String {
    let total_seconds = (ms / 1000) % 86_400;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

fn format_segment(
    segment: &TranscriptSegment,
    include_speaker: bool,
    include_timestamp: bool,
) -> String {
    let mut parts = Vec::new();
    if include_timestamp {
        parts.push(format!(
            "[{} - {}]",
            format_timestamp(segment.start_ms),
            format_timestamp(segment.end_ms)
        ));
    }
    if include_speaker {
        parts.push(format!(
            "{}:",
            segment
                .speaker
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("Unknown speaker")
        ));
    }
    parts.push(segment.text.trim().to_string());
    parts.join(" ")
}

fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}
