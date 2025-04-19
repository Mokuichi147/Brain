use serde::{Deserialize, Serialize};
use std::{collections::HashMap, io::BufRead};

#[derive(Debug, Serialize, Deserialize)]
struct McpSetting {
    name: String,
    #[serde(rename = "type")]
    connection_type: String,
    url: Option<String>,
    command: Option<String>,
    args: Option<Vec<String>>,
}


fn load_setting_file(file_path: &str) -> Vec<McpSetting> {
    if !std::path::Path::new(file_path).exists() {
        return Vec::new();
    }

    let file = std::fs::File::open(file_path).unwrap();
    let reader = std::io::BufReader::new(file);
    let json_data: String = reader.lines().filter_map(Result::ok).collect();
    let map: HashMap<String, serde_json::Value> = serde_json::from_str(&json_data).expect("Unable to parse settings file");

    let mut settings: Vec<McpSetting> = Vec::new();
    for (name, value) in map {
        let entry_type = value["type"].as_str().unwrap_or_default().to_string();
        let url = value["url"].as_str().map(|s| s.to_string());
        let command = value["command"].as_str().map(|s| s.to_string());
        let args = value["args"].as_array().map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect()
        });

        let setting = McpSetting {
            name: name.to_string(),
            connection_type: entry_type,
            url,
            command,
            args,
        };
        settings.push(setting);
    }
    settings
}