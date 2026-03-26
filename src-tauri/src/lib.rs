mod local_ai;
mod local_db;
mod local_jobs;
mod local_settings;
mod system;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            local_ai::delete_ai_model,
            local_ai::delete_ai_summary_run,
            local_ai::delete_ai_template,
            local_ai::list_ai_models,
            local_ai::list_ai_summary_runs,
            local_ai::list_ai_templates,
            local_ai::request_ai_chat_completion,
            local_ai::save_ai_model,
            local_ai::save_ai_summary_run,
            local_ai::save_ai_template,
            local_ai::set_active_ai_summary_run,
            local_jobs::create_job,
            local_jobs::delete_job,
            local_jobs::get_job,
            local_jobs::get_job_result,
            local_jobs::list_jobs,
            local_jobs::rename_job_speaker,
            local_jobs::retry_job,
            local_settings::get_settings,
            local_settings::save_settings,
            system::open_external_url
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
