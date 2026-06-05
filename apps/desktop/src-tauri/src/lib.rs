mod application;
mod commands;
mod desktop_pet;
mod domain;
mod infrastructure;
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
    let builder = tauri::Builder::default()
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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::diagnostics::export_desktop_pet_diagnostic_log,
            commands::diagnostics::get_diagnostics,
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
            local_runtime::detect_system_runtime,
            local_runtime::get_runtime_install_log,
            local_runtime::get_runtime_status,
            local_runtime::install_runtime,
            local_runtime::list_runtime_download_sources,
            local_pet::save_pet_settings,
            local_settings::get_settings,
            local_settings::save_settings,
            system::get_process_metrics,
            system::open_external_url,
            system::prompt_pet_name
        ]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
