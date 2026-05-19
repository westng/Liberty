#[cfg(target_os = "macos")]
use std::process::Command;

use crate::domain::error::AppError;

pub type CredentialResult<T> = Result<T, AppError>;

const SERVICE_NAME: &str = "LibertyDesktop";

pub trait CredentialStore {
    fn get_secret(&self, key: &str) -> CredentialResult<Option<String>>;
    fn set_secret(&self, key: &str, value: &str) -> CredentialResult<()>;
    fn delete_secret(&self, key: &str) -> CredentialResult<()>;
}

#[derive(Debug, Default)]
pub struct SystemCredentialStore;

impl CredentialStore for SystemCredentialStore {
    fn get_secret(&self, key: &str) -> CredentialResult<Option<String>> {
        get_system_secret(key)
    }

    fn set_secret(&self, key: &str, value: &str) -> CredentialResult<()> {
        set_system_secret(key, value)
    }

    fn delete_secret(&self, key: &str) -> CredentialResult<()> {
        delete_system_secret(key)
    }
}

pub fn default_credential_store() -> SystemCredentialStore {
    SystemCredentialStore
}

pub fn credential_key_for_ai_model(model_id: &str) -> String {
    format!("ai-model:{model_id}:api-key")
}

#[cfg(target_os = "macos")]
fn get_system_secret(key: &str) -> CredentialResult<Option<String>> {
    let output = Command::new("security")
        .args(["find-generic-password", "-s", SERVICE_NAME, "-a", key, "-w"])
        .output()
        .map_err(|err| AppError::Infrastructure(format!("读取 macOS Keychain 失败: {err}")))?;

    if output.status.success() {
        return Ok(Some(
            String::from_utf8_lossy(&output.stdout)
                .trim_end_matches(['\r', '\n'])
                .to_string(),
        ));
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("could not be found") || stderr.contains("-25300") {
        return Ok(None);
    }

    Err(AppError::Infrastructure(format!(
        "读取 macOS Keychain 失败: {}",
        stderr.trim()
    )))
}

#[cfg(target_os = "macos")]
fn set_system_secret(key: &str, value: &str) -> CredentialResult<()> {
    let status = Command::new("security")
        .args([
            "add-generic-password",
            "-U",
            "-s",
            SERVICE_NAME,
            "-a",
            key,
            "-w",
            value,
        ])
        .status()
        .map_err(|err| AppError::Infrastructure(format!("写入 macOS Keychain 失败: {err}")))?;

    if status.success() {
        Ok(())
    } else {
        Err(AppError::Infrastructure(
            "写入 macOS Keychain 失败，请检查钥匙串权限。".into(),
        ))
    }
}

#[cfg(target_os = "macos")]
fn delete_system_secret(key: &str) -> CredentialResult<()> {
    let output = Command::new("security")
        .args(["delete-generic-password", "-s", SERVICE_NAME, "-a", key])
        .output()
        .map_err(|err| AppError::Infrastructure(format!("删除 macOS Keychain 凭据失败: {err}")))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("could not be found") || stderr.contains("-25300") {
        return Ok(());
    }

    Err(AppError::Infrastructure(format!(
        "删除 macOS Keychain 凭据失败: {}",
        stderr.trim()
    )))
}

#[cfg(windows)]
mod windows_credential_store {
    use std::ptr;

    use windows_sys::Win32::Foundation::{GetLastError, ERROR_NOT_FOUND};
    use windows_sys::Win32::Security::Credentials::{
        CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE,
        CRED_TYPE_GENERIC,
    };

    use super::{AppError, CredentialResult, SERVICE_NAME};

    pub fn get_system_secret(key: &str) -> CredentialResult<Option<String>> {
        let target = to_wide(&windows_target_name(key));
        let mut credential: *mut CREDENTIALW = ptr::null_mut();
        let success = unsafe {
            CredReadW(
                target.as_ptr(),
                CRED_TYPE_GENERIC,
                0,
                &mut credential as *mut _,
            )
        };

        if success == 0 {
            let error = unsafe { GetLastError() };
            if error == ERROR_NOT_FOUND {
                return Ok(None);
            }
            return Err(AppError::Infrastructure(format!(
                "读取 Windows Credential Manager 失败，错误码: {error}"
            )));
        }

        let secret = unsafe {
            let credential_ref = &*credential;
            let bytes = std::slice::from_raw_parts(
                credential_ref.CredentialBlob,
                credential_ref.CredentialBlobSize as usize,
            );
            let value = String::from_utf8(bytes.to_vec()).map_err(|err| {
                AppError::Infrastructure(format!("解析 Windows Credential Manager 凭据失败: {err}"))
            })?;
            CredFree(credential.cast());
            value
        };

        Ok(Some(secret))
    }

    pub fn set_system_secret(key: &str, value: &str) -> CredentialResult<()> {
        let mut target = to_wide(&windows_target_name(key));
        let mut username = to_wide("Liberty");
        let mut credential_blob = value.as_bytes().to_vec();
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: target.as_mut_ptr(),
            CredentialBlobSize: credential_blob.len() as u32,
            CredentialBlob: credential_blob.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            UserName: username.as_mut_ptr(),
            ..Default::default()
        };

        let success = unsafe { CredWriteW(&credential, 0) };
        if success == 0 {
            let error = unsafe { GetLastError() };
            return Err(AppError::Infrastructure(format!(
                "写入 Windows Credential Manager 失败，错误码: {error}"
            )));
        }

        Ok(())
    }

    pub fn delete_system_secret(key: &str) -> CredentialResult<()> {
        let target = to_wide(&windows_target_name(key));
        let success = unsafe { CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) };
        if success == 0 {
            let error = unsafe { GetLastError() };
            if error != ERROR_NOT_FOUND {
                return Err(AppError::Infrastructure(format!(
                    "删除 Windows Credential Manager 凭据失败，错误码: {error}"
                )));
            }
        }
        Ok(())
    }

    fn windows_target_name(key: &str) -> String {
        format!("{SERVICE_NAME}:{key}")
    }

    fn to_wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }
}

#[cfg(windows)]
use windows_credential_store::{delete_system_secret, get_system_secret, set_system_secret};

#[cfg(not(any(target_os = "macos", windows)))]
fn get_system_secret(_key: &str) -> CredentialResult<Option<String>> {
    Ok(None)
}

#[cfg(not(any(target_os = "macos", windows)))]
fn set_system_secret(_key: &str, _value: &str) -> CredentialResult<()> {
    Err(AppError::Infrastructure(
        "当前平台暂不支持系统凭据存储。".into(),
    ))
}

#[cfg(not(any(target_os = "macos", windows)))]
fn delete_system_secret(_key: &str) -> CredentialResult<()> {
    Ok(())
}
