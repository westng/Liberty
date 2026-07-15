#[cfg(not(any(target_os = "android", target_os = "ios")))]
use tauri::Manager;

const MAIN_WINDOW_LABEL: &str = "main";

pub(crate) fn builder() -> tauri::Builder<tauri::Wry> {
    let builder = tauri::Builder::default();

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
    }));

    builder
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::MAIN_WINDOW_LABEL;

    #[test]
    fn single_instance_registration_precedes_other_startup_hooks() {
        let startup_source = include_str!("lib.rs");
        let single_instance = startup_source
            .find("single_instance::builder()")
            .expect("run must start from the single-instance builder");
        let dialog = startup_source
            .find(".plugin(tauri_plugin_dialog::init())")
            .expect("dialog plugin must be registered");
        let file_system = startup_source
            .find(".plugin(tauri_plugin_fs::init())")
            .expect("file-system plugin must be registered");
        let setup = startup_source
            .find(".setup(")
            .expect("application setup must be registered");

        assert!(
            single_instance < dialog && single_instance < file_system && single_instance < setup
        );
        assert!(
            !startup_source.contains("tauri::Builder::default()"),
            "run must not bypass the single-instance builder"
        );
    }

    #[test]
    fn main_window_is_declared_in_tauri_config() {
        let config: Value = serde_json::from_str(include_str!("../tauri.conf.json"))
            .expect("tauri.conf.json must contain valid JSON");
        let windows = config
            .pointer("/app/windows")
            .and_then(Value::as_array)
            .expect("tauri.conf.json must declare app.windows");

        assert!(windows.iter().any(|window| {
            window.get("label").and_then(Value::as_str) == Some(MAIN_WINDOW_LABEL)
        }));
    }
}
