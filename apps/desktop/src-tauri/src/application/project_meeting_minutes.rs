use crate::domain::meeting_minutes::{
    project_meeting_metadata, MeetingMetadata, MeetingMetadataProjection, PersistedMeetingMetadata,
};

pub struct ProjectMeetingMinutesRequest {
    pub persisted: Option<PersistedMeetingMetadata>,
    pub ai_metadata: MeetingMetadata,
}

pub fn project_meeting_minutes(request: ProjectMeetingMinutesRequest) -> MeetingMetadataProjection {
    project_meeting_metadata(request.persisted, request.ai_metadata)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::meeting_minutes::MeetingInfoSource;

    #[test]
    fn projects_ai_metadata_without_runtime_dependencies() {
        let result = project_meeting_minutes(ProjectMeetingMinutesRequest {
            persisted: None,
            ai_metadata: MeetingMetadata {
                host: "主持人".into(),
                ..MeetingMetadata::default()
            },
        });

        assert_eq!(result.source, MeetingInfoSource::Ai);
        assert_eq!(result.metadata.host, "主持人");
    }
}
