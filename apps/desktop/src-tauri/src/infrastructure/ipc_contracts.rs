use typify::import_types;

mod meeting_job_v1 {
    use super::*;
    import_types!(
        schema = "../../../packages/shared-types/schemas/meeting-job/v1/meeting-job.schema.json"
    );
}

mod settings_v1 {
    use super::*;
    import_types!(
        schema = "../../../packages/shared-types/schemas/settings/v1/settings.schema.json"
    );
}

mod ai_v1 {
    use super::*;
    import_types!(schema = "../../../packages/shared-types/schemas/ai/v1/ai.schema.json");
}

mod runtime_v1 {
    use super::*;
    import_types!(schema = "../../../packages/shared-types/schemas/runtime/v1/runtime.schema.json");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_ipc_fixtures_parse_with_generated_rust_types() {
        serde_json::from_str::<meeting_job_v1::MeetingJobV1>(include_str!(
            "../../../../../packages/shared-types/fixtures/meeting-job/v1/current.json"
        ))
        .expect("meeting job fixture");
        serde_json::from_str::<settings_v1::SettingsSnapshotV1>(include_str!(
            "../../../../../packages/shared-types/fixtures/settings/v1/current.json"
        ))
        .expect("settings fixture");
        serde_json::from_str::<ai_v1::AiIpcContractV1>(include_str!(
            "../../../../../packages/shared-types/fixtures/ai/v1/current.json"
        ))
        .expect("AI fixture");
        serde_json::from_str::<runtime_v1::ManagedRuntimeStateV1>(include_str!(
            "../../../../../packages/shared-types/fixtures/runtime/v1/current.json"
        ))
        .expect("runtime fixture");
    }
}
