use std::process::Command;

type LocalResult<T> = Result<T, String>;

#[tauri::command]
pub fn open_external_url(url: String) -> LocalResult<()> {
    let normalized = url.trim();
    if normalized.is_empty() {
        return Err("URL 不能为空。".into());
    }

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut cmd = Command::new("open");
        cmd.arg(normalized);
        cmd
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", "start", "", normalized]);
        cmd
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut cmd = Command::new("xdg-open");
        cmd.arg(normalized);
        cmd
    };

    command.spawn().map_err(|err| err.to_string())?;
    Ok(())
}
