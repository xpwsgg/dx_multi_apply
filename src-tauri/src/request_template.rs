use chrono::NaiveDate;
use serde_json::Value;

use crate::date_mapping::{to_date_text, to_midnight_timestamp_ms};
use crate::reception_client::ReceptionInfo;
use crate::visitor_client::VisitorInfo;

const VALUE_TEMPLATE: &str = include_str!("request_template.json");

fn build_visitor_row(visitor: &VisitorInfo) -> Value {
    serde_json::json!([
        {
            "componentName": "SelectField",
            "fieldId": "selectField_lxv44orx",
            "label": "有效身份证件",
            "fieldData": { "value": "身份证", "text": "身份证" },
            "options": [{
                "defaultChecked": false,
                "syncLabelValue": true,
                "__sid": "item_lxjzgsg1",
                "text": "身份证",
                "__sid__": "serial_lxjzgsg0",
                "value": "身份证",
                "sid": "serial_lxjzgsg0"
            }]
        },
        {
            "componentName": "TextField",
            "fieldId": "textField_lxv44ory",
            "label": "证件号码",
            "fieldData": { "value": visitor.id_card }
        },
        {
            "componentName": "TextField",
            "fieldId": "textField_lxv44orw",
            "label": "姓名",
            "fieldData": { "value": visitor.name }
        },
        {
            "componentName": "SelectField",
            "fieldId": "selectField_mbyjhot6",
            "label": "区号",
            "fieldData": { "value": "86", "text": "+86" },
            "options": [{
                "defaultChecked": true,
                "syncLabelValue": false,
                "__sid": "item_megqe4lm",
                "text": "+86",
                "__sid__": "serial_megqe4ll",
                "value": "86",
                "sid": "serial_mbyjf8gm"
            }]
        },
        {
            "componentName": "TextField",
            "fieldId": "textField_lxv44orz",
            "label": "联系方式",
            "fieldData": { "value": visitor.phone }
        },
        {
            "componentName": "ImageField",
            "fieldId": "imageField_ly9i5k5q",
            "label": "免冠照片",
            "fieldData": { "value": visitor.photo }
        },
        {
            "componentName": "AttachmentField",
            "fieldId": "attachmentField_lxv44osj",
            "label": "身份证照片",
            "fieldData": { "value": visitor.id_photo }
        },
        {
            "componentName": "AttachmentField",
            "fieldId": "attachmentField_lxv44osk",
            "label": "社保/在职证明",
            "fieldData": { "value": [] }
        },
        {
            "componentName": "AttachmentField",
            "fieldId": "attachmentField_lxv44osn",
            "label": "其他附件",
            "fieldData": { "value": [] }
        }
    ])
}

pub fn build_payload(
    date: NaiveDate,
    account: &str,
    visitors: &[VisitorInfo],
    reception: &ReceptionInfo,
) -> Result<Value, String> {
    let mut payload: Value = serde_json::from_str(VALUE_TEMPLATE).map_err(|err| err.to_string())?;
    let fields = payload
        .as_array_mut()
        .ok_or_else(|| "模板数据不是数组".to_string())?;

    let mut updated_date = false;
    let mut updated_text = false;

    for field in fields.iter_mut() {
        let component_name = field
            .get("componentName")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let label = field
            .get("label")
            .and_then(Value::as_str)
            .map(str::to_owned);

        let (Some(component_name), Some(label)) = (component_name, label) else {
            continue;
        };

        match (component_name.as_str(), label.as_str()) {
            ("TextField", "申请人ID") => {
                field["fieldData"]["value"] = Value::from(account);
            }
            ("TableField", "人员信息") => {
                let rows: Vec<Value> = visitors.iter().map(build_visitor_row).collect();
                field["fieldData"]["value"] = Value::Array(rows);
            }
            ("TextField", "接待人工号") => {
                field["fieldData"]["value"] = Value::from(reception.employee_id.as_str());
            }
            ("TextField", "接待人员") => {
                field["fieldData"]["value"] = Value::from(reception.name.as_str());
            }
            ("TextField", "接待部门") => {
                field["fieldData"]["value"] = Value::from(reception.department.as_str());
            }
            ("TextField", "接待人联系方式") => {
                field["fieldData"]["value"] = Value::from(reception.phone.as_str());
            }
            ("DateField", "到访日期") => {
                field["fieldData"]["value"] = Value::from(to_midnight_timestamp_ms(date));
                updated_date = true;
            }
            ("TextField", "到访日期文本") => {
                field["fieldData"]["value"] = Value::from(to_date_text(date));
                updated_text = true;
            }
            _ => {}
        }
    }

    if !updated_date {
        return Err("未找到到访日期字段".to_string());
    }
    if !updated_text {
        return Err("未找到到访日期文本字段".to_string());
    }

    Ok(payload)
}

/// Backward-compatible: builds payload using the hardcoded template data (only updates date fields).
#[cfg(test)]
pub fn build_payload_for_date(date: NaiveDate) -> Result<Value, String> {
    let mut payload: Value = serde_json::from_str(VALUE_TEMPLATE).map_err(|err| err.to_string())?;
    let fields = payload
        .as_array_mut()
        .ok_or_else(|| "模板数据不是数组".to_string())?;

    let mut updated_date = false;
    let mut updated_text = false;

    for field in fields.iter_mut() {
        let component_name = field
            .get("componentName")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let label = field
            .get("label")
            .and_then(Value::as_str)
            .map(str::to_owned);

        let (Some(component_name), Some(label)) = (component_name, label) else {
            continue;
        };

        if label == "到访日期" && component_name == "DateField" {
            field["fieldData"]["value"] = Value::from(to_midnight_timestamp_ms(date));
            updated_date = true;
        }

        if label == "到访日期文本" && component_name == "TextField" {
            field["fieldData"]["value"] = Value::from(to_date_text(date));
            updated_text = true;
        }
    }

    if !updated_date {
        return Err("未找到到访日期字段".to_string());
    }
    if !updated_text {
        return Err("未找到到访日期文本字段".to_string());
    }

    Ok(payload)
}

#[cfg(test)]
pub fn extract_field_value(payload: &Value, label: &str) -> Result<String, String> {
    let fields = payload
        .as_array()
        .ok_or_else(|| "数据不是数组格式".to_string())?;

    for field in fields {
        if field.get("label").and_then(Value::as_str) != Some(label) {
            continue;
        }

        let value = field
            .get("fieldData")
            .and_then(|node| node.get("value"))
            .ok_or_else(|| format!("字段 {} 缺少 fieldData.value", label))?;

        return match value {
            Value::String(text) => Ok(text.clone()),
            Value::Number(number) => Ok(number.to_string()),
            _ => Err(format!("字段 {} 的值类型不支持", label)),
        };
    }

    Err(format!("未找到字段 {}", label))
}
