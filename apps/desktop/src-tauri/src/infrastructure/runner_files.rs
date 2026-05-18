use std::{fs, path::Path};

use serde::de::DeserializeOwned;

use crate::local_db::LocalResult;

pub fn reset_runner_files(job_dir: &Path) -> LocalResult<()> {
    for name in ["result.json", "progress.json", "job.json"] {
        let path = job_dir.join(name);
        if path.exists() {
            fs::remove_file(path).map_err(|err| err.to_string())?;
        }
    }

    Ok(())
}

pub fn read_json_file<T>(job_dir: &Path, file_name: &str) -> LocalResult<T>
where
    T: DeserializeOwned,
{
    let bytes = fs::read(job_dir.join(file_name)).map_err(|err| err.to_string())?;
    serde_json::from_slice(&bytes).map_err(|err| err.to_string())
}

pub fn read_optional_json_file<T>(job_dir: &Path, file_name: &str) -> Option<T>
where
    T: DeserializeOwned,
{
    let bytes = fs::read(job_dir.join(file_name)).ok()?;
    serde_json::from_slice(&bytes).ok()
}
