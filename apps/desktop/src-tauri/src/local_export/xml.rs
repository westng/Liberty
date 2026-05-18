use crate::local_db::LocalResult;
use xmltree::{Element, XMLNode};

pub fn set_cell_value(row: &mut Element, index: usize, value: &str) -> LocalResult<()> {
    let mut cells = child_elements_mut(row, "tc");
    let cell = cells
        .get_mut(index)
        .ok_or_else(|| format!("会议纪要模板缺少第 {index} 个单元格。"))?;
    set_first_text(cell, value);
    Ok(())
}

pub fn set_first_text(element: &mut Element, value: &str) {
    if replace_first_text(element, value) {
        return;
    }

    let mut text = Element::new("w:t");
    text.children.push(XMLNode::Text(value.to_string()));
    let mut run = Element::new("w:r");
    run.children.push(XMLNode::Element(text));
    let mut paragraph = Element::new("w:p");
    paragraph.children.push(XMLNode::Element(run));
    element.children.push(XMLNode::Element(paragraph));
}

pub fn clone_with_text(template: &Element, value: &str) -> Element {
    let mut cloned = template.clone();
    set_first_text(&mut cloned, value);
    cloned
}

pub fn child_elements<'a>(element: &'a Element, name: &str) -> Vec<&'a Element> {
    element
        .children
        .iter()
        .filter_map(|child| match child {
            XMLNode::Element(child_element) if local_name(&child_element.name) == name => {
                Some(child_element)
            }
            _ => None,
        })
        .collect()
}

pub fn child_elements_mut<'a>(element: &'a mut Element, name: &str) -> Vec<&'a mut Element> {
    element
        .children
        .iter_mut()
        .filter_map(|child| match child {
            XMLNode::Element(child_element) if local_name(&child_element.name) == name => {
                Some(child_element)
            }
            _ => None,
        })
        .collect()
}

pub fn child_elements_count(element: &Element, name: &str) -> usize {
    child_elements(element, name).len()
}

pub fn remove_last_child_element(element: &mut Element, name: &str) {
    if let Some(index) = element.children.iter().rposition(|child| {
        matches!(child, XMLNode::Element(child_element) if local_name(&child_element.name) == name)
    }) {
        element.children.remove(index);
    }
}

pub fn find_child_mut<'a>(element: &'a mut Element, name: &str) -> Option<&'a mut Element> {
    element.children.iter_mut().find_map(|child| match child {
        XMLNode::Element(child_element) if local_name(&child_element.name) == name => {
            Some(child_element)
        }
        _ => None,
    })
}

pub fn local_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn replace_first_text(element: &mut Element, value: &str) -> bool {
    for child in &mut element.children {
        match child {
            XMLNode::Element(child_element) => {
                if local_name(&child_element.name) == "t" {
                    if let Some(XMLNode::Text(text)) = child_element
                        .children
                        .iter_mut()
                        .find(|node| matches!(node, XMLNode::Text(_)))
                    {
                        *text = value.to_string();
                        return true;
                    }
                }

                if replace_first_text(child_element, value) {
                    return true;
                }
            }
            XMLNode::Text(text) => {
                *text = value.to_string();
                return true;
            }
            _ => {}
        }
    }

    false
}
