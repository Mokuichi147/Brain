use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::pin::Pin;
use std::future::Future;
use tokio_stream::StreamExt;

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none", default)]
    call_type: Option<String>,
    function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FunctionCall {
    name: String,
    arguments: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    index: Option<u32>,
}


#[derive(Debug, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "type")]
    tool_type: String,
    function: FunctionDefinition,
}

#[derive(Debug, Serialize, Deserialize)]
struct FunctionDefinition {
    name: String,
    description: String,
    parameters: Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Tool>>,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    model: String,
    created_at: String,
    message: Message,
    done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    done_reason: Option<String>,
}

pub struct OllamaClient {
    client: Client,
    base_url: String,
}

impl OllamaClient {
    pub fn new(base_url: Option<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.unwrap_or_else(|| "http://localhost:11434".to_string()),
        }
    }

    pub fn chat_stream_with_tools<'a>(
        &'a self,
        messages: Vec<Message>,
        model: &'a str,
        tools: Option<Vec<Tool>>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error>>> + Send + 'a>> {
        Box::pin(self.chat_stream_with_tools_impl(messages, model, tools))
    }

    pub async fn chat_stream_with_tools_impl(
        &self,
        messages: Vec<Message>,
        model: &str,
        tools: Option<Vec<Tool>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let request = ChatRequest {
            model: model.to_string(),
            messages,
            stream: true,
            tools,
        };

        let url = format!("{}/api/chat", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("HTTP error: {}", response.status()).into());
        }

        let mut stream = response.bytes_stream();
        let mut accumulated_content = String::new();
        let mut current_tool_calls: Vec<ToolCall> = Vec::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            let text = String::from_utf8_lossy(&chunk);
            
            for line in text.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                
                match serde_json::from_str::<ChatResponse>(line) {
                    Ok(response) => {
                        // コンテンツの蓄積
                        if !response.message.content.is_empty() {
                            accumulated_content.push_str(&response.message.content);
                            print!("{}", response.message.content);
                            std::io::Write::flush(&mut std::io::stdout())?;
                        }

                        // ツール呼び出しの処理
                        if let Some(tool_calls) = &response.message.tool_calls {
                            current_tool_calls.extend(tool_calls.clone());
                        }

                        if response.done {
                            println!("\n");
                            
                            // ツール呼び出しがある場合の処理
                            if !current_tool_calls.is_empty() {
                                println!("🔧 Tool calls detected:");
                                for tool_call in &current_tool_calls {
                                    println!("  - Function: {}", tool_call.function.name);
                                    println!("  - Arguments: {}", serde_json::to_string_pretty(&tool_call.function.arguments)?);
                                    
                                    // ツールを実行
                                    let result = self.execute_tool(&tool_call.function).await?;
                                    println!("  - Result: {}", result);
                                }
                                
                                // ツールの結果をメッセージに追加して続行
                                return Box::pin(self.handle_tool_results(current_tool_calls, model)).await;
                            }
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to parse response: {} - Line: {}", e, line);
                    }
                }
            }
        }

        Ok(())
    }

    async fn execute_tool(&self, function: &FunctionCall) -> Result<String, Box<dyn std::error::Error>> {
        match function.name.as_str() {
            "get_weather" => {
                let location = function.arguments["location"].as_str().unwrap_or("Unknown");
                Ok(format!("Weather in {}: Sunny, 22°C", location))
            }
            "calculate" => {
                let expression = function.arguments["expression"].as_str().unwrap_or("0");
                // 簡単な計算の例
                match expression {
                    expr if expr.contains("+") => {
                        let parts: Vec<&str> = expr.split("+").collect();
                        if parts.len() == 2 {
                            let a: f64 = parts[0].trim().parse().unwrap_or(0.0);
                            let b: f64 = parts[1].trim().parse().unwrap_or(0.0);
                            Ok((a + b).to_string())
                        } else {
                            Ok("Invalid expression".to_string())
                        }
                    }
                    expr if expr.contains("*") => {
                        let parts: Vec<&str> = expr.split("*").collect();
                        if parts.len() == 2 {
                            let a: f64 = parts[0].trim().parse().unwrap_or(0.0);
                            let b: f64 = parts[1].trim().parse().unwrap_or(0.0);
                            Ok((a * b).to_string())
                        } else {
                            Ok("Invalid expression".to_string())
                        }
                    }
                    _ => Ok("Calculation not supported".to_string())
                }
            }
            _ => Ok("Tool not implemented".to_string())
        }
    }

    pub fn handle_tool_results<'a>(
        &'a self,
        tool_calls: Vec<ToolCall>,
        model: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error>>> + Send + 'a>> {
        Box::pin(self.handle_tool_results_impl(tool_calls, model))
    }

    async fn handle_tool_results_impl(
        &self,
        tool_calls: Vec<ToolCall>,
        model: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut messages = vec![
            Message {
                role: "assistant".to_string(),
                content: "".to_string(),
                tool_calls: Some(tool_calls.clone()),
            }
        ];

        // ツールの結果をメッセージに追加
        for tool_call in tool_calls {
            let result = self.execute_tool(&tool_call.function).await?;
            messages.push(Message {
                role: "tool".to_string(),
                content: result,
                tool_calls: None,
            });
        }

        // ツールの結果を含めて再度リクエスト
        self.chat_stream_with_tools(messages, model, None).await
    }
}

pub fn create_tools() -> Vec<Tool> {
    vec![
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "get_weather".to_string(),
                description: "Get current weather information for a location".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "location": {
                            "type": "string",
                            "description": "The city or location to get weather for"
                        }
                    },
                    "required": ["location"]
                }),
            },
        },
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "calculate".to_string(),
                description: "Perform basic mathematical calculations".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "expression": {
                            "type": "string",
                            "description": "Mathematical expression to evaluate"
                        }
                    },
                    "required": ["expression"]
                }),
            },
        },
    ]
}