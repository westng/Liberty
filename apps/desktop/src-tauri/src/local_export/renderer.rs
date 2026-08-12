use std::{
    collections::HashSet,
    fs::{self, File},
    io::{self, Cursor, Read, Write},
    path::Path,
};

use xmltree::{Element, EmitterConfig, XMLNode};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use crate::local_db::LocalResult;
use crate::local_export::model::{ExportDocData, SpeechBlock};
use crate::local_export::output::{
    create_temporary_output, open_regular_file_without_links, prepare_output_path, replace_output,
    SafeOutputPath,
};
use crate::local_export::parser::{is_missing_value, starts_with_numbered_item};
use crate::local_export::xml::{
    child_elements, child_elements_count, child_elements_mut, clone_with_text, find_child_mut,
    local_name, remove_last_child_element, set_cell_value, set_first_text,
};

pub const TEMPLATE_DOCX_BYTES: &[u8] =
    include_bytes!("../../resources/templates/meeting-minutes.docx");

pub fn export_summary_docx(data: &ExportDocData, output_path: &Path) -> LocalResult<()> {
    export_summary_docx_with_template(data, output_path, TEMPLATE_DOCX_BYTES)
}

fn export_summary_docx_with_template(
    data: &ExportDocData,
    output_path: &Path,
    template: &[u8],
) -> LocalResult<()> {
    export_summary_docx_with_template_and_replace(data, output_path, template, replace_output)
}

fn export_summary_docx_with_template_and_replace<F>(
    data: &ExportDocData,
    output_path: &Path,
    template: &[u8],
    replace: F,
) -> LocalResult<()>
where
    F: FnOnce(&Path, &SafeOutputPath) -> LocalResult<()>,
{
    let output = prepare_output_path(output_path)?;
    let (temp_path, temp_file) = create_temporary_output(&output)?;
    let result = (|| {
        write_summary_docx(data, temp_file, template)?;
        validate_docx(&temp_path)?;
        replace(&temp_path, &output)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn write_summary_docx(data: &ExportDocData, output: File, template: &[u8]) -> LocalResult<()> {
    let mut archive = ZipArchive::new(Cursor::new(template))
        .map_err(|err| format!("会议纪要模板读取失败: {err}"))?;
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

    let output = writer.finish().map_err(|err| err.to_string())?;
    output.sync_all().map_err(|err| err.to_string())?;
    Ok(())
}

fn validate_docx(path: &Path) -> LocalResult<()> {
    let file = open_regular_file_without_links(path, "会议纪要临时文件")?;
    let mut archive = ZipArchive::new(file).map_err(|err| format!("导出文件校验失败: {err}"))?;
    if archive.is_empty() {
        return Err("导出文件校验失败: ZIP 为空。".into());
    }

    let mut entries = HashSet::new();
    let mut required_parts = HashSet::new();
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|err| format!("导出文件 ZIP 条目校验失败: {err}"))?;
        let name = entry.name().to_string();
        if name.contains('\\') || entry.enclosed_name().is_none() || entry.is_symlink() {
            return Err(format!("导出文件包含不安全的 ZIP 条目: {name}"));
        }
        if !entries.insert(name.to_ascii_lowercase()) {
            return Err(format!("导出文件包含重复的 ZIP 条目: {name}"));
        }

        let lower_name = name.to_ascii_lowercase();
        if matches!(
            lower_name.as_str(),
            "[content_types].xml" | "_rels/.rels" | "word/document.xml"
        ) {
            required_parts.insert(lower_name.clone());
        }

        if lower_name.ends_with(".xml") || lower_name.ends_with(".rels") {
            let mut xml = Vec::new();
            entry
                .read_to_end(&mut xml)
                .map_err(|err| format!("导出文件 ZIP 条目读取失败 ({name}): {err}"))?;
            Element::parse(xml.as_slice())
                .map_err(|err| format!("导出文件 XML 校验失败 ({name}): {err}"))?;
        } else {
            io::copy(&mut entry, &mut io::sink())
                .map_err(|err| format!("导出文件 ZIP 条目读取失败 ({name}): {err}"))?;
        }
    }

    for required in ["[content_types].xml", "_rels/.rels", "word/document.xml"] {
        if !required_parts.contains(required) {
            return Err(format!("导出文件缺少 DOCX 核心部件: {required}"));
        }
    }
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

    let (section_heading_template, section_item_template) = {
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
        let sample_cells = child_elements(&sample_row, "tc");
        let sample_paragraphs = sample_cells
            .get(1)
            .map(|cell| child_elements(cell, "p"))
            .ok_or_else(|| "会议纪要模板缺少发言内容单元格。".to_string())?;
        if sample_paragraphs.len() < 2 {
            return Err("会议纪要模板缺少总结段落样式。".into());
        }
        let heading_template = sample_paragraphs[0].clone();
        let item_template = sample_paragraphs[1].clone();
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
        (heading_template, item_template)
    };

    append_document_section(
        body,
        "全局总结：",
        &data.closing_summary,
        &section_heading_template,
        &section_item_template,
    );
    append_document_section(
        body,
        "决策、行动项、风险与跟进：",
        &data.fallback_overview,
        &section_heading_template,
        &section_item_template,
    );

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
    append_paragraph(
        content_cell,
        clone_with_text(&heading_template, "个人总结："),
    );
    append_section_items(content_cell, &block.summary, &item_template);

    Ok(())
}

fn append_document_section(
    body: &mut Element,
    heading: &str,
    items: &[String],
    heading_template: &Element,
    item_template: &Element,
) {
    if items.iter().all(|item| item.trim().is_empty()) {
        return;
    }
    append_body_paragraph(body, clone_with_text(heading_template, heading));
    for (index, item) in items
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .enumerate()
    {
        let text = if item.ends_with(['：', ':']) || starts_with_numbered_item(item) {
            item.to_string()
        } else {
            format!("{}、{item}", index + 1)
        };
        append_body_paragraph(body, clone_with_text(item_template, &text));
    }
}

fn append_body_paragraph(body: &mut Element, paragraph: Element) {
    let index = body
        .children
        .iter()
        .position(|node| {
            matches!(node, XMLNode::Element(element) if local_name(&element.name) == "sectPr")
        })
        .unwrap_or(body.children.len());
    body.children.insert(index, XMLNode::Element(paragraph));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(name: &str) -> Self {
            let path = env::current_dir()
                .unwrap()
                .join("target")
                .join("local-export-tests")
                .join(format!(
                    "{name}-{}-{}-{}",
                    std::process::id(),
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_nanos(),
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

    fn template_xml() -> String {
        let mut archive = ZipArchive::new(Cursor::new(TEMPLATE_DOCX_BYTES)).unwrap();
        let mut document = archive.by_name("word/document.xml").unwrap();
        let mut xml = String::new();
        document.read_to_string(&mut xml).unwrap();
        xml
    }

    #[test]
    fn renders_personal_and_global_summary_fields() {
        let data = ExportDocData {
            title: "周会会议纪要".into(),
            meeting_name: "周会".into(),
            closing_summary: vec!["全局结论".into()],
            fallback_overview: vec![
                "决策：".into(),
                "批准上线".into(),
                "行动项：".into(),
                "任务：发布；负责人：李兰；截止日期：周五".into(),
                "风险：".into(),
                "容量不足".into(),
                "跟进事项：".into(),
                "复盘容量".into(),
            ],
            speech_blocks: vec![SpeechBlock {
                department: "营销部".into(),
                name: "李兰".into(),
                weekly_summary: vec!["上周事项".into()],
                next_week_plan: vec!["本周计划".into()],
                summary: vec!["个人结论".into()],
                original_index: 0,
            }],
            ..ExportDocData::default()
        };

        let rendered = render_document_xml(&template_xml(), &data).unwrap();

        for expected in [
            "上周事项",
            "本周计划",
            "个人结论",
            "全局结论",
            "批准上线",
            "任务：发布；负责人：李兰；截止日期：周五",
            "容量不足",
            "复盘容量",
        ] {
            assert!(rendered.contains(expected), "missing field: {expected}");
        }
    }

    #[test]
    fn failed_export_preserves_existing_target() {
        let directory = TestDir::new("preserve-invalid-template");
        let output_path = directory.path().join("meeting.docx");
        fs::write(&output_path, b"existing document").unwrap();

        let error = export_summary_docx_with_template(
            &ExportDocData::default(),
            &output_path,
            b"not a zip",
        )
        .unwrap_err();

        assert!(error.contains("模板读取失败"));
        assert_eq!(fs::read(&output_path).unwrap(), b"existing document");
    }

    #[test]
    fn replaces_existing_regular_target_with_valid_docx() {
        let directory = TestDir::new("replace-existing");
        let output_path = directory.path().join("meeting.docx");
        fs::write(&output_path, b"existing document").unwrap();

        export_summary_docx(&ExportDocData::default(), &output_path).unwrap();

        assert_ne!(fs::read(&output_path).unwrap(), b"existing document");
        validate_docx(&output_path).unwrap();
    }

    #[test]
    fn replacement_failure_preserves_existing_target() {
        let directory = TestDir::new("preserve-replace-failure");
        let output_path = directory.path().join("meeting.docx");
        fs::write(&output_path, b"existing document").unwrap();

        let error = export_summary_docx_with_template_and_replace(
            &ExportDocData::default(),
            &output_path,
            TEMPLATE_DOCX_BYTES,
            |_temp_path, _output| Err("simulated replacement failure".into()),
        )
        .unwrap_err();

        assert_eq!(error, "simulated replacement failure");
        assert_eq!(fs::read(&output_path).unwrap(), b"existing document");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_target_without_changing_link_destination() {
        use std::os::unix::fs::symlink;

        let directory = TestDir::new("symlink-target");
        let destination = directory.path().join("destination.docx");
        let output_path = directory.path().join("meeting.docx");
        fs::write(&destination, b"linked document").unwrap();
        symlink(&destination, &output_path).unwrap();

        let error = export_summary_docx(&ExportDocData::default(), &output_path).unwrap_err();

        assert!(error.contains("不能是符号链接"), "{error}");
        assert_eq!(fs::read(&destination).unwrap(), b"linked document");
        assert!(fs::symlink_metadata(&output_path)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_in_parent_chain() {
        use std::os::unix::fs::symlink;

        let directory = TestDir::new("symlink-parent");
        let real_parent = directory.path().join("real");
        let linked_parent = directory.path().join("linked");
        fs::create_dir(&real_parent).unwrap();
        symlink(&real_parent, &linked_parent).unwrap();
        let output_path = linked_parent.join("meeting.docx");

        let error = export_summary_docx(&ExportDocData::default(), &output_path).unwrap_err();

        assert!(error.contains("不能穿过符号链接"), "{error}");
        assert!(!real_parent.join("meeting.docx").exists());
    }
}
