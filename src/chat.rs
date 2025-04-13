use ollama_rs::{generation::chat::{request::ChatMessageRequest, ChatMessage}, Ollama};
use regex::Regex;

pub struct Chat {
    context: Ollama,
    history: Vec<ChatMessage>,
    tool_model: String,
    vision_model: String,
    thinking_regex: Regex,
}

impl Chat {
    pub fn new(host: &str, port: u16, tool_model: &str, vision_model: &str) -> Self {
        let url = format!("http://{}", host);
        let thinking_regex = Regex::new(r"<think>([\s\S]+)</think>").unwrap();

        let context = Ollama::new(url, port);
        let history = Vec::new();

        let tool_model = tool_model.to_string();
        let vision_model = vision_model.to_string();

        Self { context, history, tool_model, vision_model, thinking_regex }
    }

    pub fn add_message(&mut self, message: ChatMessage) {
        self.history.push(message);
    }

    pub fn get_history(&self) -> &Vec<ChatMessage> {
        &self.history
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    pub async fn generate_response(&mut self, prompt: &str) {
        let message = ChatMessage::user(prompt.to_string());
        let res = self.context.send_chat_messages_with_history(
            &mut self.history,
            ChatMessageRequest::new(
                self.tool_model.clone(),
                vec![message.clone()],
            ),
        ).await.unwrap();

        let text = res.message.content.clone();

        println!("{}", text);

        let thinking_result = self.get_thinking(&text, true);
        if let Some(thinking) = thinking_result {
            if let Some(res) = self.history.last_mut() {
                res.content = thinking.clone();
            }
        }
    }

    fn get_thinking(&self, text: &str, is_result: bool) -> Option<String> {
        if let Some(captures) = self.thinking_regex.captures(text) {
            if is_result {
                if let Some(matched) = captures.get(0) {
                    return Some(text.replace(matched.as_str(), "").trim().to_string());
                }
            }
            else {
                if let Some(matched) = captures.get(1) {
                    return Some(matched.as_str().to_string());
                }
            }
        }
        if is_result {
            return Some(text.to_string());
        }
        else {
            return None;
        }
    }
}

impl Default for Chat {
    fn default() -> Self {
        Self::new("localhost", 11434, "qwq:32b", "gemma3:27b")
    }
}