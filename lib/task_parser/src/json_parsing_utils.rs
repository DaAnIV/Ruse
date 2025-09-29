use crate::{error::SynthesisTaskResult, parse_err};

pub fn get_string_single_multi_line(value: &serde_json::Value) -> SynthesisTaskResult<String> {
    if let Some(single_line) = value.as_str() {
        return Ok(single_line.to_string());
    }
    let multi_lines: Vec<String> = serde_json::from_value(value.clone()).map_err(|e| {
        parse_err!(
            "Failed to parse single/multi line as string or array of strings: {}",
            e
        )
    })?;

    Ok(multi_lines.join("\n"))
}
