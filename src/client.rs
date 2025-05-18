use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::io::AsyncBufReadExt; // .lines() メソッドのため
use tokio_util::io::StreamReader; // Stream を AsyncRead に変換するため
use tokio_stream::StreamExt; // .map() を reqwest の Stream に適用するため
use std::io::Write; // stdout を flush するため

/// Ollama API へのリクエストボディ (/api/generate)
#[derive(Serialize)]
struct OllamaGenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    // 必要に応じて他のオプションも追加できます (例: system, template, options)
    // options: Option<serde_json::Value>,
}

/// Ollama API からのストリーミングレスポンスの各行の形式
#[derive(Deserialize, Debug)]
struct OllamaGenerateResponse {
    model: String,
    created_at: String,
    response: Option<String>, // ストリーミング中は生成されたテキストの一部
    done: bool,               // ストリームの完了フラグ

    // done: true の場合に含まれることがある追加フィールド
    // #[serde(default)] はフィールドが存在しない場合にデフォルト値を使用する
    #[serde(default)]
    total_duration: Option<u64>,
    #[serde(default)]
    load_duration: Option<u64>,
    #[serde(default)]
    prompt_eval_count: Option<usize>,
    #[serde(default)]
    prompt_eval_duration: Option<u64>,
    #[serde(default)]
    eval_count: Option<usize>,
    #[serde(default)]
    eval_duration: Option<u64>,
    #[serde(default)]
    context: Option<Vec<i64>>,
}


pub struct OllamaClient {
    pub client: Client,
    pub host: String,
    pub port: u16,
    pub model: String,
    pub prompt: String,
}

impl OllamaClient {
    pub fn new(host: &str, port: u16, model: &str, prompt: &str) -> Self {
        OllamaClient {
            client: Client::new(),
            host: host.to_string(),
            port,
            model: model.to_string(),
            prompt: prompt.to_string(),
        }
    }

    fn get_url(&self) -> String {
        format!("http://{}:{}/api", self.host, self.port)
    }
}

impl OllamaClient {
    pub async fn generate(&self) -> Result<(), Box<dyn std::error::Error>> {
        let request_payload = OllamaGenerateRequest {
            // 使用するモデル名を指定してください (例: "llama3", "mistral:7b")
            model: self.model.to_string(),
            // プロンプトを指定してください
            prompt: self.prompt.to_string(),
            stream: true, // ストリーミングを有効にする
        };

        // OllamaサーバーのURL。デフォルトは http://localhost:11434
        // `/api/generate` またはチャット用の `/api/chat` エンドポイントを使用します。
        let ollama_api_url = self.get_url() + "/generate";

        println!("Ollamaにリクエストを送信中 (model: {}, prompt: \"{}\") ...\n",
                request_payload.model, request_payload.prompt);

        let response = self.client
            .post(ollama_api_url)
            .json(&request_payload)
            .send()
            .await?;

        // HTTPステータスコードを確認
        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await?;
            eprintln!("APIリクエストエラー (ステータス: {}): {}", status, error_body);
            return Err(format!("API request failed with status {}", status).into());
        }

        // レスポンスボディをバイトストリームとして取得
        // reqwest::Error を std::io::Error に変換する必要があるため .map_err を使用
        let byte_stream = response
            .bytes_stream()
            .map(|result| result.map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err)));

        // バイトストリームを非同期リーダーに変換
        let stream_reader = StreamReader::new(byte_stream);
        let mut lines = stream_reader.lines();

        println!("--- Ollamaからのストリーミング応答 ---");
        let mut full_response_text = String::new();

        let mut is_thinking = false;
        while let Some(line_result) = lines.next_line().await? {
            let line = line_result;
            if line.trim().is_empty() {
                continue; // 空行はスキップ
            }
            match serde_json::from_str::<OllamaGenerateResponse>(&line) {
                Ok(parsed_response) => {
                    if let Some(text_chunk) = parsed_response.response {
                        if text_chunk.contains("<think>") {
                            is_thinking = true;
                            println!("thinking...");
                        } else if text_chunk.contains("</think>") {
                            is_thinking = false;
                            continue;
                        }

                        if is_thinking {
                            continue;
                        }
                        print!("{}", text_chunk); // テキストチャンクを標準出力に表示
                        full_response_text.push_str(&text_chunk);
                        std::io::stdout().flush()?; // バッファをフラッシュして即時表示
                    }

                    if parsed_response.done {
                        println!("\n\n--- ストリーム完了 ---");
                        if let Some(context) = parsed_response.context {
                            println!("Context token count: {}", context.len());
                        }
                        // 必要であれば他の完了時情報も表示
                        // dbg!(parsed_response);
                        break; // ストリーム処理を終了
                    }
                }
                Err(e) => {
                    eprintln!("\nJSONパースエラー: {} (line: '{}')", e, line);
                    // パースエラーが発生しても、ストリームの次の行の処理を試みる
                }
            }
        }
        println!("------------------------------------");
        // println!("\n完全な応答:\n{}", full_response_text); // 必要であれば完全な応答をまとめて表示

        Ok(())
    }
}