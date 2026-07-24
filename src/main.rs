mod agent;
mod config;
mod tools;

use agent::Messages;
use rustyline::DefaultEditor;
use serde_json::json;

#[tokio::main]
async fn main() {
    let config = match config::load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("启动失败: {}", e);
            std::process::exit(1);
        }
    };

    let providers = config::list_providers();

    println!("\x1b[1m\n╔══════════════════════════════════════╗\x1b[0m");
    println!("\x1b[1m║     🤖 Agent CLI Rust - 学习版       ║\x1b[0m");
    println!("\x1b[1m╚══════════════════════════════════════╝\x1b[0m");
    println!("\x1b[2m  Provider: \x1b[36m{}\x1b[0m", config.provider);
    println!("\x1b[2m  模型:     {}\x1b[0m", config.model);
    println!("\x1b[2m  API:      {}\x1b[0m", config.base_url);
    println!("\x1b[2m  可用 Provider: {}\x1b[0m", providers.join(", "));
    println!("\x1b[2m\n  输入问题开始对话，'reset' 清空记忆，'quit' 退出\n\x1b[0m");

    let client = reqwest::Client::new();

    let mut messages: Messages = vec![json!({
        "role": "system",
        "content": "你是一个有用的 AI 助手。用中文简洁、友好地回复用户。"
    })];

    let mut rl = DefaultEditor::new().expect("无法初始化 readline");

    loop {
        let line = match rl.readline("\x1b[1m\n👤 你: \x1b[0m") {
            Ok(line) => line,
            Err(_) => break,
        };

        let input = line.trim();
        if input.is_empty() {
            continue;
        }

        let lower = input.to_lowercase();
        if lower == "quit" || lower == "exit" {
            println!("\x1b[2m\n👋 再见！\x1b[0m");
            break;
        }

        if lower == "reset" {
            println!("\x1b[35m\n🔄 已清空对话记忆\x1b[0m");
            println!("\x1b[2m   清空前消息数: {}\x1b[0m", messages.len());
            messages.truncate(1);
            println!("\x1b[2m   清空后消息数: {}\x1b[0m\n", messages.len());
            continue;
        }

        messages.push(json!({
            "role": "user",
            "content": input,
        }));

        match agent::run_agent_loop(&client, &config, &mut messages).await {
            Ok(_) => {}
            Err(e) => {
                println!("\x1b[31m\n❌ 错误: {}\x1b[0m", e);
            }
        }
    }
}
