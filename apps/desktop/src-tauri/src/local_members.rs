use crate::local_db::{self, LocalResult, MeetingMember, MeetingMemberImportResult};
use calamine::{open_workbook_auto, Data, Reader};
use chrono::Utc;
use rust_xlsxwriter::Workbook;
use std::{collections::HashSet, path::Path};
use tauri::AppHandle;

#[tauri::command]
pub fn list_meeting_members(app: AppHandle) -> LocalResult<Vec<MeetingMember>> {
    local_db::list_meeting_members(&app)
}

#[tauri::command]
pub fn save_meeting_member(app: AppHandle, member: MeetingMember) -> LocalResult<()> {
    local_db::save_meeting_member(&app, &member)
}

#[tauri::command]
pub fn delete_meeting_member(app: AppHandle, id: String) -> LocalResult<()> {
    local_db::delete_meeting_member(&app, &id)
}

#[tauri::command]
pub fn import_meeting_members_excel(app: AppHandle, file_path: String) -> LocalResult<MeetingMemberImportResult> {
    if file_path.trim().is_empty() {
        return Err("导入文件路径不能为空。".into());
    }

    let rows = parse_members_excel(Path::new(file_path.trim()))?;
    local_db::import_meeting_members(&app, &rows)
}

#[tauri::command]
pub fn export_meeting_members_excel(app: AppHandle, file_path: String) -> LocalResult<()> {
    if file_path.trim().is_empty() {
        return Err("导出路径不能为空。".into());
    }

    let members = local_db::list_meeting_members(&app)?;
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    worksheet
        .set_name("人员信息")
        .map_err(|err| err.to_string())?;

    let headers = ["姓名", "部门", "排序", "是否设置会议记录人"];
    for (column, header) in headers.iter().enumerate() {
        worksheet
            .write_string(0, column as u16, *header)
            .map_err(|err| err.to_string())?;
    }

    for (index, member) in members.iter().enumerate() {
        let row = (index + 1) as u32;
        worksheet
            .write_string(row, 0, &member.name)
            .map_err(|err| err.to_string())?;
        worksheet
            .write_string(row, 1, &member.department)
            .map_err(|err| err.to_string())?;
        worksheet
            .write_number(row, 2, member.sort_order as f64)
            .map_err(|err| err.to_string())?;
        worksheet
            .write_string(row, 3, if member.is_recorder { "是" } else { "否" })
            .map_err(|err| err.to_string())?;
    }

    workbook
        .save(file_path.trim())
        .map_err(|err| err.to_string())?;

    Ok(())
}

fn parse_members_excel(path: &Path) -> LocalResult<Vec<MeetingMember>> {
    let mut workbook = open_workbook_auto(path).map_err(|err| err.to_string())?;
    let first_sheet = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| "Excel 文件中没有可读取的工作表。".to_string())?;
    let range = workbook
        .worksheet_range(&first_sheet)
        .map_err(|err| err.to_string())?;

    let mut rows_iter = range.rows();
    let header_row = rows_iter
        .next()
        .ok_or_else(|| "Excel 文件为空，无法导入。".to_string())?;

    let mut name_col = None;
    let mut department_col = None;
    let mut sort_order_col = None;
    let mut recorder_col = None;

    for (index, cell) in header_row.iter().enumerate() {
        match cell_to_string(cell).trim() {
            "姓名" | "name" | "Name" => name_col = Some(index),
            "部门" | "department" | "Department" => department_col = Some(index),
            "排序" | "sortOrder" | "Sort Order" => sort_order_col = Some(index),
            "是否设置会议记录人" | "会议记录人" | "isRecorder" | "Recorder" => recorder_col = Some(index),
            _ => {}
        }
    }

    let name_col = name_col.ok_or_else(|| "Excel 缺少“姓名”列。".to_string())?;
    let department_col = department_col.ok_or_else(|| "Excel 缺少“部门”列。".to_string())?;
    let sort_order_col = sort_order_col.ok_or_else(|| "Excel 缺少“排序”列。".to_string())?;
    let recorder_col = recorder_col.ok_or_else(|| "Excel 缺少“是否设置会议记录人”列。".to_string())?;

    let mut rows = Vec::new();
    let mut seen_names = HashSet::new();
    let mut recorder_count = 0usize;

    for row in rows_iter {
        let name = cell_to_string(row.get(name_col).unwrap_or(&Data::Empty))
            .trim()
            .to_string();
        let department = cell_to_string(row.get(department_col).unwrap_or(&Data::Empty))
            .trim()
            .to_string();
        let sort_order_text = cell_to_string(row.get(sort_order_col).unwrap_or(&Data::Empty));
        let recorder_text = cell_to_string(row.get(recorder_col).unwrap_or(&Data::Empty));

        if name.is_empty() && department.is_empty() && sort_order_text.trim().is_empty() && recorder_text.trim().is_empty() {
            continue;
        }

        if name.is_empty() {
            return Err("导入失败：存在姓名为空的行。".into());
        }

        if !seen_names.insert(name.clone()) {
            return Err(format!("导入失败：Excel 中存在重复姓名“{name}”。"));
        }

        let sort_order = parse_sort_order(&sort_order_text, &name)?;
        let is_recorder = parse_recorder_flag(&recorder_text, &name)?;
        if is_recorder {
            recorder_count += 1;
        }

        rows.push(MeetingMember {
            id: String::new(),
            name,
            department,
            sort_order,
            is_recorder,
            created_at: current_iso_timestamp(),
            updated_at: current_iso_timestamp(),
        });
    }

    if rows.is_empty() {
        return Err("Excel 中没有可导入的人员数据。".into());
    }

    if recorder_count > 1 {
        return Err("导入失败：Excel 中最多只能有一位会议记录人。".into());
    }

    Ok(rows)
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(value) => value.clone(),
        Data::Float(value) => {
            if value.fract() == 0.0 {
                (*value as i64).to_string()
            } else {
                value.to_string()
            }
        }
        Data::Int(value) => value.to_string(),
        Data::Bool(value) => {
            if *value {
                "true".into()
            } else {
                "false".into()
            }
        }
        other => other.to_string(),
    }
}

fn parse_sort_order(value: &str, name: &str) -> LocalResult<i64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(0);
    }

    trimmed
        .parse::<i64>()
        .map_err(|_| format!("导入失败：人员“{name}”的排序不是有效整数。"))
}

fn parse_recorder_flag(value: &str, name: &str) -> LocalResult<bool> {
    let trimmed = value.trim().to_lowercase();
    if trimmed.is_empty() {
        return Ok(false);
    }

    match trimmed.as_str() {
        "1" | "true" | "yes" | "y" | "是" => Ok(true),
        "0" | "false" | "no" | "n" | "否" => Ok(false),
        _ => Err(format!("导入失败：人员“{name}”的会议记录人标记无法识别。")),
    }
}

fn current_iso_timestamp() -> String {
    Utc::now().to_rfc3339()
}
