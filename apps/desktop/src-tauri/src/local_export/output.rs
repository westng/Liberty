use crate::local_db::LocalResult;
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::Webview;
use tauri_plugin_fs::FsExt;

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) struct SafeOutputPath {
    path: PathBuf,
    parent: PathBuf,
    target_exists: bool,
}

pub(crate) fn authorized_output_path(webview: &Webview, file_path: &str) -> LocalResult<PathBuf> {
    let file_path = file_path.trim();
    if file_path.is_empty() {
        return Err("导出路径不能为空。".into());
    }

    let output_path = PathBuf::from(file_path);
    if !webview.fs_scope().is_allowed(&output_path) {
        return Err("导出失败：目标路径未经保存对话框授权。".into());
    }
    Ok(output_path)
}

pub(crate) fn write_text_atomically(output_path: &Path, content: &str) -> LocalResult<()> {
    write_text_atomically_with_replace(output_path, content, replace_output)
}

fn write_text_atomically_with_replace<F>(
    output_path: &Path,
    content: &str,
    replace: F,
) -> LocalResult<()>
where
    F: FnOnce(&Path, &SafeOutputPath) -> LocalResult<()>,
{
    let output = prepare_output_path(output_path)?;
    let (temp_path, mut temp_file) = create_temporary_output(&output)?;
    let write_result = temp_file
        .write_all(content.as_bytes())
        .map_err(|error| format!("导出内容写入失败: {error}"))
        .and_then(|()| {
            temp_file
                .sync_all()
                .map_err(|error| format!("导出内容同步失败: {error}"))
        });
    drop(temp_file);
    let result = write_result.and_then(|()| replace(&temp_path, &output));
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

pub(crate) fn prepare_output_path(output_path: &Path) -> LocalResult<SafeOutputPath> {
    if output_path.as_os_str().is_empty() || output_path.file_name().is_none() {
        return Err("导出文件名无效。".into());
    }
    if output_path
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return Err("导出路径不能包含父目录跳转。".into());
    }

    let path = if output_path.is_absolute() {
        output_path.to_path_buf()
    } else {
        env::current_dir()
            .map_err(|error| format!("无法解析导出路径: {error}"))?
            .join(output_path)
    };
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| "导出文件缺少有效父目录。".to_string())?
        .to_path_buf();
    let target_exists = validate_output_location(&path, &parent)?;
    Ok(SafeOutputPath {
        path,
        parent,
        target_exists,
    })
}

fn validate_output_location(path: &Path, parent: &Path) -> LocalResult<bool> {
    let mut ancestors = parent.ancestors().collect::<Vec<_>>();
    ancestors.reverse();
    for ancestor in ancestors {
        let metadata = fs::symlink_metadata(ancestor)
            .map_err(|error| format!("导出父目录不可用 ({}): {error}", ancestor.display()))?;
        if is_dangerous_link(&metadata) {
            return Err(format!(
                "导出路径不能穿过符号链接或重解析点: {}",
                ancestor.display()
            ));
        }
        if !metadata.is_dir() {
            return Err(format!("导出路径父链包含非目录: {}", ancestor.display()));
        }
    }

    match fs::symlink_metadata(path) {
        Ok(metadata) if is_dangerous_link(&metadata) => Err(format!(
            "导出目标不能是符号链接或重解析点: {}",
            path.display()
        )),
        Ok(metadata) if !metadata.is_file() => {
            Err(format!("导出目标必须是普通文件: {}", path.display()))
        }
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("无法检查导出目标 ({}): {error}", path.display())),
    }
}

pub(crate) fn create_temporary_output(output: &SafeOutputPath) -> LocalResult<(PathBuf, File)> {
    let file_name = output
        .path
        .file_name()
        .ok_or_else(|| "导出文件名无效。".to_string())?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();

    for _ in 0..128 {
        let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut temp_name = std::ffi::OsString::from(".");
        temp_name.push(file_name);
        temp_name.push(format!(".{}.{}.{}.tmp", std::process::id(), nonce, counter));
        let temp_path = output.parent.join(temp_name);
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        match options.open(&temp_path) {
            Ok(file) => return Ok((temp_path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("导出临时文件创建失败: {error}")),
        }
    }

    Err("导出临时文件创建失败: 无法分配唯一文件名。".into())
}

pub(crate) fn open_regular_file_without_links(path: &Path, label: &str) -> LocalResult<File> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("{label}不可用 ({}): {error}", path.display()))?;
    if is_dangerous_link(&metadata) || !metadata.is_file() {
        return Err(format!("{label}必须是真实普通文件: {}", path.display()));
    }

    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options
        .open(path)
        .map_err(|error| format!("{label}打开失败 ({}): {error}", path.display()))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("{label}元数据读取失败: {error}"))?;
    if is_dangerous_link(&metadata) || !metadata.is_file() {
        return Err(format!("{label}必须是真实普通文件: {}", path.display()));
    }
    Ok(file)
}

fn is_dangerous_link(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    false
}

pub(crate) fn replace_output(temp_path: &Path, output: &SafeOutputPath) -> LocalResult<()> {
    let target_exists = validate_output_location(&output.path, &output.parent)?;
    if target_exists != output.target_exists {
        return Err("导出期间目标文件状态发生变化，已取消原子替换。".into());
    }
    let temp_file = open_regular_file_without_links(temp_path, "导出临时文件")?;
    sync_parent_directory(&output.parent);
    drop(temp_file);

    platform_replace(temp_path, &output.path, target_exists).map_err(format_replace_error)?;
    sync_committed_output(&output.path);
    sync_parent_directory(&output.parent);
    Ok(())
}

#[cfg(unix)]
fn platform_replace(temp_path: &Path, output_path: &Path, _target_exists: bool) -> io::Result<()> {
    fs::rename(temp_path, output_path)
}

#[cfg(windows)]
fn platform_replace(temp_path: &Path, output_path: &Path, target_exists: bool) -> io::Result<()> {
    use std::{ffi::c_void, os::windows::ffi::OsStrExt, ptr};

    #[link(name = "kernel32")]
    unsafe extern "system" {
        #[link_name = "ReplaceFileW"]
        fn replace_file_w(
            replaced_file_name: *const u16,
            replacement_file_name: *const u16,
            backup_file_name: *const u16,
            replace_flags: u32,
            exclude: *const c_void,
            reserved: *const c_void,
        ) -> i32;
        #[link_name = "MoveFileExW"]
        fn move_file_ex_w(
            existing_file_name: *const u16,
            new_file_name: *const u16,
            flags: u32,
        ) -> i32;
    }

    fn wide_path(path: &Path) -> io::Result<Vec<u16>> {
        let mut value = path.as_os_str().encode_wide().collect::<Vec<_>>();
        if value.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "path contains a NUL character",
            ));
        }
        value.push(0);
        Ok(value)
    }

    const MOVEFILE_WRITE_THROUGH: u32 = 0x0000_0008;
    let temp = wide_path(temp_path)?;
    let output = wide_path(output_path)?;
    let succeeded = unsafe {
        if target_exists {
            replace_file_w(
                output.as_ptr(),
                temp.as_ptr(),
                ptr::null(),
                0,
                ptr::null(),
                ptr::null(),
            )
        } else {
            move_file_ex_w(temp.as_ptr(), output.as_ptr(), MOVEFILE_WRITE_THROUGH)
        }
    };
    if succeeded == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(any(unix, windows)))]
fn platform_replace(temp_path: &Path, output_path: &Path, _target_exists: bool) -> io::Result<()> {
    fs::rename(temp_path, output_path)
}

fn format_replace_error(error: io::Error) -> String {
    if atomic_replace_is_unsupported(&error) {
        format!("导出目标文件系统不支持可靠的原子替换: {error}")
    } else {
        format!("导出文件原子替换失败，已有目标保持不变: {error}")
    }
}

#[cfg(unix)]
fn atomic_replace_is_unsupported(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::Unsupported
        || error.raw_os_error() == Some(libc::EXDEV)
        || error.raw_os_error() == Some(libc::ENOTSUP)
        || error.raw_os_error() == Some(libc::EOPNOTSUPP)
}

#[cfg(windows)]
fn atomic_replace_is_unsupported(error: &io::Error) -> bool {
    matches!(error.raw_os_error(), Some(1 | 17 | 50 | 120))
        || error.kind() == io::ErrorKind::Unsupported
}

#[cfg(not(any(unix, windows)))]
fn atomic_replace_is_unsupported(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::Unsupported
}

#[cfg(unix)]
fn sync_parent_directory(parent: &Path) {
    if let Ok(directory) = File::open(parent) {
        let _ = directory.sync_all();
    }
}

#[cfg(not(unix))]
fn sync_parent_directory(_parent: &Path) {}

fn sync_committed_output(path: &Path) {
    if let Ok(file) = File::open(path) {
        let _ = file.sync_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(name: &str) -> Self {
            let path = env::current_dir()
                .unwrap()
                .join("target")
                .join("local-export-output-tests")
                .join(format!(
                    "{name}-{}-{}",
                    std::process::id(),
                    TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed)
                ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn atomically_writes_new_text_file() {
        let directory = TestDir::new("new-text");
        let output_path = directory.path().join("meeting.md");

        write_text_atomically(&output_path, "会议内容").unwrap();

        assert_eq!(fs::read_to_string(output_path).unwrap(), "会议内容");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn atomically_replaces_existing_text_file() {
        let directory = TestDir::new("replace-text");
        let output_path = directory.path().join("meeting.md");
        fs::write(&output_path, "旧内容").unwrap();

        write_text_atomically(&output_path, "新内容").unwrap();

        assert_eq!(fs::read_to_string(output_path).unwrap(), "新内容");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn failed_replacement_preserves_existing_target() {
        let directory = TestDir::new("preserve-text");
        let output_path = directory.path().join("meeting.md");
        fs::write(&output_path, "旧内容").unwrap();
        let error =
            write_text_atomically_with_replace(&output_path, "新内容", |_temp_path, _output| {
                Err("simulated replacement failure".into())
            })
            .unwrap_err();

        assert_eq!(error, "simulated replacement failure");
        assert_eq!(fs::read_to_string(&output_path).unwrap(), "旧内容");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_text_target() {
        use std::os::unix::fs::symlink;

        let directory = TestDir::new("symlink-text");
        let destination = directory.path().join("destination.md");
        let output_path = directory.path().join("meeting.md");
        fs::write(&destination, "保留内容").unwrap();
        symlink(&destination, &output_path).unwrap();

        let error = write_text_atomically(&output_path, "恶意覆盖").unwrap_err();

        assert!(error.contains("不能是符号链接"), "{error}");
        assert_eq!(fs::read_to_string(destination).unwrap(), "保留内容");
    }
}
