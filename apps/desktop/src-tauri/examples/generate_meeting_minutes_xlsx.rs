use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook, Worksheet, XlsxError};

#[derive(Clone)]
struct SpeechBlock {
    department: &'static str,
    name: &'static str,
    weekly_summary: &'static [&'static str],
    next_week_plan: &'static [&'static str],
    summary: &'static [&'static str],
}

fn main() -> Result<(), XlsxError> {
    let output = "/tmp/liberty-meeting-minutes.xlsx";

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("会议纪要")?;

    let title = Format::new()
        .set_bold()
        .set_font_size(20.)
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter);
    let label = Format::new()
        .set_bold()
        .set_border(FormatBorder::Thin)
        .set_background_color(Color::RGB(0xF2F2F2))
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
        .set_text_wrap();
    let value = Format::new()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Left)
        .set_align(FormatAlign::VerticalCenter)
        .set_text_wrap();
    let section = Format::new()
        .set_bold()
        .set_font_color(Color::RGB(0xC00000))
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Left)
        .set_align(FormatAlign::VerticalCenter)
        .set_text_wrap();
    let speaker = Format::new()
        .set_bold()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
        .set_text_wrap();
    let content = Format::new()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Left)
        .set_align(FormatAlign::Top)
        .set_text_wrap();

    worksheet.set_column_width(0, 18)?;
    worksheet.set_column_width(1, 88)?;
    worksheet.set_default_row_height(22);
    worksheet.merge_range(0, 0, 0, 1, "标准录音 16会议纪要", &title)?;
    worksheet.set_row_height(0, 32)?;

    let mut row = 2u32;
    write_kv(
        worksheet,
        &mut row,
        "会议名称",
        "标准录音 16",
        &label,
        &value,
    )?;
    write_kv(worksheet, &mut row, "会议时间", "待补充", &label, &value)?;
    write_kv(worksheet, &mut row, "会议地点", "待补充", &label, &value)?;
    write_kv(worksheet, &mut row, "记录人", "待补充", &label, &value)?;
    write_kv(worksheet, &mut row, "出席人员", "待补充", &label, &value)?;
    write_kv(worksheet, &mut row, "缺席人员", "待补充", &label, &value)?;
    write_kv(
        worksheet,
        &mut row,
        "主要议题",
        "五一假期接待准备、卫生安全检查、团队与市场拓展、员工工作安排",
        &label,
        &value,
    )?;
    write_kv(worksheet, &mut row, "会议主持人", "待补充", &label, &value)?;
    write_kv(worksheet, &mut row, "审阅", "待补充", &label, &value)?;

    worksheet.merge_range(row, 0, row, 1, "发言内容", &section)?;
    row += 1;

    let speech_blocks = vec![
        SpeechBlock {
            department: "营销部",
            name: "李兰",
            weekly_summary: &[
                "1、接待福州社团三天的工作已协调。",
                "2、交易所协助餐厅事宜。",
                "3、几天都在审查厨房，上周送料较多，基本都在厂内。",
            ],
            next_week_plan: &[
                "1、接待五一成都林总的团队。",
                "2、全力接待五一期间的接待。",
                "3、继续推进和家的商业项目文案。",
                "4、继续审查厨房。",
                "5、跟踪回族客人的每日行程。",
            ],
            summary: &[
                "1、已完成节前接待协调、餐厅协同和厨房审查相关工作。",
                "2、本周重点保障五一团队接待并持续推进项目文案与客情跟踪。",
            ],
        },
        SpeechBlock {
            department: "营销部",
            name: "段世琼",
            weekly_summary: &[
                "1、永中团队48人，5月1日到，车子不坐50座。",
                "2、康联的习老师团队，5月1日至5日离店。",
                "3、客人不吃早餐，需多问。",
                "4、成都团队要求高，房间卫生细节需注意，可能安排二楼或三楼。",
                "5、定金已收。",
                "6、团队吃饭AA制，原则上不安排餐，待客人上车后根据意愿决定。",
            ],
            next_week_plan: &[
                "1、继续联系旅行社和平台做宣传。",
                "2、快品客单效果不好，需调整。",
                "3、昆明和重庆市场拓展，年底高速通车，费用较高。",
                "4、邀请内江团队做康养，但客房条件需改进。",
                "5、宜苗新门店29号开业已取消，预计6月3-4日启动，约400人。",
                "6、胡老师团队对服务满意，下半年至少再来两次。",
                "7、争取缩短亏损，联系新客户。",
                "8、游泳班教练价格需商议。",
            ],
            summary: &[
                "1、已落实重点团队接待信息、定金及餐宿特殊要求。",
                "2、本周继续推进宣传获客、异地市场拓展和新客户开发。",
            ],
        },
        SpeechBlock {
            department: "温泉部",
            name: "杨小容",
            weekly_summary: &[
                "1、帖子已贴完。",
                "2、发烧产品，参加几个采摘活动。",
                "3、桌子需声灭改造。",
                "4、新员工注意卫生。",
                "5、三楼需更换设备。",
            ],
            next_week_plan: &[
                "1、阳光房更换，十样全部更换。",
                "2、房间检查，随时准备，放卫生卡。",
                "3、日常维护。",
            ],
            summary: &[
                "1、已完成基础张贴和活动参与安排，发现设备与卫生管理问题。",
                "2、本周重点推进阳光房更换、房间检查和日常维护。",
            ],
        },
        SpeechBlock {
            department: "客房部",
            name: "陈丽",
            weekly_summary: &["1、本周三周会已准备。"],
            next_week_plan: &["1、待补充。"],
            summary: &[
                "1、已完成周会准备工作。",
                "2、后续工作内容待补充原始记录后完善。",
            ],
        },
    ];

    for block in &speech_blocks {
        let left = format!("{}\n{}", block.department, block.name);
        let right = format!(
            "上周总结：\n{}\n\n本周计划：\n{}\n\n总结：\n{}",
            block.weekly_summary.join("\n"),
            block.next_week_plan.join("\n"),
            block.summary.join("\n")
        );

        worksheet.write_string_with_format(row, 0, left, &speaker)?;
        worksheet.write_string_with_format(row, 1, right, &content)?;
        worksheet.set_row_height(row, 170)?;
        row += 1;
    }

    worksheet.set_freeze_panes(2, 0)?;
    workbook.save(output)?;
    println!("{output}");
    Ok(())
}

fn write_kv(
    worksheet: &mut Worksheet,
    row: &mut u32,
    label_text: &str,
    value_text: &str,
    label: &Format,
    value: &Format,
) -> Result<(), XlsxError> {
    worksheet.write_string_with_format(*row, 0, label_text, label)?;
    worksheet.write_string_with_format(*row, 1, value_text, value)?;
    *row += 1;
    Ok(())
}
