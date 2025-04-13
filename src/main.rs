use clap::{self, Parser};
mod chat;

#[derive(clap::Parser, Debug)]
#[clap(about = "Brain", version = "1.0")]
pub struct Args {
    #[clap(long, default_value = "localhost", env = "BRAIN_LLM_HOST")]
    pub host: String,

    #[clap(short, long, default_value = "11434", env = "BRAIN_LLM_PORT")]
    pub port: u16,

    #[clap(short, long, default_value = "qwq:32b", env = "BRAIN_LLM_TOOL_MODEL")]
    pub tool_model: String,
    
    #[clap(short, long, default_value = "gemma3:27b", env = "BRAIN_LLM_VISION_MODEL")]
    pub vision_model: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let mut chat = chat::Chat::new(&args.host, args.port, &args.tool_model, &args.vision_model);

    loop {
        let mut input = String::new();
        println!("user:");
        std::io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();

        if input == "exit" {
            break;
        }
        else if input.is_empty() {
            chat.clear_history();
            println!("History cleared.");
        }

        chat.generate_response(input).await;
    }

    println!("\nhistory:");
    chat.get_history().iter().for_each(|message| {
        println!("{:?}:", message.role);
        println!("    {}", message.content);
    });
}
