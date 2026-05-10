mod app_updater;
mod desktop_pet;
mod local_ai;
mod local_db;
mod local_export;
mod local_jobs;
mod local_members;
mod local_pet;
mod local_runtime;
mod local_settings;
mod process_utils;
mod system;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            app_updater::manage_update_state(&app.handle());
            desktop_pet::manage_desktop_pet_state(&app.handle());
            desktop_pet::sync_desktop_pet_on_startup(&app.handle());
            app_updater::configure_app_menu(&app.handle())?;
            app_updater::start_background_update_check(app.handle().clone());
            Ok(())
        })
        .on_menu_event(app_updater::handle_menu_event)
        .invoke_handler(tauri::generate_handler![
            app_updater::check_for_updates,
            app_updater::get_update_status,
            app_updater::install_update,
            app_updater::restart_after_update,
            local_ai::delete_ai_model,
            local_ai::delete_ai_summary_run,
            local_ai::delete_ai_template,
            local_ai::generate_ai_summary,
            local_ai::list_ai_models,
            local_ai::list_ai_summary_runs,
            local_ai::list_ai_templates,
            local_ai::request_ai_chat_completion,
            local_ai::save_ai_model,
            local_ai::save_ai_summary_run,
            local_ai::save_ai_template,
            local_ai::set_active_ai_summary_run,
            local_export::export_job_summary_docx,
            local_jobs::create_job,
            local_jobs::delete_job,
            local_jobs::get_job,
            local_jobs::get_job_result,
            local_jobs::list_jobs,
            local_jobs::rename_job_speaker,
            local_jobs::retry_job,
            local_members::delete_meeting_member,
            local_members::export_meeting_members_excel,
            local_members::import_meeting_members_excel,
            local_members::list_meeting_members,
            local_members::save_meeting_member,
            local_pet::apply_pet_interaction,
            local_pet::apply_pet_workflow_event,
            local_pet::get_pet_profile,
            local_pet::get_pet_settings,
            desktop_pet::get_desktop_pet_status,
            desktop_pet::hide_desktop_pet,
            local_pet::list_pet_cosmetic_unlocks,
            local_pet::list_pet_event_ledger,
            desktop_pet::open_extra_desktop_pet,
            local_pet::save_pet_profile,
            desktop_pet::show_desktop_pet,
            desktop_pet::start_desktop_pet_drag,
            local_runtime::get_runtime_install_log,
            local_runtime::get_runtime_status,
            local_runtime::install_runtime,
            local_pet::save_pet_settings,
            local_settings::get_settings,
            local_settings::save_settings,
            system::get_process_metrics,
            system::open_external_url
        ]);

    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_plugin_sparkle_updater::init());

    #[cfg(target_os = "windows")]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
