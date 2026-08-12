mod application;
mod commands;
mod desktop_pet;
mod domain;
mod infrastructure;
mod local_ai;
mod local_db;
mod local_export;
mod local_farm;
mod local_jobs;
mod local_members;
mod local_pet;
mod local_remote;
mod local_runtime;
mod local_settings;
mod local_work_game;
mod process_utils;
mod single_instance;
mod system;
mod webview_policy;
mod window_scope;

#[cfg(target_os = "macos")]
use tauri::menu::{AboutMetadata, Menu, PredefinedMenuItem, Submenu, WINDOW_SUBMENU_ID};
#[cfg(windows)]
use tauri::Manager;

#[cfg(target_os = "macos")]
fn set_macos_menu(app: &tauri::App) -> tauri::Result<()> {
    let package_info = app.package_info();
    let config = app.config();
    let about_metadata = AboutMetadata {
        name: Some(package_info.name.clone()),
        version: Some(package_info.version.to_string()),
        copyright: config.bundle.copyright.clone(),
        authors: config
            .bundle
            .publisher
            .clone()
            .map(|publisher| vec![publisher]),
        ..Default::default()
    };

    let app_menu = Submenu::with_items(
        app,
        "Liberty",
        true,
        &[
            &PredefinedMenuItem::about(app, Some("关于 Liberty"), Some(about_metadata))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::services(app, Some("服务"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, Some("隐藏 Liberty"))?,
            &PredefinedMenuItem::hide_others(app, Some("隐藏其他"))?,
            &PredefinedMenuItem::show_all(app, Some("显示全部"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, Some("退出 Liberty"))?,
        ],
    )?;
    let edit_menu = Submenu::with_items(
        app,
        "编辑",
        true,
        &[
            &PredefinedMenuItem::undo(app, Some("撤销"))?,
            &PredefinedMenuItem::redo(app, Some("重做"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, Some("剪切"))?,
            &PredefinedMenuItem::copy(app, Some("拷贝"))?,
            &PredefinedMenuItem::paste(app, Some("粘贴"))?,
            &PredefinedMenuItem::select_all(app, Some("全选"))?,
        ],
    )?;
    let window_menu = Submenu::with_id_and_items(
        app,
        WINDOW_SUBMENU_ID,
        "窗口",
        true,
        &[
            &PredefinedMenuItem::minimize(app, Some("最小化"))?,
            &PredefinedMenuItem::maximize(app, Some("缩放"))?,
            &PredefinedMenuItem::fullscreen(app, Some("进入全屏"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::close_window(app, Some("关闭窗口"))?,
            &PredefinedMenuItem::bring_all_to_front(app, Some("全部置于前台"))?,
        ],
    )?;
    let menu = Menu::with_items(app, &[&app_menu, &edit_menu, &window_menu])?;
    app.set_menu(menu)?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = single_instance::builder()
        .plugin(webview_policy::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            set_macos_menu(app)?;

            #[cfg(windows)]
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_decorations(false);
            }
            desktop_pet::manage_desktop_pet_state(app.handle());
            desktop_pet::sync_desktop_pet_on_startup(app.handle());
            local_jobs::start_job_scheduler(app.handle()).map_err(std::io::Error::other)?;
            local_ai::resume_ai_summary_runs(app.handle()).map_err(std::io::Error::other)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::diagnostics::export_desktop_pet_diagnostic_log,
            commands::diagnostics::get_diagnostics,
            local_ai::delete_ai_model,
            local_ai::delete_ai_summary_run,
            local_ai::delete_ai_template,
            local_ai::get_ai_summary_options,
            local_ai::list_ai_models,
            local_ai::list_ai_summary_runs,
            local_ai::list_ai_templates,
            local_ai::save_ai_model,
            local_ai::save_ai_template,
            local_ai::start_or_resume_ai_summary_run,
            local_ai::set_active_ai_summary_run,
            local_export::export_job_text,
            local_export::export_job_summary_docx,
            local_farm::get_farm_state,
            local_farm::get_work_market_state,
            local_farm::harvest_farm_plot,
            local_farm::list_farm_harvest_ledger,
            local_farm::plant_farm_crop,
            local_farm::water_farm_plot,
            local_work_game::care_work_game_task,
            local_work_game::claim_work_game_task,
            local_work_game::get_work_game_state,
            local_work_game::start_work_game_task,
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
            local_pet::claim_pet_daily_check_in,
            local_pet::draw_pet_blind_box,
            local_pet::get_pet_daily_check_in_state,
            local_pet::get_pet_blind_box_state,
            local_pet::get_pet_profile,
            local_pet::get_pet_store_state,
            local_pet::get_pet_settings,
            desktop_pet::get_desktop_pet_status,
            desktop_pet::hide_desktop_pet,
            local_pet::list_pet_cosmetic_unlocks,
            local_pet::list_pet_event_ledger,
            local_pet::list_pet_redeem_key_redemptions,
            desktop_pet::open_extra_desktop_pet,
            local_pet::open_pet_gift_box,
            local_pet::purchase_pet_store_item,
            local_pet::repair_pet_daily_check_in,
            local_pet::redeem_pet_key,
            local_pet::save_pet_profile,
            desktop_pet::show_desktop_pet,
            desktop_pet::start_desktop_pet_drag,
            local_pet::equip_pet_inventory_item,
            local_pet::unequip_pet_inventory_slot,
            local_pet::use_pet_inventory_item,
            local_remote::get_remote_capabilities,
            local_remote::remote_delete_job,
            local_remote::remote_get_job,
            local_remote::remote_get_job_result,
            local_remote::remote_list_jobs,
            local_remote::remote_rename_job_speaker,
            local_remote::remote_retry_job,
            local_runtime::detect_runtime_component,
            local_runtime::get_runtime_component_log,
            local_runtime::get_runtime_install_log,
            local_runtime::get_runtime_status,
            local_runtime::install_runtime,
            local_runtime::install_runtime_component,
            local_runtime::list_runtime_download_sources,
            local_runtime::set_runtime_component_source,
            local_pet::save_pet_settings,
            local_settings::get_settings,
            local_settings::get_ui_preferences,
            local_settings::save_settings,
            system::get_process_metrics,
            system::open_external_url,
            system::prompt_pet_name,
            window_scope::close_current_window,
            window_scope::destroy_current_window,
            window_scope::issue_job_window_scope,
            window_scope::set_current_window_theme,
            window_scope::set_current_window_title
        ]);

    let app = builder
        .build(tauri::generate_context!())
        .expect("error while building tauri application");
    app.run(|_, event| {
        if matches!(
            event,
            tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
        ) {
            if let Err(error) = local_jobs::shutdown_job_scheduler() {
                eprintln!("failed to stop local job scheduler: {error}");
            }
        }
    });
}
