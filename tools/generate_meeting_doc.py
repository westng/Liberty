from __future__ import annotations

import copy
import zipfile
from pathlib import Path
import xml.etree.ElementTree as ET


W = "{http://schemas.openxmlformats.org/wordprocessingml/2006/main}"


DATA = {
    "title": "标准录音 16会议纪要",
    "meeting_name": "标准录音 16",
    "meeting_time": "待补充",
    "meeting_location": "待补充",
    "recorder": "待补充",
    "attendees": "待补充",
    "absentees": "待补充",
    "topics": "五一假期接待准备、卫生安全检查、团队与市场拓展、员工工作安排",
    "host": "待补充",
    "reviewer": "待补充",
    "speech_blocks": [
        {
            "department": "营销部",
            "name": "李兰",
            "weekly_summary": [
                "1、接待福州社团三天的工作已协调。",
                "2、交易所协助餐厅事宜。",
                "3、几天都在审查厨房，上周送料较多，基本都在厂内。",
            ],
            "next_week_plan": [
                "1、接待五一成都林总的团队。",
                "2、全力接待五一期间的接待。",
                "3、继续推进和家的商业项目文案。",
                "4、继续审查厨房。",
                "5、跟踪回族客人的每日行程。",
            ],
            "summary": [
                "1、已完成节前接待协调、餐厅协同和厨房审查相关工作。",
                "2、本周重点保障五一团队接待并持续推进项目文案与客情跟踪。",
            ],
        },
        {
            "department": "营销部",
            "name": "段世琼",
            "weekly_summary": [
                "1、永中团队48人，5月1日到，车子不坐50座。",
                "2、康联的习老师团队，5月1日至5日离店。",
                "3、客人不吃早餐，需多问。",
                "4、成都团队要求高，房间卫生细节需注意，可能安排二楼或三楼。",
                "5、定金已收。",
                "6、团队吃饭AA制，原则上不安排餐，待客人上车后根据意愿决定。",
            ],
            "next_week_plan": [
                "1、继续联系旅行社和平台做宣传。",
                "2、快品客单效果不好，需调整。",
                "3、昆明和重庆市场拓展，年底高速通车，费用较高。",
                "4、邀请内江团队做康养，但客房条件需改进。",
                "5、宜苗新门店29号开业已取消，预计6月3-4日启动，约400人。",
                "6、胡老师团队对服务满意，下半年至少再来两次。",
                "7、争取缩短亏损，联系新客户。",
                "8、游泳班教练价格需商议。",
            ],
            "summary": [
                "1、已落实重点团队接待信息、定金及餐宿特殊要求。",
                "2、本周继续推进宣传获客、异地市场拓展和新客户开发。",
            ],
        },
        {
            "department": "温泉部",
            "name": "杨小容",
            "weekly_summary": [
                "1、帖子已贴完。",
                "2、发烧产品，参加几个采摘活动。",
                "3、桌子需声灭改造。",
                "4、新员工注意卫生。",
                "5、三楼需更换设备。",
            ],
            "next_week_plan": [
                "1、阳光房更换，十样全部更换。",
                "2、房间检查，随时准备，放卫生卡。",
                "3、日常维护。",
            ],
            "summary": [
                "1、已完成基础张贴和活动参与安排，发现设备与卫生管理问题。",
                "2、本周重点推进阳光房更换、房间检查和日常维护。",
            ],
        },
        {
            "department": "客房部",
            "name": "陈丽",
            "weekly_summary": [
                "1、本周三周会已准备。",
            ],
            "next_week_plan": [
                "1、待补充。",
            ],
            "summary": [
                "1、已完成周会准备工作。",
                "2、后续工作内容待补充原始记录后完善。",
            ],
        },
    ],
}


def qn(tag: str) -> str:
    return f"{W}{tag}"


def first_text_node(element: ET.Element) -> ET.Element | None:
    for node in element.iter():
        if node.tag == qn("t"):
            return node
    return None


def set_first_text(element: ET.Element, value: str) -> None:
    node = first_text_node(element)
    if node is not None:
        node.text = value


def paragraphs(element: ET.Element) -> list[ET.Element]:
    return [child for child in element if child.tag == qn("p")]


def cells(row: ET.Element) -> list[ET.Element]:
    return [child for child in row if child.tag == qn("tc")]


def clone_with_text(template: ET.Element, text: str) -> ET.Element:
    cloned = copy.deepcopy(template)
    set_first_text(cloned, text)
    return cloned


def append_items(cell: ET.Element, items: list[str], template: ET.Element) -> None:
    values = items or ["待补充"]
    for idx, item in enumerate(values, start=1):
        text = item.strip() or f"{idx}、待补充"
        if not text[:1].isdigit():
            text = f"{idx}、{text}"
        cell.append(clone_with_text(template, text))


def fill_block(row: ET.Element, block: dict) -> None:
    left, right = cells(row)
    left_ps = paragraphs(left)
    set_first_text(left_ps[0], block["department"] or "待补充部门")
    set_first_text(left_ps[1], block["name"] or "待补充姓名")

    right_ps = paragraphs(right)
    heading_template = right_ps[0]
    item_template = right_ps[1]
    summary_template = right_ps[-1]

    for child in right_ps:
        right.remove(child)

    right.append(clone_with_text(heading_template, "上周总结："))
    append_items(right, block["weekly_summary"], item_template)
    right.append(clone_with_text(heading_template, "本周计划："))
    append_items(right, block["next_week_plan"], item_template)
    right.append(clone_with_text(heading_template, "总结："))
    append_items(right, block["summary"], summary_template)


def main() -> None:
    template = Path("apps/desktop/src-tauri/resources/templates/meeting-minutes.docx")
    output = Path("/tmp/liberty-meeting-minutes-filled.docx")

    with zipfile.ZipFile(template, "r") as zin:
        xml = zin.read("word/document.xml")
        root = ET.fromstring(xml)

        body = root.find(qn("body"))
        title_p = body.find(qn("p"))
        set_first_text(title_p, DATA["title"])

        table = body.find(qn("tbl"))
        rows = [child for child in table if child.tag == qn("tr")]

        def set_cell(row_idx: int, cell_idx: int, value: str) -> None:
            set_first_text(cells(rows[row_idx])[cell_idx], value)

        set_cell(0, 1, DATA["meeting_name"])
        set_cell(0, 3, DATA["meeting_time"])
        set_cell(0, 5, DATA["meeting_location"])
        set_cell(0, 6, f"记录人： {DATA['recorder']}")
        set_cell(1, 1, DATA["attendees"])
        set_cell(2, 1, DATA["absentees"])
        set_cell(3, 1, DATA["topics"])
        set_cell(4, 1, DATA["host"])
        set_cell(4, 3, DATA["reviewer"])

        sample_row = copy.deepcopy(rows[6])
        for row in rows[6:]:
            table.remove(row)

        for block in DATA["speech_blocks"]:
            row = copy.deepcopy(sample_row)
            fill_block(row, block)
            table.append(row)

        rendered_xml = ET.tostring(root, encoding="utf-8", xml_declaration=True)

        with zipfile.ZipFile(output, "w", zipfile.ZIP_DEFLATED) as zout:
            for item in zin.infolist():
                if item.filename == "word/document.xml":
                    zout.writestr(item, rendered_xml)
                else:
                    zout.writestr(item, zin.read(item.filename))

    print(output)


if __name__ == "__main__":
    main()
