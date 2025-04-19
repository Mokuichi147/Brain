use rmcp::transport::TokioChildProcess;
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use std::{collections::HashMap, io::BufRead};
use rmcp::model::{ClientCapabilities, ClientInfo, Implementation};
use rmcp::{ServiceExt, transport::SseTransport};


#[derive(Debug, Serialize, Deserialize)]
struct McpSetting {
    name: String,
    #[serde(rename = "type")]
    connection_type: String,
    url: Option<String>,
    command: Option<String>,
    args: Option<Vec<String>>,
}

pub struct Mcp {
    pub tools: Vec<rmcp::model::Tool>,
}


impl Mcp {
    pub fn new() -> Self {
        Mcp {
            tools: Vec::new(),
        }
    }
}


impl Mcp {
    pub async fn load_setting(&mut self, file_path: &str) {
        let mcp_settings = load_setting_file(file_path);
        for mcp_setting in mcp_settings {
            if mcp_setting.connection_type.to_lowercase() == "sse" {
                if mcp_setting.url.is_none() {
                    println!("SSEのURLが指定されていません: {}", mcp_setting.name);
                    continue;
                }

                self.add_mcp_server_sse(&mcp_setting.name, &mcp_setting.url.unwrap()).await;

            } else if mcp_setting.connection_type.to_lowercase() == "stdio" {
                if mcp_setting.command.is_none() {
                    println!("stdioのコマンドが指定されていません: {}", mcp_setting.name);
                    continue;
                }

                self.add_mcp_server_stdio(&mcp_setting.name, &mcp_setting.command.unwrap(), &mcp_setting.args).await;

            } else {
                println!("この接続方式はサポートしていません: {}", mcp_setting.connection_type);

            }
        }
    }

    pub async fn add_mcp_server_sse(&mut self, name: &str, url: &str) {
        let transport = SseTransport::start(url).await;
        if transport.is_err() {
            println!("SSEサーバーに接続できません: {} {}", name, url);
            return;
        }
        let transport = transport.unwrap();

        let client_info = ClientInfo {
            protocol_version: Default::default(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: name.to_string(),
                version: "0.0.1".to_string(),
            },
        };

        let client = client_info.serve(transport).await;
        if client.is_err() {
            println!("クライアントが作成できません: {}", name);
            return;
        }
        let client = client.unwrap();

        let tool_list = client.list_tools(Default::default()).await;
        if tool_list.is_err() {
            println!("ツールの取得に失敗しました: {}", name);
            return;
        }
        let tool_list = tool_list.unwrap();

        for tool in tool_list.tools {
            self.tools.push(tool);
        }
    }

    pub async fn add_mcp_server_stdio(&mut self, name: &str, command: &str, args: &Option<Vec<String>>) {
        let mut command = Command::new(command);
        if let Some(args) = args.as_ref() {
            for arg in args {
                command.arg(arg);
            }
        }

        let transport = TokioChildProcess::new(&mut command);
        if transport.is_err() {
            println!("stdioサーバーに接続できません: {}", name);
            return;
        }
        let transport = transport.unwrap();

        let service = ().serve(transport).await;
        if service.is_err() {
            println!("サービスに接続できません: {}", name);
            return;
        }
        let service = service.unwrap();

        // List tools
        let tool_list = service.list_tools(Default::default()).await;
        if tool_list.is_err() {
            println!("ツールの取得に失敗しました: {}", name);
            return;
        }
        let tool_list = tool_list.unwrap();

        for tool in tool_list.tools {
            self.tools.push(tool);
        }
    }


    pub fn show_tools(&self) {
        for tool in &self.tools {
            println!("name: {}", tool.name);
            println!("description: {}", tool.description);
            println!();
        }
    }
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
        let url = value["url"].as_str().map(|s| s.to_string() + "/sse");
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