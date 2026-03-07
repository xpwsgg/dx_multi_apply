use chrono::NaiveDate;
use serde_json::Value;

use crate::date_mapping::{to_date_text, to_midnight_timestamp_ms};

const VALUE_TEMPLATE: &str = include_str!("request_template.json");

pub fn build_payload_for_date(date: NaiveDate) -> Result<Value, String> {
    let mut payload: Value = serde_json::from_str(VALUE_TEMPLATE).map_err(|err| err.to_string())?;
    let fields = payload
        .as_array_mut()
        .ok_or_else(|| "template payload is not an array".to_string())?;

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
        return Err("failed to locate DateField for 到访日期".to_string());
    }

    if !updated_text {
        return Err("failed to locate TextField for 到访日期文本".to_string());
    }

    Ok(payload)
}

pub fn extract_field_value(payload: &Value, label: &str) -> Result<String, String> {
    let fields = payload
        .as_array()
        .ok_or_else(|| "payload is not an array".to_string())?;

    for field in fields {
        if field.get("label").and_then(Value::as_str) != Some(label) {
            continue;
        }

        let value = field
            .get("fieldData")
            .and_then(|node| node.get("value"))
            .ok_or_else(|| format!("missing fieldData.value for {label}"))?;

        return match value {
            Value::String(text) => Ok(text.clone()),
            Value::Number(number) => Ok(number.to_string()),
            _ => Err(format!("unsupported value type for {label}")),
        };
    }

    Err(format!("field {label} not found"))
}
