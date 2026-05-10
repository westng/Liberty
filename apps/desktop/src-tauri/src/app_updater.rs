use std::sync::Mutex;

use chrono::Utc;
use semver::Version;
use serde::Serialize;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem, HELP_SUBMENU_ID},
    AppHandle, Emitter, Manager, Runtime,
};

const MENU_CHECK_UPDATES_ID: &str = "liberty.check-for-updates";
pub const MENU_CHECK_UPDATES_EVENT: &str = "liberty://menu-check-updates";
pub const UPDATE_STATUS_EVENT: &str = "liberty://update-status";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateStatus {
    pub status: String,
    pub platform: String,
    pub channel: String,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub last_checked_at: Option<String>,
    pub release_notes: Option<String>,
    pub pub_date: Option<String>,
    pub message: Option<String>,
    pub download_percent: Option<u8>,
    pub feed_url: String,
    pub can_auto_install: bool,
}

impl AppUpdateStatus {
    fn idle(current_version: String) -> Self {
        Self {
            status: if is_update_supported() {
                "idle".into()
            } else {
                "unsupported".into()
            },
            platform: current_platform().into(),
            channel: current_channel().into(),
            current_version,
            latest_version: None,
            last_checked_at: None,
            release_notes: None,
            pub_date: None,
            message: None,
            download_percent: None,
            feed_url: current_feed_url(),
            can_auto_install: cfg!(target_os = "windows"),
        }
    }
}

pub struct UpdateState(pub Mutex<AppUpdateStatus>);

impl UpdateState {
    pub fn new(current_version: String) -> Self {
        Self(Mutex::new(AppUpdateStatus::idle(current_version)))
    }
}

#[derive(Debug, Clone)]
struct AppcastItem {
    version: String,
    pub_date: Option<String>,
    release_notes: Option<String>,
}

#[tauri::command]
pub fn get_update_status(app: AppHandle) -> Result<AppUpdateStatus, String> {
    Ok(app
        .state::<UpdateState>()
        .0
        .lock()
        .map_err(|err| err.to_string())?
        .clone())
}

#[tauri::command]
pub async fn check_for_updates(
    app: AppHandle,
    interactive: Option<bool>,
) -> Result<AppUpdateStatus, String> {
    perform_update_check(app, interactive.unwrap_or(true)).await
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<AppUpdateStatus, String> {
    #[cfg(target_os = "windows")]
    {
        install_windows_update(app).await
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        Err("当前平台不支持从设置页直接安装更新。".into())
    }
}

#[tauri::command]
pub fn restart_after_update(app: AppHandle) -> Result<(), String> {
    app.request_restart();
    Ok(())
}

pub fn manage_update_state(app: &AppHandle) {
    app.manage(UpdateState::new(app.package_info().version.to_string()));
}

pub fn configure_app_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let menu = Menu::default(app)?;
    let Some(help_menu) = menu
        .get(HELP_SUBMENU_ID)
        .and_then(|item| item.as_submenu().cloned())
    else {
        menu.set_as_app_menu()?;
        return Ok(());
    };

    let check_item = MenuItem::with_id(
        app,
        MENU_CHECK_UPDATES_ID,
        check_updates_menu_label(app),
        true,
        Some("CmdOrCtrl+Shift+U"),
    )?;

    if !help_menu.items()?.is_empty() {
        help_menu.prepend(&PredefinedMenuItem::separator(app)?)?;
    }
    help_menu.prepend(&check_item)?;
    menu.set_as_app_menu()?;
    Ok(())
}

pub fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: tauri::menu::MenuEvent) {
    if event.id().as_ref() == MENU_CHECK_UPDATES_ID {
        let _ = app.emit(MENU_CHECK_UPDATES_EVENT, ());
    }
}

pub fn start_background_update_check(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let _ = perform_update_check(app, false).await;
    });
}

async fn perform_update_check(app: AppHandle, interactive: bool) -> Result<AppUpdateStatus, String> {
    let current_version = app.package_info().version.to_string();
    let checking = AppUpdateStatus {
        status: "checking".into(),
        platform: current_platform().into(),
        channel: current_channel().into(),
        current_version: current_version.clone(),
        latest_version: None,
        last_checked_at: Some(Utc::now().to_rfc3339()),
        release_notes: None,
        pub_date: None,
        message: None,
        download_percent: None,
        feed_url: current_feed_url(),
        can_auto_install: cfg!(target_os = "windows"),
    };
    set_update_state(&app, checking.clone());

    #[cfg(target_os = "windows")]
    {
        return check_windows_for_updates(app, interactive).await;
    }

    #[cfg(target_os = "macos")]
    {
        return check_macos_for_updates(app, interactive).await;
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let unsupported = AppUpdateStatus {
            status: "unsupported".into(),
            platform: current_platform().into(),
            channel: current_channel().into(),
            current_version,
            latest_version: None,
            last_checked_at: Some(Utc::now().to_rfc3339()),
            release_notes: None,
            pub_date: None,
            message: Some("当前平台暂不支持应用内更新。".into()),
            download_percent: None,
            feed_url: current_feed_url(),
            can_auto_install: false,
        };
        set_update_state(&app, unsupported.clone());
        return Ok(unsupported);
    }
}

#[cfg(target_os = "windows")]
async fn check_windows_for_updates(
    app: AppHandle,
    _interactive: bool,
) -> Result<AppUpdateStatus, String> {
    use tauri_plugin_updater::UpdaterExt;

    let updater = app.updater().map_err(|err| err.to_string())?;
    let checked_at = Utc::now().to_rfc3339();

    match updater.check().await.map_err(|err| err.to_string())? {
        Some(update) => {
            let status = AppUpdateStatus {
                status: "updateAvailable".into(),
                platform: current_platform().into(),
                channel: current_channel().into(),
                current_version: app.package_info().version.to_string(),
                latest_version: Some(update.version),
                last_checked_at: Some(checked_at),
                release_notes: update.body,
                pub_date: update.date.map(|date| date.to_string()),
                message: Some("发现新版本，可以直接下载安装。".into()),
                download_percent: None,
                feed_url: current_feed_url(),
                can_auto_install: true,
            };
            set_update_state(&app, status.clone());
            Ok(status)
        }
        None => {
            let status = AppUpdateStatus {
                status: "upToDate".into(),
                platform: current_platform().into(),
                channel: current_channel().into(),
                current_version: app.package_info().version.to_string(),
                latest_version: None,
                last_checked_at: Some(checked_at),
                release_notes: None,
                pub_date: None,
                message: Some("当前已经是最新版本。".into()),
                download_percent: None,
                feed_url: current_feed_url(),
                can_auto_install: true,
            };
            set_update_state(&app, status.clone());
            Ok(status)
        }
    }
}

#[cfg(target_os = "windows")]
async fn install_windows_update(app: AppHandle) -> Result<AppUpdateStatus, String> {
    use tauri_plugin_updater::UpdaterExt;

    let updater = app.updater().map_err(|err| err.to_string())?;
    let Some(update) = updater.check().await.map_err(|err| err.to_string())? else {
        let status = AppUpdateStatus {
            status: "upToDate".into(),
            platform: current_platform().into(),
            channel: current_channel().into(),
            current_version: app.package_info().version.to_string(),
            latest_version: None,
            last_checked_at: Some(Utc::now().to_rfc3339()),
            release_notes: None,
            pub_date: None,
            message: Some("当前已经是最新版本。".into()),
            download_percent: None,
            feed_url: current_feed_url(),
            can_auto_install: true,
        };
        set_update_state(&app, status.clone());
        return Ok(status);
    };

    let latest_version = update.version.clone();
    let release_notes = update.body.clone();
    let pub_date = update.date.map(|date| date.to_string());

    let downloading = AppUpdateStatus {
        status: "downloading".into(),
        platform: current_platform().into(),
        channel: current_channel().into(),
        current_version: app.package_info().version.to_string(),
        latest_version: Some(latest_version.clone()),
        last_checked_at: Some(Utc::now().to_rfc3339()),
        release_notes: release_notes.clone(),
        pub_date: pub_date.clone(),
        message: Some("正在下载更新包。".into()),
        download_percent: Some(0),
        feed_url: current_feed_url(),
        can_auto_install: true,
    };
    set_update_state(&app, downloading);

    let progress_app = app.clone();
    let install_app = app.clone();
    let latest_version_for_progress = latest_version.clone();
    let latest_version_for_install = latest_version.clone();
    let latest_version_for_error = latest_version.clone();
    let release_notes_for_progress = release_notes.clone();
    let release_notes_for_install = release_notes.clone();
    let release_notes_for_error = release_notes.clone();
    let pub_date_for_progress = pub_date.clone();
    let pub_date_for_install = pub_date.clone();
    let pub_date_for_error = pub_date.clone();
    update
        .download_and_install(
            move |downloaded, total| {
                let percent = total
                    .and_then(|size| {
                        if size == 0 {
                            None
                        } else {
                            Some(((downloaded as f64 / size as f64) * 100.0).round() as u8)
                        }
                    })
                    .unwrap_or(0)
                    .min(100);

                let status = AppUpdateStatus {
                    status: "downloading".into(),
                    platform: current_platform().into(),
                    channel: current_channel().into(),
                    current_version: progress_app.package_info().version.to_string(),
                    latest_version: Some(latest_version_for_progress.clone()),
                    last_checked_at: Some(Utc::now().to_rfc3339()),
                    release_notes: release_notes_for_progress.clone(),
                    pub_date: pub_date_for_progress.clone(),
                    message: Some("正在下载更新包。".into()),
                    download_percent: Some(percent),
                    feed_url: current_feed_url(),
                    can_auto_install: true,
                };
                set_update_state(&progress_app, status);
            },
            move || {
                let status = AppUpdateStatus {
                    status: "installing".into(),
                    platform: current_platform().into(),
                    channel: current_channel().into(),
                    current_version: install_app.package_info().version.to_string(),
                    latest_version: Some(latest_version_for_install.clone()),
                    last_checked_at: Some(Utc::now().to_rfc3339()),
                    release_notes: release_notes_for_install.clone(),
                    pub_date: pub_date_for_install.clone(),
                    message: Some("更新包已下载，正在安装。".into()),
                    download_percent: Some(100),
                    feed_url: current_feed_url(),
                    can_auto_install: true,
                };
                set_update_state(&install_app, status);
            },
        )
        .await
        .map_err(|err| {
            let status = AppUpdateStatus {
                status: "error".into(),
                platform: current_platform().into(),
                channel: current_channel().into(),
                current_version: app.package_info().version.to_string(),
                latest_version: Some(latest_version_for_error.clone()),
                last_checked_at: Some(Utc::now().to_rfc3339()),
                release_notes: release_notes_for_error.clone(),
                pub_date: pub_date_for_error.clone(),
                message: Some(err.to_string()),
                download_percent: None,
                feed_url: current_feed_url(),
                can_auto_install: true,
            };
            set_update_state(&app, status);
            err.to_string()
        })?;

    let status = AppUpdateStatus {
        status: "restartRequired".into(),
        platform: current_platform().into(),
        channel: current_channel().into(),
        current_version: app.package_info().version.to_string(),
        latest_version: Some(latest_version),
        last_checked_at: Some(Utc::now().to_rfc3339()),
        release_notes,
        pub_date,
        message: Some("更新已安装，重启应用后生效。".into()),
        download_percent: Some(100),
        feed_url: current_feed_url(),
        can_auto_install: true,
    };
    set_update_state(&app, status.clone());
    Ok(status)
}

#[cfg(target_os = "macos")]
async fn check_macos_for_updates(app: AppHandle, interactive: bool) -> Result<AppUpdateStatus, String> {
    use tauri_plugin_sparkle_updater::SparkleUpdaterExt;

    let Some(sparkle) = app.sparkle_updater() else {
        let status = AppUpdateStatus {
            status: "unsupported".into(),
            platform: current_platform().into(),
            channel: current_channel().into(),
            current_version: app.package_info().version.to_string(),
            latest_version: None,
            last_checked_at: Some(Utc::now().to_rfc3339()),
            release_notes: None,
            pub_date: None,
            message: Some("当前不在有效的 macOS bundle 环境中，Sparkle 无法启动。".into()),
            download_percent: None,
            feed_url: current_feed_url(),
            can_auto_install: false,
        };
        set_update_state(&app, status.clone());
        return Ok(status);
    };

    if interactive {
        sparkle.check_for_updates().map_err(|err| err.to_string())?;
        let status = AppUpdateStatus {
            status: "checking".into(),
            platform: current_platform().into(),
            channel: current_channel().into(),
            current_version: app.package_info().version.to_string(),
            latest_version: None,
            last_checked_at: Some(Utc::now().to_rfc3339()),
            release_notes: None,
            pub_date: None,
            message: Some("系统更新窗口已打开，Sparkle 正在检查更新。".into()),
            download_percent: None,
            feed_url: current_feed_url(),
            can_auto_install: false,
        };
        set_update_state(&app, status.clone());
        return Ok(status);
    }

    sparkle
        .check_for_updates_in_background()
        .map_err(|err| err.to_string())?;

    let appcast = load_latest_appcast_item().await.map_err(|err| {
        let status = AppUpdateStatus {
            status: "error".into(),
            platform: current_platform().into(),
            channel: current_channel().into(),
            current_version: app.package_info().version.to_string(),
            latest_version: None,
            last_checked_at: Some(Utc::now().to_rfc3339()),
            release_notes: None,
            pub_date: None,
            message: Some(err.clone()),
            download_percent: None,
            feed_url: current_feed_url(),
            can_auto_install: false,
        };
        set_update_state(&app, status);
        err
    })?;

    let current_version = parse_version(&app.package_info().version.to_string())?;
    let latest_version = parse_version(&appcast.version)?;

    let status = if latest_version > current_version {
        AppUpdateStatus {
            status: "updateAvailable".into(),
            platform: current_platform().into(),
            channel: current_channel().into(),
            current_version: app.package_info().version.to_string(),
            latest_version: Some(appcast.version),
            last_checked_at: Some(Utc::now().to_rfc3339()),
            release_notes: appcast.release_notes,
            pub_date: appcast.pub_date,
            message: Some("发现新版本，Sparkle 会接管下载和安装流程。".into()),
            download_percent: None,
            feed_url: current_feed_url(),
            can_auto_install: false,
        }
    } else {
        AppUpdateStatus {
            status: "upToDate".into(),
            platform: current_platform().into(),
            channel: current_channel().into(),
            current_version: app.package_info().version.to_string(),
            latest_version: None,
            last_checked_at: Some(Utc::now().to_rfc3339()),
            release_notes: None,
            pub_date: appcast.pub_date,
            message: Some("当前已经是最新版本。".into()),
            download_percent: None,
            feed_url: current_feed_url(),
            can_auto_install: false,
        }
    };

    set_update_state(&app, status.clone());
    Ok(status)
}

#[cfg(target_os = "macos")]
async fn load_latest_appcast_item() -> Result<AppcastItem, String> {
    let response = reqwest::get(macos_appcast_url())
        .await
        .map_err(|err| err.to_string())?;
    let xml = response.text().await.map_err(|err| err.to_string())?;
    parse_appcast(&xml)
}

#[cfg(target_os = "macos")]
fn parse_appcast(xml: &str) -> Result<AppcastItem, String> {
    let root = xmltree::Element::parse(xml.as_bytes()).map_err(|err| err.to_string())?;
    let channel = find_child(&root, "channel").ok_or_else(|| "appcast 缺少 channel 节点。".to_string())?;
    let items = channel
        .children
        .iter()
        .filter_map(|node| node.as_element())
        .filter(|element| element.name == "item")
        .collect::<Vec<_>>();

    let current_arch = if cfg!(target_arch = "aarch64") {
        ["aarch64", "arm64"]
    } else {
        ["x86_64", "x64"]
    };

    let mut selected: Option<(Version, AppcastItem)> = None;

    for item in &items {
        let enclosure = find_child(item, "enclosure").ok_or_else(|| "appcast item 缺少 enclosure 节点。".to_string())?;
        let url = enclosure
            .attributes
            .get("url")
            .cloned()
            .unwrap_or_default()
            .to_lowercase();

        if !current_arch.iter().any(|marker| url.contains(marker)) && items.len() > 1 {
            continue;
        }

        let version_text = enclosure
            .attributes
            .get("sparkle:shortVersionString")
            .or_else(|| enclosure.attributes.get("sparkle:version"))
            .cloned()
            .ok_or_else(|| "appcast enclosure 缺少 sparkle 版本字段。".to_string())?;
        let parsed = parse_version(&version_text)?;
        let release_notes = child_text(item, "description");
        let pub_date = child_text(item, "pubDate");
        let candidate = AppcastItem {
            version: version_text,
            pub_date,
            release_notes,
        };

        match &selected {
            Some((current, _)) if parsed <= *current => {}
            _ => selected = Some((parsed, candidate)),
        }
    }

    selected
        .map(|(_, item)| item)
        .ok_or_else(|| "没有在 appcast.xml 中找到当前架构可用的更新条目。".to_string())
}

#[cfg(target_os = "macos")]
fn find_child<'a>(element: &'a xmltree::Element, name: &str) -> Option<&'a xmltree::Element> {
    element
        .children
        .iter()
        .filter_map(|node| node.as_element())
        .find(|child| child.name == name)
}

#[cfg(target_os = "macos")]
fn child_text(element: &xmltree::Element, name: &str) -> Option<String> {
    find_child(element, name)
        .and_then(|child| child.get_text())
        .map(|text| text.into_owned().trim().to_string())
        .filter(|text| !text.is_empty())
}

fn set_update_state<R: Runtime>(app: &AppHandle<R>, next: AppUpdateStatus) {
    if let Ok(mut state) = app.state::<UpdateState>().0.lock() {
        *state = next.clone();
    }
    let _ = app.emit(UPDATE_STATUS_EVENT, next);
}

fn parse_version(input: &str) -> Result<Version, String> {
    Version::parse(input.trim().trim_start_matches('v')).map_err(|err| err.to_string())
}

fn check_updates_menu_label<R: Runtime>(_app: &AppHandle<R>) -> &'static str {
    "Check for Updates"
}

fn current_feed_url() -> String {
    #[cfg(target_os = "macos")]
    {
        macos_appcast_url()
    }

    #[cfg(target_os = "windows")]
    {
        windows_manifest_url()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        String::new()
    }
}

fn current_channel() -> &'static str {
    "GitHub Releases"
}

fn current_platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "unsupported"
    }
}

fn is_update_supported() -> bool {
    cfg!(target_os = "macos") || cfg!(target_os = "windows")
}

fn updater_repository() -> &'static str {
    option_env!("LIBERTY_UPDATER_REPOSITORY").unwrap_or("westng/Liberty")
}

fn macos_appcast_url() -> String {
    format!(
        "https://github.com/{}/releases/latest/download/appcast.xml",
        updater_repository()
    )
}

#[cfg(target_os = "windows")]
fn windows_manifest_url() -> String {
    format!(
        "https://github.com/{}/releases/latest/download/latest.json",
        updater_repository()
    )
}
