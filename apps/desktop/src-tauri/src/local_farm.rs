use crate::local_db::{
    self, FarmHarvestLedgerEntry, FarmHarvestResult, FarmState, LocalResult, WorkMarketState,
};
use serde::Deserialize;
use tauri::AppHandle;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FarmPlantInput {
    pub plot_id: String,
    pub crop_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FarmPlotInput {
    pub plot_id: String,
}

#[tauri::command]
pub fn get_work_market_state(app: AppHandle) -> LocalResult<WorkMarketState> {
    local_db::get_work_market_state(&app)
}

#[tauri::command]
pub fn get_farm_state(app: AppHandle) -> LocalResult<FarmState> {
    local_db::get_farm_state(&app)
}

#[tauri::command]
pub fn plant_farm_crop(app: AppHandle, input: FarmPlantInput) -> LocalResult<FarmState> {
    local_db::plant_farm_crop(&app, input.plot_id.trim(), input.crop_key.trim())
}

#[tauri::command]
pub fn water_farm_plot(app: AppHandle, input: FarmPlotInput) -> LocalResult<FarmState> {
    local_db::water_farm_plot(&app, input.plot_id.trim())
}

#[tauri::command]
pub fn harvest_farm_plot(app: AppHandle, input: FarmPlotInput) -> LocalResult<FarmHarvestResult> {
    local_db::harvest_farm_plot(&app, input.plot_id.trim())
}

#[tauri::command]
pub fn list_farm_harvest_ledger(
    app: AppHandle,
    limit: Option<usize>,
) -> LocalResult<Vec<FarmHarvestLedgerEntry>> {
    local_db::list_farm_harvest_ledger(&app, limit.unwrap_or(20).clamp(1, 100))
}
