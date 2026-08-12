use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const LEGACY_FIXED_METADATA_SHA256: &str =
    "87108e7392f309f74b2ddf92210cd982639c82bfe553467b54642c1dbb24a073";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeetingInfoSource {
    User,
    Ai,
    #[default]
    Empty,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MeetingMetadata {
    pub meeting_name: String,
    pub meeting_time: String,
    pub meeting_location: String,
    pub recorder: String,
    pub attendees: String,
    pub absentees: String,
    pub host: String,
    pub reviewer: String,
}

impl MeetingMetadata {
    pub fn is_empty(&self) -> bool {
        [
            &self.meeting_name,
            &self.meeting_time,
            &self.meeting_location,
            &self.recorder,
            &self.attendees,
            &self.absentees,
            &self.host,
            &self.reviewer,
        ]
        .iter()
        .all(|value| value.trim().is_empty())
    }

    pub fn trim(mut self) -> Self {
        self.meeting_name = normalize_metadata_value(&self.meeting_name);
        self.meeting_time = normalize_metadata_value(&self.meeting_time);
        self.meeting_location = normalize_metadata_value(&self.meeting_location);
        self.recorder = normalize_metadata_value(&self.recorder);
        self.attendees = normalize_metadata_value(&self.attendees);
        self.absentees = normalize_metadata_value(&self.absentees);
        self.host = normalize_metadata_value(&self.host);
        self.reviewer = normalize_metadata_value(&self.reviewer);
        self
    }
}

fn normalize_metadata_value(value: &str) -> String {
    match value.trim() {
        "待补充" | "待补充部门" | "待补充姓名" => String::new(),
        value => value.to_string(),
    }
}

#[derive(Debug, Clone)]
pub struct PersistedMeetingMetadata {
    pub schema_version: u32,
    pub source: MeetingInfoSource,
    pub metadata: MeetingMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeetingMetadataProjection {
    pub metadata: MeetingMetadata,
    pub source: MeetingInfoSource,
    pub warnings: Vec<String>,
}

pub fn project_meeting_metadata(
    persisted: Option<PersistedMeetingMetadata>,
    ai_metadata: MeetingMetadata,
) -> MeetingMetadataProjection {
    let mut warnings = Vec::new();
    let legacy_fallback = if let Some(mut persisted) = persisted {
        if persisted.schema_version == 2 && persisted.source == MeetingInfoSource::User {
            return MeetingMetadataProjection {
                metadata: persisted.metadata.trim(),
                source: MeetingInfoSource::User,
                warnings,
            };
        }
        if persisted.schema_version == 1 && is_legacy_fixed_metadata(&persisted.metadata) {
            warnings.push("legacy_fixed_meeting_metadata_ignored".into());
            persisted.metadata.meeting_time.clear();
            persisted.metadata.meeting_location.clear();
            persisted.metadata.host.clear();
        }
        (persisted.schema_version == 1).then_some(persisted.metadata.trim())
    } else {
        None
    };

    let ai_metadata = ai_metadata.trim();
    if !ai_metadata.is_empty() {
        MeetingMetadataProjection {
            metadata: ai_metadata,
            source: MeetingInfoSource::Ai,
            warnings,
        }
    } else if let Some(legacy_fallback) = legacy_fallback.filter(|metadata| !metadata.is_empty()) {
        MeetingMetadataProjection {
            metadata: legacy_fallback,
            source: MeetingInfoSource::Ai,
            warnings,
        }
    } else {
        MeetingMetadataProjection {
            metadata: MeetingMetadata::default(),
            source: MeetingInfoSource::Empty,
            warnings,
        }
    }
}

fn is_legacy_fixed_metadata(metadata: &MeetingMetadata) -> bool {
    let value = format!(
        "{}\0{}\0{}",
        metadata.meeting_time.trim(),
        metadata.meeting_location.trim(),
        metadata.host.trim()
    );
    format!("{:x}", Sha256::digest(value.as_bytes())) == LEGACY_FIXED_METADATA_SHA256
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_metadata() -> MeetingMetadata {
        MeetingMetadata {
            meeting_time: "9:00".into(),
            meeting_location: "小会议室".into(),
            host: "冯吉琼".into(),
            ..MeetingMetadata::default()
        }
    }

    #[test]
    fn user_v2_metadata_is_authoritative() {
        let projection = project_meeting_metadata(
            Some(PersistedMeetingMetadata {
                schema_version: 2,
                source: MeetingInfoSource::User,
                metadata: MeetingMetadata {
                    meeting_time: " 10:00 ".into(),
                    ..MeetingMetadata::default()
                },
            }),
            MeetingMetadata {
                meeting_time: "11:00".into(),
                ..MeetingMetadata::default()
            },
        );

        assert_eq!(projection.source, MeetingInfoSource::User);
        assert_eq!(projection.metadata.meeting_time, "10:00");
    }

    #[test]
    fn ai_metadata_fills_partial_fields_without_placeholders() {
        let projection = project_meeting_metadata(
            None,
            MeetingMetadata {
                meeting_location: "线上".into(),
                ..MeetingMetadata::default()
            },
        );

        assert_eq!(projection.source, MeetingInfoSource::Ai);
        assert_eq!(projection.metadata.meeting_location, "线上");
        assert!(projection.metadata.host.is_empty());
    }

    #[test]
    fn empty_ai_metadata_stays_empty() {
        let projection = project_meeting_metadata(None, MeetingMetadata::default());
        assert_eq!(projection.source, MeetingInfoSource::Empty);
        assert!(projection.metadata.is_empty());
    }

    #[test]
    fn renderer_placeholders_are_not_persisted_as_metadata() {
        let projection = project_meeting_metadata(
            None,
            MeetingMetadata {
                meeting_time: " 待补充 ".into(),
                recorder: "待补充姓名".into(),
                ..MeetingMetadata::default()
            },
        );

        assert_eq!(projection.source, MeetingInfoSource::Empty);
        assert!(projection.metadata.is_empty());
    }

    #[test]
    fn legacy_fixed_triple_is_ignored_but_single_match_is_not_special() {
        let legacy = project_meeting_metadata(
            Some(PersistedMeetingMetadata {
                schema_version: 1,
                source: MeetingInfoSource::Empty,
                metadata: legacy_metadata(),
            }),
            MeetingMetadata::default(),
        );
        assert_eq!(
            legacy.warnings,
            vec!["legacy_fixed_meeting_metadata_ignored"]
        );
        assert!(legacy.metadata.is_empty());

        let single = project_meeting_metadata(
            Some(PersistedMeetingMetadata {
                schema_version: 1,
                source: MeetingInfoSource::Empty,
                metadata: MeetingMetadata {
                    meeting_time: legacy_metadata().meeting_time,
                    ..MeetingMetadata::default()
                },
            }),
            MeetingMetadata::default(),
        );
        assert!(single.warnings.is_empty());
        assert_eq!(single.metadata.meeting_time, "9:00");
    }
}
