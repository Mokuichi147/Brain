use fasteval::Evaler;
use ollama_rs::{coordinator::Coordinator, generation::chat::{request::ChatMessageRequest, ChatMessage}, Ollama};
use regex::Regex;
use chrono::Local;

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
        let thinking_regex = Regex::new(r"(?s)<think>\s*(.*?)\s*(?:</think>|\z)").unwrap();

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
        let mut coordinator = Coordinator::new(self.context.clone(), self.tool_model.to_string(), self.history.clone())
            .add_tool(get_datetime_now)
            .add_tool(calculator);

        let message = ChatMessage::user(prompt.to_string());
        let res = coordinator.chat(vec![message.clone()]).await;
        if res.is_err() {
            println!("Error: {}", res.unwrap_err());
            return;
        }
        let res = res.unwrap();

        let text = res.message.content.clone();
        println!("{}", text);

        self.history.push(message);
        self.history.push(res.message);

        // thinkingモデルの場合は、会話履歴からthinkingタグを削除することでコンテキスト長を節約する
        let thinking_result = self.get_thinking(&text, true);
        if let Some(thinking) = thinking_result {
            if let Some(res) = self.history.last_mut() {
                res.content = thinking.clone();
            }
        }
    }

    pub async fn generate_title(&mut self) -> String {
        let prompt = "長文は禁止されています。また、余計な文章も禁止されています。会話内容からユーザー目線でのタイトルを日本語で生成してください。";
        let message = ChatMessage::user(prompt.to_string());
        let res = self.context.send_chat_messages_with_history(
            &mut self.history.clone(),
            ChatMessageRequest::new(
                self.vision_model.clone(),
                vec![message.clone()],
            ),
        ).await.unwrap();

        // thinkingモデルの場合は、会話履歴からthinkingタグを削除することでコンテキスト長を節約する
        let thinking_result = self.get_thinking(&res.message.content, false);
        if let Some(thinking) = thinking_result {
            return thinking;
        }
        return res.message.content;
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


/// 現在の時刻を取得します。
#[ollama_rs::function]
async fn get_datetime_now() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let now = Local::now();
    let result: String = format!("現在時刻: {}", now);
    Ok(result)
}


/// 計算時の使用が義務付けられています。与えられた計算式を計算します。
/// 
/// * formula: 計算式、例: "1+sum(2,3)*abs(4-5)/6^2"
#[ollama_rs::function]
async fn calculator(formula: String) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let parser = fasteval::Parser::new();
    let mut slab = fasteval::Slab::new();
    let val = parser.parse(&formula, &mut slab.ps);
    if let Err(e) = val {
        return Err(Box::new(e));
    }

    let val = val.unwrap()
        .from(&slab.ps)
        .eval(&slab, &mut fasteval::EmptyNamespace);

    if let Err(e) = val {
        return Err(Box::new(e));
    }
    Ok(val.unwrap().to_string())
}