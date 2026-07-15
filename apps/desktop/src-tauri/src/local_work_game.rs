use crate::local_db::{self, LocalResult, WorkGameClaimResult, WorkGameState};
use serde::Deserialize;
use tauri::AppHandle;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkGameStateInput {
    pub game_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkGameStartInput {
    pub game_key: String,
    pub task_id: String,
    pub job_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkGameTaskInput {
    pub game_key: String,
    pub task_id: String,
}

#[tauri::command]
pub fn get_work_game_state(
    app: AppHandle,
    input: WorkGameStateInput,
) -> LocalResult<WorkGameState> {
    local_db::get_work_game_state(&app, input.game_key.trim())
}

#[tauri::command]
pub fn start_work_game_task(
    app: AppHandle,
    input: WorkGameStartInput,
) -> LocalResult<WorkGameState> {
    local_db::start_work_game_task(
        &app,
        input.game_key.trim(),
        input.task_id.trim(),
        input.job_key.trim(),
    )
}

#[tauri::command]
pub fn care_work_game_task(app: AppHandle, input: WorkGameTaskInput) -> LocalResult<WorkGameState> {
    local_db::care_work_game_task(&app, input.game_key.trim(), input.task_id.trim())
}

#[tauri::command]
pub fn claim_work_game_task(
    app: AppHandle,
    input: WorkGameTaskInput,
) -> LocalResult<WorkGameClaimResult> {
    local_db::claim_work_game_task(&app, input.game_key.trim(), input.task_id.trim())
}
