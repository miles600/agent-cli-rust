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

    // 读取 AGENTS.md 作为跨会话记忆
    let agents_md_path = "AGENTS.md";
    let agents_memory = std::fs::read_to_string(agents_md_path).unwrap_or_default();

    let system_content = if agents_memory.is_empty() {
        "你是一个有用的 AI 助手。用中文简洁、友好地回复用户。\n\n\
        你有一个持久记忆文件 AGENTS.md（位于项目根目录）。\
        当用户要求你记住某些信息时，使用 write_file 工具将内容追加写入 AGENTS.md。\
        每次会话启动时，AGENTS.md 的内容会自动加载到你的记忆中。".to_string()
    } else {
        format!(
            "你是一个有用的 AI 助手。用中文简洁、友好地回复用户。\n\n\
            你有一个持久记忆文件 AGENTS.md（位于项目根目录）。\
            当用户要求你记住某些信息时，使用 write_file 工具将内容追加写入 AGENTS.md。\n\n\
            === 你的跨会话记忆（AGENTS.md）===\n{}\n=== 记忆结束 ===",
            agents_memory
        )
    };

    if !agents_memory.is_empty() {
        println!("\x1b[2m  📝 已加载 AGENTS.md 记忆 ({} 字符)\x1b[0m", agents_memory.len());
    }

    let mut messages: Messages = vec![json!({
        "role": "system",
        "content": system_content,
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
