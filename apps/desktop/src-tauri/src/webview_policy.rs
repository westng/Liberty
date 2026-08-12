use tauri::{plugin::TauriPlugin, Runtime};

const WEBVIEW_POLICY_SCRIPT: &str = r#"
window.addEventListener("contextmenu", (event) => {
  event.preventDefault();
}, { capture: true });

window.addEventListener("keydown", (event) => {
  const key = event.key.toLowerCase();
  const isReloadShortcut = key === "f5" || ((event.metaKey || event.ctrlKey) && key === "r");
  if (isReloadShortcut) {
    event.preventDefault();
    event.stopImmediatePropagation();
  }
}, { capture: true });
"#;

pub(crate) fn init<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("liberty-webview-policy")
        .js_init_script_on_all_frames(WEBVIEW_POLICY_SCRIPT)
        .on_webview_ready(configure_native_webview)
        .build()
}

fn configure_native_webview<R: Runtime>(_webview: tauri::Webview<R>) {
    #[cfg(windows)]
    {
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
        use windows::core::Interface;

        let label = _webview.label().to_string();
        if let Err(error) = _webview.with_webview(move |platform_webview| {
            let result = (|| -> windows::core::Result<()> {
                let controller = platform_webview.controller();
                let webview = unsafe { controller.CoreWebView2()? };
                let settings = unsafe { webview.Settings()? };
                unsafe { settings.SetAreDefaultContextMenusEnabled(false)? };
                if let Ok(settings) = settings.cast::<ICoreWebView2Settings3>() {
                    unsafe { settings.SetAreBrowserAcceleratorKeysEnabled(false)? };
                }
                Ok(())
            })();
            if let Err(error) = result {
                eprintln!("[webview-policy] failed to configure native webview {label}: {error}");
            }
        }) {
            eprintln!(
                "[webview-policy] failed to access native webview {}: {error}",
                _webview.label()
            );
        }
    }
}
