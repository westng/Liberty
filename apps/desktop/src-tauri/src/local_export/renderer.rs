use std::{
    fs::File,
    io::{Cursor, Read, Write},
    path::Path,
};

use xmltree::{Element, EmitterConfig, XMLNode};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use crate::local_db::LocalResult;
use crate::local_export::model::{ExportDocData, SpeechBlock};
use crate::local_export::parser::{is_missing_value, starts_with_numbered_item};
use crate::local_export::xml::{
    child_elements, child_elements_count, child_elements_mut, clone_with_text, find_child_mut,
    local_name, remove_last_child_element, set_cell_value, set_first_text,
};

pub const TEMPLATE_DOCX_BYTES: &[u8] =
    include_bytes!("../../resources/templates/meeting-minutes.docx");

pub fn export_summary_docx(data: &ExportDocData, output_path: &Path) -> LocalResult<()> {
    let mut archive = ZipArchive::new(Cursor::new(TEMPLATE_DOCX_BYTES))
        .map_err(|err| format!("会议纪要模板读取失败: {err}"))?;
    let output = File::create(output_path).map_err(|err| err.to_string())?;
    let mut writer = ZipWriter::new(output);

    for index in 0..archive.len() {
        let mut source = archive.by_index(index).map_err(|err| err.to_string())?;
        let name = source.name().to_string();
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        writer
            .start_file(name.clone(), options)
            .map_err(|err| err.to_string())?;

        if name == "word/document.xml" {
            let mut xml = String::new();
            source
                .read_to_string(&mut xml)
                .map_err(|err| err.to_string())?;
            let updated = render_document_xml(&xml, data)?;
            writer
                .write_all(updated.as_bytes())
                .map_err(|err| err.to_string())?;
        } else {
            let mut buffer = Vec::new();
            source
                .read_to_end(&mut buffer)
                .map_err(|err| err.to_string())?;
            writer.write_all(&buffer).map_err(|err| err.to_string())?;
        }
    }

    writer.finish().map_err(|err| err.to_string())?;
    Ok(())
}

pub fn render_document_xml(xml: &str, data: &ExportDocData) -> LocalResult<String> {
    let mut root =
        Element::parse(xml.as_bytes()).map_err(|err| format!("会议纪要模板解析失败: {err}"))?;
    let body = find_child_mut(&mut root, "body")
        .ok_or_else(|| "会议纪要模板缺少文档主体。".to_string())?;
    let title_paragraph =
        find_child_mut(body, "p").ok_or_else(|| "会议纪要模板缺少标题段落。".to_string())?;
    set_first_text(title_paragraph, &data.title);

    let table =
        find_child_mut(body, "tbl").ok_or_else(|| "会议纪要模板缺少主表格。".to_string())?;
    let mut rows = child_elements_mut(table, "tr");
    if rows.len() < 7 {
        return Err("会议纪要模板结构不完整，缺少发言内容样板行。".into());
    }

    set_cell_value(rows[0], 1, &fallback_text(&data.meeting_name, "待补充"))?;
    set_cell_value(rows[0], 3, &fallback_text(&data.meeting_time, "待补充"))?;
    set_cell_value(rows[0], 5, &fallback_text(&data.meeting_location, "待补充"))?;
    set_cell_value(
        rows[0],
        6,
        &format!("记录人： {}", fallback_text(&data.recorder, "待补充")),
    )?;
    set_cell_value(rows[1], 1, &fallback_text(&data.attendees, "待补充"))?;
    set_cell_value(rows[2], 1, &data.absentees)?;
    set_cell_value(rows[3], 1, &fallback_text(&data.topics, "待补充"))?;
    set_cell_value(rows[4], 1, &fallback_text(&data.host, "待补充"))?;
    set_cell_value(rows[4], 3, &data.reviewer)?;

    let sample_row = rows[6].clone();
    while child_elements_count(table, "tr") > 6 {
        remove_last_child_element(table, "tr");
    }

    let speech_blocks = if data.speech_blocks.is_empty() {
        vec![SpeechBlock::default()]
    } else {
        data.speech_blocks.clone()
    };

    for block in speech_blocks {
        let mut row = sample_row.clone();
        fill_speech_row(&mut row, &block)?;
        table.children.push(XMLNode::Element(row));
    }

    let mut output = Vec::new();
    root.write_with_config(
        &mut output,
        EmitterConfig::new()
            .perform_indent(false)
            .write_document_declaration(true),
    )
    .map_err(|err| err.to_string())?;

    String::from_utf8(output).map_err(|err| err.to_string())
}

fn fill_speech_row(row: &mut Element, block: &SpeechBlock) -> LocalResult<()> {
    let mut cells = child_elements_mut(row, "tc");
    if cells.len() < 2 {
        return Err("会议纪要模板的发言内容行结构不正确。".into());
    }

    let mut left_paragraphs = child_elements_mut(cells[0], "p");
    if left_paragraphs.len() < 2 {
        return Err("会议纪要模板的发言人信息单元格结构不正确。".into());
    }

    set_first_text(
        left_paragraphs[0],
        if block.department.trim().is_empty() {
            "待补充部门"
        } else {
            block.department.trim()
        },
    );
    set_first_text(
        left_paragraphs[1],
        if block.name.trim().is_empty() {
            "待补充姓名"
        } else {
            block.name.trim()
        },
    );

    let content_cell = &mut cells[1];
    let paragraphs = child_elements(content_cell, "p");
    if paragraphs.len() < 13 {
        return Err("会议纪要模板的发言内容样板段落数量不足。".into());
    }

    let heading_template = paragraphs[0].clone();
    let item_template = paragraphs[1].clone();

    content_cell.children.retain(
        |node| !matches!(node, XMLNode::Element(element) if local_name(&element.name) == "p"),
    );

    append_paragraph(
        content_cell,
        clone_with_text(&heading_template, "上周总结："),
    );
    append_section_items(content_cell, &block.weekly_summary, &item_template);
    append_paragraph(
        content_cell,
        clone_with_text(&heading_template, "本周计划："),
    );
    append_section_items(content_cell, &block.next_week_plan, &item_template);

    Ok(())
}

fn append_section_items(cell: &mut Element, items: &[String], template: &Element) {
    let values = if items.is_empty() {
        vec!["待补充".to_string()]
    } else {
        items.to_vec()
    };

    for (index, item) in values.iter().enumerate() {
        let text = if item.trim().is_empty() {
            format!("{}、待补充", index + 1)
        } else if starts_with_numbered_item(item) {
            item.trim().to_string()
        } else {
            format!("{}、{}", index + 1, item.trim())
        };
        append_paragraph(cell, clone_with_text(template, &text));
    }
}

fn append_paragraph(cell: &mut Element, paragraph: Element) {
    cell.children.push(XMLNode::Element(paragraph));
}

fn fallback_text(value: &str, fallback: &str) -> String {
    if is_missing_value(value) {
        fallback.to_string()
    } else {
        value.trim().to_string()
    }
}
