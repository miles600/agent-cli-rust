use crate::config::AgentConfig;
use crate::tools;
use serde_json::{json, Value};
use std::io::Write;

pub type Messages = Vec<Value>;

const MAX_TURNS: usize = 30;

/// 危险工具列表，执行前需要用户确认
const DANGEROUS_TOOLS: &[&str] = &["write_file", "run_shell"];

/// 询问用户是否确认执行
fn confirm_execution(tool_name: &str, args_str: &str) -> bool {
    print!("\x1b[31m⚠️  危险操作: {}({})\x1b[0m", tool_name, args_str);
    print!("\n\x1b[31m   确认执行？[y/N]: \x1b[0m");
    std::io::stdout().flush().unwrap_or(());

    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap_or(0);
    let answer = input.trim().to_lowercase();
    answer == "y" || answer == "yes"
}

pub async fn run_agent_loop(
    client: &reqwest::Client,
    config: &AgentConfig,
    messages: &mut Messages,
) -> Result<String, String> {
    let tool_defs = tools::get_tool_definitions();

    for turn in 0..MAX_TURNS {
        println!("\x1b[2m\n--- 循环 第 {} 轮 ---\x1b[0m", turn + 1);
        println!("\x1b[36m📡 发送请求到 LLM...\x1b[0m");

        let url = format!("{}/chat/completions", config.base_url);
        let body = json!({
            "model": config.model,
            "messages": messages,
            "tools": tool_defs,
        });

        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("请求失败: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("API 返回 {}: {}", status, text));
        }

        let data: Value = resp.json().await.map_err(|e| format!("解析响应失败: {}", e))?;

        let choice = &data["choices"][0]["message"];

        // 检查是否有 tool_calls
        if let Some(tool_calls) = choice["tool_calls"].as_array() {
            // 先把 assistant 的完整消息（含 tool_calls）加入历史
            messages.push(choice.clone());

            // 逐个执行工具
            for tool_call in tool_calls {
                let call_id = tool_call["id"].as_str().unwrap_or("");
                let func_name = tool_call["function"]["name"].as_str().unwrap_or("");
                let args_str = tool_call["function"]["arguments"].as_str().unwrap_or("{}");
                let args: Value = serde_json::from_str(args_str).unwrap_or(json!({}));

                println!("\x1b[33m🔧 调用工具: {}({})\x1b[0m", func_name, args_str);

                // 危险工具需要用户确认
                let result = if DANGEROUS_TOOLS.contains(&func_name) {
                    if confirm_execution(func_name, args_str) {
                        tools::execute_tool(client, func_name, &args).await
                    } else {
                        println!("\x1b[31m   ✖ 用户拒绝执行\x1b[0m");
                        format!("[用户拒绝了 {} 的执行]", func_name)
                    }
                } else {
                    tools::execute_tool(client, func_name, &args).await
                };

                println!("\x1b[33m   ↳ 结果: {}\x1b[0m",
                    if result.len() > 200 { &result[..200] } else { &result });

                // 将工具结果加入消息历史
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": call_id,
                    "content": result,
                }));
            }

            // 继续循环，让 LLM 根据工具结果生成回复
            continue;
        }

        // 没有 tool_calls，说明 LLM 给出了最终回复
        let content = choice["content"].as_str().unwrap_or("").to_string();

        if content.is_empty() {
            return Err("LLM 未返回内容".to_string());
        }

        println!("\x1b[34m🤖 {}\x1b[0m", content);

        messages.push(json!({
            "role": "assistant",
            "content": content,
        }));

        println!("\x1b[32m\n✅ LLM 给出了最终回复\x1b[0m");
        return Ok(content);
    }

    Err("达到最大循环次数限制".to_string())
}
